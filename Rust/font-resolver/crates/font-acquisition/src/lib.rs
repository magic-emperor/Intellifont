use async_trait::async_trait;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use font_core::{FontDescriptor, FontFormat, FontError, FontResult};
use font_compressor::{CompressedFontData, FontCategory};
use serde::{Deserialize, Serialize};
use reqwest::Client;
use tokio::fs;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FontSource {
    pub name: String,
    pub api_url: String,
    pub api_key: Option<String>,
    pub license_filter: LicenseFilter,
    pub rate_limit: RateLimit,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LicenseFilter {
    pub allowed_licenses: Vec<String>,
    pub require_open_source: bool,
    pub allow_commercial_use: bool,
    pub allow_modification: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RateLimit {
    pub requests_per_minute: u32,
    pub requests_per_day: u32,
}

#[derive(Debug, Clone)]
pub struct FontDownload {
    pub font_data: CompressedFontData,
    pub download_url: String,
    pub format: FontFormat,
    pub estimated_size_kb: u32,
}

#[async_trait]
pub trait FontProvider: Send + Sync {
    fn name(&self) -> &str;
    async fn search_fonts(&self, query: &str, limit: usize) -> FontResult<Vec<CompressedFontData>>;
    async fn download_font(&self, font: &CompressedFontData, format: FontFormat) -> FontResult<FontDownload>;
    fn get_license_info(&self, font: &CompressedFontData) -> LicenseInfo;
}

pub struct FontAcquisitionManager {
    providers: HashMap<String, Arc<dyn FontProvider + Send + Sync>>,
    client: Client,
    download_cache: PathBuf,
    #[allow(dead_code)]
    max_concurrent_downloads: usize,
}

impl FontAcquisitionManager {
    pub fn new() -> Self {
        let client = Client::builder()
            .user_agent("Font-Resolver/1.0")
            .timeout(std::time::Duration::from_secs(30))
            .build()
            .unwrap();
        
        let cache_dir = dirs::cache_dir()
            .unwrap_or_else(|| PathBuf::from("/tmp/font-resolver"))
            .join("downloads");
        
        std::fs::create_dir_all(&cache_dir).ok();
        
        Self {
            providers: HashMap::new(),
            client,
            download_cache: cache_dir,
            max_concurrent_downloads: 5,
        }
    }
    
    pub fn add_provider(&mut self, name: &str, provider: Box<dyn FontProvider + Send + Sync>) {
        self.providers.insert(name.to_string(), Arc::from(provider));
    }
    
    pub async fn parallel_search(
        &self,
        query: &str,
        limit_per_provider: usize,
    ) -> FontResult<Vec<CompressedFontData>> {
        let mut handles = vec![];
        
        for (name, provider) in &self.providers {
            let query = query.to_string();
            let provider_name = name.clone();
            let provider = Arc::clone(provider);
            
            handles.push(tokio::spawn(async move {
                // Individual provider timeout: 2 seconds
                match tokio::time::timeout(std::time::Duration::from_secs(2), provider.search_fonts(&query, limit_per_provider)).await {
                    Ok(Ok(fonts)) => (provider_name, fonts),
                    Ok(Err(_e)) => {
                        (provider_name, Vec::new())
                    }
                    Err(_) => {
                        (provider_name, Vec::new())
                    }
                }
            }));
        }
        
        let mut all_fonts = Vec::new();
        let mut seen_families = std::collections::HashSet::new();
        
        for handle in handles {
            match handle.await {
                Ok((_provider_name, fonts)) => {
                    for font in fonts {
                        if !seen_families.contains(&font.family) {
                            seen_families.insert(font.family.clone());
                            all_fonts.push(font);
                        }
                    }
                }
                Err(_e) => {}
            }
        }
        
        // Sort by relevance (simplified: by name match)
        all_fonts.sort_by(|a, b| {
            let a_match = a.family.to_lowercase().contains(&query.to_lowercase());
            let b_match = b.family.to_lowercase().contains(&query.to_lowercase());
            b_match.cmp(&a_match).then(a.family.cmp(&b.family))
        });
        
        Ok(all_fonts)
    }
    
    pub async fn download_and_verify(
        &self,
        font: &CompressedFontData,
        format: FontFormat,
        provider_name: &str,
    ) -> FontResult<FontDescriptor> {
        // Check cache first
        let cache_key = format!("{}_{:?}", font.postscript_name, format);
        let cache_path = self.download_cache.join(&cache_key);
        
        if cache_path.exists() {
            return self.load_from_cache(&cache_path, font).await;
        }
        
        // Get provider
        let provider = self.providers.get(provider_name)
            .ok_or_else(|| FontError::NotFound(format!("Provider {} not found", provider_name)))?;
        
        // Download font
        let download = provider.download_font(font, format).await?;
        
        // Verify license
        let license_info = provider.get_license_info(font);
        if !license_info.is_safe_for_distribution() {
            return Err(FontError::LicenseRestriction(
                format!("Font {} has restrictive license", font.family)
            ));
        }
        
        // Save to cache
        self.save_to_cache(&cache_path, &download).await?;
        
        // Convert to FontDescriptor
        self.create_font_descriptor(font, &download, &license_info).await
    }
    
    async fn load_from_cache(
        &self,
        cache_path: &Path,
        font_data: &CompressedFontData,
    ) -> FontResult<FontDescriptor> {
        // Just read to verify file exists, don't store the data
        let _ = fs::read(cache_path).await
            .map_err(|e| FontError::Io(e))?;
        
        Ok(FontDescriptor {
            family: font_data.family.clone(),
            subfamily: None,
            postscript_name: font_data.postscript_name.clone(),
            full_name: Some(font_data.family.clone()),
            path: cache_path.to_path_buf(),
            format: FontFormat::Ttf, // Assume TTF for cached files
            weight: font_data.weight,
            italic: font_data.italic,
            monospaced: font_data.monospaced,
            variable: false,
            metrics: None, // Would need to parse
            license: Some(font_core::LicenseInfo {
                name: font_data.license.name.clone(),
                url: Some(font_data.license.url.clone()),
                allows_embedding: font_data.license.allows_embedding,
                allows_modification: font_data.license.allows_modification,
                requires_attribution: font_data.license.requires_attribution,
                allows_commercial_use: font_data.license.allows_commercial_use, // ADDED THIS FIELD
            }),
        })
    }
    
    async fn save_to_cache(
        &self,
        cache_path: &Path,
        download: &FontDownload,
    ) -> FontResult<()> {
        // Download the actual font file
        let response = self.client.get(&download.download_url).send().await
            .map_err(|e| FontError::Parse(format!("Network error: {}", e)))?;
        
        let bytes = response.bytes().await
            .map_err(|e| FontError::Parse(format!("Network error: {}", e)))?;
        
        fs::write(cache_path, &bytes).await
            .map_err(|e| FontError::Io(e))?;
        
        Ok(())
    }
    
    async fn create_font_descriptor(
        &self,
        font_data: &CompressedFontData,
        download: &FontDownload,
        license_info: &LicenseInfo,
    ) -> FontResult<FontDescriptor> {
        // Parse font file to get metrics
        // This would use font-parser
        // For now, return basic descriptor
        
        Ok(FontDescriptor {
            family: font_data.family.clone(),
            subfamily: None,
            postscript_name: font_data.postscript_name.clone(),
            full_name: Some(font_data.family.clone()),
            path: PathBuf::from(format!("/downloaded/{}", font_data.postscript_name)),
            format: download.format,
            weight: font_data.weight,
            italic: font_data.italic,
            monospaced: font_data.monospaced,
            variable: false,
            metrics: font_data.metrics.as_ref().map(|m| font_core::FontMetrics {
                units_per_em: m.units_per_em,
                ascender: m.ascender,
                descender: m.descender,
                x_height: m.x_height,
                cap_height: m.cap_height,
                average_width: m.average_width,
                max_advance_width: 1000, // Default
            }),
            license: Some(font_core::LicenseInfo {
                name: license_info.name.clone(),
                url: Some(license_info.url.clone()),
                allows_embedding: license_info.allows_embedding,
                allows_modification: license_info.allows_modification,
                requires_attribution: license_info.requires_attribution,
                allows_commercial_use: license_info.allows_commercial_use, // ADDED THIS FIELD
            }),
        })
    }
}

// Google Fonts Provider Implementation
pub struct GoogleFontsProvider {
    client: Client,
    api_key: Option<String>,
}

impl GoogleFontsProvider {
    pub fn new(api_key: Option<String>) -> Self {
        Self {
            client: Client::new(),
            api_key,
        }
    }
    
    async fn parse_google_font(&self, item: &serde_json::Value) -> Option<CompressedFontData> {
        let family = item["family"].as_str()?.to_string();
        let category = match item["category"].as_str()? {
            "serif" => FontCategory::Serif,
            "sans-serif" => FontCategory::SansSerif,
            "monospace" => FontCategory::Monospace,
            "display" => FontCategory::Display,
            "handwriting" => FontCategory::Handwriting,
            _ => FontCategory::Other,
        };
        
        let _variants = item["variants"].as_array()?;
        
        Some(CompressedFontData {
            family: family.clone(), // Clone for the struct field
            postscript_name: family.replace(' ', "-").to_lowercase(), // Use the original
            weight: 400,
            italic: false,
            monospaced: category == FontCategory::Monospace,
            metrics: None,
            license: font_compressor::CompressedLicense {
                name: "SIL Open Font License".to_string(),
                url: "http://scripts.sil.org/OFL".to_string(),
                allows_embedding: true,
                allows_modification: true,
                requires_attribution: false,
                allows_commercial_use: true, // ADDED THIS FIELD
            },
            category,
            similar_fonts: Vec::new(),
            download_urls: HashMap::new(),
            file_size_kb: 50,
            popularity: 50,
        })
    }
}

#[async_trait]
impl FontProvider for GoogleFontsProvider {
    fn name(&self) -> &str {
        "Google Fonts"
    }
    
    async fn search_fonts(&self, query: &str, limit: usize) -> FontResult<Vec<CompressedFontData>> {
        let query_lower = query.to_lowercase();
        
        let url = if let Some(key) = &self.api_key {
            format!("https://www.googleapis.com/webfonts/v1/webfonts?key={}&sort=popularity", key)
        } else {
            "https://www.googleapis.com/webfonts/v1/webfonts?sort=popularity".to_string()
        };
        
        let response = self.client.get(&url).send().await
            .map_err(|e| FontError::Parse(format!("Google Fonts API error: {}", e)))?;
        
        let json: serde_json::Value = response.json().await
            .map_err(|e| FontError::Parse(format!("JSON parse error: {}", e)))?;
        
        let items = json["items"].as_array()
            .ok_or_else(|| FontError::Parse("No items in response".to_string()))?;
        
        let mut fonts = Vec::new();
        
        for item in items.iter() {
            if fonts.len() >= limit {
                break;
            }
            
            if let Some(font) = self.parse_google_font(item).await {
                // Filter by query if provided
                if query.is_empty() || font.family.to_lowercase().contains(&query_lower) {
                    fonts.push(font);
                }
            }
        }
        
        Ok(fonts)
    }
    
    async fn download_font(&self, font: &CompressedFontData, format: FontFormat) -> FontResult<FontDownload> {
        let css_url = match format {
            FontFormat::Woff2 => format!("https://fonts.googleapis.com/css2?family={}:wght@{}",
                font.family.replace(' ', "+"), font.weight),
            _ => format!("https://fonts.googleapis.com/css?family={}:{}",
                font.family.replace(' ', "+"), font.weight),
        };
        
        let mut headers = reqwest::header::HeaderMap::new();
        if format == FontFormat::Woff2 {
            // Google Fonts requires a specific user agent for WOFF2
            headers.insert("User-Agent", "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/91.0.4472.124 Safari/537.36".parse().unwrap());
        }

        let response = self.client.get(&css_url).headers(headers).send().await
            .map_err(|e| FontError::Parse(format!("Failed to fetch font CSS: {}", e)))?;
        
        let css = response.text().await
            .map_err(|e| FontError::Parse(format!("Failed to read font CSS: {}", e)))?;
        
        // Extract URL using regex (simplified)
        let re = regex::Regex::new(r"url\(([^)]+)\)").unwrap();
        let download_url = re.captures(&css)
            .and_then(|cap| cap.get(1))
            .map(|m| m.as_str().trim_matches('\'').trim_matches('"').to_string())
            .ok_or_else(|| FontError::Parse("Could not find download URL in CSS".to_string()))?;
        
        Ok(FontDownload {
            font_data: font.clone(),
            download_url,
            format,
            estimated_size_kb: font.file_size_kb,
        })
    }
    
    fn get_license_info(&self, _font: &CompressedFontData) -> LicenseInfo {
        LicenseInfo {
            name: "SIL Open Font License".to_string(),
            url: "http://scripts.sil.org/OFL".to_string(),
            allows_embedding: true,
            allows_modification: true,
            requires_attribution: false,
            allows_commercial_use: true, // ADDED THIS FIELD
        }
    }
}

// Fontsource Provider
pub struct FontsourceProvider {
    client: Client,
}

impl FontsourceProvider {
    pub fn new() -> Self {
        Self { client: Client::new() }
    }
}

#[async_trait]
impl FontProvider for FontsourceProvider {
    fn name(&self) -> &str {
        "Fontsource"
    }
    
    async fn search_fonts(&self, query: &str, limit: usize) -> FontResult<Vec<CompressedFontData>> {
        let url = format!("https://api.fontsource.org/v1/fonts?search={}", query);
        
        let response = self.client.get(&url).send().await
            .map_err(|e| FontError::Parse(format!("Fontsource API error: {}", e)))?;
        
        let fonts: Vec<serde_json::Value> = response.json().await
            .map_err(|e| FontError::Parse(format!("Failed to parse Fontsource JSON: {}", e)))?;
        
        let mut results = Vec::new();
        for item in fonts.iter().take(limit) {
            if let Some(family) = item["family"].as_str() {
                results.push(CompressedFontData {
                    family: family.to_string(),
                    postscript_name: item["id"].as_str().unwrap_or(family).to_string(),
                    weight: 400, // Default weight
                    italic: false,
                    monospaced: false,
                    metrics: None,
                    license: font_compressor::CompressedLicense {
                        name: "Various (Check Fontsource)".to_string(),
                        url: "https://fontsource.org/".to_string(),
                        allows_embedding: true,
                        allows_modification: true,
                        requires_attribution: false,
                        allows_commercial_use: true,
                    },
                    category: FontCategory::Other,
                    similar_fonts: Vec::new(),
                    download_urls: std::collections::HashMap::new(),
                    file_size_kb: 50,
                    popularity: 50,
                });
            }
        }
        
        Ok(results)
    }
    
    async fn download_font(&self, font: &CompressedFontData, format: FontFormat) -> FontResult<FontDownload> {
        let url = format!("https://cdn.jsdelivr.net/fontsource/fonts/{}/{}",
            font.family.to_lowercase().replace(' ', "-"),
            match format {
                FontFormat::Woff2 => "woff2",
                _ => "ttf",
            });
        
        Ok(FontDownload {
            font_data: font.clone(),
            download_url: url,
            format,
            estimated_size_kb: font.file_size_kb,
        })
    }
    
    fn get_license_info(&self, _font: &CompressedFontData) -> LicenseInfo {
        LicenseInfo {
            name: "Various Open Font Licenses".to_string(),
            url: "https://fontsource.org/licenses".to_string(),
            allows_embedding: true,
            allows_modification: true,
            requires_attribution: false,
            allows_commercial_use: true,
        }
    }
}

// Adobe Fonts Provider (Skeleton)
pub struct AdobeFontsProvider {
    _client: Client,
}

impl AdobeFontsProvider {
    pub fn new() -> Self {
        Self { _client: Client::new() }
    }
}

#[async_trait]
impl FontProvider for AdobeFontsProvider {
    fn name(&self) -> &str {
        "Adobe Fonts (Free Tier)"
    }
    
    async fn search_fonts(&self, _query: &str, _limit: usize) -> FontResult<Vec<CompressedFontData>> {
        Ok(Vec::new()) 
    }
    
    async fn download_font(&self, _font: &CompressedFontData, _format: FontFormat) -> FontResult<FontDownload> {
        Err(FontError::NotFound("Adobe direct download requires enterprise API".to_string()))
    }
    
    fn get_license_info(&self, _font: &CompressedFontData) -> LicenseInfo {
        LicenseInfo {
            name: "Adobe Font License".to_string(),
            url: "https://www.adobe.com/products/type/font-licensing.html".to_string(),
            allows_embedding: true,
            allows_modification: false,
            requires_attribution: true,
            allows_commercial_use: true,
        }
    }
}

#[derive(Debug, Clone)]
pub struct LicenseInfo {
    pub name: String,
    pub url: String,
    pub allows_embedding: bool,
    pub allows_modification: bool,
    pub requires_attribution: bool,
    pub allows_commercial_use: bool, // ADDED THIS FIELD
}

impl LicenseInfo {
    pub fn is_safe_for_distribution(&self) -> bool {
        self.allows_embedding && !self.requires_attribution
    }
}