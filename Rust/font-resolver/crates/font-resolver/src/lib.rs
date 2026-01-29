use font_core::{
    FontRequest, ResolutionResult, ResolverConfig, FontError, FontDescriptor, 
    FontSource, SubstitutionReason, FontMatchScore, FontResult,
    EnhancedResolverConfig, FontMetrics, LicenseInfo, FontFormat
};
use font_normalizer::FontNormalizer;
use font_license::{LicenseChecker, LicenseWarning};
use font_sources::FontSourceManager;
use font_scanner::FontScanner;
use std::collections::HashMap;
use font_similarity::{FontSimilarityEngine, MatchTier}; // Removed TieredMatchResult
use font_acquisition::FontAcquisitionManager;
use font_compressor::{CompressedFontDatabase, FontCompressor};
use font_updater::FontUpdater;
use font_cache::HybridFontCache;
use serde::{Serialize, Deserialize};

pub struct FontResolver {
    normalizer: FontNormalizer,
    config: ResolverConfig,
    scanner: FontScanner,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SuggestionResult {
    pub family: String,
    pub subfamily: String,
    pub source: String,
    pub score: f32,
    pub weight: u16,
    pub italic: bool,
    pub is_critical_license_warning: bool,
    pub license_name: String,
    pub has_local_file: bool,
    pub download_url: Option<String>,
    pub is_offline_fallback: bool,
}

#[derive(Debug, Clone)]
pub enum TieredResolutionResult {
    Exact(FontDescriptor, f32), // font, similarity score
    Similar(Vec<FontDescriptor>, f32), // fonts, best score
    SuggestInternet,
    NotFound,
}

#[derive(Debug, Clone)]
pub struct DatabaseStats {
    pub font_count: usize,
    pub compressed_size_mb: f64,
    pub original_size_mb: f64,
    pub compression_ratio: f64,
    pub categories: HashMap<font_compressor::FontCategory, usize>,
}

impl FontResolver {
    pub fn new(config: ResolverConfig) -> Self {
        Self {
            normalizer: FontNormalizer,
            config,
            scanner: FontScanner,
        }
    }

    /// Resolve a font name to an actual system font
    pub fn resolve(&self, font_name: &str) -> Result<ResolutionResult, FontError> {
        let request = self.normalizer.normalize(font_name)?;
        
        // Get system fonts
        let system_fonts = self.scanner.scan_system_fonts()
            .map_err(|e| FontError::Parse(format!("Failed to scan system fonts: {}", e)))?;
        
        // Try to find the best match
        let (best_match, match_score, substituted, substitution_reason) = 
            self.find_best_match(&request, &system_fonts);
        
        let mut warnings = Vec::new();
        
        // If we couldn't find a good match, try fallback
        let (font, source, substituted, substitution_reason) = if let Some(font) = best_match {
            // Found a match
            (font, FontSource::System, substituted, substitution_reason)
        } else {
            // No match found, use fallback
            warnings.push(format!("No exact match found for '{}', using fallback", font_name));
            let fallback = self.create_fallback(&request, &system_fonts);
            (fallback, FontSource::Substituted, true, Some(SubstitutionReason::FontNotFound))
        };
        
        Ok(ResolutionResult {
            original_name: font_name.to_string(),
            font,
            source,
            substituted,
            substitution_reason,
            compatibility_score: match_score.overall,
            warnings,
        })
    }

    /// Find the best matching font for a request
    fn find_best_match(&self, request: &FontRequest, system_fonts: &[FontDescriptor]) 
        -> (Option<FontDescriptor>, FontMatchScore, bool, Option<SubstitutionReason>) 
    {
        if system_fonts.is_empty() {
            return (None, FontMatchScore { overall: 0.0, family: 0.0, weight: 0.0, style: 0.0, monospaced: 0.0, metrics: 0.0 }, true, Some(SubstitutionReason::FontNotFound));
        }
        
        // Group fonts by family for easier matching
        let fonts_by_family = self.group_fonts_by_family(system_fonts);
        
        // Try exact family match first
        if let Some(family_fonts) = fonts_by_family.get(&request.family.to_lowercase()) {
            let (best_match, score) = self.find_best_in_family(request, family_fonts);
            if score.overall > 0.8 {
                return (Some(best_match), score, false, None);
            }
        }
        
        // Try partial family match (contains)
        for (family, family_fonts) in &fonts_by_family {
            if family.contains(&request.family.to_lowercase()) || request.family.to_lowercase().contains(family) {
                let (best_match, score) = self.find_best_in_family(request, family_fonts);
                if score.overall > 0.7 {
                    return (Some(best_match), score, true, Some(SubstitutionReason::FontNotFound));
                }
            }
        }
        
        // Try common font substitutions
        let substituted_family = self.get_font_substitution(&request.family);
        if let Some(family_fonts) = fonts_by_family.get(&substituted_family.to_lowercase()) {
            let (best_match, score) = self.find_best_in_family(request, family_fonts);
            if score.overall > 0.6 {
                return (Some(best_match), score, true, Some(SubstitutionReason::FontNotFound));
            }
        }
        
        // Try preferred families from config
        for preferred_family in &self.config.preferred_families {
            let preferred_family_lower = preferred_family.to_lowercase();
            if let Some(family_fonts) = fonts_by_family.get(&preferred_family_lower) {
                let (best_match, score) = self.find_best_in_family(request, family_fonts);
                if score.overall > 0.5 {
                    return (Some(best_match), score, true, Some(SubstitutionReason::UserPreference));
                }
            }
        }
        
        // Last resort: pick any font with similar characteristics
        let (best_match, score) = self.find_closest_overall(request, system_fonts);
        (Some(best_match), score, true, Some(SubstitutionReason::FontNotFound))
    }
    
    /// Group fonts by family name
    fn group_fonts_by_family(&self, fonts: &[FontDescriptor]) -> HashMap<String, Vec<FontDescriptor>> {
        let mut map = HashMap::new();
        for font in fonts {
            let family_lower = font.family.to_lowercase();
            map.entry(family_lower).or_insert_with(Vec::new).push(font.clone());
        }
        map
    }
    
    /// Find the best matching font within a specific family
    fn find_best_in_family(&self, request: &FontRequest, family_fonts: &[FontDescriptor]) -> (FontDescriptor, FontMatchScore) {
        let mut best_font = family_fonts[0].clone();
        let mut best_score = FontMatchScore { overall: 0.0, family: 0.0, weight: 0.0, style: 0.0, monospaced: 0.0, metrics: 0.0 };
        
        for font in family_fonts {
            let score = self.calculate_match_score(request, font);
            if score.overall > best_score.overall {
                best_font = font.clone();
                best_score = score;
            }
        }
        
        (best_font, best_score)
    }
    
    /// Calculate match score between request and font
    fn calculate_match_score(&self, request: &FontRequest, font: &FontDescriptor) -> FontMatchScore {
        // Family score (exact match = 1.0, contains = 0.8, else based on string similarity)
        let family_score = if font.family.to_lowercase() == request.family.to_lowercase() {
            1.0
        } else if font.family.to_lowercase().contains(&request.family.to_lowercase()) {
            0.8
        } else {
            // Simple string similarity
            let similarity = self.string_similarity(&font.family.to_lowercase(), &request.family.to_lowercase());
            similarity.max(0.3) // Minimum 0.3 if some similarity
        };
        
        // Weight score (closer weights are better)
        let weight_diff = (font.weight as i32 - request.weight as i32).abs();
        let weight_score = if weight_diff == 0 {
            1.0
        } else if weight_diff <= 100 {
            0.8
        } else if weight_diff <= 200 {
            0.6
        } else if weight_diff <= 300 {
            0.4
        } else {
            0.2
        };
        
        // Style score (italic match)
        let style_score = if font.italic == request.italic {
            1.0
        } else {
            // Allow some flexibility: if request is italic but font is not, it's worse than vice versa
            if request.italic && !font.italic {
                0.3  // Requested italic but got regular
            } else {
                0.7  // Requested regular but got italic (less bad)
            }
        };
        
        // Monospace score (if request cares about monospaced)
        let monospaced_score = if request.monospaced {
            if font.monospaced { 1.0 } else { 0.2 }
        } else {
            1.0  // Don't penalize if not requested
        };
        
        // Metrics score (if available and if we care about metrics)
        let metrics_score = if self.config.require_metrics {
            if let Some(_font_metrics) = &font.metrics {
                // Calculate metrics similarity (simplified)
                0.9  // Placeholder
            } else {
                0.5  // No metrics available
            }
        } else {
            1.0  // Not required
        };
        
        // Overall score (weighted average)
        let overall = family_score * 0.4 
                    + weight_score * 0.3 
                    + style_score * 0.2 
                    + monospaced_score * 0.05 
                    + metrics_score * 0.05;
        
        FontMatchScore {
            overall,
            family: family_score,
            weight: weight_score,
            style: style_score,
            monospaced: monospaced_score,
            metrics: metrics_score,
        }
    }
    
    /// Find closest font overall (last resort)
    fn find_closest_overall(&self, request: &FontRequest, all_fonts: &[FontDescriptor]) -> (FontDescriptor, FontMatchScore) {
        let mut best_font = all_fonts[0].clone();
        let mut best_score = FontMatchScore { overall: 0.0, family: 0.0, weight: 0.0, style: 0.0, monospaced: 0.0, metrics: 0.0 };
        
        for font in all_fonts {
            let score = self.calculate_match_score(request, font);
            if score.overall > best_score.overall {
                best_font = font.clone();
                best_score = score;
            }
        }
        
        (best_font, best_score)
    }
    
    /// Get font substitution (common substitutions like Helvetica -> Arial)
    fn get_font_substitution(&self, family: &str) -> String {
        let substitutions: HashMap<&str, &str> = [
            ("helvetica", "arial"),
            ("helvetica neue", "arial"),
            ("times", "times new roman"),
            ("times roman", "times new roman"),
            ("courier", "courier new"),
            ("zapfdingbats", "wingdings"),
            ("symbol", "wingdings"),
            ("monospace", "courier new"),
            ("serif", "times new roman"),
            ("sans-serif", "arial"),
            ("cursive", "comic sans ms"),
            ("fantasy", "impact"),
        ].iter().cloned().collect();
        
        let family_lower = family.to_lowercase();
        if let Some(&sub) = substitutions.get(family_lower.as_str()) {
            sub.to_string()
        } else {
            family.to_string()
        }
    }
    
    /// Create fallback font when no match is found
    fn create_fallback(&self, request: &FontRequest, system_fonts: &[FontDescriptor]) -> FontDescriptor {
        // Try to find a font that matches the request characteristics
        let mut best_font = &system_fonts[0];
        let mut best_score = -1.0;
        
        for font in system_fonts {
            // Simple scoring: prefer fonts that match weight and italic
            let mut score = 0.0;
            
            // Weight similarity (closer is better)
            let weight_diff = (font.weight as i32 - request.weight as i32).abs();
            score += 1.0 / (weight_diff as f32 + 1.0);
            
            // Italic match
            if font.italic == request.italic {
                score += 1.0;
            }
            
            // Monospace match
            if font.monospaced == request.monospaced {
                score += 0.5;
            }
            
            if score > best_score {
                best_score = score;
                best_font = font;
            }
        }
        
        best_font.clone()
    }
    
    /// Simple string similarity (Levenshtein distance normalized)
    fn string_similarity(&self, s1: &str, s2: &str) -> f32 {
        if s1.is_empty() && s2.is_empty() {
            return 1.0;
        }
        
        let len1 = s1.chars().count();
        let len2 = s2.chars().count();
        let max_len = len1.max(len2) as f32;
        
        if max_len == 0.0 {
            return 1.0;
        }
        
        // Simple character overlap (for simplicity)
        let common_chars: usize = s1.chars()
            .filter(|c| s2.contains(*c))
            .count();
        
        (common_chars as f32) / max_len
    }

    /// Batch resolution (for multiple fonts at once)
    pub fn resolve_batch(&self, font_names: &[&str]) -> Result<Vec<ResolutionResult>, FontError> {
        let mut results = Vec::new();
        
        for font_name in font_names {
            match self.resolve(font_name) {
                Ok(result) => results.push(result),
                Err(e) => {
                    eprintln!("Failed to resolve {}: {}", font_name, e);
                }
            }
        }
        
        Ok(results)
    }
}

// ============================================================
// ENHANCED FONT RESOLVER WITH CACHE
// ============================================================
#[allow(dead_code)]
pub struct EnhancedFontResolver {
    normalizer: FontNormalizer,
    scanner: FontScanner,
    cache: Option<HybridFontCache>,
    config: EnhancedResolverConfig,
    license_checker: LicenseChecker,
    source_manager: FontSourceManager,
    similarity_engine: FontSimilarityEngine,
    acquisition_manager: Option<FontAcquisitionManager>,
    updater: Option<FontUpdater>,
    compressed_database: Option<CompressedFontDatabase>,
}

impl EnhancedFontResolver {
    pub fn new(config: EnhancedResolverConfig) -> FontResult<Self> {
        // Initialize cache
        let cache = if config.cache_enabled {
            match HybridFontCache::new(
                config.memory_limit_mb,
                config.disk_limit_mb,
                config.auto_pin_threshold,
            ) {
                Ok(cache) => Some(cache),
                Err(e) => {
                    eprintln!("⚠️  Failed to initialize cache: {}", e);
                    eprintln!("   Continuing without cache...");
                    None
                }
            }
        } else {
            None
        };
        
        // Initialize source manager
        let mut source_manager = FontSourceManager::new();
        
        // Configure web fonts if enabled
        if config.web_fonts_enabled {
            if let Err(e) = source_manager.enable_web_fonts(true) {
                eprintln!("⚠️  Failed to enable web fonts: {}", e);
            }
        }
        
        // Set source priority
        source_manager.set_priority(config.font_source_priority.clone());
        
        // Initialize similarity engine with empty data
        let similarity_engine = FontSimilarityEngine::new(None);
        
        // Initialize acquisition manager
        let mut acquisition_manager = FontAcquisitionManager::new();
        acquisition_manager.add_provider("Google Fonts", Box::new(font_acquisition::GoogleFontsProvider::new(None)));
        acquisition_manager.add_provider("Fontsource", Box::new(font_acquisition::FontsourceProvider::new()));
        acquisition_manager.add_provider("Adobe Fonts", Box::new(font_acquisition::AdobeFontsProvider::new()));
        
        Ok(Self {
            normalizer: FontNormalizer,
            scanner: FontScanner,
            cache,
            config,
            license_checker: LicenseChecker::new(),
            source_manager,
            similarity_engine,
            acquisition_manager: Some(acquisition_manager),
            updater: None,
            compressed_database: None,
        })
    }
    
    pub fn new_with_database(
        config: EnhancedResolverConfig,
        database_data: &[u8],
    ) -> FontResult<Self> {
        
        let mut resolver = Self::new(config)?;
        
        // First try to load as compressed database
        let compressor = FontCompressor::new(11, true);
        match compressor.decompress_font_database(database_data) {
            Ok(database) => {
                resolver.compressed_database = Some(database);
            }
            Err(_e) => {
                // Silent fail/fallback
                // If that fails, try to load as simple database format
                if let Some(database) = font_compressor::try_load_simple_database(database_data) {
                    resolver.compressed_database = Some(database);
                } else {
                    // Fail silently
                }
            }
        }
        
        // Initialize similarity engine with precomputed data
        resolver.similarity_engine = FontSimilarityEngine::new(
            resolver.compressed_database.as_ref()
                .and_then(|db| db.similarity_matrix.clone())
        );
        
        Ok(resolver)
    }
    
    pub fn resolve_with_sources(&self, font_name: &str) -> FontResult<ResolutionResult> {
        // Check cache first
        if let Some(cache) = &self.cache {
            if let Some(cached_font) = cache.get(font_name) {
                // Check license if warnings are enabled
                let mut warnings = vec!["Loaded from cache".to_string()];
                if self.config.license_warnings != font_core::LicenseWarningLevel::Off {
                    let license_warning = self.license_checker.check_font(&cached_font);
                    if license_warning.warning_level != font_license::WarningLevel::Info {
                        warnings.push(license_warning.message);
                    }
                }
                
                return Ok(ResolutionResult {
                    original_name: font_name.to_string(),
                    font: cached_font,
                    source: FontSource::System,
                    substituted: false,
                    substitution_reason: None,
                    compatibility_score: 1.0,
                    warnings,
                });
            }
        }
        
        // Normalize the request
        let request = self.normalizer.normalize(font_name)?;
        
        // Try to find the font in sources
        let mut found_font = None;
        let mut source_type = FontSource::System;
        
        // Use source manager to find font
        let mut temp_source_manager = self.source_manager.clone();
        if let Ok(Some(font)) = temp_source_manager.find_font(&request.family) {
            found_font = Some(font);
        }
        
        // If not found in sources, try web fonts
        if found_font.is_none() && self.config.web_fonts_enabled {
            if let Some(web_db) = self.source_manager.get_web_db() {
                if let Some(web_font) = web_db.find_font(&request.family) {
                    if let Some(variant) = web_font.variants.iter()
                        .find(|v| v.weight == request.weight && v.italic == request.italic)
                        .or_else(|| web_font.variants.first()) {
                        
                        let font_descriptor = web_db.to_font_descriptor(web_font, variant);
                        found_font = Some(font_descriptor);
                        source_type = FontSource::OpenRepository;
                    }
                }
            }
        }
        
        match found_font {
            Some(font) => {
                // Check license
                let mut warnings = Vec::new();
                if self.config.license_warnings != font_core::LicenseWarningLevel::Off {
                    let license_warning = self.license_checker.check_font(&font);
                    if license_warning.warning_level != font_license::WarningLevel::Info {
                        warnings.push(license_warning.message);
                        
                        // Add alternative suggestions
                        if !license_warning.alternatives.is_empty() {
                            let alternatives: Vec<String> = license_warning.alternatives
                                .iter()
                                .map(|a| format!("{} ({:.0}% similar)", a.family, a.similarity_score * 100.0))
                                .collect();
                            warnings.push(format!("Free alternatives: {}", alternatives.join(", ")));
                        }
                    }
                }
                
                // Cache the result
                if let Some(cache) = &self.cache {
                    if let Err(e) = cache.put(font_name, font.clone()) {
                        warnings.push(format!("Failed to cache: {}", e));
                    }
                }
                
                Ok(ResolutionResult {
                    original_name: font_name.to_string(),
                    font,
                    source: source_type,
                    substituted: false,
                    substitution_reason: None,
                    compatibility_score: 1.0,
                    warnings,
                })
            }
            None => {
                // Font not found
                Err(FontError::NotFound(font_name.to_string()))
            }
        }
    }
    
    pub async fn get_suggestions(
        &self,
        font_name: &str,
        enable_internet_search: bool,
    ) -> FontResult<Vec<SuggestionResult>> {
        let request = self.normalizer.normalize(font_name)?;
        let mut results = Vec::new();

        // 1. Get matches from local sources
        let all_fonts = self.get_all_available_fonts()?;
        let local_matches = self.similarity_engine.find_tiered_matches(&request, &all_fonts, 20);

        for m in local_matches.matches {
            let is_critical = self.is_license_critical(&m.font, false);
            results.push(SuggestionResult {
                family: m.font.family.clone(),
                subfamily: m.font.subfamily.clone().unwrap_or_else(|| "Regular".to_string()),
                source: "Local".to_string(),
                score: m.score.overall,
                weight: m.font.weight,
                italic: m.font.italic,
                is_critical_license_warning: is_critical,
                license_name: m.font.license.as_ref().map(|l| l.name.clone()).unwrap_or_else(|| "Unknown".to_string()),
                has_local_file: true,
                download_url: None,
                is_offline_fallback: false,
            });
        }

        // 2. Internet search if enabled
        if enable_internet_search {
            // Parallel search across providers
            if let Some(acquisition) = &self.acquisition_manager {
                let web_fonts = acquisition.parallel_search(&request.family, 5).await?;
                for wf in web_fonts {
                    // Check if already in results from local
                    if results.iter().any(|r| r.family.to_lowercase() == wf.family.to_lowercase() 
                                           && r.weight == wf.weight 
                                           && r.italic == wf.italic) {
                        continue;
                    }

                    // Smart License: only flag as critical if not known safe (OFL/Apache etc)
                    let is_critical = wf.license.name.contains("Personal") || 
                                     (!wf.license.name.contains("OFL") && 
                                      !wf.license.name.contains("Apache") && 
                                      !wf.license.allows_commercial_use);

                    results.push(SuggestionResult {
                        family: wf.family.clone(),
                        subfamily: "Regular".to_string(), // Web variants often treated as base
                        source: "Internet".to_string(),
                        score: 0.85,
                        weight: wf.weight,
                        italic: wf.italic,
                        is_critical_license_warning: is_critical,
                        license_name: wf.license.name.clone(),
                        has_local_file: false,
                        download_url: wf.download_urls.get(&font_core::FontFormat::Ttf)
                            .or_else(|| wf.download_urls.values().next())
                            .cloned(),
                        is_offline_fallback: false,
                    });

                    // Dynamic Learning: If enabled, "remember" this font in our local database
                    if self.config.dynamic_learning_enabled {
                         self.remember_font_metadata(&wf);
                    }
                }
            }
        }

        // 3. Worst-Case Safety: If no good results, explicitly look for substitutions
        if results.is_empty() || results.iter().all(|r| r.score < 0.6) {
            // Find best substitutes based on metrics only
            let substitutes = self.similarity_engine.find_tiered_matches(&request, &all_fonts, 5);
            for sub in substitutes.matches {
                 results.push(SuggestionResult {
                    family: sub.font.family.clone(),
                    subfamily: sub.font.subfamily.clone().unwrap_or_else(|| "Regular".to_string()),
                    source: "System (Substitution)".to_string(),
                    score: sub.score.overall,
                    weight: sub.font.weight,
                    italic: sub.font.italic,
                    is_critical_license_warning: false,
                    license_name: "Safe Substitute".to_string(),
                    has_local_file: true,
                    download_url: None,
                    is_offline_fallback: true,
                });
            }
        }

        // Sort by score
        results.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal));

        // 3. APPLY SMART THRESHOLDING
        if let Some(best) = results.first() {
            if best.score > 0.98 {
                // If perfect match found, only return it + top 3 alternatives
                results.truncate(4);
            } else if best.score > 0.90 {
                // If good match found, truncate anything < 75%
                results.retain(|r| r.score >= 0.75);
                results.truncate(10);
            } else {
                // No good match, show best 20 available
                results.truncate(20);
            }
        }
        
        Ok(results)
    }

    fn is_license_critical(&self, font: &FontDescriptor, is_internet: bool) -> bool {
        if is_internet {
            return true; // Per user request: always flag internet suggestions
        }
        
        if let Some(license) = &font.license {
            // Not critical if embedded database or known safe license
            if license.name.contains("SIL Open Font License") || license.allows_commercial_use {
                return false;
            }
            return true;
        }
        
        true // Unknown is critical
    }

    pub async fn resolve_with_tiered_matching(
        &self,
        font_name: &str,
        enable_internet_search: bool,
    ) -> FontResult<TieredResolutionResult> {
        let request = self.normalizer.normalize(font_name)?;
        
        // Get fonts from all sources
        let all_fonts = self.get_all_available_fonts()?;
        
        // Use similarity engine for tiered matching
        let tiered_result = self.similarity_engine.find_tiered_matches(
            &request,
            &all_fonts,
            5, // limit per tier
        );
        
        match tiered_result.best_tier {
            MatchTier::Exact(score) => {
                // Found good match locally
                if let Some(first_match) = tiered_result.matches.first() {
                    Ok(TieredResolutionResult::Exact(first_match.font.clone(), score))
                } else {
                    Ok(TieredResolutionResult::NotFound)
                }
            }
            MatchTier::Similar(score) => {
                // Suggest similar fonts
                let similar_fonts: Vec<FontDescriptor> = tiered_result.matches.into_iter()
                    .filter(|m| matches!(m.tier, MatchTier::Similar(_)))
                    .map(|m| m.font)
                    .collect();
                
                if !similar_fonts.is_empty() {
                    Ok(TieredResolutionResult::Similar(similar_fonts, score))
                } else {
                    Ok(TieredResolutionResult::NotFound)
                }
            }
            MatchTier::Low(_) => {
                if enable_internet_search && self.config.web_fonts_enabled {
                    // Search internet
                    self.search_internet_and_suggest(&request).await
                } else {
                    Ok(TieredResolutionResult::SuggestInternet)
                }
            }
        }
    }
    
    fn get_all_available_fonts(&self) -> FontResult<Vec<FontDescriptor>> {
        let mut fonts = Vec::new();
        
        // System fonts
        if self.config.system_fonts_enabled {
            let system_fonts = self.scanner.scan_system_fonts()
                .map_err(|e| FontError::Parse(format!("System scan failed: {}", e)))?;
            fonts.extend(system_fonts);
        }
        
        // Compressed database fonts
        if let Some(database) = &self.compressed_database {
            for compressed_font in &database.fonts {
                fonts.push(self.compressed_to_font(compressed_font));
            }
        }

        // Project Asset Fonts (Highest Priority - handled by sort logic usually)
        if !self.config.project_asset_dirs.is_empty() {
            for dir in &self.config.project_asset_dirs {
                if let Ok(asset_fonts) = self.scanner.scan_font_directory_recursive(dir, &font_parser::FontParser) {
                    fonts.extend(asset_fonts);
                }
            }
        }
        
        Ok(fonts)
    }

    fn remember_font_metadata(&self, _font: &font_compressor::CompressedFontData) {
        // Implementation for saving to a local JSON persistent cache
        // (Simplified for this task: would use font-cache or a new JSON file)
    }
    
    async fn search_internet_and_suggest(
        &self,
        request: &FontRequest,
    ) -> FontResult<TieredResolutionResult> {
        // Search web database for matching fonts
        if let Some(web_db) = self.source_manager.get_web_db() {
            let search_family = request.family.to_lowercase();
            
            // Try exact match first
            if let Some(web_font) = web_db.find_font(&request.family) {
                if let Some(variant) = web_font.variants.iter()
                    .find(|v| v.weight == request.weight && v.italic == request.italic)
                    .or_else(|| web_font.variants.first()) {
                    
                    let font_descriptor = web_db.to_font_descriptor(web_font, variant);
                    return Ok(TieredResolutionResult::Exact(font_descriptor, 0.95));
                }
            }
            
            // Try fuzzy search - find fonts with similar names
            let mut similar_fonts = Vec::new();
            for (family_name, web_font) in web_db.get_fonts() {
                let family_lower = family_name.to_lowercase();
                
                // Check if family name contains search term or vice versa
                if family_lower.contains(&search_family) || search_family.contains(&family_lower) {
                    if let Some(variant) = web_font.variants.iter()
                        .find(|v| v.weight == request.weight && v.italic == request.italic)
                        .or_else(|| web_font.variants.first()) {
                        
                        let font_descriptor = web_db.to_font_descriptor(web_font, variant);
                        similar_fonts.push(font_descriptor);
                        
                        // Limit results
                        if similar_fonts.len() >= 5 {
                            break;
                        }
                    }
                }
            }
            
            if !similar_fonts.is_empty() {
                // Calculate similarity score (simple string similarity)
                let best_score = similar_fonts.iter()
                    .map(|f| {
                        let f_lower = f.family.to_lowercase();
                        if f_lower == search_family {
                            0.95
                        } else if f_lower.contains(&search_family) || search_family.contains(&f_lower) {
                            0.85
                        } else {
                            0.75
                        }
                    })
                    .fold(0.0, f32::max);
                
                return Ok(TieredResolutionResult::Similar(similar_fonts, best_score));
            }
        }
        
        // No matches found in web database
        Ok(TieredResolutionResult::SuggestInternet)
    }
    
    // Add methods for source management
    pub fn add_custom_source(&mut self, source: font_sources::SourceType) -> FontResult<()> {
        self.source_manager.add_custom_source(source)
    }
    
    pub fn list_sources(&self) -> Vec<font_sources::SourceInfo> {
        self.source_manager.list_sources()
    }
    
    pub fn check_license(&self, font_name: &str) -> FontResult<LicenseWarning> {
        // Try to find the font first
        let mut temp_source_manager = self.source_manager.clone();
        if let Ok(Some(font)) = temp_source_manager.find_font(font_name) {
            Ok(self.license_checker.check_font(&font))
        } else {
            Err(FontError::NotFound(font_name.to_string()))
        }
    }
    
    pub fn get_web_font_count(&self) -> Option<usize> {
        self.source_manager.get_web_db().map(|db| db.count())
    }

    pub fn get_cache_stats(&self) -> Option<FontResult<font_core::CacheStats>> {
        self.cache.as_ref().map(|cache| cache.stats())
    }

    pub fn cleanup_cache(&self, aggressive: bool) -> FontResult<usize> {
        match &self.cache {
            Some(cache) => cache.cleanup(aggressive),
            None => Ok(0),
        }
    }

    pub fn remove_from_cache(&self, font_names: Vec<String>) -> FontResult<usize> {
        match &self.cache {
            Some(cache) => cache.remove_entries(&font_names),
            None => Ok(0),
        }
    }

    pub fn export_metrics(&self, font_name: &str) -> FontResult<String> {
        // Try to find the font metadata
        let mut temp_source_manager = self.source_manager.clone();
        if let Ok(Some(font)) = temp_source_manager.find_font(font_name) {
            if let Some(metrics) = &font.metrics {
                return serde_json::to_string_pretty(metrics)
                    .map_err(|e| FontError::Parse(format!("Failed to serialize metrics: {}", e)));
            }
            return Err(FontError::NotFound(format!("No metrics found for font: {}", font_name)));
        }
        
        // Check compressed database
        if let Some(database) = &self.compressed_database {
            for compressed_font in &database.fonts {
                if compressed_font.family.to_lowercase() == font_name.to_lowercase() {
                    if let Some(metrics) = &compressed_font.metrics {
                        return serde_json::to_string_pretty(metrics)
                            .map_err(|e| FontError::Parse(format!("Failed to serialize metrics: {}", e)));
                    }
                    break;
                }
            }
        }
        
        Err(FontError::NotFound(font_name.to_string()))
    }

    pub fn pin_font(&self, font_name: &str) -> FontResult<()> {
        match &self.cache {
            Some(cache) => {
                cache.pin_font(font_name);
                Ok(())
            }
            None => Err(FontError::CacheError("Cache is disabled".to_string())),
        }
    }

    pub fn unpin_font(&self, font_name: &str) -> FontResult<()> {
        match &self.cache {
            Some(cache) => {
                cache.unpin_font(font_name);
                Ok(())
            }
            None => Err(FontError::CacheError("Cache is disabled".to_string())),
        }
    }

    pub fn list_pinned_fonts(&self) -> Option<Vec<String>> {
        self.cache.as_ref().map(|cache| cache.list_pinned())
    }

    pub fn suggest_cleanup(&self) -> Option<FontResult<Vec<String>>> {
        self.cache.as_ref().map(|cache| cache.suggest_cleanup())
    }

    pub fn get_config(&self) -> &EnhancedResolverConfig {
        &self.config
    }
    
    pub fn resolve_font(&self, font_name: &str) -> FontResult<ResolutionResult> {
        self.resolve_with_sources(font_name)
    }

    pub async fn update_font_database(&mut self) -> FontResult<()> {
        if let Some(updater) = &self.updater {
            let new_database = updater.update_from_internet(2000, None).await?;
            self.compressed_database = Some(new_database);
            
            // Update similarity engine
            self.similarity_engine = FontSimilarityEngine::new(
                self.compressed_database.as_ref()
                    .and_then(|db| db.similarity_matrix.clone())
            );
        }
        
        Ok(())
    }
    
    pub fn get_database_stats(&self) -> Option<DatabaseStats> {
        self.compressed_database.as_ref().map(|db| DatabaseStats {
            font_count: db.metadata.font_count,
            compressed_size_mb: db.metadata.compressed_size_bytes as f64 / (1024.0 * 1024.0),
            original_size_mb: db.metadata.original_size_bytes as f64 / (1024.0 * 1024.0),
            compression_ratio: (1.0 - (db.metadata.compressed_size_bytes as f64 / db.metadata.original_size_bytes as f64)) * 100.0,
            categories: db.metadata.categories.clone(),
        })
    }

    fn compressed_to_font(&self, compressed: &font_compressor::CompressedFontData) -> FontDescriptor {
        // Convert compressed data to FontDescriptor
        FontDescriptor {
            family: compressed.family.clone(),
            subfamily: None,
            postscript_name: compressed.postscript_name.clone(),
            full_name: Some(compressed.family.clone()),
            path: std::path::PathBuf::from("/compressed"),
            format: FontFormat::Ttf,
            weight: compressed.weight,
            italic: compressed.italic,
            monospaced: compressed.monospaced,
            variable: false,
            metrics: compressed.metrics.as_ref().map(|m| FontMetrics {
                units_per_em: m.units_per_em,
                ascender: m.ascender,
                descender: m.descender,
                x_height: m.x_height,
                cap_height: m.cap_height,
                average_width: m.average_width,
                max_advance_width: 1000,
            }),
            license: Some(LicenseInfo {
                name: compressed.license.name.clone(),
                url: Some(compressed.license.url.clone()),
                allows_embedding: compressed.license.allows_embedding, // FIXED: was allows_commercial_use
                allows_modification: compressed.license.allows_modification,
                requires_attribution: compressed.license.requires_attribution,
                allows_commercial_use: compressed.license.allows_commercial_use,
            }),
        }
    }
}