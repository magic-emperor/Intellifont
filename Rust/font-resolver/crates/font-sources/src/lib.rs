use font_core::{FontDescriptor, FontError, FontResult, FontSourcePriority};
use font_parser::FontParser;
use font_web_db::WebFontDatabase;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use walkdir::WalkDir;

#[derive(Debug, Clone)]
pub enum SourceType {
    System,
    Web,
    File(PathBuf),
    Directory(PathBuf),
    Url(String),
    Api {
        endpoint: String,
        api_key: Option<String>,
        cache_ttl_seconds: u64,
    },
}

pub struct FontSourceManager {
    system_sources: Vec<SourceType>,
    web_sources: Vec<SourceType>,
    custom_sources: Vec<SourceType>,
    priority: FontSourcePriority,
    web_db: Option<WebFontDatabase>,
    font_cache: HashMap<String, FontDescriptor>,
    parser: FontParser,
}

impl FontSourceManager {
    pub fn new() -> Self {
        Self {
            system_sources: vec![SourceType::System],
            web_sources: Vec::new(),
            custom_sources: Vec::new(),
            priority: FontSourcePriority::SystemOnly,
            web_db: None,
            font_cache: HashMap::new(),
            parser: FontParser,
        }
    }
    
    pub fn enable_web_fonts(&mut self, enable: bool) -> FontResult<()> {
        if enable {
            if self.web_db.is_none() {
                let db = WebFontDatabase::load_embedded();
                if db.is_loaded() {
                    self.web_db = Some(db);
                    self.web_sources = vec![SourceType::Web];
                } else {
                    return Err(FontError::Parse("Failed to load web font database".to_string()));
                }
            }
        } else {
            self.web_db = None;
            self.web_sources.clear();
        }
        Ok(())
    }
    
    pub fn add_custom_source(&mut self, source: SourceType) -> FontResult<()> {
        match &source {
            SourceType::File(path) => {
                if !path.exists() {
                    return Err(FontError::NotFound(format!("File not found: {:?}", path)));
                }
                self.custom_sources.push(source);
            }
            SourceType::Directory(path) => {
                if !path.exists() {
                    return Err(FontError::NotFound(format!("Directory not found: {:?}", path)));
                }
                self.custom_sources.push(source);
            }
            SourceType::Url(url) => {
                // Validate URL format
                if !url.starts_with("http://") && !url.starts_with("https://") {
                    return Err(FontError::Parse(format!("Invalid URL: {}", url)));
                }
                self.custom_sources.push(source);
            }
            SourceType::Api { endpoint, .. } => {
                if !endpoint.starts_with("http://") && !endpoint.starts_with("https://") {
                    return Err(FontError::Parse(format!("Invalid API endpoint: {}", endpoint)));
                }
                self.custom_sources.push(source);
            }
            _ => {
                return Err(FontError::Parse("Invalid source type".to_string()));
            }
        }
        Ok(())
    }
    
    pub fn remove_custom_source(&mut self, index: usize) -> FontResult<()> {
        if index < self.custom_sources.len() {
            self.custom_sources.remove(index);
            Ok(())
        } else {
            Err(FontError::Parse(format!("Invalid source index: {}", index)))
        }
    }
    
    pub fn set_priority(&mut self, priority: FontSourcePriority) {
        self.priority = priority;
    }
    
    pub fn scan_sources(&mut self) -> FontResult<Vec<FontDescriptor>> {
        let mut all_fonts = Vec::new();
        
        // Scan based on priority
        match self.priority {
            FontSourcePriority::SystemOnly => {
                all_fonts.extend(self.scan_system_sources()?);
            }
            FontSourcePriority::SystemThenWeb => {
                all_fonts.extend(self.scan_system_sources()?);
                all_fonts.extend(self.scan_web_sources()?);
            }
            FontSourcePriority::SystemThenCustom => {
                all_fonts.extend(self.scan_system_sources()?);
                all_fonts.extend(self.scan_custom_sources()?);
            }
            FontSourcePriority::SystemThenWebThenCustom => {
                all_fonts.extend(self.scan_system_sources()?);
                all_fonts.extend(self.scan_web_sources()?);
                all_fonts.extend(self.scan_custom_sources()?);
            }
            FontSourcePriority::CustomThenSystemThenWeb => {
                all_fonts.extend(self.scan_custom_sources()?);
                all_fonts.extend(self.scan_system_sources()?);
                all_fonts.extend(self.scan_web_sources()?);
            }
            FontSourcePriority::AllCustomFirst => {
                all_fonts.extend(self.scan_custom_sources()?);
                all_fonts.extend(self.scan_system_sources()?);
                all_fonts.extend(self.scan_web_sources()?);
            }
            FontSourcePriority::AllWebFirst => {
                all_fonts.extend(self.scan_web_sources()?);
                all_fonts.extend(self.scan_system_sources()?);
                all_fonts.extend(self.scan_custom_sources()?);
            }
            FontSourcePriority::PriorityList(_) => {
                // Custom order - for now use default
                all_fonts.extend(self.scan_system_sources()?);
                all_fonts.extend(self.scan_web_sources()?);
                all_fonts.extend(self.scan_custom_sources()?);
            }
        }
        
        // Cache the fonts
        for font in &all_fonts {
            self.font_cache.insert(font.family.clone(), font.clone());
        }
        
        Ok(all_fonts)
    }
    
    fn scan_system_sources(&self) -> FontResult<Vec<FontDescriptor>> {
        // Use the existing font-scanner crate
        let scanner = font_scanner::FontScanner;
        scanner.scan_system_fonts()
            .map_err(|e| FontError::Parse(format!("System scan failed: {}", e)))
    }
    
    fn scan_web_sources(&self) -> FontResult<Vec<FontDescriptor>> {
        let mut fonts = Vec::new();
        
        if let Some(db) = &self.web_db {
            // Use the public getter method instead of accessing private field
            for web_font in db.get_fonts().values() {
                if let Some(variant) = web_font.variants.iter()
                    .find(|v| v.weight == 400 && !v.italic) 
                    .or_else(|| web_font.variants.first()) {
                    
                    let font_descriptor = db.to_font_descriptor(web_font, variant);
                    fonts.push(font_descriptor);
                }
            }
        }
        
        Ok(fonts)
    }
    
    fn scan_custom_sources(&self) -> FontResult<Vec<FontDescriptor>> {
        let mut fonts = Vec::new();
        
        for source in &self.custom_sources {
            match source {
                SourceType::File(path) => {
                    if let Ok(font) = self.parser.parse_font_file(path) {
                        fonts.push(font);
                    }
                }
                SourceType::Directory(path) => {
                    fonts.extend(self.scan_directory(path)?);
                }
                SourceType::Url(_) | SourceType::Api { .. } => {
                    // URL sources require async download - skip for now
                    continue;
                }
                _ => {}
            }
        }
        
        Ok(fonts)
    }
    
    fn scan_directory(&self, dir: &Path) -> FontResult<Vec<FontDescriptor>> {
        let mut fonts = Vec::new();
        
        for entry in WalkDir::new(dir)
            .follow_links(true)
            .into_iter()
            .filter_map(Result::ok) 
        {
            let path = entry.path();
            
            if path.is_file() && self.is_font_file(path) {
                if let Ok(font) = self.parser.parse_font_file(path) {
                    fonts.push(font);
                }
            }
        }
        
        Ok(fonts)
    }
    
    fn is_font_file(&self, path: &Path) -> bool {
        if let Some(ext) = path.extension() {
            let ext_lower = ext.to_string_lossy().to_lowercase();
            matches!(ext_lower.as_str(), "ttf" | "otf" | "woff" | "woff2" | "ttc")
        } else {
            false
        }
    }
    
    pub fn find_font(&mut self, family: &str) -> FontResult<Option<FontDescriptor>> {
        // Check cache first
        if let Some(font) = self.font_cache.get(family) {
            return Ok(Some(font.clone()));
        }
        
        // Scan sources if not in cache
        let fonts = self.scan_sources()?;
        
        // Find the font
        let font = fonts.into_iter()
            .find(|f| f.family.to_lowercase() == family.to_lowercase());
        
        // Cache if found
        if let Some(ref font) = font {
            self.font_cache.insert(family.to_string(), font.clone());
        }
        
        Ok(font)
    }
    
    pub fn list_sources(&self) -> Vec<SourceInfo> {
        let mut sources = Vec::new();
        
        // System sources
        for _source in &self.system_sources {  // Add underscore to silence warning
            sources.push(SourceInfo {
                source_type: "System".to_string(),
                description: "Built-in system fonts".to_string(),
                status: SourceStatus::Enabled,
                index: None,
            });
        }
        
        // Web sources
        for _source in &self.web_sources {  // Add underscore to silence warning
            let status = if self.web_db.is_some() {
                SourceStatus::Enabled
            } else {
                SourceStatus::Disabled
            };
            
            sources.push(SourceInfo {
                source_type: "Web".to_string(),
                description: "Google Fonts database".to_string(),
                status,
                index: None,
            });
        }
        
        // Custom sources (rest of the function remains the same)
        for (i, source) in self.custom_sources.iter().enumerate() {
            let (source_type, description) = match source {
                SourceType::File(path) => (
                    "File".to_string(),
                    format!("File: {}", path.display())
                ),
                SourceType::Directory(path) => (
                    "Directory".to_string(),
                    format!("Directory: {}", path.display())
                ),
                SourceType::Url(url) => (
                    "URL".to_string(),
                    format!("URL: {}", url)
                ),
                SourceType::Api { endpoint, .. } => (
                    "API".to_string(),
                    format!("API: {}", endpoint)
                ),
                _ => ("Unknown".to_string(), "Unknown source".to_string()),
            };
            
            sources.push(SourceInfo {
                source_type,
                description,
                status: SourceStatus::Enabled,
                index: Some(i),
            });
        }
        
        sources
    }
    
    pub fn get_priority(&self) -> &FontSourcePriority {
        &self.priority
    }
    
    pub fn get_web_db(&self) -> Option<&WebFontDatabase> {
        self.web_db.as_ref()
    }
    
    pub fn clear_cache(&mut self) {
        self.font_cache.clear();
    }
}

impl Clone for FontSourceManager {
    fn clone(&self) -> Self {
        Self {
            system_sources: self.system_sources.clone(),
            web_sources: self.web_sources.clone(),
            custom_sources: self.custom_sources.clone(),
            priority: self.priority.clone(),
            web_db: self.web_db.clone(),
            font_cache: self.font_cache.clone(),
            parser: FontParser,  // FontParser is unit struct, just create new instance
        }
    }
}

impl std::fmt::Debug for FontSourceManager {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("FontSourceManager")
            .field("system_sources", &self.system_sources)
            .field("web_sources", &self.web_sources)
            .field("custom_sources_count", &self.custom_sources.len())
            .field("priority", &self.priority)
            .field("web_db_loaded", &self.web_db.is_some())
            .field("font_cache_count", &self.font_cache.len())
            .finish()
    }
}

#[derive(Debug, Clone)]
pub struct SourceInfo {
    pub source_type: String,
    pub description: String,
    pub status: SourceStatus,
    pub index: Option<usize>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum SourceStatus {
    Enabled,
    Disabled,
    Error(String),
}