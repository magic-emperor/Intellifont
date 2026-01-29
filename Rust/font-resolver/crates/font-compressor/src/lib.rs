use std::collections::{HashMap, HashSet};
use serde::{Serialize, Deserialize};
use font_core::{FontDescriptor, FontFormat};

// Add Write trait import
use std::io::{Write, Cursor};

// Fix FontCategory to derive Eq and Hash
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum FontCategory {
    Serif,
    SansSerif,
    Monospace,
    Display,
    Handwriting,
    Decorative,
    Symbol,
    Other,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompressedFontData {
    pub family: String,
    pub postscript_name: String,
    pub weight: u16,
    pub italic: bool,
    pub monospaced: bool,
    pub metrics: Option<CompressedMetrics>,
    pub license: CompressedLicense,
    pub category: FontCategory,
    pub similar_fonts: Vec<String>,
    pub download_urls: HashMap<FontFormat, String>,
    pub file_size_kb: u32,
    pub popularity: u8, // 0-100
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompressedMetrics {
    pub units_per_em: u16,
    pub ascender: i16,
    pub descender: i16,
    pub x_height: i16,
    pub cap_height: i16,
    pub average_width: i16,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompressedLicense {
    pub name: String,
    pub url: String,
    pub allows_embedding: bool,
    pub allows_modification: bool,
    pub requires_attribution: bool,
    pub allows_commercial_use: bool, // ADD THIS FIELD
}

pub struct FontCompressor {
    quality: u32, // 0-11 for brotli
    include_full_data: bool,
}


impl FontCompressor {
    pub fn new(quality: u32, include_full_data: bool) -> Self {
        Self {
            quality: quality.min(11),
            include_full_data,
        }
    }
    
    /// Compress font database with intelligent strategy - HANDLES DUPLICATES
    pub fn compress_font_database(
        &self,
        fonts: &[FontDescriptor],
        include_similarity_data: bool,
    ) -> Result<Vec<u8>, String> {
        // 1. First, remove duplicate fonts
        let unique_fonts = self.remove_duplicates(fonts);
        
        // 2. Convert to compressed format
        let compressed_fonts: Vec<CompressedFontData> = unique_fonts
            .iter()
            .map(|font| self.font_to_compressed(font))
            .collect();
        
        // 3. Build similarity matrix BEFORE creating database
        let similarity_matrix = if include_similarity_data {
            Some(self.build_similarity_matrix(&compressed_fonts))
        } else {
            None
        };
        
        // 4. Create metadata (with placeholder for compressed_size_bytes)
        let metadata = FontDatabaseMetadata {
            version: env!("CARGO_PKG_VERSION").to_string(),
            font_count: compressed_fonts.len(),
            compressed_size_bytes: 0, // Placeholder - will be updated later
            original_size_bytes: self.calculate_original_size(&unique_fonts),
            created_at: chrono::Utc::now().to_rfc3339(),
            categories: self.categorize_fonts(&compressed_fonts),
            include_full_data: self.include_full_data,
        };
        
        // 5. Create database with the pre-built similarity matrix
        let mut database = CompressedFontDatabase {
            metadata,
            fonts: compressed_fonts,
            similarity_matrix,
        };
        
        // 6. Serialize with bincode
        let serialized = bincode::serialize(&database)
            .map_err(|e| format!("Serialization failed: {}", e))?;
        
        // 7. Compress with brotli - FIX: Use scope to ensure encoder is dropped
        let mut compressed_data = Vec::new();
        
        {
            // Create encoder in a separate scope
            let mut encoder = brotli::CompressorWriter::new(
                &mut compressed_data,
                4096, // buffer size
                self.quality,
                22, // brotli window size
            );
            
            // Write data to encoder
            let mut cursor = Cursor::new(serialized);
            std::io::copy(&mut cursor, &mut encoder)
                .map_err(|e| format!("Compression failed: {}", e))?;
            
            // IMPORTANT: Flush encoder
            encoder.flush()
                .map_err(|e| format!("Flush failed: {}", e))?;
            
            // encoder is dropped here when it goes out of scope
        }
        
        // 8. Update the compressed size in the metadata
        // We need to update both the compressed_data metadata and the returned database
        database.metadata.compressed_size_bytes = compressed_data.len();
        
        // Re-serialize with updated metadata
        let final_serialized = bincode::serialize(&database)
            .map_err(|e| format!("Serialization failed: {}", e))?;
        
        // Recompress with updated metadata
        let mut final_compressed_data = Vec::new();
        {
            let mut encoder = brotli::CompressorWriter::new(
                &mut final_compressed_data,
                4096,
                self.quality,
                22,
            );
            
            let mut cursor = Cursor::new(final_serialized);
            std::io::copy(&mut cursor, &mut encoder)
                .map_err(|e| format!("Compression failed: {}", e))?;
            
            encoder.flush()
                .map_err(|e| format!("Flush failed: {}", e))?;
        }
        
        // 9. Return compressed data with correct metadata
        Ok(final_compressed_data)
    }
    
    /// Remove duplicate fonts based on family + weight + italic combination
    fn remove_duplicates(&self, fonts: &[FontDescriptor]) -> Vec<FontDescriptor> {
        let mut seen = HashSet::new();
        let mut unique_fonts = Vec::new();
        
        for font in fonts {
            let key = format!("{}-{}-{}", 
                font.family.to_lowercase(), 
                font.weight, 
                font.italic
            );
            
            if !seen.contains(&key) {
                seen.insert(key);
                unique_fonts.push(font.clone());
            } else {
                println!("⚠️  Removing duplicate font: {} (weight: {}, italic: {})", 
                    font.family, font.weight, font.italic);
            }
        }
        
        println!("📊 Removed {} duplicate fonts, kept {}", 
            fonts.len() - unique_fonts.len(), 
            unique_fonts.len());
        
        unique_fonts
    }
    
    /// Decompress font database
    pub fn decompress_font_database(
        &self,
        compressed_data: &[u8],
    ) -> Result<CompressedFontDatabase, String> {
        let mut decompressed = Vec::new();
        let mut decoder = brotli::Decompressor::new(compressed_data, 4096);
        
        std::io::copy(&mut decoder, &mut decompressed)
            .map_err(|e| format!("Decompression failed: {}", e))?;
        
        bincode::deserialize(&decompressed)
            .map_err(|e| format!("Deserialization failed: {}", e))
    }
    
    /// Smart compression: Only include full data for top 1000 fonts - HANDLES DUPLICATES
    pub fn smart_compress(
        &self,
        fonts: Vec<FontDescriptor>,
        popularity_scores: &HashMap<String, u8>,
    ) -> Result<(Vec<u8>, Vec<u8>), String> {
        // 1. Remove duplicates first
        let unique_fonts = self.remove_duplicates(&fonts);
        
        // 2. Split into core (top 1000) and extended (rest)
        let mut fonts_with_popularity: Vec<(FontDescriptor, u8)> = unique_fonts
            .into_iter()
            .map(|font| {
                let popularity = popularity_scores
                    .get(&font.family.to_lowercase())
                    .copied()
                    .unwrap_or(50);
                (font, popularity)
            })
            .collect();
        
        // 3. Sort by popularity
        fonts_with_popularity.sort_by(|a, b| b.1.cmp(&a.1));
        
        let (core_fonts, extended_fonts): (Vec<_>, Vec<_>) = 
            fonts_with_popularity.into_iter()
                .enumerate()
                .partition(|(i, _)| *i < 1000);
        
        // 4. Extract FontDescriptor from (usize, (FontDescriptor, u8))
        let core_fonts_data: Vec<FontDescriptor> = core_fonts.into_iter()
            .map(|(_, (font, _))| font)
            .collect();
        let extended_fonts_data: Vec<FontDescriptor> = extended_fonts.into_iter()
            .map(|(_, (font, _))| font)
            .collect();
        
        // 5. Compress core with full data
        let core_compressor = FontCompressor::new(11, true);
        let core_data = core_compressor.compress_font_database(&core_fonts_data, true)?;
        
        // 6. Compress extended with minimal data
        let ext_compressor = FontCompressor::new(9, false);
        let ext_data = ext_compressor.compress_font_database(&extended_fonts_data, false)?;
        
        Ok((core_data, ext_data))
    }

    /// Calculate compression ratio
    pub fn calculate_compression_ratio(
        &self,
        original_data: &[u8],
        compressed_data: &[u8],
    ) -> f64 {
        if original_data.is_empty() {
            return 0.0;
        }
        let original_len = original_data.len() as f64;
        let compressed_len = compressed_data.len() as f64;
        (1.0 - (compressed_len / original_len)) * 100.0
    }
    
    fn font_to_compressed(&self, font: &FontDescriptor) -> CompressedFontData {
        let category = self.detect_category(font);
        
        CompressedFontData {
            family: font.family.clone(),
            postscript_name: font.postscript_name.clone(),
            weight: font.weight,
            italic: font.italic,
            monospaced: font.monospaced,
            metrics: font.metrics.as_ref().map(|m| CompressedMetrics {
                units_per_em: m.units_per_em,
                ascender: m.ascender,
                descender: m.descender,
                x_height: m.x_height,
                cap_height: m.cap_height,
                average_width: m.average_width,
            }),
            license: CompressedLicense {
                name: font.license.as_ref().map(|l| l.name.clone()).unwrap_or_default(),
                url: font.license.as_ref().and_then(|l| l.url.clone()).unwrap_or_default(),
                allows_embedding: font.license.as_ref().map(|l| l.allows_embedding).unwrap_or(false),
                allows_modification: font.license.as_ref().map(|l| l.allows_modification).unwrap_or(false),
                requires_attribution: font.license.as_ref().map(|l| l.requires_attribution).unwrap_or(false),
                allows_commercial_use: font.license.as_ref().map(|l| l.allows_commercial_use).unwrap_or(false), // ADD THIS FIELD
            },
            category,
            similar_fonts: Vec::new(),
            download_urls: HashMap::new(),
            file_size_kb: self.estimate_file_size(font),
            popularity: 50,
        }
    }
    
    fn detect_category(&self, font: &FontDescriptor) -> FontCategory {
        let family_lower = font.family.to_lowercase();
        
        if font.monospaced {
            FontCategory::Monospace
        } else if family_lower.contains("serif") || 
                  family_lower.contains("times") || 
                  family_lower.contains("garamond") ||
                  family_lower.contains("georgia") {
            FontCategory::Serif
        } else if family_lower.contains("sans") || 
                  family_lower.contains("arial") || 
                  family_lower.contains("helvetica") ||
                  family_lower.contains("roboto") {
            FontCategory::SansSerif
        } else if family_lower.contains("script") || 
                  family_lower.contains("hand") || 
                  family_lower.contains("cursive") {
            FontCategory::Handwriting
        } else if family_lower.contains("display") || 
                  family_lower.contains("decorative") ||
                  family_lower.contains("fantasy") {
            FontCategory::Display
        } else if family_lower.contains("symbol") || 
                  family_lower.contains("dingbat") ||
                  family_lower.contains("wingding") {
            FontCategory::Symbol
        } else {
            FontCategory::Other
        }
    }
    
    fn estimate_file_size(&self, font: &FontDescriptor) -> u32 {
        // Estimate based on font characteristics
        let base_size = 50; // 50KB base
        let variable_penalty = if font.variable { 100 } else { 0 };
        let weight_penalty = if font.weight > 400 { (font.weight - 400) / 100 } else { 0 };
        
        (base_size + variable_penalty + weight_penalty as u32).max(20)
    }
    
    fn calculate_original_size(&self, fonts: &[FontDescriptor]) -> usize {
        fonts.iter().map(|f| self.estimate_file_size(f) as usize * 1024).sum()
    }
    
    fn categorize_fonts(&self, fonts: &[CompressedFontData]) -> HashMap<FontCategory, usize> {
        let mut categories = HashMap::new();
        for font in fonts {
            *categories.entry(font.category.clone()).or_insert(0) += 1;
        }
        categories
    }
    
    fn build_similarity_matrix(&self, fonts: &[CompressedFontData]) -> HashMap<String, Vec<(String, f32)>> {
        let mut matrix = HashMap::new();
        
        for i in 0..fonts.len() {
            let font1 = &fonts[i];
            let mut similarities = Vec::new();
            
            for j in 0..fonts.len() {
                if i == j {
                    continue;
                }
                let font2 = &fonts[j];
                let similarity = self.calculate_font_similarity(font1, font2);
                if similarity > 0.5 {
                    similarities.push((font2.family.clone(), similarity));
                }
            }
            
            // Sort by similarity
            similarities.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap());
            
            // Keep top 10
            similarities.truncate(10);
            matrix.insert(font1.family.clone(), similarities);
        }
        
        matrix
    }
    
    fn calculate_font_similarity(&self, font1: &CompressedFontData, font2: &CompressedFontData) -> f32 {
        let mut score = 0.0;
        
        // Category match (30%)
        if font1.category == font2.category {
            score += 0.3;
        }
        
        // Weight similarity (20%)
        let weight_diff = (font1.weight as i32 - font2.weight as i32).abs();
        score += (1.0 - (weight_diff as f32 / 800.0)).max(0.0) * 0.2;
        
        // Style match (20%)
        if font1.italic == font2.italic {
            score += 0.2;
        }
        
        // Monospace match (10%)
        if font1.monospaced == font2.monospaced {
            score += 0.1;
        }
        
        // Name similarity (20%)
        let name_similarity = self.name_similarity(&font1.family, &font2.family);
        score += name_similarity * 0.2;
        
        score
    }
    
    fn name_similarity(&self, s1: &str, s2: &str) -> f32 {
        let s1_lower = s1.to_lowercase();
        let s2_lower = s2.to_lowercase();
        
        if s1_lower == s2_lower {
            return 1.0;
        }
        
        if s1_lower.contains(&s2_lower) || s2_lower.contains(&s1_lower) {
            return 0.8;
        }
        
        // Simple Jaccard similarity
        let set1: std::collections::HashSet<char> = s1_lower.chars().collect();
        let set2: std::collections::HashSet<char> = s2_lower.chars().collect();
        
        let intersection: usize = set1.intersection(&set2).count();
        let union: usize = set1.union(&set2).count();
        
        if union == 0 {
            return 0.0;
        }
        
        intersection as f32 / union as f32
    }
}

/// Try to load the simple database format created by build.rs
pub fn try_load_simple_database(data: &[u8]) -> Option<CompressedFontDatabase> {
    if data.len() < 14 {
        return None;
    }
    
    // Check for our simple format header
    if !data.starts_with(b"FONTDBv1.0") {
        return None;
    }
    
    let mut offset = 10; // Skip "FONTDBv1.0"
    
    // Read font count
    if offset + 4 > data.len() {
        return None;
    }
    let font_count = u32::from_le_bytes([data[offset], data[offset+1], data[offset+2], data[offset+3]]) as usize;
    offset += 4;
    
    let mut fonts = Vec::new();
    
    for _ in 0..font_count {
        if offset >= data.len() {
            break;
        }
        
        // Read font name
        let name_len = data[offset] as usize;
        offset += 1;
        
        if offset + name_len + 3 > data.len() {
            break;
        }
        
        let family = String::from_utf8_lossy(&data[offset..offset+name_len]).to_string();
        offset += name_len;
        
        // Read weight
        let weight = u16::from_le_bytes([data[offset], data[offset+1]]);
        offset += 2;
        
        // Read italic
        let italic = data[offset] != 0;
        offset += 1;
        
        // **FIX: Create postscript_name BEFORE moving family**
        let postscript_name = family.to_lowercase().replace(' ', "-");
        let category = if family.to_lowercase().contains("courier") || 
                         family.to_lowercase().contains("console") {
            FontCategory::Monospace
        } else if family.to_lowercase().contains("times") ||
                  family.to_lowercase().contains("serif") {
            FontCategory::Serif
        } else {
            FontCategory::SansSerif
        };
        
        // Create compressed font entry
        fonts.push(CompressedFontData {
            family,  // Now this move is OK
            postscript_name,  // We already created this
            weight,
            italic,
            monospaced: false,
            metrics: None,
            license: CompressedLicense {
                name: "System Font".to_string(),
                url: String::new(),
                allows_embedding: true,
                allows_modification: false,
                requires_attribution: false,
                allows_commercial_use: true,
            },
            category,  // We already created this
            similar_fonts: Vec::new(),
            download_urls: HashMap::new(),
            file_size_kb: 50,
            popularity: 50,
        });
    }
    
    if fonts.is_empty() {
        return None;
    }
    
    // Create database metadata
    let metadata = FontDatabaseMetadata {
        version: "1.0.0-simple".to_string(),
        font_count: fonts.len(),
        compressed_size_bytes: data.len(),
        original_size_bytes: fonts.len() * 50 * 1024, // Estimate
        created_at: chrono::Utc::now().to_rfc3339(),
        categories: {
            let mut map = HashMap::new();
            for font in &fonts {
                *map.entry(font.category.clone()).or_insert(0) += 1;
            }
            map
        },
        include_full_data: false,
    };
    
    Some(CompressedFontDatabase {
        metadata,
        fonts,
        similarity_matrix: None,
    })
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FontDatabaseMetadata {
    pub version: String,
    pub font_count: usize,
    pub compressed_size_bytes: usize,
    pub original_size_bytes: usize,
    pub created_at: String,
    pub categories: HashMap<FontCategory, usize>,
    pub include_full_data: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompressedFontDatabase {
    pub metadata: FontDatabaseMetadata,
    pub fonts: Vec<CompressedFontData>,
    pub similarity_matrix: Option<HashMap<String, Vec<(String, f32)>>>,
}