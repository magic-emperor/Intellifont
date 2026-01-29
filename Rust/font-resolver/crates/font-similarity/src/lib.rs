use std::collections::HashMap;
use font_core::{FontDescriptor, FontRequest, FontMatchScore};
use font_compressor::FontCategory;

#[derive(Debug, Clone, PartialEq)]
pub enum MatchTier {
    Exact(f32),      // 0.9-1.0
    Similar(f32),    // 0.8-0.9
    Low(f32),        // < 0.8
}

impl MatchTier {
    pub fn from_score(score: f32) -> Self {
        match score {
            s if s >= 0.9 => MatchTier::Exact(s),
            s if s >= 0.8 => MatchTier::Similar(s),
            s => MatchTier::Low(s),
        }
    }
    
    pub fn should_suggest_internet_search(&self) -> bool {
        matches!(self, MatchTier::Low(_))
    }
    
    pub fn is_good_match(&self) -> bool {
        matches!(self, MatchTier::Exact(_) | MatchTier::Similar(_))
    }
}

#[derive(Debug, Clone)]
pub struct TieredMatchResult {
    pub original_request: FontRequest,
    pub matches: Vec<FontMatch>,
    pub best_tier: MatchTier,
    pub suggestions: Vec<FontSuggestion>,
}

#[derive(Debug, Clone)]
pub struct FontMatch {
    pub font: FontDescriptor,
    pub score: FontMatchScore,
    pub tier: MatchTier,
    pub similarity_details: SimilarityDetails,
}

#[derive(Debug, Clone)]
pub struct FontSuggestion {
    pub font: FontDescriptor,
    pub similarity_score: f32,
    pub reason: String,
    pub source: SuggestionSource,
}

#[derive(Debug, Clone)]
pub enum SuggestionSource {
    LocalDatabase,
    InternetSearch,
    Substitution,
}

#[derive(Debug, Clone)]
pub struct SimilarityDetails {
    pub name_similarity: f32,
    pub weight_similarity: f32,
    pub style_similarity: f32,
    pub category_similarity: f32,
    pub metrics_similarity: f32,
}

pub struct FontSimilarityEngine {
    precomputed_similarities: HashMap<String, Vec<(String, f32)>>,
}

impl FontSimilarityEngine {
    pub fn new(similarity_matrix: Option<HashMap<String, Vec<(String, f32)>>>) -> Self {
        Self {
            precomputed_similarities: similarity_matrix.unwrap_or_default(),
        }
    }
    
    pub fn calculate_comprehensive_similarity(
        &self,
        request: &FontRequest,
        font: &FontDescriptor,
        use_precomputed: bool,
    ) -> (FontMatchScore, SimilarityDetails) {
        // Try precomputed first for speed
        if use_precomputed {
            if let Some(precomputed) = self.get_precomputed_similarity(&request.family, &font.family) {
                let score = FontMatchScore {
                    overall: precomputed,
                    family: precomputed,
                    weight: 1.0,
                    style: 1.0,
                    monospaced: 1.0,
                    metrics: 1.0,
                };
                
                return (score, SimilarityDetails {
                    name_similarity: precomputed,
                    weight_similarity: 1.0,
                    style_similarity: 1.0,
                    category_similarity: 1.0,
                    metrics_similarity: 1.0,
                });
            }
        }
        
        // Calculate detailed similarity
        let name_similarity = self.calculate_name_similarity(&request.family, &font.family);
        let weight_similarity = self.calculate_weight_similarity(request.weight, font.weight);
        let style_similarity = self.calculate_style_similarity(request.italic, font.italic);
        let category_similarity = self.calculate_category_similarity(request, font);
        let metrics_similarity = self.calculate_metrics_similarity(request, font);
        
        let overall = self.combine_scores(
            name_similarity,
            weight_similarity,
            style_similarity,
            category_similarity,
            metrics_similarity,
            request.monospaced,
            font.monospaced,
        );
        
        let score = FontMatchScore {
            overall,
            family: name_similarity,
            weight: weight_similarity,
            style: style_similarity,
            monospaced: if request.monospaced == font.monospaced { 1.0 } else { 0.0 },
            metrics: metrics_similarity,
        };
        
        let details = SimilarityDetails {
            name_similarity,
            weight_similarity,
            style_similarity,
            category_similarity,
            metrics_similarity,
        };
        
        (score, details)
    }
    
    pub fn find_tiered_matches(
        &self,
        request: &FontRequest,
        fonts: &[FontDescriptor],
        limit_per_tier: usize,
    ) -> TieredMatchResult {
        let mut exact_matches = Vec::new();
        let mut similar_matches = Vec::new();
        let mut low_matches = Vec::new();
        
        // Calculate scores for all fonts
        for font in fonts {
            let (score, details) = self.calculate_comprehensive_similarity(request, font, true);
            let tier = MatchTier::from_score(score.overall);
            
            let match_result = FontMatch {
                font: font.clone(),
                score,
                tier: tier.clone(),
                similarity_details: details,
            };
            
            match tier {
                MatchTier::Exact(_) => exact_matches.push(match_result),
                MatchTier::Similar(_) => similar_matches.push(match_result),
                MatchTier::Low(_) => low_matches.push(match_result),
            }
        }
        
        // Sort each tier by score
        exact_matches.sort_by(|a, b| b.score.overall.partial_cmp(&a.score.overall).unwrap());
        similar_matches.sort_by(|a, b| b.score.overall.partial_cmp(&a.score.overall).unwrap());
        low_matches.sort_by(|a, b| b.score.overall.partial_cmp(&a.score.overall).unwrap());
        
        // Limit results
        exact_matches.truncate(limit_per_tier);
        similar_matches.truncate(limit_per_tier);
        low_matches.truncate(limit_per_tier);
        
        // Combine all matches
        let mut all_matches = Vec::new();
        all_matches.extend(exact_matches);
        all_matches.extend(similar_matches);
        all_matches.extend(low_matches);
        
        // Determine best tier
        let best_tier = all_matches.first()
            .map(|m| m.tier.clone())
            .unwrap_or(MatchTier::Low(0.0));
        
        TieredMatchResult {
            original_request: request.clone(),
            matches: all_matches,
            best_tier,
            suggestions: Vec::new(), // Will be populated by caller
        }
    }
    
    pub fn generate_suggestions(
        &self,
        tiered_result: &TieredMatchResult,
        include_reasons: bool,
    ) -> Vec<FontSuggestion> {
        let mut suggestions = Vec::new();
        
        for font_match in &tiered_result.matches {
            let suggestion = FontSuggestion {
                font: font_match.font.clone(),
                similarity_score: font_match.score.overall,
                reason: if include_reasons {
                    self.generate_suggestion_reason(font_match)
                } else {
                    String::new()
                },
                source: SuggestionSource::LocalDatabase,
            };
            
            suggestions.push(suggestion);
        }
        
        // Add substitution suggestions for low matches
        if tiered_result.best_tier.should_suggest_internet_search() {
            let substitution = self.get_best_substitution(&tiered_result.original_request);
            suggestions.push(substitution);
        }
        
        suggestions
    }
    
    fn calculate_name_similarity(&self, name1: &str, name2: &str) -> f32 {
        let name1_lower = name1.to_lowercase();
        let name2_lower = name2.to_lowercase();
        
        // Exact match
        if name1_lower == name2_lower {
            return 1.0;
        }
        
        // Contains match
        if name1_lower.contains(&name2_lower) || name2_lower.contains(&name1_lower) {
            return 0.85;
        }
        
        // Word overlap
        let words1: Vec<&str> = name1_lower.split_whitespace().collect();
        let words2: Vec<&str> = name2_lower.split_whitespace().collect();
        
        let common_words: f32 = words1.iter()
            .filter(|w| words2.contains(w))
            .count() as f32;
        
        let total_words = words1.len().max(words2.len()) as f32;
        
        if total_words > 0.0 {
            common_words / total_words
        } else {
            0.0
        }
    }
    
    fn calculate_weight_similarity(&self, weight1: u16, weight2: u16) -> f32 {
        let diff = (weight1 as i32 - weight2 as i32).abs();
        
        match diff {
            0 => 1.0,
            1..=100 => 0.8,
            101..=200 => 0.6,
            201..=300 => 0.4,
            _ => 0.2,
        }
    }
    
    fn calculate_style_similarity(&self, italic1: bool, italic2: bool) -> f32 {
        if italic1 == italic2 {
            1.0
        } else if italic1 && !italic2 {
            0.4 // Requested italic, got regular
        } else {
            0.7 // Requested regular, got italic
        }
    }
    
    fn calculate_category_similarity(&self, request: &FontRequest, font: &FontDescriptor) -> f32 {
        let request_category = self.detect_request_category(request);
        let font_category = self.detect_font_category(font);
        
        if request_category == font_category {
            return 1.0;
        }
        
        // Category compatibility matrix
        match (&request_category, &font_category) {
            (FontCategory::Serif, FontCategory::SansSerif) => 0.3,
            (FontCategory::SansSerif, FontCategory::Serif) => 0.3,
            (FontCategory::Monospace, _) if request.monospaced && !font.monospaced => 0.1,
            (_, FontCategory::Monospace) if !request.monospaced && font.monospaced => 0.2,
            (FontCategory::Handwriting, FontCategory::Display) => 0.6,
            (FontCategory::Display, FontCategory::Handwriting) => 0.6,
            _ => 0.4,
        }
    }
    
    fn calculate_metrics_similarity(&self, _request: &FontRequest, font: &FontDescriptor) -> f32 {
        // If no metrics available, return neutral score
        if font.metrics.is_none() {
            return 0.7;
        }
        
        // Calculate metrics similarity
        // This is simplified - real implementation would compare x-height, cap-height, etc.
        0.8
    }
    
    fn combine_scores(
        &self,
        name: f32,
        weight: f32,
        style: f32,
        category: f32,
        metrics: f32,
        requested_mono: bool,
        font_mono: bool,
    ) -> f32 {
        let mut weights = HashMap::new();
        weights.insert("name", 0.35);
        weights.insert("weight", 0.25);
        weights.insert("style", 0.20);
        weights.insert("category", 0.15);
        weights.insert("metrics", 0.05);
        
        let base_score = name * weights["name"]
            + weight * weights["weight"]
            + style * weights["style"]
            + category * weights["category"]
            + metrics * weights["metrics"];
        
        // Penalize monospace mismatch
        if requested_mono != font_mono {
            base_score * 0.7
        } else {
            base_score
        }
    }
    
    fn detect_request_category(&self, request: &FontRequest) -> FontCategory {
        let name_lower = request.family.to_lowercase();
        
        if request.monospaced {
            FontCategory::Monospace
        } else if name_lower.contains("serif") {
            FontCategory::Serif
        } else if name_lower.contains("sans") {
            FontCategory::SansSerif
        } else if name_lower.contains("script") || name_lower.contains("hand") {
            FontCategory::Handwriting
        } else if name_lower.contains("display") || name_lower.contains("decorative") {
            FontCategory::Display
        } else {
            FontCategory::SansSerif // Default
        }
    }
    
    fn detect_font_category(&self, font: &FontDescriptor) -> FontCategory {
        let name_lower = font.family.to_lowercase();
        
        if font.monospaced {
            FontCategory::Monospace
        } else if name_lower.contains("serif") {
            FontCategory::Serif
        } else if name_lower.contains("sans") {
            FontCategory::SansSerif
        } else if name_lower.contains("script") || name_lower.contains("hand") {
            FontCategory::Handwriting
        } else if name_lower.contains("display") || name_lower.contains("decorative") {
            FontCategory::Display
        } else {
            FontCategory::SansSerif
        }
    }
    
    fn get_precomputed_similarity(&self, name1: &str, name2: &str) -> Option<f32> {
        self.precomputed_similarities
            .get(name1)
            .and_then(|similarities| {
                similarities.iter()
                    .find(|(name, _)| name == name2)
                    .map(|(_, score)| *score)
            })
    }
    
    fn generate_suggestion_reason(&self, font_match: &FontMatch) -> String {
        let details = &font_match.similarity_details;
        
        match font_match.tier {
            MatchTier::Exact(_) => {
                format!("Exact match ({}% similarity)", (font_match.score.overall * 100.0) as u8)
            }
            MatchTier::Similar(_) => {
                format!("Similar font ({}% match)", (font_match.score.overall * 100.0) as u8)
            }
            MatchTier::Low(_) => {
                let mut reasons = Vec::new();
                
                if details.name_similarity < 0.5 {
                    reasons.push("different name".to_string());
                }
                if details.weight_similarity < 0.6 {
                    reasons.push("different weight".to_string());
                }
                if details.style_similarity < 0.6 {
                    reasons.push("different style".to_string());
                }
                
                if reasons.is_empty() {
                    "Best available alternative".to_string()
                } else {
                    format!("Similar but with {}", reasons.join(", "))
                }
            }
        }
    }
    
    fn get_best_substitution(&self, request: &FontRequest) -> FontSuggestion {
        // Common font substitutions
        let substitutions = vec![
            ("helvetica", "arial"),
            ("helvetica neue", "arial"),
            ("times", "times new roman"),
            ("courier", "courier new"),
            ("garamond", "adobe garamond pro"),
            ("futura", "century gothic"),
            ("gill sans", "myriad pro"),
        ];
        
        let request_lower = request.family.to_lowercase();
        
        for (from, to) in substitutions {
            if request_lower.contains(from) {
                return FontSuggestion {
                    font: FontDescriptor {
                        family: to.to_string(),
                        subfamily: Some("Regular".to_string()),
                        postscript_name: to.replace(' ', "").to_lowercase(),
                        full_name: Some(format!("{} Regular", to)),
                        path: std::path::PathBuf::from(format!("/system/fonts/{}.ttf", to.replace(' ', ""))),
                        format: font_core::FontFormat::Ttf,
                        weight: request.weight,
                        italic: request.italic,
                        monospaced: request.monospaced,
                        variable: false,
                        metrics: None,
                        license: None,
                    },
                    similarity_score: 0.7,
                    reason: format!("Common substitution for {}", from),
                    source: SuggestionSource::Substitution,
                };
            }
        }
        
        // Fallback to system default
        FontSuggestion {
            font: FontDescriptor {
                family: "Arial".to_string(),
                subfamily: Some("Regular".to_string()),
                postscript_name: "arial".to_string(),
                full_name: Some("Arial Regular".to_string()),
                path: std::path::PathBuf::from("/system/fonts/arial.ttf"),
                format: font_core::FontFormat::Ttf,
                weight: 400,
                italic: false,
                monospaced: false,
                variable: false,
                metrics: None,
                license: None,
            },
            similarity_score: 0.5,
            reason: "System default font".to_string(),
            source: SuggestionSource::Substitution,
        }
    }
}