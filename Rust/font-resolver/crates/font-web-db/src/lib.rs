use font_core::{FontDescriptor, FontFormat, FontMetrics, LicenseInfo};
use serde::{Serialize, Deserialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WebFont {
    pub family: String,
    pub variants: Vec<WebFontVariant>,
    pub category: FontCategory,
    pub popularity: u8, // 0-100
    pub last_updated: String,
    pub license: WebFontLicense,
    pub similar_fonts: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WebFontVariant {
    pub weight: u16,
    pub italic: bool,
    pub style: String,
    pub file_url: String,
    pub file_format: FontFormat,
    pub file_size_kb: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum FontCategory {
    Serif,
    SansSerif,
    Monospace,
    Display,
    Handwriting,
    Other,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WebFontLicense {
    pub name: String,
    pub url: String,
    pub allows_commercial_use: bool,
    pub allows_modification: bool,
    pub requires_attribution: bool,
}

#[derive(Debug, Clone)]
pub struct WebFontDatabase {
    fonts: HashMap<String, WebFont>,
    family_aliases: HashMap<String, String>,
    version: String,
}

impl WebFontDatabase {
    /// Load web font database from embedded binary data
    pub fn load_embedded() -> Self {
        // Check if we have embedded data
        #[cfg(feature = "embedded-db")]
        {
            let compressed_data = include_bytes!("../data/web_fonts.bin");
            
            // If file exists and has content
            if !compressed_data.is_empty() {
                match std::io::Cursor::new(compressed_data)
                    .and_then(|cursor| {
                        let mut decoder = flate2::read::GzDecoder::new(cursor);
                        let mut decompressed_data = Vec::new();
                        std::io::copy(&mut decoder, &mut decompressed_data)?;
                        Ok(decompressed_data)
                    })
                    .and_then(|data| {
                        bincode::deserialize(&data)
                            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))
                    }) {
                    Ok((fonts, aliases, version)) => {
                        return Self {
                            fonts,
                            family_aliases: aliases,
                            version,
                        };
                    }
                    Err(e) => {
                        eprintln!("⚠️  Failed to load embedded web fonts: {}", e);
                    }
                }
            }
        }
        
        // Fall back to minimal database
        Self::create_minimal_database()
    }
    
    fn create_minimal_database() -> Self {
        use FontCategory::*;
        
        let mut fonts = HashMap::new();
        let mut aliases = HashMap::new();
        
        // Add some common free fonts
        fonts.insert("roboto".to_string(), WebFont {
            family: "Roboto".to_string(),
            variants: vec![
                WebFontVariant {
                    weight: 400,
                    italic: false,
                    style: "Regular".to_string(),
                    file_url: "".to_string(),
                    file_format: font_core::FontFormat::Ttf,
                    file_size_kb: 150,
                },
                WebFontVariant {
                    weight: 700,
                    italic: false,
                    style: "Bold".to_string(),
                    file_url: "".to_string(),
                    file_format: font_core::FontFormat::Ttf,
                    file_size_kb: 160,
                },
            ],
            category: SansSerif,
            popularity: 95,
            last_updated: "2024-01-01".to_string(),
            license: WebFontLicense {
                name: "Apache License 2.0".to_string(),
                url: "http://www.apache.org/licenses/LICENSE-2.0".to_string(),
                allows_commercial_use: true,
                allows_modification: true,
                requires_attribution: false,
            },
            similar_fonts: vec!["open-sans".to_string(), "lato".to_string()],
        });
        
        // Add common aliases
        aliases.insert("arial".to_string(), "roboto".to_string());
        aliases.insert("helvetica".to_string(), "roboto".to_string());
        aliases.insert("sans-serif".to_string(), "roboto".to_string());
        
        Self {
            fonts,
            family_aliases: aliases,
            version: "0.1.0-minimal".to_string(),
        }
    }
    
    /// Public getter for fonts (to avoid exposing private field)
    pub fn get_fonts(&self) -> &HashMap<String, WebFont> {
        &self.fonts
    }
    
    /// Check if database is loaded
    pub fn is_loaded(&self) -> bool {
        !self.fonts.is_empty()
    }
    
    /// Get number of fonts in database
    pub fn count(&self) -> usize {
        self.fonts.len()
    }
    
    /// Find a font by family name
    pub fn find_font(&self, family: &str) -> Option<&WebFont> {
        let normalized = family.to_lowercase().replace(' ', "-");
        
        // Direct match
        if let Some(font) = self.fonts.get(&normalized) {
            return Some(font);
        }
        
        // Alias match
        if let Some(real_family) = self.family_aliases.get(&normalized) {
            return self.fonts.get(real_family);
        }
        
        // Partial match
        for (key, font) in &self.fonts {
            if key.contains(&normalized) || normalized.contains(key) {
                return Some(font);
            }
        }
        
        None
    }
    
    /// Find similar fonts (for substitutions)
    pub fn find_similar_fonts(&self, family: &str, limit: usize) -> Vec<&WebFont> {
        let search_family = family.to_lowercase();
        let mut results = Vec::new();
        let mut used_families = std::collections::HashSet::new();
        
        // First, check if this font is in our database
        if let Some(font) = self.find_font(family) {
            // Get fonts marked as similar
            for similar_name in &font.similar_fonts {
                if let Some(similar_font) = self.find_font(similar_name) {
                    if !used_families.contains(&similar_font.family) {
                        results.push(similar_font);
                        used_families.insert(similar_font.family.clone());
                        if results.len() >= limit {
                            return results;
                        }
                    }
                }
            }
            
            // If we still need more fonts, use the same category
            let category = font.category.clone();
            for font_in_category in self.fonts.values() {
                if font_in_category.category == category && 
                   font_in_category.family != font.family &&
                   !used_families.contains(&font_in_category.family) {
                    results.push(font_in_category);
                    used_families.insert(font_in_category.family.clone());
                    if results.len() >= limit {
                        return results;
                    }
                }
            }
        } else {
            // Font not found in database, try to guess category
            let category = if search_family.contains("mono") || search_family.contains("console") {
                FontCategory::Monospace
            } else if search_family.contains("serif") {
                FontCategory::Serif
            } else if search_family.contains("script") || search_family.contains("hand") {
                FontCategory::Handwriting
            } else {
                FontCategory::SansSerif
            };
            
            // Find fonts in same category
            for font in self.fonts.values() {
                if font.category == category && !used_families.contains(&font.family) {
                    results.push(font);
                    used_families.insert(font.family.clone());
                    if results.len() >= limit {
                        return results;
                    }
                }
            }
        }
        
        results
    }
    
    /// Convert web font to font descriptor
    pub fn to_font_descriptor(&self, web_font: &WebFont, variant: &WebFontVariant) -> FontDescriptor {
        FontDescriptor {
            family: web_font.family.clone(),
            subfamily: Some(variant.style.clone()),
            postscript_name: format!("{}-{}", web_font.family, variant.style),
            full_name: Some(format!("{} {}", web_font.family, variant.style)),
            path: std::path::PathBuf::from(&variant.file_url),
            format: variant.file_format,
            weight: variant.weight,
            italic: variant.italic,
            monospaced: web_font.category == FontCategory::Monospace,
            variable: false, // Web fonts typically aren't variable
            metrics: Some(FontMetrics {
                units_per_em: 1000, // Default for web fonts
                ascender: 800,
                descender: -200,
                x_height: 500,
                cap_height: 700,
                average_width: 500,
                max_advance_width: 1200,
            }),
            license: Some(LicenseInfo {
                name: web_font.license.name.clone(),
                url: Some(web_font.license.url.clone()),
                allows_embedding: true,
                allows_modification: web_font.license.allows_modification,
                requires_attribution: web_font.license.requires_attribution,
                allows_commercial_use: web_font.license.allows_commercial_use,
                // allows_commercial_use: compressed.license.allows_commercial_use, // Removed as it is duplicated
            }),
        }
    }
    
    /// Get database version
    pub fn version(&self) -> &str {
        &self.version
    }
    
    /// Get memory usage estimate
    pub fn memory_usage_kb(&self) -> usize {
        // Rough estimate: 2KB per font
        self.fonts.len() * 2
    }
}

// Script to generate web font database
#[cfg(feature = "download")]
pub mod download {
    use super::*;
    use serde_json::Value;
    use std::fs::File;
    use std::io::Write;
    
    /// Download web font data from Google Fonts API
    pub async fn download_google_fonts(api_key: &str) -> Result<Vec<WebFont>, Box<dyn std::error::Error>> {
        let url = format!("https://www.googleapis.com/webfonts/v1/webfonts?key={}", api_key);
        let response = reqwest::get(&url).await?.json::<Value>().await?;
        
        let mut web_fonts = Vec::new();
        
        if let Some(items) = response["items"].as_array() {
            for item in items {
                let family = item["family"].as_str().unwrap_or("Unknown").to_string();
                
                let category = match item["category"].as_str().unwrap_or("sans-serif") {
                    "serif" => FontCategory::Serif,
                    "sans-serif" => FontCategory::SansSerif,
                    "monospace" => FontCategory::Monospace,
                    "display" => FontCategory::Display,
                    "handwriting" => FontCategory::Handwriting,
                    _ => FontCategory::Other,
                };
                
                let variants = if let Some(variant_list) = item["variants"].as_array() {
                    variant_list.iter()
                        .filter_map(|v| parse_variant(v.as_str()?, &family))
                        .collect()
                } else {
                    Vec::new()
                };
                
                let web_font = WebFont {
                    family: family.clone(),
                    variants,
                    category,
                    popularity: item["popularity"].as_str()
                        .and_then(|s| s.parse::<f32>().ok())
                        .map(|p| (p * 100.0) as u8)
                        .unwrap_or(50),
                    last_updated: chrono::Utc::now().format("%Y-%m-%d").to_string(),
                    license: WebFontLicense {
                        name: "SIL Open Font License".to_string(),
                        url: "http://scripts.sil.org/OFL".to_string(),
                        allows_commercial_use: true,
                        allows_modification: true,
                        requires_attribution: true,
                    },
                    similar_fonts: Vec::new(),
                };
                
                web_fonts.push(web_font);
            }
        }
        
        Ok(web_fonts)
    }
    
    fn parse_variant(variant: &str, family: &str) -> Option<WebFontVariant> {
        let (weight, italic) = parse_weight_style(variant);
        let style = variant.to_string();
        
        Some(WebFontVariant {
            weight,
            italic,
            style,
            file_url: format!("https://fonts.googleapis.com/css2?family={}:wght@{}", 
                family.replace(' ', "+"), weight),
            file_format: FontFormat::Woff2,
            file_size_kb: 50, // Average size
        })
    }
    
    fn parse_weight_style(variant: &str) -> (u16, bool) {
        match variant {
            "100" => (100, false),
            "100italic" => (100, true),
            "200" => (200, false),
            "200italic" => (200, true),
            "300" => (300, false),
            "300italic" => (300, true),
            "regular" => (400, false),
            "italic" => (400, true),
            "500" => (500, false),
            "500italic" => (500, true),
            "600" => (600, false),
            "600italic" => (600, true),
            "700" => (700, false),
            "700italic" => (700, true),
            "800" => (800, false),
            "800italic" => (800, true),
            "900" => (900, false),
            "900italic" => (900, true),
            _ => (400, false),
        }
    }
    
    /// Save web font database to file
    pub fn save_database(fonts: &[WebFont], aliases: &HashMap<String, String>, 
                         version: &str, output_path: &str) -> Result<(), Box<dyn std::error::Error>> {
        let font_map: HashMap<String, WebFont> = fonts
            .iter()
            .map(|f| (f.family.to_lowercase().replace(' ', "-"), f.clone()))
            .collect();
        
        let data = (font_map, aliases.clone(), version.to_string());
        let serialized = bincode::serialize(&data)?;
        
        // Compress
        let mut encoder = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::default());
        encoder.write_all(&serialized)?;
        let compressed = encoder.finish()?;
        
        let mut file = File::create(output_path)?;
        file.write_all(&compressed)?;
        
        Ok(())
    }
}   