// use font_core::{FontDescriptor, LicenseInfo, FontError};
// use regex::Regex;
use font_core::FontDescriptor;
use once_cell::sync::Lazy;
use std::collections::HashSet;

pub struct LicenseChecker {
    commercial_fonts: HashSet<String>,
    commercial_postscript: HashSet<String>,
    free_alternatives: Vec<FreeAlternative>,
}

#[derive(Debug, Clone)]
pub struct LicenseWarning {
    pub font_name: String,
    pub license_type: LicenseType,
    pub warning_level: WarningLevel,
    pub message: String,
    pub alternatives: Vec<FreeAlternative>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum LicenseType {
    OpenSource,
    Commercial,
    Unknown,
    SystemEmbedded, // Fonts that come with OS (may have restrictions)
}

#[derive(Debug, Clone, PartialEq)]
pub enum WarningLevel {
    Info,
    Warning,
    Critical,
}

#[derive(Debug, Clone)]
pub struct FreeAlternative {
    pub family: String,
    pub similarity_score: f32,
    pub license: LicenseType,
    pub reason: String,
}

impl LicenseChecker {
    pub fn new() -> Self {
        static COMMERCIAL_FONTS: Lazy<HashSet<String>> = Lazy::new(|| {
            let mut set = HashSet::new();
            // Known commercial font families
            set.insert("helvetica".to_string());
            set.insert("helvetica neue".to_string());
            set.insert("futura".to_string());
            set.insert("gill sans".to_string());
            set.insert("optima".to_string());
            set.insert("palatino".to_string());
            set.insert("didot".to_string());
            set.insert("bembo".to_string());
            set.insert("garamond premium".to_string());
            set.insert("minion pro".to_string());
            set.insert("myriad pro".to_string());
            set.insert("trajan pro".to_string());
            set.insert("univers".to_string());
            set.insert("franklin gothic".to_string());
            set.insert("copperplate gothic".to_string());
            set
        });
        
        static COMMERCIAL_POSTSCRIPT: Lazy<HashSet<String>> = Lazy::new(|| {
            let mut set = HashSet::new();
            // Known commercial PostScript names
            set.insert("helveticaneue".to_string());
            set.insert("helveticaneuepro".to_string());
            set.insert("futura".to_string());
            set.insert("gill-sans".to_string());
            set.insert("optima".to_string());
            set.insert("palatino".to_string());
            set.insert("didot".to_string());
            set.insert("bembo".to_string());
            set
        });
        
        let free_alternatives = vec![
            FreeAlternative {
                family: "Roboto".to_string(),
                similarity_score: 0.9,
                license: LicenseType::OpenSource,
                reason: "Apache 2.0 license, similar to Helvetica".to_string(),
            },
            FreeAlternative {
                family: "Open Sans".to_string(),
                similarity_score: 0.85,
                license: LicenseType::OpenSource,
                reason: "Apache 2.0 license, humanist sans-serif".to_string(),
            },
            FreeAlternative {
                family: "Lato".to_string(),
                similarity_score: 0.8,
                license: LicenseType::OpenSource,
                reason: "OFL license, professional sans-serif".to_string(),
            },
            FreeAlternative {
                family: "Montserrat".to_string(),
                similarity_score: 0.75,
                license: LicenseType::OpenSource,
                reason: "OFL license, geometric sans-serif".to_string(),
            },
            FreeAlternative {
                family: "Source Sans Pro".to_string(),
                similarity_score: 0.7,
                license: LicenseType::OpenSource,
                reason: "OFL license, Adobe's first open source font".to_string(),
            },
            FreeAlternative {
                family: "Noto Sans".to_string(),
                similarity_score: 0.9,
                license: LicenseType::OpenSource,
                reason: "OFL license, Google's universal font".to_string(),
            },
            FreeAlternative {
                family: "Liberation Sans".to_string(),
                similarity_score: 0.95,
                license: LicenseType::OpenSource,
                reason: "SIL Open Font License, metric-compatible with Arial".to_string(),
            },
            FreeAlternative {
                family: "DejaVu Sans".to_string(),
                similarity_score: 0.8,
                license: LicenseType::OpenSource,
                reason: "Bitstream Vera License, extensive character set".to_string(),
            },
        ];
        
        Self {
            commercial_fonts: COMMERCIAL_FONTS.clone(),
            commercial_postscript: COMMERCIAL_POSTSCRIPT.clone(),
            free_alternatives,
        }
    }
    
    pub fn check_font(&self, font: &FontDescriptor) -> LicenseWarning {
        let license_type = self.detect_license_type(font);
        let warning_level = self.determine_warning_level(&license_type, font);
        let alternatives = self.find_alternatives(font);
        
        let message = match license_type {
            LicenseType::Commercial => {
                format!("Commercial font '{}' may require a license for distribution.", font.family)
            }
            LicenseType::SystemEmbedded => {
                format!("System font '{}' may have redistribution restrictions.", font.family)
            }
            LicenseType::Unknown => {
                format!("License for '{}' is unknown. Verify before distribution.", font.family)
            }
            LicenseType::OpenSource => {
                format!("Open source font '{}' is safe for distribution.", font.family)
            }
        };
        
        LicenseWarning {
            font_name: font.family.clone(),
            license_type,
            warning_level,
            message,
            alternatives,
        }
    }
    
    fn detect_license_type(&self, font: &FontDescriptor) -> LicenseType {
        // Check if we have license info in the font metadata
        if let Some(license_info) = &font.license {
            let license_name = license_info.name.to_lowercase();
            
            if license_name.contains("commercial") || 
               license_name.contains("proprietary") ||
               license_name.contains("copyright") {
                return LicenseType::Commercial;
            }
            
            if license_name.contains("ofl") ||
               license_name.contains("sil") ||
               license_name.contains("apache") ||
               license_name.contains("mit") ||
               license_name.contains("bsd") ||
               license_name.contains("gpl") {
                return LicenseType::OpenSource;
            }
        }
        
        // Check against known commercial fonts
        let font_lower = font.family.to_lowercase();
        let postscript_lower = font.postscript_name.to_lowercase();
        
        if self.commercial_fonts.contains(&font_lower) ||
           self.commercial_postscript.iter().any(|ps| postscript_lower.contains(ps)) {
            return LicenseType::Commercial;
        }
        
        // Check for system fonts (Windows, macOS)
        if self.is_system_font(font) {
            return LicenseType::SystemEmbedded;
        }
        
        // Check for known open source fonts
        if self.is_open_source_font(font) {
            return LicenseType::OpenSource;
        }
        
        LicenseType::Unknown
    }
    
    fn is_system_font(&self, font: &FontDescriptor) -> bool {
        let path_str = font.path.to_string_lossy().to_lowercase();
        
        // Common system font directories
        path_str.contains("windows\\fonts") ||
        path_str.contains("system\\library\\fonts") ||
        path_str.contains("/usr/share/fonts") ||
        path_str.contains("/system/fonts")
    }
    
    fn is_open_source_font(&self, font: &FontDescriptor) -> bool {
        let font_lower = font.family.to_lowercase();
        
        // Known open source font families
        font_lower.contains("noto") ||
        font_lower.contains("roboto") ||
        font_lower.contains("open") ||
        font_lower.contains("source") ||
        font_lower.contains("ubuntu") ||
        font_lower.contains("dejavu") ||
        font_lower.contains("liberation") ||
        font_lower.contains("fira") ||
        font_lower.contains("lato") ||
        font_lower.contains("montserrat") ||
        font_lower.contains("raleway") ||
        font_lower.contains("pt ") || // PT Sans, PT Serif
        font_lower.contains("droid")
    }
    
    fn determine_warning_level(&self, license_type: &LicenseType, font: &FontDescriptor) -> WarningLevel {
        match license_type {
            LicenseType::Commercial => {
                // Check if it's a very common commercial font
                let font_lower = font.family.to_lowercase();
                let common_system_fonts = [
                    "arial", "times new roman", "courier new", "verdana",
                    "tahoma", "segoe ui", "calibri", "cambria", "consolas",
                    "ms sans serif", "ms serif", "wingdings"
                ];
                
                if common_system_fonts.contains(&font_lower.as_str()) {
                    WarningLevel::Info  // Common system font - just informational
                } else {
                    WarningLevel::Warning  // Less common commercial font
                }
            }
            LicenseType::SystemEmbedded => WarningLevel::Info,  // Changed from Warning to Info
            LicenseType::Unknown => WarningLevel::Info,
            LicenseType::OpenSource => WarningLevel::Info,
        }
    }
    
    fn find_alternatives(&self, font: &FontDescriptor) -> Vec<FreeAlternative> {
        let font_lower = font.family.to_lowercase();
        let mut alternatives = Vec::new();
        
        // Match based on font characteristics
        let is_serif = font_lower.contains("serif") || 
                      font_lower.contains("times") ||
                      font_lower.contains("garamond") ||
                      font_lower.contains("baskerville");
        
        let is_sans_serif = font_lower.contains("sans") || 
                           font_lower.contains("helvetica") ||
                           font_lower.contains("arial") ||
                           font_lower.contains("futura");
        
        let is_monospace = font.monospaced || 
                          font_lower.contains("mono") ||
                          font_lower.contains("console") ||
                          font_lower.contains("courier");
        
        // Filter and score alternatives
        for alt in &self.free_alternatives {
            let alt_lower = alt.family.to_lowercase();
            
            let matches_style = if is_serif {
                alt_lower.contains("serif") || alt_lower.contains("noto serif")
            } else if is_sans_serif {
                alt_lower.contains("sans") && !alt_lower.contains("serif")
            } else if is_monospace {
                alt_lower.contains("mono") || alt_lower.contains("source code")
            } else {
                true // Generic match
            };
            
            if matches_style {
                // Adjust score based on specific matches
                let mut score = alt.similarity_score;
                
                // Boost score for direct alternatives
                if font_lower.contains("helvetica") && alt_lower.contains("roboto") {
                    score = 0.95;
                } else if font_lower.contains("arial") && alt_lower.contains("liberation sans") {
                    score = 0.98; // Metric compatible
                } else if font_lower.contains("times") && alt_lower.contains("liberation serif") {
                    score = 0.98; // Metric compatible
                } else if font_lower.contains("courier") && alt_lower.contains("liberation mono") {
                    score = 0.98; // Metric compatible
                }
                
                let mut alt_clone = alt.clone();
                alt_clone.similarity_score = score;
                alternatives.push(alt_clone);
            }
        }
        
        // Sort by similarity score (highest first)
        alternatives.sort_by(|a, b| {
            b.similarity_score.partial_cmp(&a.similarity_score).unwrap()
        });
        
        // Take top 3
        alternatives.truncate(3);
        
        alternatives
    }
    
    pub fn generate_report(&self, fonts: &[FontDescriptor]) -> LicenseReport {
        let mut warnings = Vec::new();
        let mut has_critical = false;
        let mut has_warning = false;
        
        for font in fonts {
            let warning = self.check_font(font);
            if warning.warning_level == WarningLevel::Critical {
                has_critical = true;
            } else if warning.warning_level == WarningLevel::Warning {
                has_warning = true;
            }
            warnings.push(warning);
        }
        
        LicenseReport {
            warnings,
            has_critical,
            has_warning,
            total_fonts: fonts.len(),
        }
    }
    
    pub fn get_license_summary(&self, font: &FontDescriptor) -> String {
        let warning = self.check_font(font);
        
        match warning.license_type {
            LicenseType::OpenSource => {
                "✅ Open Source - Safe for distribution".to_string()
            }
            LicenseType::Commercial => {
                "❌ Commercial - May require license".to_string()
            }
            LicenseType::SystemEmbedded => {
                "⚠️  System Font - Check redistribution rights".to_string()
            }
            LicenseType::Unknown => {
                "❓ Unknown - Verify license before use".to_string()
            }
        }
    }
}

#[derive(Debug, Clone)]
pub struct LicenseReport {
    pub warnings: Vec<LicenseWarning>,
    pub has_critical: bool,
    pub has_warning: bool,
    pub total_fonts: usize,
}

impl LicenseReport {
    pub fn to_markdown(&self) -> String {
        let mut md = String::new();
        
        md.push_str("# Font License Report\n\n");
        md.push_str(&format!("Total fonts analyzed: {}\n\n", self.total_fonts));
        
        if self.has_critical {
            md.push_str("## ⚠️ Critical Issues\n\n");
            md.push_str("The following fonts may require licenses:\n\n");
            
            for warning in &self.warnings {
                if warning.warning_level == WarningLevel::Critical {
                    md.push_str(&format!("### {}\n", warning.font_name));
                    md.push_str(&format!("{}\n\n", warning.message));
                    
                    if !warning.alternatives.is_empty() {
                        md.push_str("**Free alternatives:**\n");
                        for alt in &warning.alternatives {
                            md.push_str(&format!("- {} ({:.0}% similar) - {}\n", 
                                alt.family, alt.similarity_score * 100.0, alt.reason));
                        }
                        md.push_str("\n");
                    }
                }
            }
        }
        
        if self.has_warning {
            md.push_str("## ⚠️ Warnings\n\n");
            
            for warning in &self.warnings {
                if warning.warning_level == WarningLevel::Warning {
                    md.push_str(&format!("### {}\n", warning.font_name));
                    md.push_str(&format!("{}\n\n", warning.message));
                }
            }
        }
        
        // Info section
        let info_count = self.warnings.iter()
            .filter(|w| w.warning_level == WarningLevel::Info)
            .count();
        
        if info_count > 0 {
            md.push_str("## ℹ️ Information\n\n");
            md.push_str("The following fonts appear to be safe:\n\n");
            
            for warning in &self.warnings {
                if warning.warning_level == WarningLevel::Info {
                    md.push_str(&format!("- {}: {}\n", warning.font_name, warning.message));
                }
            }
        }
        
        md
    }
}