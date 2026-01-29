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