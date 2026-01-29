use std::fs;
use std::path::Path;
use ttf_parser::Face;
use font_core::{FontDescriptor, FontFormat, FontMetrics, LicenseInfo, FontError};

#[derive(Debug, Clone)]
pub struct FontParser;

impl FontParser {
    pub fn parse_font_file<P: AsRef<Path>>(&self, path: P) -> Result<FontDescriptor, FontError> {
        let path = path.as_ref();
        
        // Read font file
        let font_data = fs::read(path).map_err(FontError::Io)?;
        
        // Parse with ttf-parser
        let face = Face::parse(&font_data, 0)
            .map_err(|e| FontError::Parse(format!("Failed to parse font: {}", e)))?;
        
        // Extract metadata
        let family = self.extract_string(&face, ttf_parser::name_id::FAMILY)
            .unwrap_or_else(|| "Unknown".to_string());
            
        let subfamily = self.extract_string(&face, ttf_parser::name_id::SUBFAMILY);
        let postscript_name = self.extract_string(&face, ttf_parser::name_id::POST_SCRIPT_NAME)
            .unwrap_or_else(|| "Unknown".to_string());
            
        let full_name = self.extract_string(&face, ttf_parser::name_id::FULL_NAME);
        
        // Determine format
        let format = self.determine_format(path, &font_data);
        
        // Extract weight and style
        let (weight, italic) = self.extract_weight_style(&face);
        
        // Check if monospaced
        let monospaced = self.is_monospaced(&face);
        
        // Check if variable font
        let variable = face.is_variable();
        
        // Extract metrics
        let metrics = self.extract_metrics(&face);
        
        // Detect license
        let license = self.detect_license(&face, &family);
        
        Ok(FontDescriptor {
            family,
            subfamily,
            postscript_name,
            full_name,
            path: path.to_path_buf(),
            format,
            weight,
            italic,
            monospaced,
            variable,
            metrics,
            license,
        })
    }
    
    fn extract_string(&self, face: &Face, name_id: u16) -> Option<String> {
        face.names()
            .into_iter()
            .find(|name| name.name_id == name_id)
            .and_then(|name| name.to_string())
    }
    
    fn determine_format(&self, path: &Path, data: &[u8]) -> FontFormat {
        // Check by extension
        if let Some(ext) = path.extension() {
            match ext.to_str().unwrap_or("").to_lowercase().as_str() {
                "ttf" | "ttc" => return FontFormat::Ttf,
                "otf" => return FontFormat::Otf,
                "woff" => return FontFormat::Woff,
                "woff2" => return FontFormat::Woff2,
                _ => {}
            }
        }
        
        // Check by magic bytes
        if data.len() >= 4 {
            match &data[0..4] {
                b"OTTO" => return FontFormat::Otf,
                b"ttcf" => return FontFormat::Ttf,
                b"wOFF" => return FontFormat::Woff,
                b"wOF2" => return FontFormat::Woff2,
                _ => {
                    if u32::from_be_bytes([data[0], data[1], data[2], data[3]]) == 0x00010000 {
                        return FontFormat::Ttf;
                    }
                }
            }
        }
        
        FontFormat::Other
    }
    
    fn extract_weight_style(&self, face: &Face) -> (u16, bool) {
        let weight = match face.weight() {
            ttf_parser::Weight::Thin => 100,
            ttf_parser::Weight::ExtraLight => 200,
            ttf_parser::Weight::Light => 300,
            ttf_parser::Weight::Normal => 400,
            ttf_parser::Weight::Medium => 500,
            ttf_parser::Weight::SemiBold => 600,
            ttf_parser::Weight::Bold => 700,
            ttf_parser::Weight::ExtraBold => 800,
            ttf_parser::Weight::Black => 900,
            ttf_parser::Weight::Other(value) => value as u16,
        };
        
        let italic = face.is_italic();
        
        (weight, italic)
    }
    
    fn is_monospaced(&self, face: &Face) -> bool {
        // Check first 100 glyphs for consistent width
        if face.number_of_glyphs() == 0 {
            return false;
        }
        
        let mut widths = std::collections::HashSet::new();
        for i in 0..std::cmp::min(100, face.number_of_glyphs()) {
            if let Some(width) = face.glyph_hor_advance(ttf_parser::GlyphId(i)) {
                widths.insert(width);
                if widths.len() > 1 {
                    return false;
                }
            }
        }
        
        widths.len() == 1
    }
    
    fn extract_metrics(&self, face: &Face) -> Option<FontMetrics> {
        let units_per_em = face.units_per_em();
        let ascender = face.ascender();
        let descender = face.descender();
        
        // x-height (height of lowercase 'x')
        let x_height = face.x_height().unwrap_or_else(|| {
            (ascender as f32 * 0.48) as i16
        });
        
        // Capital height
        let cap_height = face.capital_height().unwrap_or_else(|| {
            (ascender as f32 * 0.7) as i16
        });
        
        // Calculate average width of common characters
        let common_chars = "abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789";
        let mut total_width: u32 = 0; // Use u32 to prevent overflow
        let mut count = 0;
        
        for ch in common_chars.chars() {
            if let Some(glyph_id) = face.glyph_index(ch) {
                if let Some(advance) = face.glyph_hor_advance(glyph_id) {
                    total_width += advance as u32; // Cast to u32
                    count += 1;
                }
            }
        }
        
        let average_width = if count > 0 {
            (total_width / count as u32) as i16
        } else {
            0
        };
        
        // Find max advance width
        let mut max_advance: u16 = 0;
        for i in 0..std::cmp::min(200, face.number_of_glyphs()) {
            if let Some(advance) = face.glyph_hor_advance(ttf_parser::GlyphId(i)) {
                if advance > max_advance {
                    max_advance = advance;
                }
            }
        }
        
        Some(FontMetrics {
            units_per_em,
            ascender,
            descender,
            x_height,
            cap_height,
            average_width,
            max_advance_width: max_advance,
        })
    }
    
    fn detect_license(&self, face: &Face, family: &str) -> Option<LicenseInfo> {
        // Try to extract license info from font names
        let license_text = self.extract_string(face, 13)  // 13 = License Description in OpenType spec
        .or_else(|| self.extract_string(face, 14));  // 14 = License URL
        
        // Simple license detection based on family name patterns
        let family_lower = family.to_lowercase();
        let (name, allows_embedding, allows_modification, requires_attribution, allows_commercial_use) = 
            if family_lower.contains("noto") {
                ("SIL Open Font License".to_string(), true, true, false, true)
            } else if family_lower.contains("liberation") {
                ("SIL Open Font License".to_string(), true, true, false, true)
            } else if family_lower.contains("dejavu") {
                ("Bitstream Vera License".to_string(), true, true, true, true)
            } else if family_lower.contains("roboto") {
                ("Apache License 2.0".to_string(), true, true, true, true)
            } else if family_lower.contains("opensans") || family_lower.contains("open sans") {
                ("Apache License 2.0".to_string(), true, true, true, true)
            } else {
                // Unknown - assume restrictive
                ("Unknown (Commercial?)".to_string(), false, false, true, false)
            };
        
        Some(LicenseInfo {
            name,
            url: license_text,
            allows_embedding,
            allows_modification,
            requires_attribution,
            allows_commercial_use, // ADDED THIS FIELD
        })
    }
}