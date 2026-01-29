@echo off
echo Fixing file locations...

echo 1. Creating correct font-core/src/lib.rs...
(
echo #![allow(dead_code)] // Remove in production
echo.
echo use std::path::PathBuf;
echo use serde::^\{Serialize, Deserialize^};
echo.
echo /// Represents a font file with all metadata
echo #[derive(Debug, Clone, Serialize, Deserialize)]
echo pub struct FontDescriptor ^{
echo     /// Font family name (e.g., "Arial", "Times New Roman")
echo     pub family: String,
echo     
echo     /// Font subfamily/style (e.g., "Regular", "Bold", "Italic")
echo     pub subfamily: Option^<String^>,
echo     
echo     /// PostScript name (e.g., "ArialMT", "TimesNewRomanPSMT")
echo     pub postscript_name: String,
echo     
echo     /// Full display name
echo     pub full_name: Option^<String^>,
echo     
echo     /// Font file path
echo     pub path: PathBuf,
echo     
echo     /// Font format
echo     pub format: FontFormat,
echo     
echo     /// Font weight (100-900)
echo     pub weight: u16,
echo     
echo     /// Is italic^?
echo     pub italic: bool,
echo     
echo     /// Is monospaced^?
echo     pub monospaced: bool,
echo     
echo     /// Is variable font^?
echo     pub variable: bool,
echo     
echo     /// Font metrics (optional, computed on demand)
echo     pub metrics: Option^<FontMetrics^>,
echo     
echo     /// License information
echo     pub license: Option^<LicenseInfo^>,
echo ^}
echo.
echo /// Font file format
echo #[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
echo pub enum FontFormat ^{
echo     Ttf,
echo     Otf,
echo     Woff,
echo     Woff2,
echo     Other,
echo ^}
echo.
echo /// Font metrics for layout preservation
echo #[derive(Debug, Clone, Serialize, Deserialize)]
echo pub struct FontMetrics ^{
echo     /// Units per em (design grid size)
echo     pub units_per_em: u16,
echo     
echo     /// Ascender (distance above baseline)
echo     pub ascender: i16,
echo     
echo     /// Descender (distance below baseline)
echo     pub descender: i16,
echo     
echo     /// x-height (height of lowercase 'x')
echo     pub x_height: i16,
echo     
echo     /// Cap height (height of uppercase letters)
echo     pub cap_height: i16,
echo     
echo     /// Average character width
echo     pub average_width: i16,
echo     
echo     /// Maximum advance width
echo     pub max_advance_width: u16,
echo ^}
echo.
echo /// License information
echo #[derive(Debug, Clone, Serialize, Deserialize)]
echo pub struct LicenseInfo ^{
echo     pub name: String,
echo     pub url: Option^<String^>,
echo     pub allows_embedding: bool,
echo     pub allows_modification: bool,
echo     pub requires_attribution: bool,
echo ^}
echo.
echo /// Normalized font request
echo #[derive(Debug, Clone)]
echo pub struct FontRequest ^{
echo     /// Original font name from PDF
echo     pub original_name: String,
echo     
echo     /// Normalized name
echo     pub normalized_name: String,
echo     
echo     /// Extracted family
echo     pub family: String,
echo     
echo     /// Requested weight (100-900)
echo     pub weight: u16,
echo     
echo     /// Requested style
echo     pub style: FontStyle,
echo     
echo     /// Is italic requested^?
echo     pub italic: bool,
echo ^}
echo.
echo /// Font style
echo #[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
echo pub enum FontStyle ^{
echo     Normal,
echo     Italic,
echo     Oblique,
echo ^}
echo.
echo /// Font resolution result
echo #[derive(Debug, Clone, Serialize, Deserialize)]
echo pub struct ResolutionResult ^{
echo     /// Original requested font name
echo     pub original_name: String,
echo     
echo     /// Resolved font descriptor
echo     pub font: FontDescriptor,
echo     
echo     /// How the font was resolved
echo     pub source: FontSource,
echo     
echo     /// Was this a substitution^?
echo     pub substituted: bool,
echo     
echo     /// Substitution reason if applicable
echo     pub substitution_reason: Option^<SubstitutionReason^>,
echo     
echo     /// Compatibility score (0.0 to 1.0)
echo     pub compatibility_score: f32,
echo     
echo     /// Any warnings
echo     pub warnings: Vec^<String^>,
echo ^}
echo.
echo /// Source of the font
echo #[derive(Debug, Clone, Serialize, Deserialize)]
echo pub enum FontSource ^{
echo     System,
echo     User,
echo     OpenRepository,
echo     Embedded,
echo     Substituted,
echo ^}
echo.
echo /// Reason for substitution
echo #[derive(Debug, Clone, Serialize, Deserialize)]
echo pub enum SubstitutionReason ^{
echo     FontNotFound,
echo     LicenseRestriction,
echo     MetricsMismatch,
echo     UserPreference,
echo ^}
echo.
echo /// Configuration for font resolution
echo #[derive(Debug, Clone, Serialize, Deserialize)]
echo pub struct ResolverConfig ^{
echo     /// Search system fonts
echo     pub search_system: bool,
echo     
echo     /// Search user directories
echo     pub search_user: bool,
echo     
echo     /// Use open font repositories
echo     pub use_open_fonts: bool,
echo     
echo     /// Allow substitution
echo     pub allow_substitution: bool,
echo     
echo     /// Require metrics compatibility
echo     pub require_metrics: bool,
echo     
echo     /// Maximum metrics deviation (0.0 to 1.0)
echo     pub max_metrics_deviation: f32,
echo     
echo     /// User font directories
echo     pub user_font_dirs: Vec^<PathBuf^>,
echo     
echo     /// Preferred fallback families
echo     pub preferred_families: Vec^<String^>,
echo     
echo     /// Whether to cache results
echo     pub cache_results: bool,
echo ^}
echo.
echo impl Default for ResolverConfig ^{
echo     fn default^(^) -> Self ^{
echo         Self ^{
echo             search_system: true,
echo             search_user: true,
echo             use_open_fonts: true,
echo             allow_substitution: true,
echo             require_metrics: false,
echo             max_metrics_deviation: 0.2,
echo             user_font_dirs: Vec::new^(^),
echo             preferred_families: vec![
echo                 "Noto Sans".to_string^(^),
echo                 "Noto Serif".to_string^(^),
echo                 "Liberation Sans".to_string^(^),
echo                 "DejaVu Sans".to_string^(^),
echo             ],
echo             cache_results: true,
echo         ^}
echo     ^}
echo ^}
echo.
echo /// Error types for the library
echo #[derive(Debug, thiserror::Error)]
echo pub enum FontError ^{
echo     #[error("IO error: {0}")]
echo     Io(#[from] std::io::Error),
echo     
echo     #[error("Font parsing error: {0}")]
echo     Parse(String),
echo     
echo     #[error("Font not found: {0}")]
echo     NotFound(String),
echo     
echo     #[error("Unsupported font format")]
echo     UnsupportedFormat,
echo     
echo     #[error("License restriction: {0}")]
echo     LicenseRestriction(String),
echo     
echo     #[error("Invalid font name: {0}")]
echo     InvalidFontName(String),
echo     
echo     #[error("Platform not supported: {0}")]
echo     PlatformNotSupported(String),
echo ^}
echo.
echo /// Result type for font operations
echo pub type FontResult^<T^> = Result^<T, FontError^>;
) > crates\font-core\src\lib.rs

echo 2. Creating correct font-resolver/src/lib.rs...
(
echo use font_core::^\{ResolutionResult, ResolverConfig, FontError^};
echo use font_normalizer::FontNormalizer;
echo.
echo pub struct FontResolver ^{
echo     normalizer: FontNormalizer,
echo     config: ResolverConfig,
echo ^}
echo.
echo impl FontResolver ^{
echo     pub fn new(config: ResolverConfig) -> Self ^{
echo         Self ^{
echo             normalizer: FontNormalizer,
echo             config,
echo         ^}
echo     ^}
echo.
echo     pub fn resolve(&self, font_name: &str) -> Result^<ResolutionResult, FontError^> ^{
echo         let request = self.normalizer.normalize(font_name)^?;
echo         
echo         // TODO: Implement actual resolution logic
echo         // For now, return a dummy result
echo         Ok(ResolutionResult ^{
echo             original_name: font_name.to_string^(^),
echo             font: font_core::FontDescriptor ^{
echo                 family: request.family.clone^(^),
echo                 subfamily: None,
echo                 postscript_name: "".to_string^(^),
echo                 full_name: None,
echo                 path: std::path::PathBuf::new^(^),
echo                 format: font_core::FontFormat::Other,
echo                 weight: request.weight,
echo                 italic: request.italic,
echo                 monospaced: false,
echo                 variable: false,
echo                 metrics: None,
echo                 license: None,
echo             ^},
echo             source: font_core::FontSource::System,
echo             substituted: false,
echo             substitution_reason: None,
echo             compatibility_score: 1.0,
echo             warnings: Vec::new^(^),
echo         ^})
echo     ^}
echo.
echo     pub fn resolve_batch(&self, font_names: &[&str]) -> Result^<Vec^<ResolutionResult^>, FontError^> ^{
echo         let mut results = Vec::new^(^);
echo         
echo         for font_name in font_names ^{
echo             match self.resolve(font_name) ^{
echo                 Ok(result) => results.push(result),
echo                 Err(e) => ^{
echo                     // Return error for batch^?
echo                     // Or collect errors^? Let's skip failed ones for now
echo                     eprintln^("Failed to resolve ^{}: ^{}", font_name, e^);
echo                 ^}
echo             ^}
echo         ^}
echo         
echo         Ok(results)
echo     ^}
echo ^}
) > crates\font-resolver\src\lib.rs

echo Done! Now run: cargo build