use std::path::{Path, PathBuf};
#[cfg(target_os = "windows")]
use winreg::enums::*;
#[cfg(target_os = "windows")]
use winreg::RegKey;
use walkdir::WalkDir;
use font_core::FontDescriptor;
use font_parser::FontParser;

pub struct FontScanner;

impl FontScanner {
    /// Scan system fonts based on the current operating system
    pub fn scan_system_fonts(&self) -> Result<Vec<FontDescriptor>, Box<dyn std::error::Error>> {
        let mut fonts: Vec<FontDescriptor> = Vec::new();
        let parser = FontParser;
        
        #[cfg(target_os = "windows")]
        {
            // 1. Scan fonts from Windows Registry
            match self.scan_fonts_from_registry(&parser) {
                Ok(registry_fonts) => {
                    fonts.extend(registry_fonts);
                }
                Err(_e) => {
                    // Silent fail for professional production
                }
            }
            
            // 2. Scan Windows Fonts directory
            let system_fonts_dir = PathBuf::from("C:\\Windows\\Fonts");
            if let Ok(dir_fonts) = self.scan_font_directory_recursive(&system_fonts_dir, &parser) {
                fonts.extend(dir_fonts);
            }

            // 3. Scan user fonts
            if let Ok(user_profile) = std::env::var("USERPROFILE") {
                let user_fonts_dir = PathBuf::from(user_profile).join("AppData\\Local\\Microsoft\\Windows\\Fonts");
                if let Ok(user_fonts) = self.scan_font_directory_recursive(&user_fonts_dir, &parser) {
                    fonts.extend(user_fonts);
                }
            }
        }

        #[cfg(target_os = "macos")]
        {
            let mac_paths = vec![
                PathBuf::from("/Library/Fonts"),
                PathBuf::from("/System/Library/Fonts"),
                dirs::home_dir().map(|h| h.join("Library/Fonts")).unwrap_or_default(),
            ];
            
            for path in mac_paths {
                if path.exists() {
                    if let Ok(dir_fonts) = self.scan_font_directory_recursive(&path, &parser) {
                        fonts.extend(dir_fonts);
                    }
                }
            }
        }

        #[cfg(target_os = "linux")]
        {
            let linux_paths = vec![
                PathBuf::from("/usr/share/fonts"),
                PathBuf::from("/usr/local/share/fonts"),
                dirs::home_dir().map(|h| h.join(".local/share/fonts")).unwrap_or_default(),
                dirs::home_dir().map(|h| h.join(".fonts")).unwrap_or_default(),
            ];
            
            for path in linux_paths {
                if path.exists() {
                    if let Ok(dir_fonts) = self.scan_font_directory_recursive(&path, &parser) {
                        fonts.extend(dir_fonts);
                    }
                }
            }
        }
        
        // Deduplicate by file path
        fonts.sort_by(|a, b| a.path.cmp(&b.path));
        fonts.dedup_by(|a, b| a.path == b.path);
        
        Ok(fonts)
    }
    
    #[cfg(target_os = "windows")]
    fn scan_fonts_from_registry(&self, parser: &FontParser) 
        -> Result<Vec<FontDescriptor>, Box<dyn std::error::Error>> 
    {
        let mut fonts = Vec::new();
        let hklm = RegKey::predef(HKEY_LOCAL_MACHINE);
        let font_key_path = "SOFTWARE\\Microsoft\\Windows NT\\CurrentVersion\\Fonts";
        
        if let Ok(font_key) = hklm.open_subkey(font_key_path) {
            for (_name, value) in font_key.enum_values().filter_map(|x| x.ok()) {
                let font_path_str = value.to_string();
                if font_path_str.trim().is_empty() { continue; }
                let font_path = self.resolve_windows_font_path(&font_path_str);
                if font_path.exists() && Self::is_font_file(&font_path) {
                    if let Ok(font) = parser.parse_font_file(&font_path) {
                        fonts.push(font);
                    }
                }
            }
        }
        Ok(fonts)
    }
    
    pub fn scan_font_directory_recursive(&self, dir: &Path, parser: &FontParser) 
        -> Result<Vec<FontDescriptor>, Box<dyn std::error::Error>> 
    {
        let mut fonts = Vec::new();
        if !dir.exists() { return Ok(fonts); }
        
        for entry in WalkDir::new(dir).follow_links(true).into_iter().filter_map(Result::ok) {
            let path = entry.path();
            if path.is_file() && Self::is_font_file(path) {
                if let Ok(font) = parser.parse_font_file(path) {
                    fonts.push(font);
                }
            }
        }
        Ok(fonts)
    }
    
    #[cfg(target_os = "windows")]
    fn resolve_windows_font_path(&self, font_path_str: &str) -> PathBuf {
        let font_path = PathBuf::from(font_path_str);
        if font_path.is_absolute() && font_path.exists() { return font_path; }
        
        let windir = std::env::var("WINDIR").unwrap_or_else(|_| "C:\\Windows".to_string());
        let possible_dirs = vec![
            PathBuf::from(windir).join("Fonts"),
            PathBuf::from("C:\\Windows\\Fonts"),
        ];
        
        for dir in possible_dirs {
            let full_path = dir.join(&font_path);
            if full_path.exists() { return full_path; }
            if let Some(file_name) = font_path.file_name() {
                let full_path = dir.join(file_name);
                if full_path.exists() { return full_path; }
            }
        }
        font_path
    }
    
    fn is_font_file(path: &Path) -> bool {
        if let Some(ext) = path.extension() {
            let ext_lower = ext.to_string_lossy().to_lowercase();
            matches!(ext_lower.as_str(), "ttf" | "otf" | "woff" | "woff2" | "ttc")
        } else {
            false
        }
    }
}