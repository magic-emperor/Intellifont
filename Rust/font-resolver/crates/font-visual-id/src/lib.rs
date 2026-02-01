//! # Font Visual ID
//! 
//! Visual font identification engine that identifies fonts from glyph shapes.
//! 
//! This crate provides:
//! - `VisualIdentifier` - Main engine for font identification
//! - Sub-millisecond lookups using LSH acceleration
//! - Multi-character matching for higher accuracy

use std::path::Path;
use serde::{Serialize, Deserialize};
use font_glyph::{GlyphExtractor, MicroSignature};
use font_glyph_db::{GlyphDatabase, GlyphDatabaseBuilder, load_database, load_database_from_file, DatabaseStats};

// Re-export key types for convenience
pub use font_glyph::{MicroSignature as Signature, GlyphOutline};
pub use font_glyph_db::MatchResult;

// =============================================================================
// IDENTIFICATION RESULT
// =============================================================================

/// Result of a font identification query
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IdentificationResult {
    /// Font family name
    pub family: String,
    /// Font subfamily (e.g., "Regular", "Bold")
    pub subfamily: Option<String>,
    /// Confidence score (0.0 - 1.0)
    pub confidence: f32,
    /// Which characters were matched
    pub matched_chars: Vec<char>,
    /// Source of the match ("Database" or "Local")
    pub source: String,
}

impl IdentificationResult {
    /// Create from a MatchResult
    fn from_match_result(result: &font_glyph_db::MatchResult, chars: Vec<char>) -> Self {
        Self {
            family: result.family.clone(),
            subfamily: result.subfamily.clone(),
            confidence: result.similarity,
            matched_chars: chars,
            source: "Database".to_string(),
        }
    }
}

// =============================================================================
// VISUAL IDENTIFIER
// =============================================================================

/// Main visual font identification engine
/// 
/// Identifies fonts by comparing glyph signatures against a pre-built database.
/// Uses LSH (Locality-Sensitive Hashing) for sub-millisecond candidate filtering.
pub struct VisualIdentifier {
    /// Loaded glyph database
    database: GlyphDatabase,
    /// Glyph extractor for extracting signatures from font files
    extractor: GlyphExtractor,
}

impl VisualIdentifier {
    /// Create a new visual identifier with the given database
    pub fn new(database: GlyphDatabase) -> Self {
        Self {
            database,
            extractor: GlyphExtractor::new(),
        }
    }
    
    /// Load a visual identifier from compressed database bytes
    pub fn from_bytes(data: &[u8]) -> Result<Self, IdentificationError> {
        let database = load_database(data)
            .map_err(|e| IdentificationError::DatabaseError(e.to_string()))?;
        Ok(Self::new(database))
    }
    
    /// Load a visual identifier from a database file
    pub fn from_file<P: AsRef<Path>>(path: P) -> Result<Self, IdentificationError> {
        let database = load_database_from_file(path)
            .map_err(|e| IdentificationError::DatabaseError(e.to_string()))?;
        Ok(Self::new(database))
    }
    
    /// Identify a font from a single character
    /// 
    /// This is the fastest method but may have lower accuracy for similar fonts.
    /// For better accuracy, use `identify_multi` with multiple characters.
    /// 
    /// # Arguments
    /// * `font_path` - Path to the font file to identify
    /// * `character` - Character to analyze
    /// * `limit` - Maximum number of results to return
    pub fn identify_single<P: AsRef<Path>>(
        &self,
        font_path: P,
        character: char,
        limit: usize,
    ) -> Result<Vec<IdentificationResult>, IdentificationError> {
        // Extract signature from the font
        let outline = self.extractor.extract_from_file(&font_path, character)
            .map_err(|e| IdentificationError::ExtractionError(e.to_string()))?;
        
        let signature = MicroSignature::from_outline(&outline);
        
        // Find matches
        let matches = self.database.find_matches(&signature, limit);
        
        Ok(matches.iter()
            .map(|m| IdentificationResult::from_match_result(m, vec![character]))
            .collect())
    }
    
    /// Identify a font from multiple characters (higher accuracy)
    /// 
    /// Analyzes multiple characters and combines the results for more accurate
    /// font identification. Recommended for production use.
    /// 
    /// # Arguments
    /// * `font_path` - Path to the font file to identify
    /// * `characters` - Characters to analyze (e.g., "RQWM")
    /// * `limit` - Maximum number of results to return
    pub fn identify_multi<P: AsRef<Path>>(
        &self,
        font_path: P,
        characters: &str,
        limit: usize,
    ) -> Result<Vec<IdentificationResult>, IdentificationError> {
        // Extract signatures for all characters
        let signatures = self.extractor.extract_signatures(&font_path, characters)
            .map_err(|e| IdentificationError::ExtractionError(e.to_string()))?;
        
        if signatures.is_empty() {
            return Ok(Vec::new());
        }
        
        let chars: Vec<char> = signatures.iter().map(|(c, _)| *c).collect();
        
        // Find matches using multiple signatures
        let matches = self.database.find_matches_multi(&signatures, limit);
        
        Ok(matches.iter()
            .map(|m| IdentificationResult::from_match_result(m, chars.clone()))
            .collect())
    }
    
    /// Identify font from pre-extracted signature
    /// 
    /// Use this when you've already extracted the signature elsewhere.
    pub fn identify_from_signature(
        &self,
        signature: &MicroSignature,
        limit: usize,
    ) -> Vec<IdentificationResult> {
        let matches = self.database.find_matches(signature, limit);
        matches.iter()
            .map(|m| IdentificationResult::from_match_result(m, vec![]))
            .collect()
    }
    
    /// Identify font from multiple pre-extracted signatures
    pub fn identify_from_signatures(
        &self,
        signatures: &[(char, MicroSignature)],
        limit: usize,
    ) -> Vec<IdentificationResult> {
        let chars: Vec<char> = signatures.iter().map(|(c, _)| *c).collect();
        let matches = self.database.find_matches_multi(signatures, limit);
        matches.iter()
            .map(|m| IdentificationResult::from_match_result(m, chars.clone()))
            .collect()
    }
    
    /// Get database statistics
    pub fn database_stats(&self) -> DatabaseInfo {
        DatabaseInfo {
            font_count: self.database.fonts.len(),
            char_count: self.database.header.char_count as usize,
            lsh_stats: self.database.lsh_index.stats(),
        }
    }
    
    /// Extract a signature from a font file
    pub fn extract_signature<P: AsRef<Path>>(
        &self,
        font_path: P,
        character: char,
    ) -> Result<MicroSignature, IdentificationError> {
        let outline = self.extractor.extract_from_file(&font_path, character)
            .map_err(|e| IdentificationError::ExtractionError(e.to_string()))?;
        
        Ok(MicroSignature::from_outline(&outline))
    }
    
    /// Extract signatures for multiple characters
    pub fn extract_signatures<P: AsRef<Path>>(
        &self,
        font_path: P,
        characters: &str,
    ) -> Result<Vec<(char, MicroSignature)>, IdentificationError> {
        self.extractor.extract_signatures(&font_path, characters)
            .map_err(|e| IdentificationError::ExtractionError(e.to_string()))
    }
}

// =============================================================================
// DATABASE INFO
// =============================================================================

/// Information about the loaded database
#[derive(Debug, Clone)]
pub struct DatabaseInfo {
    pub font_count: usize,
    pub char_count: usize,
    pub lsh_stats: font_glyph_db::LshIndexStats,
}

impl std::fmt::Display for DatabaseInfo {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Database Info:\n")?;
        write!(f, "  Fonts indexed: {}\n", self.font_count)?;
        write!(f, "  Characters per font: {}\n", self.char_count)?;
        write!(f, "  LSH non-empty buckets: {}", self.lsh_stats.non_empty_buckets)
    }
}

// =============================================================================
// BUILDER HELPER
// =============================================================================

/// Build a new glyph database from font files
pub fn build_database<P: AsRef<Path>>(
    font_paths: &[P],
) -> Result<DatabaseStats, IdentificationError> {
    let mut builder = GlyphDatabaseBuilder::new();
    
    for path in font_paths {
        if let Err(e) = builder.add_font_auto(path) {
            // Log but continue - some fonts may fail to parse
            eprintln!("Warning: Failed to add font {:?}: {}", path.as_ref(), e);
        }
    }
    
    // Build in-memory (caller can save)
    let db = builder.build();
    
    Ok(DatabaseStats {
        font_count: db.fonts.len(),
        char_count: 62,
        uncompressed_size: db.fonts.len() * 62 * 16,
        compressed_size: 0, // Not compressed yet
        compression_ratio: 0.0,
        build_time_seconds: 0.0,
    })
}

/// Build and save a glyph database to file
pub fn build_database_to_file<P: AsRef<Path>, Q: AsRef<Path>>(
    font_paths: &[P],
    output_path: Q,
) -> Result<DatabaseStats, IdentificationError> {
    let mut builder = GlyphDatabaseBuilder::new();
    
    for path in font_paths {
        if let Err(e) = builder.add_font_auto(path) {
            eprintln!("Warning: Failed to add font {:?}: {}", path.as_ref(), e);
        }
    }
    
    builder.save_to_file(output_path)
        .map_err(|e| IdentificationError::DatabaseError(e.to_string()))
}

// =============================================================================
// ERRORS
// =============================================================================

/// Errors that can occur during identification
#[derive(Debug, Clone)]
pub enum IdentificationError {
    /// Error extracting glyph from font
    ExtractionError(String),
    /// Error with the database
    DatabaseError(String),
    /// Font file not found
    FontNotFound(String),
}

impl std::fmt::Display for IdentificationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            IdentificationError::ExtractionError(msg) => write!(f, "Extraction error: {}", msg),
            IdentificationError::DatabaseError(msg) => write!(f, "Database error: {}", msg),
            IdentificationError::FontNotFound(msg) => write!(f, "Font not found: {}", msg),
        }
    }
}

impl std::error::Error for IdentificationError {}

// =============================================================================
// TESTS
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_identification_result_creation() {
        let result = IdentificationResult {
            family: "Arial".to_string(),
            subfamily: Some("Regular".to_string()),
            confidence: 0.95,
            matched_chars: vec!['R', 'A'],
            source: "Database".to_string(),
        };
        
        assert_eq!(result.family, "Arial");
        assert_eq!(result.confidence, 0.95);
    }
    
    #[test]
    fn test_database_info_display() {
        let info = DatabaseInfo {
            font_count: 1000,
            char_count: 62,
            lsh_stats: font_glyph_db::LshIndexStats {
                table_count: 8,
                bucket_count: 256,
                total_entries: 5000,
                non_empty_buckets: 200,
                max_bucket_size: 50,
            },
        };
        
        let display = format!("{}", info);
        assert!(display.contains("1000"));
        assert!(display.contains("62"));
    }
}
