use regex::Regex;
use lazy_static::lazy_static;
use font_core::{FontRequest, FontStyle, FontError};

pub struct FontNormalizer;

impl FontNormalizer {
    pub fn normalize(&self, font_name: &str) -> Result<FontRequest, FontError> {
        let without_subset = Self::remove_subset_prefix(font_name);
        let without_encoding = Self::remove_encoding_suffix(&without_subset);
        let (weight, _style, italic) = Self::extract_weight_style(&without_encoding); 
        let family = Self::extract_family_name(&without_encoding);
        let normalized_family = Self::normalize_family_name(&family);
        
        // Check for monospaced indicators
        let original_lower = font_name.to_lowercase();
        let monospaced = original_lower.contains("mono") 
            || original_lower.contains("console") 
            || original_lower.contains("typewriter")
            || original_lower.contains("courier")
            || original_lower.contains("fixedsys")
            || original_lower.contains("terminal");
        
        // Also check family name for monospace indicators
        let family_lower = family.to_lowercase();
        let family_monospaced = family_lower.contains("mono") 
            || family_lower.contains("console") 
            || family_lower.contains("typewriter")
            || family_lower.contains("courier")
            || family_lower.contains("fixedsys")
            || family_lower.contains("terminal");
        
        let final_monospaced = monospaced || family_monospaced;

        Ok(FontRequest {
            original_name: font_name.to_string(),
            normalized_name: normalized_family.clone(),
            family: normalized_family,
            weight,
            style: if italic { FontStyle::Italic } else { FontStyle::Normal },
            italic,
            monospaced: final_monospaced,
        })
    }
    
    fn remove_subset_prefix(name: &str) -> String {
        lazy_static! {
            static ref RE: Regex = Regex::new(r"^[A-Z]{6}\+").unwrap();
        }
        RE.replace(name, "").to_string()
    }
    
    fn remove_encoding_suffix(name: &str) -> String {
        lazy_static! {
            static ref RE: Regex = Regex::new(
                r"-(Identity|WinAnsi|MacRoman|Uni[A-Z]+|W1|W2|W3|W4|W5|W6|Com|Expert|Subset|It|Oblique)(-H)?$"
            ).unwrap();
        }
        let after_hyphen = RE.replace(name, "").to_string();
        
        // Also remove common suffixes without hyphens
        let suffixes = ["MT", "PS", "PSMT", "Std", "Pro", "Regular", "Bold", "Italic"];
        let mut result = after_hyphen;
        
        for suffix in &suffixes {
            if result.ends_with(suffix) && result.len() > suffix.len() {
                // Check if it's preceded by a hyphen or part of the name
                let slice = &result[..result.len() - suffix.len()];
                // Only remove if it's at the end and not part of a word
                let last_char = slice.chars().last();
                if slice.ends_with('-') || last_char.map_or(false, |c| c.is_uppercase()) {
                    result = slice.to_string();
                    break;
                }
            }
        }
        result
    }
    
    fn extract_weight_style(name: &str) -> (u16, FontStyle, bool) {
        let lower = name.to_lowercase();
        
        let weight = if lower.contains("thin") || lower.contains("hairline") {
            100
        } else if lower.contains("extralight") || lower.contains("ultralight") {
            200
        } else if lower.contains("light") {
            300
        } else if lower.contains("normal") || lower.contains("regular") || lower.contains("book") {
            400
        } else if lower.contains("medium") {
            500
        } else if lower.contains("semibold") || lower.contains("demibold") {
            600
        } else if lower.contains("bold") {
            700
        } else if lower.contains("extrabold") || lower.contains("ultrabold") {
            800
        } else if lower.contains("black") || lower.contains("heavy") {
            900
        } else {
            400
        };
        
        let (style, italic) = if lower.contains("italic") {
            (FontStyle::Italic, true)
        } else if lower.contains("oblique") {
            (FontStyle::Oblique, true)
        } else {
            (FontStyle::Normal, false)
        };
        
        (weight, style, italic)
    }
    
    fn split_camel_case(name: &str) -> String {
        let mut result = String::new();
        let chars: Vec<char> = name.chars().collect();
        
        for i in 0..chars.len() {
            if i > 0 {
                let current = chars[i];
                let prev = chars[i-1];
                
                // Insert hyphen when:
                // 1. lowercase to uppercase (TimesNew -> Times-New)
                // 2. digit to letter or vice versa
                // 3. But not if both are uppercase (like "PS" in "TimesNewRomanPS")
                if prev.is_lowercase() && current.is_uppercase() {
                    result.push('-');
                } else if prev.is_ascii_digit() && current.is_alphabetic() {
                    result.push('-');
                } else if prev.is_alphabetic() && current.is_ascii_digit() {
                    result.push('-');
                }
            }
            result.push(chars[i]);
        }
        
        result
    }


    fn extract_family_name(name: &str) -> String {
        let mut result = name.to_string();
        
        // First, try to split camel case (TimesNewRoman -> Times New Roman)
        result = Self::split_camel_case(&result);
        
        let keywords = [
            "thin", "extralight", "ultralight", "light", "normal",
            "regular", "medium", "semibold", "demibold", "bold",
            "extrabold", "ultrabold", "black", "heavy", "italic",
            "oblique", "book", "hairline", "condensed", "expanded",
            "narrow", "wide", "mono", "typewriter", "console",
        ];
        
        for keyword in &keywords {
            let pattern = format!(r"(?i)\b{}\b", keyword);
            if let Ok(re) = Regex::new(&pattern) {
                result = re.replace_all(&result, "").to_string();
            }
        }
        
        result = result.trim_matches(|c| c == '-' || c == ' ' || c == '_').to_string();
        
        // Remove any remaining PS, MT, etc.
        result = Self::remove_remaining_suffixes(&result);
        
        result
    }
    

    fn remove_remaining_suffixes(name: &str) -> String {
        let suffixes = [
            "MT", "PS", "PSMT", "Std", "Pro", "TT", "OT", "WOFF", "WOFF2"
        ];
        
        let mut result = name.to_string();
        for suffix in &suffixes {
            if result.ends_with(suffix) && result.len() > suffix.len() {
                // Check if it's a separate suffix (preceded by non-alphanumeric)
                let prefix = &result[..result.len() - suffix.len()];
                if !prefix.is_empty() && !prefix.chars().last().unwrap().is_alphanumeric() {
                    result = prefix.to_string();
                    break;
                }
            }
        }
        
        result.trim_matches(|c| c == '-' || c == ' ' || c == '_').to_string()
    }

    fn normalize_family_name(name: &str) -> String {
        let mut result = name.to_lowercase();
        result = result.replace(' ', "-").replace('_', "-");
        
        let re = Regex::new(r"[^a-z0-9\-]").unwrap();
        result = re.replace_all(&result, "").to_string();
        
        let re = Regex::new(r"-+").unwrap();
        result = re.replace_all(&result, "-").to_string();
        
        result.trim_matches('-').to_string()
    }
    
    pub fn get_common_mappings(&self) -> Vec<(&'static str, &'static str)> {
        vec![
            ("helvetica", "arial"),
            ("helvetica-neue", "arial"),
            ("times", "times-new-roman"),
            ("times-roman", "times-new-roman"),
            ("courier", "courier-new"),
            ("zapfdingbats", "zapf-dingbats"),
            ("symbol", "symbola"),
            ("wingdings", "wingdings"),
            ("calibri", "calibri"),
            ("cambria", "cambria"),
            ("consolas", "consolas"),
        ]
    }
}