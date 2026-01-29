#![allow(dead_code)] // Remove in production

use std::path::PathBuf;
use std::fmt;
use serde::{Serialize, Deserialize};

/// Represents a font file with all metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FontDescriptor {
    /// Font family name (e.g., "Arial", "Times New Roman")
    pub family: String,
    
    /// Font subfamily/style (e.g., "Regular", "Bold", "Italic")
    pub subfamily: Option<String>,
    
    /// PostScript name (e.g., "ArialMT", "TimesNewRomanPSMT")
    pub postscript_name: String,
    
    /// Full display name
    pub full_name: Option<String>,
    
    /// Font file path
    pub path: PathBuf,
    
    /// Font format
    pub format: FontFormat,
    
    /// Font weight (100-900)
    pub weight: u16,
    
    /// Is italic?
    pub italic: bool,
    
    /// Is monospaced?
    pub monospaced: bool,
    
    /// Is variable font?
    pub variable: bool,
    
    /// Font metrics (optional, computed on demand)
    pub metrics: Option<FontMetrics>,
    
    /// License information
    pub license: Option<LicenseInfo>,
}

/// Font file format
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum FontFormat {
    Ttf,
    Otf,
    Woff,
    Woff2,
    Other,
}

// Implement Display for FontFormat
impl fmt::Display for FontFormat {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            FontFormat::Ttf => write!(f, "TTF"),
            FontFormat::Otf => write!(f, "OTF"),
            FontFormat::Woff => write!(f, "WOFF"),
            FontFormat::Woff2 => write!(f, "WOFF2"),
            FontFormat::Other => write!(f, "Other"),
        }
    }
}

/// Font metrics for layout preservation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FontMetrics {
    /// Units per em (design grid size)
    pub units_per_em: u16,
    
    /// Ascender (distance above baseline)
    pub ascender: i16,
    
    /// Descender (distance below baseline)
    pub descender: i16,
    
    /// x-height (height of lowercase 'x')
    pub x_height: i16,
    
    /// Cap height (height of uppercase letters)
    pub cap_height: i16,
    
    /// Average character width
    pub average_width: i16,
    
    /// Maximum advance width
    pub max_advance_width: u16,
}

/// License information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LicenseInfo {
    pub name: String,
    pub url: Option<String>,
    pub allows_embedding: bool,
    pub allows_modification: bool,
    pub requires_attribution: bool,
    pub allows_commercial_use: bool, // ADDED THIS FIELD
}

impl LicenseInfo {
    /// Check if license is safe for commercial use
    pub fn is_commercial_use_safe(&self) -> bool {
        self.allows_commercial_use && !self.requires_attribution
    }
    
    /// Check if license is safe for embedding
    pub fn is_embedding_safe(&self) -> bool {
        self.allows_embedding
    }
    
    /// Check if license is open source
    pub fn is_open_source(&self) -> bool {
        self.allows_modification && !self.requires_attribution
    }
}

/// Normalized font request
#[derive(Debug, Clone)]
pub struct FontRequest {
    /// Original font name from PDF
    pub original_name: String,
    
    /// Normalized name
    pub normalized_name: String,
    
    /// Extracted family
    pub family: String,
    
    /// Requested weight (100-900)
    pub weight: u16,
    
    /// Requested style
    pub style: FontStyle,
    
    /// Is italic requested?
    pub italic: bool,
    
    /// Is monospaced requested? (default: false)
    pub monospaced: bool,
}

/// Font style
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum FontStyle {
    Normal,
    Italic,
    Oblique,
}

impl fmt::Display for FontStyle {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            FontStyle::Normal => write!(f, "Normal"),
            FontStyle::Italic => write!(f, "Italic"),
            FontStyle::Oblique => write!(f, "Oblique"),
        }
    }
}

/// Font resolution result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResolutionResult {
    /// Original requested font name
    pub original_name: String,
    
    /// Resolved font descriptor
    pub font: FontDescriptor,
    
    /// How the font was resolved
    pub source: FontSource,
    
    /// Was this a substitution?
    pub substituted: bool,
    
    /// Substitution reason if applicable
    pub substitution_reason: Option<SubstitutionReason>,
    
    /// Compatibility score (0.0 to 1.0)
    pub compatibility_score: f32,
    
    /// Any warnings
    pub warnings: Vec<String>,
}

/// Source of the font
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum FontSource {
    System,
    User,
    OpenRepository,
    Embedded,
    Substituted,
}

impl fmt::Display for FontSource {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            FontSource::System => write!(f, "System"),
            FontSource::User => write!(f, "User"),
            FontSource::OpenRepository => write!(f, "Open Repository"),
            FontSource::Embedded => write!(f, "Embedded"),
            FontSource::Substituted => write!(f, "Substituted"),
        }
    }
}

/// Reason for substitution
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SubstitutionReason {
    FontNotFound,
    LicenseRestriction,
    MetricsMismatch,
    UserPreference,
}

impl fmt::Display for SubstitutionReason {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SubstitutionReason::FontNotFound => write!(f, "Font not found"),
            SubstitutionReason::LicenseRestriction => write!(f, "License restriction"),
            SubstitutionReason::MetricsMismatch => write!(f, "Metrics mismatch"),
            SubstitutionReason::UserPreference => write!(f, "User preference"),
        }
    }
}

/// Helper for font matching calculations
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FontMatchScore {
    /// Overall compatibility (0.0 to 1.0)
    pub overall: f32,
    /// Family match score
    pub family: f32,
    /// Weight match score (lower difference is better)
    pub weight: f32,
    /// Style match score
    pub style: f32,
    /// Monospace match score
    pub monospaced: f32,
    /// Metrics match score (if available)
    pub metrics: f32,
}

/// Configuration for font resolution
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResolverConfig {
    /// Search system fonts
    pub search_system: bool,
    
    /// Search user directories
    pub search_user: bool,
    
    /// Use open font repositories
    pub use_open_fonts: bool,
    
    /// Allow substitution
    pub allow_substitution: bool,
    
    /// Require metrics compatibility
    pub require_metrics: bool,
    
    /// Maximum metrics deviation (0.0 to 1.0)
    pub max_metrics_deviation: f32,
    
    /// User font directories
    pub user_font_dirs: Vec<PathBuf>,
    
    /// Preferred fallback families
    pub preferred_families: Vec<String>,
    
    /// Whether to cache results
    pub cache_results: bool,
}

impl Default for ResolverConfig {
    fn default() -> Self {
        Self {
            search_system: true,
            search_user: true,
            use_open_fonts: true,
            allow_substitution: true,
            require_metrics: false,
            max_metrics_deviation: 0.2,
            user_font_dirs: Vec::new(),
            preferred_families: vec![
                "Noto Sans".to_string(),
                "Noto Serif".to_string(),
                "Liberation Sans".to_string(),
                "DejaVu Sans".to_string(),
            ],
            cache_results: true,
        }
    }
}

// ============================================================
// NEW ENHANCED CONFIGURATION STRUCTURES
// ============================================================

/// License warning level
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum LicenseWarningLevel {
    Off,
    // SystemOnly,
    All,
    Minimal,       // Only critical warnings
    Normal,        // Warnings for non-system commercial fonts
    Verbose,       // All warnings including system fonts
}

impl fmt::Display for LicenseWarningLevel {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            LicenseWarningLevel::Off => write!(f, "Off"),
            LicenseWarningLevel::All => write!(f, "All"),
            LicenseWarningLevel::Minimal => write!(f, "Minimal"),
            LicenseWarningLevel::Normal => write!(f, "Normal"),
            LicenseWarningLevel::Verbose => write!(f, "Verbose"),
        }
    }
}

/// Font source priority order
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum FontSourcePriority {
    SystemOnly,
    SystemThenWeb,
    SystemThenCustom,
    SystemThenWebThenCustom,
    CustomThenSystemThenWeb,
    AllCustomFirst,
    AllWebFirst,
    PriorityList(Vec<String>), // Custom order
}

impl fmt::Display for FontSourcePriority {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            FontSourcePriority::SystemOnly => write!(f, "System Only"),
            FontSourcePriority::SystemThenWeb => write!(f, "System Then Web"),
            FontSourcePriority::SystemThenCustom => write!(f, "System Then Custom"),
            FontSourcePriority::SystemThenWebThenCustom => write!(f, "System Then Web Then Custom"),
            FontSourcePriority::CustomThenSystemThenWeb => write!(f, "Custom Then System Then Web"),
            FontSourcePriority::AllCustomFirst => write!(f, "All Custom First"),
            FontSourcePriority::AllWebFirst => write!(f, "All Web First"),
            FontSourcePriority::PriorityList(list) => write!(f, "Custom Priority: {:?}", list),
        }
    }
}

/// Cache cleanup mode
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum CacheCleanupMode {
    Manual,
    SizeBased,
    TimeBased,
    Smart,
}

impl fmt::Display for CacheCleanupMode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CacheCleanupMode::Manual => write!(f, "Manual"),
            CacheCleanupMode::SizeBased => write!(f, "Size Based"),
            CacheCleanupMode::TimeBased => write!(f, "Time Based"),
            CacheCleanupMode::Smart => write!(f, "Smart"),
        }
    }
}

/// Cache statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CacheStats {
    pub memory_entries: usize,
    pub disk_entries: usize,
    pub pinned_fonts: usize,
    pub memory_usage_mb: f64,
    pub disk_usage_mb: f64,
}

/// Extended configuration for font resolution
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnhancedResolverConfig {
    /// Base configuration (existing)
    pub base: ResolverConfig,
    
    // Cache settings
    pub cache_enabled: bool,
    pub memory_limit_mb: usize,
    pub disk_limit_mb: usize,
    pub auto_pin_threshold: u32,
    pub cache_cleanup_mode: CacheCleanupMode,
    
    // Font sources
    pub system_fonts_enabled: bool,
    pub web_fonts_enabled: bool,
    pub custom_fonts_enabled: bool,
    pub font_source_priority: FontSourcePriority,
    
    // License settings
    pub license_warnings: LicenseWarningLevel,
    pub watermark_commercial: bool,
    
    // Performance
    pub auto_setup_completed: bool,
    pub telemetry_enabled: bool,
    
    // Learning & Custom Assets
    pub dynamic_learning_enabled: bool,
    pub project_asset_dirs: Vec<PathBuf>,
}

impl Default for EnhancedResolverConfig {
    fn default() -> Self {
        Self {
            base: ResolverConfig::default(),
            cache_enabled: true,
            memory_limit_mb: 2,
            disk_limit_mb: 10,
            auto_pin_threshold: 5,
            cache_cleanup_mode: CacheCleanupMode::Manual,
            system_fonts_enabled: true,
            web_fonts_enabled: false, // Disabled by default
            custom_fonts_enabled: false,
            font_source_priority: FontSourcePriority::SystemOnly,
            license_warnings: LicenseWarningLevel::Off,
            watermark_commercial: false,
            auto_setup_completed: false,
            telemetry_enabled: false,
            dynamic_learning_enabled: true,
            project_asset_dirs: Vec::new(),
        }
    }
}

/// Setup configuration from interactive prompts
#[derive(Debug, Clone)]
pub struct SetupConfig {
    pub memory_limit_mb: usize,
    pub enable_web_fonts: bool,
    pub enable_license_warnings: bool,
    pub auto_pin_fonts: bool,
}

impl Default for SetupConfig {
    fn default() -> Self {
        Self {
            memory_limit_mb: 2,
            enable_web_fonts: false,
            enable_license_warnings: true,
            auto_pin_fonts: true,
        }
    }
}

/// Error types for the library
#[derive(Debug, thiserror::Error)]
pub enum FontError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    
    #[error("Font parsing error: {0}")]
    Parse(String),
    
    #[error("Font not found: {0}")]
    NotFound(String),
    
    #[error("Unsupported font format")]
    UnsupportedFormat,
    
    #[error("License restriction: {0}")]
    LicenseRestriction(String),
    
    #[error("Invalid font name: {0}")]
    InvalidFontName(String),
    
    #[error("Platform not supported: {0}")]
    PlatformNotSupported(String),
    
    #[error("Cache error: {0}")]
    CacheError(String),
    
    #[error("Memory limit exceeded: {0}MB used, {1}MB limit")]
    MemoryLimitExceeded(f64, usize),
    
    #[error("Disk limit exceeded: {0}MB used, {1}MB limit")]
    DiskLimitExceeded(f64, usize),
}

/// Result type for font operations
pub type FontResult<T> = Result<T, FontError>;