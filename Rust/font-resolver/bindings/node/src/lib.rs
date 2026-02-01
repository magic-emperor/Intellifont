use napi_derive::napi;
use font_core::{ResolverConfig, EnhancedResolverConfig};
use font_resolver_engine::{FontResolver, EnhancedFontResolver};
use serde::{Deserialize, Serialize};

#[napi]
pub fn resolve_font_basic(font_name: String) -> napi::Result<String> {
    let config = ResolverConfig::default();
    let resolver = FontResolver::new(config);
    
    match resolver.resolve(&font_name) {
        Ok(result) => Ok(format!("Found: {} -> {}", font_name, result.font.family)),
        Err(e) => Ok(format!("Error: {} -> {}", font_name, e)),
    }
}

#[derive(Serialize, Deserialize)]
#[napi(object)]
pub struct JsSuggestion {
    pub family: String,
    pub subfamily: String,
    pub source: String,
    pub score: f64,
    pub weight: u32,
    pub italic: bool,
    pub is_critical_license_warning: bool,
    pub license_name: String,
    pub has_local_file: bool,
    pub download_url: Option<String>,
    pub is_offline_fallback: bool,
}

#[derive(Serialize, Deserialize)]
#[napi(object)]
pub struct JsDatabaseStats {
    pub font_count: u32,
    pub compressed_size_mb: f64,
    pub original_size_mb: f64,
    pub compression_ratio: f64,
}

#[derive(Serialize, Deserialize)]
#[napi(object)]
pub struct JsCacheStats {
    pub memory_entries: u32,
    pub disk_entries: u32,
    pub pinned_fonts: u32,
    pub memory_usage_mb: f64,
    pub disk_usage_mb: f64,
}

#[napi]
pub async fn get_font_suggestions(font_name: String, include_internet: bool) -> napi::Result<Vec<JsSuggestion>> {
    let mut config = EnhancedResolverConfig::default();
    config.web_fonts_enabled = include_internet;
    
    // Load embedded database (bundled with the .node file)
    let db_bytes = include_bytes!("../font_database.bin");
    
    let resolver = EnhancedFontResolver::new_with_database(config, db_bytes).map_err(|e| napi::Error::from_reason(e.to_string()))?;
    
    let suggestions = resolver.get_suggestions(&font_name, include_internet).await
        .map_err(|e| napi::Error::from_reason(e.to_string()))?;
    
    let js_suggestions = suggestions.into_iter().map(|s| JsSuggestion {
        family: s.family,
        subfamily: s.subfamily,
        source: s.source,
        score: s.score as f64,
        weight: s.weight as u32,
        italic: s.italic,
        is_critical_license_warning: s.is_critical_license_warning,
        license_name: s.license_name,
        has_local_file: s.has_local_file,
        download_url: s.download_url,
        is_offline_fallback: s.is_offline_fallback,
    }).collect();
    
    Ok(js_suggestions)
}

#[napi]
pub fn normalize_font_name(font_name: String) -> napi::Result<String> {
    use font_normalizer::FontNormalizer;
    let normalizer = FontNormalizer;
    
    match normalizer.normalize(&font_name) {
        Ok(request) => Ok(format!("{} -> {}", font_name, request.family)),
        Err(e) => Ok(format!("Error: {}", e)),
    }
}

#[napi]
pub fn pin_font(font_name: String) -> napi::Result<()> {
    let config = EnhancedResolverConfig::default();
    let resolver = EnhancedFontResolver::new(config).map_err(|e| napi::Error::from_reason(e.to_string()))?;
    resolver.pin_font(&font_name).map_err(|e| napi::Error::from_reason(e.to_string()))
}

#[napi]
pub fn unpin_font(font_name: String) -> napi::Result<()> {
    let config = EnhancedResolverConfig::default();
    let resolver = EnhancedFontResolver::new(config).map_err(|e| napi::Error::from_reason(e.to_string()))?;
    resolver.unpin_font(&font_name).map_err(|e| napi::Error::from_reason(e.to_string()))
}

#[napi]
pub fn remove_from_cache(font_names: Vec<String>) -> napi::Result<u32> {
    let config = EnhancedResolverConfig::default();
    let resolver = EnhancedFontResolver::new(config).map_err(|e| napi::Error::from_reason(e.to_string()))?;
    match resolver.remove_from_cache(font_names) {
        Ok(count) => Ok(count as u32),
        Err(e) => Err(napi::Error::from_reason(e.to_string()))
    }
}

#[napi]
pub fn export_metrics(font_name: String) -> napi::Result<String> {
    let config = EnhancedResolverConfig::default();
    let resolver = EnhancedFontResolver::new(config).map_err(|e| napi::Error::from_reason(e.to_string()))?;
    resolver.export_metrics(&font_name).map_err(|e| napi::Error::from_reason(e.to_string()))
}

#[napi]
pub fn get_engine_stats() -> napi::Result<JsDatabaseStats> {
    let config = EnhancedResolverConfig::default();
    let db_bytes = include_bytes!("../font_database.bin");
    let resolver = EnhancedFontResolver::new_with_database(config, db_bytes).map_err(|e| napi::Error::from_reason(e.to_string()))?;
    
    match resolver.get_database_stats() {
        Some(stats) => Ok(JsDatabaseStats {
            font_count: stats.font_count as u32,
            compressed_size_mb: stats.compressed_size_mb,
            original_size_mb: stats.original_size_mb,
            compression_ratio: stats.compression_ratio,
        }),
        None => Err(napi::Error::from_reason("Database not loaded")),
    }
}

#[napi]
pub fn get_cache_stats() -> napi::Result<JsCacheStats> {
    let config = EnhancedResolverConfig::default();
    let resolver = EnhancedFontResolver::new(config).map_err(|e| napi::Error::from_reason(e.to_string()))?;
    
    match resolver.get_cache_stats() {
        Some(Ok(stats)) => Ok(JsCacheStats {
            memory_entries: stats.memory_entries as u32,
            disk_entries: stats.disk_entries as u32,
            pinned_fonts: stats.pinned_fonts as u32,
            memory_usage_mb: stats.memory_usage_mb,
            disk_usage_mb: stats.disk_usage_mb,
        }),
        Some(Err(e)) => Err(napi::Error::from_reason(e.to_string())),
        None => Err(napi::Error::from_reason("Cache not available")),
    }
}

#[napi]
pub fn cleanup_cache(aggressive: bool) -> napi::Result<u32> {
    let config = EnhancedResolverConfig::default();
    let resolver = EnhancedFontResolver::new(config).map_err(|e| napi::Error::from_reason(e.to_string()))?;
    resolver.cleanup_cache(aggressive).map(|c| c as u32).map_err(|e| napi::Error::from_reason(e.to_string()))
}

#[napi]
pub fn list_pinned_fonts() -> napi::Result<Vec<String>> {
    let config = EnhancedResolverConfig::default();
    let resolver = EnhancedFontResolver::new(config).map_err(|e| napi::Error::from_reason(e.to_string()))?;
    Ok(resolver.list_pinned_fonts().unwrap_or_default())
}

#[napi]
pub async fn update_database() -> napi::Result<()> {
    let mut config = EnhancedResolverConfig::default();
    config.web_fonts_enabled = true;
    let mut resolver = EnhancedFontResolver::new(config).map_err(|e| napi::Error::from_reason(e.to_string()))?;
    resolver.update_font_database().await.map_err(|e| napi::Error::from_reason(e.to_string()))
}

// =============================================================================
// VISUAL FONT IDENTIFICATION
// =============================================================================

#[derive(Serialize, Deserialize)]
#[napi(object)]
pub struct JsVisualMatch {
    pub family: String,
    pub subfamily: Option<String>,
    pub confidence: f64,
    pub matched_chars: Vec<String>,
}

#[derive(Serialize, Deserialize)]
#[napi(object)]
pub struct JsGlyphSignature {
    pub character: String,
    pub aspect_ratio: u32,
    pub density: u32,
    pub curve_ratio: u32,
    pub point_count: u32,
    pub x_balance: u32,
    pub y_balance: u32,
}

#[derive(Serialize, Deserialize)]
#[napi(object)]
pub struct JsGlyphDbStats {
    pub font_count: u32,
    pub char_count: u32,
    pub uncompressed_size_mb: f64,
    pub compressed_size_mb: f64,
    pub compression_ratio: f64,
    pub build_time_seconds: f64,
}

/// Identify a font visually by analyzing glyph shapes
/// 
/// Uses the pre-built glyph signature database to find matching fonts.
/// Pass multiple characters (e.g., "RQWM") for higher accuracy.
#[napi]
pub fn identify_visual_font(
    font_path: String,
    characters: String,
    limit: Option<u32>,
) -> napi::Result<Vec<JsVisualMatch>> {
    use font_glyph::GlyphExtractor;
    use font_glyph_db::{GlyphDatabase, load_database};
    
    let limit = limit.unwrap_or(10) as usize;
    
    // Try to load the glyph database (it should be bundled)
    // For now, check if it exists in the same directory as font_database.bin
    let glyph_db_path = std::path::Path::new(&font_path)
        .parent()
        .unwrap_or(std::path::Path::new("."))
        .join("glyph_signatures.bin");
    
    // If no database exists yet, return an informative error
    if !glyph_db_path.exists() {
        return Err(napi::Error::from_reason(
            "Glyph signature database not found. Run 'npx intellifont build-glyph-db' first."
        ));
    }
    
    let db_data = std::fs::read(&glyph_db_path)
        .map_err(|e| napi::Error::from_reason(format!("Failed to read glyph database: {}", e)))?;
    
    let database = load_database(&db_data)
        .map_err(|e| napi::Error::from_reason(format!("Failed to load glyph database: {}", e)))?;
    
    let extractor = GlyphExtractor::new();
    
    // Extract signatures from the query font
    let signatures = extractor.extract_signatures(&font_path, &characters)
        .map_err(|e| napi::Error::from_reason(format!("Failed to extract signatures: {}", e)))?;
    
    if signatures.is_empty() {
        return Ok(Vec::new());
    }
    
    // Find matches
    let matches = database.find_matches_multi(&signatures, limit);
    
    Ok(matches.iter().map(|m| JsVisualMatch {
        family: m.family.clone(),
        subfamily: m.subfamily.clone(),
        confidence: m.similarity as f64,
        matched_chars: characters.chars().map(|c| c.to_string()).collect(),
    }).collect())
}

/// Identify a font visually from a memory buffer (e.g. file upload)
/// 
/// Excellent for web servers where the font file is in memory.
#[napi]
pub fn identify_visual_font_buffer(
    font_data: napi::bindgen_prelude::Buffer,
    characters: String,
    limit: Option<u32>,
) -> napi::Result<Vec<JsVisualMatch>> {
    use font_glyph::GlyphExtractor;
    use font_glyph_db::{GlyphDatabase, load_database};
    
    let limit = limit.unwrap_or(10) as usize;
    
    // Try to load the database from the same directory as the module
    // This is a bit tricky in N-API context, so we'll try relative path first
    // In a real prod app, the user might want to pass the DB path explicitly, 
    // but for now we default to expected location
    let db_path = "data/glyph_signatures.bin";
    
    // Check if we can find the DB
    if !std::path::Path::new(db_path).exists() {
         // Fallback to trying to locate it relative to CWD if data/ doesn't exist
         // or provide a helpful error
    }
    
    // For this specific helper, we'll try to load the embeded/local DB.
    // In the NAPI context we'll likely rely on the file system for the DB
    // as it's too large to embed 20MB in the binary efficiently for this demo
    // But for the user's "Web App" scenario, they would likely have the DB on disk.
    
    let db_data = std::fs::read(db_path)
        .or_else(|_| std::fs::read("font_database.bin")) // fallback
        .or_else(|_| std::fs::read("./bindings/node/font_database.bin")) // fallback
        .map_err(|_| napi::Error::from_reason(
            "Glyph signature database (glyph_signatures.bin) not found. Please ensure it exists in data/ directory."
        ))?;
    
    let database = load_database(&db_data)
        .map_err(|e| napi::Error::from_reason(format!("Failed to load glyph database: {}", e)))?;
    
    let extractor = GlyphExtractor::new();
    let data_slice: &[u8] = font_data.as_ref();
    
    // Extract signatures from the buffer
    let mut signatures = Vec::new();
    for ch in characters.chars() {
        if let Ok(outline) = extractor.extract_from_data(data_slice, ch) {
            let sig = font_visual_id::Signature::from_outline(&outline);
            signatures.push((ch, sig));
        }
    }

    
    if signatures.is_empty() {
        return Ok(Vec::new());
    }
    
    // Find matches
    let matches = database.find_matches_multi(&signatures, limit);
    
    Ok(matches.iter().map(|m| JsVisualMatch {
        family: m.family.clone(),
        subfamily: m.subfamily.clone(),
        confidence: m.similarity as f64,
        matched_chars: characters.chars().map(|c| c.to_string()).collect(),
    }).collect())
}


/// AI-powered result for font similarity suggestions
#[napi(object)]
#[derive(Clone)]
pub struct JsAiSuggestion {
    /// The matched font family name
    pub family: String,
    /// The matched font subfamily (e.g., "Regular", "Bold")
    pub subfamily: String,
    /// AI confidence score (0.0 to 1.0)
    pub confidence: f64,
    /// Similarity category: "exact", "high", "medium", "low"
    pub match_quality: String,
}

/// AI-powered font similarity suggestion
/// 
/// Analyzes the visual DNA of a font file and finds similar fonts
/// from the signature database using pattern recognition.
#[napi]
pub fn ai_suggest_similar(
    font_path: String,
    limit: Option<u32>,
) -> napi::Result<Vec<JsAiSuggestion>> {
    use font_glyph::GlyphExtractor;
    use font_glyph_db::load_database;
    
    let limit = limit.unwrap_or(10) as usize;
    let characters = "RQWM";
    
    let glyph_db_path = std::path::PathBuf::from("./data/glyph_signatures.bin");
    if !glyph_db_path.exists() {
        return Err(napi::Error::from_reason(
            "AI model database not found. Run 'npx intellifont build-glyph-db' first."
        ));
    }
    
    let db_data = std::fs::read(&glyph_db_path)
        .map_err(|e| napi::Error::from_reason(format!("Failed to load AI model: {}", e)))?;
    
    let database = load_database(&db_data)
        .map_err(|e| napi::Error::from_reason(format!("Failed to parse AI model: {}", e)))?;
    
    let extractor = GlyphExtractor::new();
    
    let signatures = extractor.extract_signatures(&font_path, characters)
        .map_err(|e| napi::Error::from_reason(format!("Failed to analyze font: {}", e)))?;
    
    if signatures.is_empty() {
        return Ok(Vec::new());
    }
    
    let matches = database.find_matches_multi(&signatures, limit);
    
    Ok(matches.iter().map(|m| {
        let quality = if m.similarity >= 0.95 { "exact" }
            else if m.similarity >= 0.85 { "high" }
            else if m.similarity >= 0.70 { "medium" }
            else { "low" };
        
        JsAiSuggestion {
            family: m.family.clone(),
            subfamily: m.subfamily.clone().unwrap_or_default(),
            confidence: m.similarity as f64,
            match_quality: quality.to_string(),
        }
    }).collect())
}

/// AI-powered font similarity from memory buffer
#[napi]
pub fn ai_suggest_similar_buffer(
    font_data: napi::bindgen_prelude::Buffer,
    limit: Option<u32>,
) -> napi::Result<Vec<JsAiSuggestion>> {
    use font_glyph::GlyphExtractor;
    use font_glyph_db::load_database;
    
    let limit = limit.unwrap_or(10) as usize;
    let characters = "RQWM";
    
    let glyph_db_path = std::path::PathBuf::from("./data/glyph_signatures.bin");
    if !glyph_db_path.exists() {
        return Err(napi::Error::from_reason(
            "AI model database not found. Run 'npx intellifont build-glyph-db' first."
        ));
    }
    
    let db_data = std::fs::read(&glyph_db_path)
        .map_err(|e| napi::Error::from_reason(format!("Failed to load AI model: {}", e)))?;
    
    let database = load_database(&db_data)
        .map_err(|e| napi::Error::from_reason(format!("Failed to parse AI model: {}", e)))?;
    
    let extractor = GlyphExtractor::new();
    let data_slice: &[u8] = font_data.as_ref();
    
    let mut signatures = Vec::new();
    for ch in characters.chars() {
        if let Ok(outline) = extractor.extract_from_data(data_slice, ch) {
            let sig = font_visual_id::Signature::from_outline(&outline);
            signatures.push((ch, sig));
        }
    }
    
    if signatures.is_empty() {
        return Ok(Vec::new());
    }
    
    let matches = database.find_matches_multi(&signatures, limit);
    
    Ok(matches.iter().map(|m| {
        let quality = if m.similarity >= 0.95 { "exact" }
            else if m.similarity >= 0.85 { "high" }
            else if m.similarity >= 0.70 { "medium" }
            else { "low" };
        
        JsAiSuggestion {
            family: m.family.clone(),
            subfamily: m.subfamily.clone().unwrap_or_default(),
            confidence: m.similarity as f64,
            match_quality: quality.to_string(),
        }
    }).collect())
}


/// Extract glyph signature from a font file
/// 
/// Returns detailed signature information for a specific character.
#[napi]
pub fn extract_glyph_signature(
    font_path: String,
    character: String,
) -> napi::Result<JsGlyphSignature> {
    use font_glyph::{GlyphExtractor, MicroSignature};
    
    let ch = character.chars().next()
        .ok_or_else(|| napi::Error::from_reason("Character string is empty"))?;
    
    let extractor = GlyphExtractor::new();
    
    let outline = extractor.extract_from_file(&font_path, ch)
        .map_err(|e| napi::Error::from_reason(format!("Failed to extract glyph: {}", e)))?;
    
    let sig = MicroSignature::from_outline(&outline);
    
    Ok(JsGlyphSignature {
        character: ch.to_string(),
        aspect_ratio: sig.aspect_ratio as u32,
        density: sig.density as u32,
        curve_ratio: sig.curve_ratio as u32,
        point_count: sig.point_count as u32,
        x_balance: sig.x_balance as u32,
        y_balance: sig.y_balance as u32,
    })
}

/// Build a glyph signature database from font files
/// 
/// Indexes the specified font files with Brotli-11 compression.
#[napi]
pub fn build_glyph_database(
    font_paths: Vec<String>,
    output_path: String,
) -> napi::Result<JsGlyphDbStats> {
    use font_glyph_db::GlyphDatabaseBuilder;
    
    let mut builder = GlyphDatabaseBuilder::new();
    
    for path in &font_paths {
        if let Err(e) = builder.add_font_auto(path) {
            // Log but continue - some fonts may fail
            eprintln!("Warning: Failed to add font {}: {}", path, e);
        }
    }
    
    let stats = builder.save_to_file(&output_path)
        .map_err(|e| napi::Error::from_reason(format!("Failed to save database: {}", e)))?;
    
    Ok(JsGlyphDbStats {
        font_count: stats.font_count as u32,
        char_count: stats.char_count as u32,
        uncompressed_size_mb: stats.uncompressed_size as f64 / 1_000_000.0,
        compressed_size_mb: stats.compressed_size as f64 / 1_000_000.0,
        compression_ratio: stats.compression_ratio as f64,
        build_time_seconds: stats.build_time_seconds as f64,
    })
}

/// Compare the visual similarity of two font glyphs
/// 
/// Returns a similarity score from 0.0 (different) to 1.0 (identical).
#[napi]
pub fn compare_glyph_signatures(
    font_path_a: String,
    font_path_b: String,
    character: String,
) -> napi::Result<f64> {
    use font_glyph::{GlyphExtractor, MicroSignature};
    
    let ch = character.chars().next()
        .ok_or_else(|| napi::Error::from_reason("Character string is empty"))?;
    
    let extractor = GlyphExtractor::new();
    
    let outline_a = extractor.extract_from_file(&font_path_a, ch)
        .map_err(|e| napi::Error::from_reason(format!("Failed to extract glyph from font A: {}", e)))?;
    
    let outline_b = extractor.extract_from_file(&font_path_b, ch)
        .map_err(|e| napi::Error::from_reason(format!("Failed to extract glyph from font B: {}", e)))?;
    
    let sig_a = MicroSignature::from_outline(&outline_a);
    let sig_b = MicroSignature::from_outline(&outline_b);
    
    Ok(sig_a.similarity(&sig_b) as f64)
}