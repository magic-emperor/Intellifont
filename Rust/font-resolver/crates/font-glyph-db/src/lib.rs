//! # Font Glyph Database
//! 
//! Build and manage compressed glyph signature databases for visual font identification.
//! 
//! This crate provides:
//! - `GlyphDatabaseBuilder` - Builds the compressed database from font files
//! - `LshIndex` - Locality-Sensitive Hashing for fast candidate retrieval
//! - Brotli-11 compression for ultra-compact storage

use std::collections::HashMap;
use std::io::{Write, Cursor, Read};
use std::path::Path;
use serde::{Serialize, Deserialize};
use font_glyph::{GlyphExtractor, MicroSignature, GlyphError};

// =============================================================================
// CONSTANTS
// =============================================================================

/// Magic bytes for database file format
pub const MAGIC_BYTES: &[u8; 8] = b"GLYPHDB1";

/// Current database format version
pub const FORMAT_VERSION: u32 = 1;

/// Number of LSH hash tables
pub const LSH_TABLE_COUNT: usize = 8;

/// Number of buckets per table
pub const LSH_BUCKET_COUNT: usize = 256;

/// Standard alphanumeric characters to index
pub const ALPHANUMERIC_CHARS: &str = "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789";

// =============================================================================
// LSH INDEX
// =============================================================================

/// Locality-Sensitive Hashing index for fast approximate matching
/// 
/// Uses multiple hash tables with different hash functions to find
/// similar signatures in O(1) time instead of O(n).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LshIndex {
    /// 8 hash tables, each with 256 buckets containing font IDs
    tables: Vec<Vec<Vec<u16>>>,
}

impl LshIndex {
    /// Create a new empty LSH index
    pub fn new() -> Self {
        let mut tables = Vec::with_capacity(LSH_TABLE_COUNT);
        for _ in 0..LSH_TABLE_COUNT {
            let mut buckets = Vec::with_capacity(LSH_BUCKET_COUNT);
            for _ in 0..LSH_BUCKET_COUNT {
                buckets.push(Vec::new());
            }
            tables.push(buckets);
        }
        Self { tables }
    }
    
    /// Add a font's signatures to the index
    pub fn add_font(&mut self, font_id: u16, signatures: &[(char, MicroSignature)]) {
        for (_, sig) in signatures {
            let hashes = self.compute_hashes(sig);
            for (table_idx, hash) in hashes.iter().enumerate() {
                let bucket_idx = *hash as usize % LSH_BUCKET_COUNT;
                let bucket = &mut self.tables[table_idx][bucket_idx];
                // Avoid duplicates
                if !bucket.contains(&font_id) {
                    bucket.push(font_id);
                }
            }
        }
    }
    
    /// Get candidate font IDs for a given signature
    /// Returns fonts that appear in multiple buckets (more likely matches)
    pub fn get_candidates(&self, signature: &MicroSignature, min_votes: usize) -> Vec<(u16, usize)> {
        let hashes = self.compute_hashes(signature);
        let mut vote_counts: HashMap<u16, usize> = HashMap::new();
        
        for (table_idx, hash) in hashes.iter().enumerate() {
            let bucket_idx = *hash as usize % LSH_BUCKET_COUNT;
            for &font_id in &self.tables[table_idx][bucket_idx] {
                *vote_counts.entry(font_id).or_insert(0) += 1;
            }
        }
        
        // Return candidates with enough votes, sorted by vote count
        let mut candidates: Vec<_> = vote_counts
            .into_iter()
            .filter(|&(_, votes)| votes >= min_votes)
            .collect();
        
        candidates.sort_by(|a, b| b.1.cmp(&a.1));
        candidates
    }
    
    /// Compute 8 different hashes for a signature
    fn compute_hashes(&self, sig: &MicroSignature) -> [u8; LSH_TABLE_COUNT] {
        let bytes = sig.to_bytes();
        
        [
            // Different projections of the signature
            bytes[0].wrapping_add(bytes[1]),  // aspect + density
            bytes[2].wrapping_add(bytes[3]),  // quadrant NW + NE
            bytes[4].wrapping_add(bytes[5]),  // quadrant SW + SE
            bytes[6].wrapping_add(bytes[7]),  // curve_ratio + point_count
            bytes[8].wrapping_add(bytes[9]),  // x_balance + y_balance
            bytes[10].wrapping_add(bytes[11]), // stroke + serif
            bytes[0].wrapping_add(bytes[6]),  // aspect + curve
            bytes[2].wrapping_add(bytes[4]),  // diagonal quadrants
        ]
    }
    
    /// Get statistics about the index
    pub fn stats(&self) -> LshIndexStats {
        let mut total_entries = 0;
        let mut non_empty_buckets = 0;
        let mut max_bucket_size = 0;
        
        for table in &self.tables {
            for bucket in table {
                if !bucket.is_empty() {
                    non_empty_buckets += 1;
                    total_entries += bucket.len();
                    max_bucket_size = max_bucket_size.max(bucket.len());
                }
            }
        }
        
        LshIndexStats {
            table_count: LSH_TABLE_COUNT,
            bucket_count: LSH_BUCKET_COUNT,
            total_entries,
            non_empty_buckets,
            max_bucket_size,
        }
    }
}

impl Default for LshIndex {
    fn default() -> Self {
        Self::new()
    }
}

/// Statistics about an LSH index
#[derive(Debug, Clone)]
pub struct LshIndexStats {
    pub table_count: usize,
    pub bucket_count: usize,
    pub total_entries: usize,
    pub non_empty_buckets: usize,
    pub max_bucket_size: usize,
}

// =============================================================================
// FONT ENTRY
// =============================================================================

/// Entry for a single font in the database
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FontEntry {
    /// Font family name
    pub family: String,
    /// Font subfamily (e.g., "Regular", "Bold", "Italic")
    pub subfamily: Option<String>,
    /// Signatures for each indexed character (char -> signature)
    pub signatures: Vec<(char, MicroSignature)>,
}

// =============================================================================
// DATABASE HEADER
// =============================================================================

/// Header for the database file
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DatabaseHeader {
    /// Format version
    pub version: u32,
    /// Number of fonts
    pub font_count: u32,
    /// Number of characters per font
    pub char_count: u8,
    /// Flags (reserved)
    pub flags: u8,
    /// Offset to LSH index data
    pub lsh_offset: u64,
    /// Offset to signature data
    pub signatures_offset: u64,
    /// Offset to font names
    pub names_offset: u64,
}

// =============================================================================
// COMPRESSED DATABASE
// =============================================================================

/// Full compressed database structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GlyphDatabase {
    /// Database header
    pub header: DatabaseHeader,
    /// LSH index for fast lookup
    pub lsh_index: LshIndex,
    /// All font entries
    pub fonts: Vec<FontEntry>,
}

impl GlyphDatabase {
    /// Find fonts matching a signature
    pub fn find_matches(&self, signature: &MicroSignature, limit: usize) -> Vec<MatchResult> {
        // Use LSH to get candidates
        let candidates = self.lsh_index.get_candidates(signature, 2);
        
        let mut results = Vec::new();
        
        for (font_id, votes) in candidates.iter().take(limit * 2) {
            if let Some(font) = self.fonts.get(*font_id as usize) {
                // Calculate actual similarity
                let mut best_similarity = 0.0f32;
                let mut matched_char = None;
                
                for (ch, font_sig) in &font.signatures {
                    let sim = signature.similarity(font_sig);
                    if sim > best_similarity {
                        best_similarity = sim;
                        matched_char = Some(*ch);
                    }
                }
                
                results.push(MatchResult {
                    font_id: *font_id,
                    family: font.family.clone(),
                    subfamily: font.subfamily.clone(),
                    similarity: best_similarity,
                    lsh_votes: *votes,
                    matched_char,
                });
            }
        }
        
        // Sort by similarity
        results.sort_by(|a, b| b.similarity.partial_cmp(&a.similarity).unwrap());
        results.truncate(limit);
        
        results
    }
    
    /// Find fonts matching multiple signatures (higher accuracy)
    pub fn find_matches_multi(&self, signatures: &[(char, MicroSignature)], limit: usize) -> Vec<MatchResult> {
        if signatures.is_empty() {
            return Vec::new();
        }
        
        // Aggregate votes from all signatures
        let mut combined_candidates: HashMap<u16, (usize, f32)> = HashMap::new();
        
        for (_, sig) in signatures {
            let candidates = self.lsh_index.get_candidates(sig, 1);
            for (font_id, votes) in candidates {
                let entry = combined_candidates.entry(font_id).or_insert((0, 0.0));
                entry.0 += votes;
            }
        }
        
        // Calculate average similarity for top candidates
        let mut results = Vec::new();
        
        for (font_id, (total_votes, _)) in combined_candidates.iter() {
            if let Some(font) = self.fonts.get(*font_id as usize) {
                let mut total_sim = 0.0;
                let mut match_count = 0;
                
                for (query_char, query_sig) in signatures {
                    // Find matching character in font
                    for (font_char, font_sig) in &font.signatures {
                        if font_char == query_char {
                            total_sim += query_sig.similarity(font_sig);
                            match_count += 1;
                            break;
                        }
                    }
                }
                
                if match_count > 0 {
                    results.push(MatchResult {
                        font_id: *font_id,
                        family: font.family.clone(),
                        subfamily: font.subfamily.clone(),
                        similarity: total_sim / match_count as f32,
                        lsh_votes: *total_votes,
                        matched_char: None,
                    });
                }
            }
        }
        
        results.sort_by(|a, b| b.similarity.partial_cmp(&a.similarity).unwrap());
        results.truncate(limit);
        
        results
    }
}

/// Result of a font match query
#[derive(Debug, Clone)]
pub struct MatchResult {
    /// Font ID in the database
    pub font_id: u16,
    /// Font family name
    pub family: String,
    /// Font subfamily
    pub subfamily: Option<String>,
    /// Similarity score (0.0 - 1.0)
    pub similarity: f32,
    /// Number of LSH votes
    pub lsh_votes: usize,
    /// Character that matched best
    pub matched_char: Option<char>,
}

// =============================================================================
// DATABASE BUILDER
// =============================================================================

/// Builder for creating compressed glyph databases
pub struct GlyphDatabaseBuilder {
    fonts: Vec<FontEntry>,
    lsh_index: LshIndex,
    extractor: GlyphExtractor,
}

impl GlyphDatabaseBuilder {
    /// Create a new database builder
    pub fn new() -> Self {
        Self {
            fonts: Vec::new(),
            lsh_index: LshIndex::new(),
            extractor: GlyphExtractor::new(),
        }
    }
    
    /// Add a font file to the database
    pub fn add_font<P: AsRef<Path>>(&mut self, font_path: P, family: &str, subfamily: Option<&str>) -> Result<(), GlyphError> {
        let signatures = self.extractor.extract_alphanumeric_signatures(&font_path)?;
        
        if signatures.is_empty() {
            return Ok(()); // Skip fonts with no supported characters
        }
        
        let font_id = self.fonts.len() as u16;
        
        // Add to LSH index
        self.lsh_index.add_font(font_id, &signatures);
        
        // Add font entry
        self.fonts.push(FontEntry {
            family: family.to_string(),
            subfamily: subfamily.map(|s| s.to_string()),
            signatures,
        });
        
        Ok(())
    }
    
    /// Add a font with auto-detected family name from metadata
    pub fn add_font_auto<P: AsRef<Path>>(&mut self, font_path: P) -> Result<(), GlyphError> {
        // Read font data
        let font_data = std::fs::read(font_path.as_ref())
            .map_err(|e| GlyphError::IoError(e.to_string()))?;
        
        let face = ttf_parser::Face::parse(&font_data, 0)
            .map_err(|e| GlyphError::ParseError(format!("Failed to parse font: {:?}", e)))?;
        
        // Extract family name
        let family = face.names()
            .into_iter()
            .find(|name| name.name_id == ttf_parser::name_id::FAMILY)
            .and_then(|name| name.to_string())
            .unwrap_or_else(|| "Unknown".to_string());
        
        // Extract subfamily
        let subfamily = face.names()
            .into_iter()
            .find(|name| name.name_id == ttf_parser::name_id::SUBFAMILY)
            .and_then(|name| name.to_string());
        
        self.add_font(font_path, &family, subfamily.as_deref())
    }
    
    /// Number of fonts currently in the builder
    pub fn font_count(&self) -> usize {
        self.fonts.len()
    }
    
    /// Build the database (uncompressed)
    pub fn build(&self) -> GlyphDatabase {
        let header = DatabaseHeader {
            version: FORMAT_VERSION,
            font_count: self.fonts.len() as u32,
            char_count: ALPHANUMERIC_CHARS.len() as u8,
            flags: 0,
            lsh_offset: 0, // Will be set during serialization
            signatures_offset: 0,
            names_offset: 0,
        };
        
        GlyphDatabase {
            header,
            lsh_index: self.lsh_index.clone(),
            fonts: self.fonts.clone(),
        }
    }
    
    /// Build and compress the database to bytes
    pub fn build_compressed(&self, compression_level: u32) -> Result<Vec<u8>, DatabaseError> {
        let database = self.build();
        
        // Serialize with bincode
        let serialized = bincode::serialize(&database)
            .map_err(|e| DatabaseError::SerializationError(e.to_string()))?;
        
        // Create output buffer with magic bytes
        let mut output = Vec::new();
        output.extend_from_slice(MAGIC_BYTES);
        
        // Compress with Brotli
        let mut compressed = Vec::new();
        {
            let mut encoder = brotli::CompressorWriter::new(
                &mut compressed,
                4096,
                compression_level.min(11),
                22, // window size
            );
            
            let mut cursor = Cursor::new(serialized);
            std::io::copy(&mut cursor, &mut encoder)
                .map_err(|e| DatabaseError::CompressionError(e.to_string()))?;
            
            encoder.flush()
                .map_err(|e| DatabaseError::CompressionError(e.to_string()))?;
        }
        
        output.extend_from_slice(&compressed);
        
        Ok(output)
    }
    
    /// Build and save to file
    pub fn save_to_file<P: AsRef<Path>>(&self, path: P) -> Result<DatabaseStats, DatabaseError> {
        let start = std::time::Instant::now();
        
        let compressed = self.build_compressed(11)?; // Maximum compression
        
        std::fs::write(path.as_ref(), &compressed)
            .map_err(|e| DatabaseError::IoError(e.to_string()))?;
        
        let elapsed = start.elapsed();
        
        // Calculate uncompressed size (approximate)
        let uncompressed_estimate = self.fonts.len() * 62 * 16; // fonts × chars × sig_size
        
        Ok(DatabaseStats {
            font_count: self.fonts.len(),
            char_count: 62,
            uncompressed_size: uncompressed_estimate,
            compressed_size: compressed.len(),
            compression_ratio: if uncompressed_estimate > 0 {
                (1.0 - (compressed.len() as f32 / uncompressed_estimate as f32)) * 100.0
            } else {
                0.0
            },
            build_time_seconds: elapsed.as_secs_f32(),
        })
    }
}

impl Default for GlyphDatabaseBuilder {
    fn default() -> Self {
        Self::new()
    }
}

// =============================================================================
// DATABASE LOADER
// =============================================================================

/// Load a compressed database from bytes
pub fn load_database(data: &[u8]) -> Result<GlyphDatabase, DatabaseError> {
    // Check magic bytes
    if data.len() < 8 || &data[0..8] != MAGIC_BYTES {
        return Err(DatabaseError::InvalidFormat("Invalid magic bytes".to_string()));
    }
    
    // Decompress
    let compressed = &data[8..];
    let mut decompressed = Vec::new();
    let mut decoder = brotli::Decompressor::new(compressed, 4096);
    
    decoder.read_to_end(&mut decompressed)
        .map_err(|e| DatabaseError::DecompressionError(e.to_string()))?;
    
    // Deserialize
    bincode::deserialize(&decompressed)
        .map_err(|e| DatabaseError::DeserializationError(e.to_string()))
}

/// Load a compressed database from file
pub fn load_database_from_file<P: AsRef<Path>>(path: P) -> Result<GlyphDatabase, DatabaseError> {
    let data = std::fs::read(path.as_ref())
        .map_err(|e| DatabaseError::IoError(e.to_string()))?;
    
    load_database(&data)
}

// =============================================================================
// STATISTICS
// =============================================================================

/// Statistics about built database
#[derive(Debug, Clone)]
pub struct DatabaseStats {
    pub font_count: usize,
    pub char_count: usize,
    pub uncompressed_size: usize,
    pub compressed_size: usize,
    pub compression_ratio: f32,
    pub build_time_seconds: f32,
}

impl std::fmt::Display for DatabaseStats {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Database Statistics:\n")?;
        write!(f, "  Fonts indexed: {}\n", self.font_count)?;
        write!(f, "  Characters per font: {}\n", self.char_count)?;
        write!(f, "  Uncompressed size: {:.2} MB\n", self.uncompressed_size as f64 / 1_000_000.0)?;
        write!(f, "  Compressed size: {:.2} MB\n", self.compressed_size as f64 / 1_000_000.0)?;
        write!(f, "  Compression ratio: {:.1}%\n", self.compression_ratio)?;
        write!(f, "  Build time: {:.2}s", self.build_time_seconds)
    }
}

// =============================================================================
// ERRORS
// =============================================================================

/// Database-related errors
#[derive(Debug, Clone)]
pub enum DatabaseError {
    IoError(String),
    SerializationError(String),
    DeserializationError(String),
    CompressionError(String),
    DecompressionError(String),
    InvalidFormat(String),
}

impl std::fmt::Display for DatabaseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DatabaseError::IoError(msg) => write!(f, "IO error: {}", msg),
            DatabaseError::SerializationError(msg) => write!(f, "Serialization error: {}", msg),
            DatabaseError::DeserializationError(msg) => write!(f, "Deserialization error: {}", msg),
            DatabaseError::CompressionError(msg) => write!(f, "Compression error: {}", msg),
            DatabaseError::DecompressionError(msg) => write!(f, "Decompression error: {}", msg),
            DatabaseError::InvalidFormat(msg) => write!(f, "Invalid format: {}", msg),
        }
    }
}

impl std::error::Error for DatabaseError {}

// =============================================================================
// TESTS
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_lsh_index_creation() {
        let index = LshIndex::new();
        let stats = index.stats();
        
        assert_eq!(stats.table_count, LSH_TABLE_COUNT);
        assert_eq!(stats.bucket_count, LSH_BUCKET_COUNT);
        assert_eq!(stats.total_entries, 0);
    }
    
    #[test]
    fn test_lsh_index_add_and_query() {
        let mut index = LshIndex::new();
        
        let sig1 = MicroSignature {
            aspect_ratio: 100,
            density: 150,
            quadrant_nw: 64,
            quadrant_ne: 64,
            quadrant_sw: 64,
            quadrant_se: 64,
            curve_ratio: 128,
            point_count: 50,
            x_balance: 128,
            y_balance: 128,
            stroke_width: 100,
            serif_score: 50,
            feature_hash: 12345,
            reserved: 0,
        };
        
        index.add_font(0, &[('A', sig1)]);
        
        let candidates = index.get_candidates(&sig1, 1);
        assert!(!candidates.is_empty(), "Should find the added font");
        assert_eq!(candidates[0].0, 0, "Should find font ID 0");
    }
    
    #[test]
    fn test_database_builder() {
        let builder = GlyphDatabaseBuilder::new();
        assert_eq!(builder.font_count(), 0);
        
        let db = builder.build();
        assert_eq!(db.fonts.len(), 0);
    }
    
    #[test]
    fn test_database_roundtrip() {
        let mut builder = GlyphDatabaseBuilder::new();
        
        // Add a fake font entry manually
        let sig = MicroSignature::default();
        builder.lsh_index.add_font(0, &[('A', sig)]);
        builder.fonts.push(FontEntry {
            family: "TestFont".to_string(),
            subfamily: Some("Regular".to_string()),
            signatures: vec![('A', sig)],
        });
        
        // Compress and decompress
        let compressed = builder.build_compressed(1).expect("Should compress");
        let loaded = load_database(&compressed).expect("Should load");
        
        assert_eq!(loaded.fonts.len(), 1);
        assert_eq!(loaded.fonts[0].family, "TestFont");
    }
}
