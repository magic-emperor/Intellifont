//! # Font Glyph
//! 
//! Ultra-compact glyph outline extraction and micro-signature generation
//! for visual font identification.
//! 
//! This crate provides:
//! - `GlyphOutline` - Vector path representation extracted from font files
//! - `MicroSignature` - 16-byte compact fingerprint for fast similarity matching
//! - `GlyphExtractor` - Extracts glyph outlines using ttf_parser

use std::path::Path;
use serde::{Serialize, Deserialize};

// =============================================================================
// PATH SEGMENT TYPES
// =============================================================================

/// Represents a single path segment in a glyph outline
#[derive(Debug, Clone, PartialEq)]
pub enum PathSegment {
    /// Move to position (x, y)
    MoveTo(f32, f32),
    /// Line to position (x, y)
    LineTo(f32, f32),
    /// Quadratic bezier curve to (x, y) with control point (cx, cy)
    QuadTo { cx: f32, cy: f32, x: f32, y: f32 },
    /// Cubic bezier curve to (x, y) with control points (cx1, cy1) and (cx2, cy2)
    CurveTo { cx1: f32, cy1: f32, cx2: f32, cy2: f32, x: f32, y: f32 },
    /// Close the current path
    Close,
}

// =============================================================================
// BOUNDING BOX
// =============================================================================

/// Bounding box for a glyph
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct BoundingBox {
    pub x_min: f32,
    pub y_min: f32,
    pub x_max: f32,
    pub y_max: f32,
}

impl BoundingBox {
    /// Create a new bounding box
    pub fn new(x_min: f32, y_min: f32, x_max: f32, y_max: f32) -> Self {
        Self { x_min, y_min, x_max, y_max }
    }
    
    /// Width of the bounding box
    pub fn width(&self) -> f32 {
        self.x_max - self.x_min
    }
    
    /// Height of the bounding box
    pub fn height(&self) -> f32 {
        self.y_max - self.y_min
    }
    
    /// Aspect ratio (width / height)
    pub fn aspect_ratio(&self) -> f32 {
        if self.height() > 0.0 {
            self.width() / self.height()
        } else {
            1.0
        }
    }
    
    /// Center X coordinate
    pub fn center_x(&self) -> f32 {
        (self.x_min + self.x_max) / 2.0
    }
    
    /// Center Y coordinate
    pub fn center_y(&self) -> f32 {
        (self.y_min + self.y_max) / 2.0
    }
}

// =============================================================================
// GLYPH OUTLINE
// =============================================================================

/// Complete outline of a glyph
#[derive(Debug, Clone)]
pub struct GlyphOutline {
    /// The character this glyph represents
    pub character: char,
    /// Bounding box of the glyph
    pub bounds: BoundingBox,
    /// Path segments making up the outline
    pub segments: Vec<PathSegment>,
    /// Advance width (horizontal spacing)
    pub advance_width: f32,
    /// Units per em from the font
    pub units_per_em: u16,
}

impl GlyphOutline {
    /// Create a new empty glyph outline
    pub fn new(character: char, units_per_em: u16) -> Self {
        Self {
            character,
            bounds: BoundingBox::default(),
            segments: Vec::new(),
            advance_width: 0.0,
            units_per_em,
        }
    }
    
    /// Calculate statistics about the outline segments
    pub fn segment_stats(&self) -> SegmentStats {
        let mut lines = 0;
        let mut quads = 0;
        let mut cubics = 0;
        let mut control_points = 0;
        
        for seg in &self.segments {
            match seg {
                PathSegment::LineTo(_, _) => {
                    lines += 1;
                }
                PathSegment::QuadTo { .. } => {
                    quads += 1;
                    control_points += 1;
                }
                PathSegment::CurveTo { .. } => {
                    cubics += 1;
                    control_points += 2;
                }
                _ => {}
            }
        }
        
        SegmentStats {
            line_count: lines,
            quad_count: quads,
            cubic_count: cubics,
            total_control_points: control_points,
        }
    }
    
    /// Calculate the center of mass of all points
    pub fn center_of_mass(&self) -> (f32, f32) {
        let mut sum_x = 0.0;
        let mut sum_y = 0.0;
        let mut count = 0;
        
        for seg in &self.segments {
            match seg {
                PathSegment::MoveTo(x, y) | PathSegment::LineTo(x, y) => {
                    sum_x += x;
                    sum_y += y;
                    count += 1;
                }
                PathSegment::QuadTo { cx, cy, x, y } => {
                    sum_x += cx + x;
                    sum_y += cy + y;
                    count += 2;
                }
                PathSegment::CurveTo { cx1, cy1, cx2, cy2, x, y } => {
                    sum_x += cx1 + cx2 + x;
                    sum_y += cy1 + cy2 + y;
                    count += 3;
                }
                PathSegment::Close => {}
            }
        }
        
        if count > 0 {
            (sum_x / count as f32, sum_y / count as f32)
        } else {
            (0.0, 0.0)
        }
    }
    
    /// Calculate density in each quadrant (NW, NE, SW, SE)
    /// Returns normalized values 0.0 - 1.0
    pub fn quadrant_densities(&self) -> [f32; 4] {
        let center_x = self.bounds.center_x();
        let center_y = self.bounds.center_y();
        
        let mut counts = [0u32; 4]; // NW, NE, SW, SE
        let mut total = 0u32;
        
        for seg in &self.segments {
            let points: Vec<(f32, f32)> = match seg {
                PathSegment::MoveTo(x, y) | PathSegment::LineTo(x, y) => {
                    vec![(*x, *y)]
                }
                PathSegment::QuadTo { cx, cy, x, y } => {
                    vec![(*cx, *cy), (*x, *y)]
                }
                PathSegment::CurveTo { cx1, cy1, cx2, cy2, x, y } => {
                    vec![(*cx1, *cy1), (*cx2, *cy2), (*x, *y)]
                }
                PathSegment::Close => vec![],
            };
            
            for (x, y) in points {
                let quadrant = if x < center_x {
                    if y > center_y { 0 } else { 2 } // NW or SW
                } else {
                    if y > center_y { 1 } else { 3 } // NE or SE
                };
                counts[quadrant] += 1;
                total += 1;
            }
        }
        
        if total > 0 {
            [
                counts[0] as f32 / total as f32,
                counts[1] as f32 / total as f32,
                counts[2] as f32 / total as f32,
                counts[3] as f32 / total as f32,
            ]
        } else {
            [0.25, 0.25, 0.25, 0.25]
        }
    }
}

/// Statistics about segment types in an outline
#[derive(Debug, Clone, Copy)]
pub struct SegmentStats {
    pub line_count: usize,
    pub quad_count: usize,
    pub cubic_count: usize,
    pub total_control_points: usize,
}

impl SegmentStats {
    /// Ratio of curves to total segments (0.0 = all lines, 1.0 = all curves)
    pub fn curve_ratio(&self) -> f32 {
        let total = self.line_count + self.quad_count + self.cubic_count;
        if total > 0 {
            (self.quad_count + self.cubic_count) as f32 / total as f32
        } else {
            0.0
        }
    }
}

// =============================================================================
// MICRO SIGNATURE (16 bytes)
// =============================================================================

/// Ultra-compact 16-byte glyph signature for fast similarity matching
/// 
/// This signature captures the essential visual characteristics of a glyph
/// in a format optimized for both compression and SIMD comparison.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct MicroSignature {
    /// Width/Height ratio × 64 (allows ratios from 0 to ~4)
    pub aspect_ratio: u8,
    /// Estimated ink density (0-255)
    pub density: u8,
    /// Density in top-left quadrant (0-255)
    pub quadrant_nw: u8,
    /// Density in top-right quadrant (0-255)
    pub quadrant_ne: u8,
    /// Density in bottom-left quadrant (0-255)
    pub quadrant_sw: u8,
    /// Density in bottom-right quadrant (0-255)
    pub quadrant_se: u8,
    /// Curves vs Lines ratio × 255
    pub curve_ratio: u8,
    /// Control point count / 256 (capped at 255)
    pub point_count: u8,
    /// Horizontal center of mass (0=left, 128=center, 255=right)
    pub x_balance: u8,
    /// Vertical center of mass (0=bottom, 128=center, 255=top)
    pub y_balance: u8,
    /// Estimated stroke width category (0-255)
    pub stroke_width: u8,
    /// Serif detection score (0=sans, 255=serif)
    pub serif_score: u8,
    /// 16-bit hash of detailed features (bytes 12-13)
    pub feature_hash: u16,
    /// Reserved for future use (bytes 14-15)
    pub reserved: u16,
}

impl MicroSignature {
    /// Create a new empty signature
    pub fn new() -> Self {
        Self {
            aspect_ratio: 0,
            density: 0,
            quadrant_nw: 0,
            quadrant_ne: 0,
            quadrant_sw: 0,
            quadrant_se: 0,
            curve_ratio: 0,
            point_count: 0,
            x_balance: 128,
            y_balance: 128,
            stroke_width: 0,
            serif_score: 0,
            feature_hash: 0,
            reserved: 0,
        }
    }
    
    /// Generate signature from a glyph outline
    pub fn from_outline(outline: &GlyphOutline) -> Self {
        let stats = outline.segment_stats();
        let quadrants = outline.quadrant_densities();
        let (com_x, com_y) = outline.center_of_mass();
        
        // Calculate aspect ratio (capped at ~4:1)
        let aspect = outline.bounds.aspect_ratio().min(4.0);
        let aspect_ratio = (aspect * 64.0).min(255.0) as u8;
        
        // Estimate density based on control point count relative to bounding box
        let area = outline.bounds.width() * outline.bounds.height();
        let density = if area > 0.0 {
            let normalized = (stats.total_control_points as f32 * 1000.0 / area).min(1.0);
            (normalized * 255.0) as u8
        } else {
            0
        };
        
        // Quadrant densities
        let quadrant_nw = (quadrants[0] * 255.0) as u8;
        let quadrant_ne = (quadrants[1] * 255.0) as u8;
        let quadrant_sw = (quadrants[2] * 255.0) as u8;
        let quadrant_se = (quadrants[3] * 255.0) as u8;
        
        // Curve ratio
        let curve_ratio = (stats.curve_ratio() * 255.0) as u8;
        
        // Point count (capped at 255)
        let point_count = stats.total_control_points.min(255) as u8;
        
        // Center of mass normalized to bounding box
        let x_balance = if outline.bounds.width() > 0.0 {
            let normalized = (com_x - outline.bounds.x_min) / outline.bounds.width();
            (normalized * 255.0).clamp(0.0, 255.0) as u8
        } else {
            128
        };
        
        let y_balance = if outline.bounds.height() > 0.0 {
            let normalized = (com_y - outline.bounds.y_min) / outline.bounds.height();
            (normalized * 255.0).clamp(0.0, 255.0) as u8
        } else {
            128
        };
        
        // Estimate stroke width based on line segment frequency
        let stroke_width = Self::estimate_stroke_width(outline);
        
        // Serif score based on segment patterns
        let serif_score = Self::estimate_serif_score(outline);
        
        // Feature hash for additional distinctiveness
        let feature_hash = Self::compute_feature_hash(outline);
        
        Self {
            aspect_ratio,
            density,
            quadrant_nw,
            quadrant_ne,
            quadrant_sw,
            quadrant_se,
            curve_ratio,
            point_count,
            x_balance,
            y_balance,
            stroke_width,
            serif_score,
            feature_hash,
            reserved: 0,
        }
    }
    
    /// Estimate stroke width category
    fn estimate_stroke_width(outline: &GlyphOutline) -> u8 {
        // Use ratio of bounding box area to control point count as proxy
        let area = outline.bounds.width() * outline.bounds.height();
        let stats = outline.segment_stats();
        let total_segments = stats.line_count + stats.quad_count + stats.cubic_count;
        
        if total_segments > 0 && area > 0.0 {
            let ratio = area / total_segments as f32;
            // Normalize to 0-255 range (higher = thicker strokes)
            (ratio / 100.0).min(1.0).max(0.0) as u8 * 255
        } else {
            128
        }
    }
    
    /// Estimate serif score based on segment patterns
    fn estimate_serif_score(outline: &GlyphOutline) -> u8 {
        let stats = outline.segment_stats();
        
        // Serifs typically have more short line segments and specific curve patterns
        // High line-to-curve ratio often indicates sans-serif
        // Complex curves with many control points often indicate serif
        
        let line_ratio = if stats.line_count + stats.quad_count + stats.cubic_count > 0 {
            stats.line_count as f32 / (stats.line_count + stats.quad_count + stats.cubic_count) as f32
        } else {
            0.5
        };
        
        // More curves = more likely serif
        let serif_likelihood = 1.0 - line_ratio;
        
        // Also factor in control point density
        let point_density = (stats.total_control_points as f32 / 50.0).min(1.0);
        
        let combined = (serif_likelihood * 0.6 + point_density * 0.4) * 255.0;
        combined.min(255.0) as u8
    }
    
    /// Compute a 16-bit feature hash for additional distinctiveness
    fn compute_feature_hash(outline: &GlyphOutline) -> u16 {
        let mut hash: u16 = 0;
        
        // Simple hash combining key features
        for (i, seg) in outline.segments.iter().enumerate() {
            if i >= 16 { break; } // Only use first 16 segments
            
            let seg_value: u8 = match seg {
                PathSegment::MoveTo(_, _) => 1,
                PathSegment::LineTo(_, _) => 2,
                PathSegment::QuadTo { .. } => 3,
                PathSegment::CurveTo { .. } => 4,
                PathSegment::Close => 5,
            };
            
            hash = hash.wrapping_add((seg_value as u16) << (i % 8));
            hash = hash.rotate_left(3);
        }
        
        // Mix in bounding box info
        hash ^= (outline.bounds.width() as u16).wrapping_mul(31);
        hash ^= (outline.bounds.height() as u16).wrapping_mul(17);
        
        hash
    }
    
    /// Convert to raw bytes
    pub fn to_bytes(&self) -> [u8; 16] {
        let mut bytes = [0u8; 16];
        bytes[0] = self.aspect_ratio;
        bytes[1] = self.density;
        bytes[2] = self.quadrant_nw;
        bytes[3] = self.quadrant_ne;
        bytes[4] = self.quadrant_sw;
        bytes[5] = self.quadrant_se;
        bytes[6] = self.curve_ratio;
        bytes[7] = self.point_count;
        bytes[8] = self.x_balance;
        bytes[9] = self.y_balance;
        bytes[10] = self.stroke_width;
        bytes[11] = self.serif_score;
        bytes[12] = (self.feature_hash & 0xFF) as u8;
        bytes[13] = (self.feature_hash >> 8) as u8;
        bytes[14] = (self.reserved & 0xFF) as u8;
        bytes[15] = (self.reserved >> 8) as u8;
        bytes
    }
    
    /// Create from raw bytes
    pub fn from_bytes(bytes: &[u8; 16]) -> Self {
        Self {
            aspect_ratio: bytes[0],
            density: bytes[1],
            quadrant_nw: bytes[2],
            quadrant_ne: bytes[3],
            quadrant_sw: bytes[4],
            quadrant_se: bytes[5],
            curve_ratio: bytes[6],
            point_count: bytes[7],
            x_balance: bytes[8],
            y_balance: bytes[9],
            stroke_width: bytes[10],
            serif_score: bytes[11],
            feature_hash: (bytes[12] as u16) | ((bytes[13] as u16) << 8),
            reserved: (bytes[14] as u16) | ((bytes[15] as u16) << 8),
        }
    }
    
    /// Calculate similarity score between two signatures (0.0 - 1.0)
    /// Uses weighted Manhattan distance converted to similarity
    pub fn similarity(&self, other: &Self) -> f32 {
        // Weights for each feature (sum to 1.0)
        const WEIGHTS: [f32; 12] = [
            0.10,  // aspect_ratio
            0.05,  // density
            0.10,  // quadrant_nw
            0.10,  // quadrant_ne
            0.10,  // quadrant_sw
            0.10,  // quadrant_se
            0.10,  // curve_ratio
            0.05,  // point_count
            0.08,  // x_balance
            0.08,  // y_balance
            0.07,  // stroke_width
            0.07,  // serif_score
        ];
        
        let self_bytes = self.to_bytes();
        let other_bytes = other.to_bytes();
        
        let mut weighted_distance = 0.0_f32;
        
        for i in 0..12 {
            let diff = (self_bytes[i] as i16 - other_bytes[i] as i16).abs() as f32;
            let normalized_diff = diff / 255.0;  // Normalize to 0-1
            weighted_distance += normalized_diff * WEIGHTS[i];
        }
        
        // Also consider feature hash (XOR distance)
        let hash_diff = (self.feature_hash ^ other.feature_hash).count_ones() as f32 / 16.0;
        weighted_distance += hash_diff * 0.05;
        
        // Convert distance to similarity (1.0 = identical, 0.0 = completely different)
        (1.0 - weighted_distance).max(0.0)
    }
}

impl Default for MicroSignature {
    fn default() -> Self {
        Self::new()
    }
}

// =============================================================================
// GLYPH EXTRACTOR
// =============================================================================

/// Extracts glyph outlines from font files using ttf_parser
pub struct GlyphExtractor;

/// Builder for constructing glyph outlines from ttf_parser callbacks
struct OutlineBuilder {
    segments: Vec<PathSegment>,
    x_min: f32,
    y_min: f32,
    x_max: f32,
    y_max: f32,
}

impl OutlineBuilder {
    fn new() -> Self {
        Self {
            segments: Vec::new(),
            x_min: f32::MAX,
            y_min: f32::MAX,
            x_max: f32::MIN,
            y_max: f32::MIN,
        }
    }
    
    fn update_bounds(&mut self, x: f32, y: f32) {
        self.x_min = self.x_min.min(x);
        self.y_min = self.y_min.min(y);
        self.x_max = self.x_max.max(x);
        self.y_max = self.y_max.max(y);
    }
}

impl ttf_parser::OutlineBuilder for OutlineBuilder {
    fn move_to(&mut self, x: f32, y: f32) {
        self.update_bounds(x, y);
        self.segments.push(PathSegment::MoveTo(x, y));
    }
    
    fn line_to(&mut self, x: f32, y: f32) {
        self.update_bounds(x, y);
        self.segments.push(PathSegment::LineTo(x, y));
    }
    
    fn quad_to(&mut self, x1: f32, y1: f32, x: f32, y: f32) {
        self.update_bounds(x1, y1);
        self.update_bounds(x, y);
        self.segments.push(PathSegment::QuadTo { cx: x1, cy: y1, x, y });
    }
    
    fn curve_to(&mut self, x1: f32, y1: f32, x2: f32, y2: f32, x: f32, y: f32) {
        self.update_bounds(x1, y1);
        self.update_bounds(x2, y2);
        self.update_bounds(x, y);
        self.segments.push(PathSegment::CurveTo { cx1: x1, cy1: y1, cx2: x2, cy2: y2, x, y });
    }
    
    fn close(&mut self) {
        self.segments.push(PathSegment::Close);
    }
}

impl GlyphExtractor {
    /// Create a new glyph extractor
    pub fn new() -> Self {
        Self
    }
    
    /// Extract glyph outline from a font file for a specific character
    pub fn extract_from_file<P: AsRef<Path>>(&self, font_path: P, character: char) -> Result<GlyphOutline, GlyphError> {
        let font_data = std::fs::read(font_path.as_ref())
            .map_err(|e| GlyphError::IoError(e.to_string()))?;
        
        self.extract_from_data(&font_data, character)
    }
    
    /// Extract glyph outline from font data bytes
    pub fn extract_from_data(&self, font_data: &[u8], character: char) -> Result<GlyphOutline, GlyphError> {
        let face = ttf_parser::Face::parse(font_data, 0)
            .map_err(|e| GlyphError::ParseError(format!("Failed to parse font: {:?}", e)))?;
        
        let glyph_id = face.glyph_index(character)
            .ok_or_else(|| GlyphError::GlyphNotFound(character))?;
        
        let units_per_em = face.units_per_em();
        let mut outline = GlyphOutline::new(character, units_per_em);
        
        // Get advance width
        if let Some(advance) = face.glyph_hor_advance(glyph_id) {
            outline.advance_width = advance as f32;
        }
        
        // Build the outline
        let mut builder = OutlineBuilder::new();
        
        if face.outline_glyph(glyph_id, &mut builder).is_some() {
            outline.segments = builder.segments;
            
            if builder.x_min != f32::MAX {
                outline.bounds = BoundingBox::new(
                    builder.x_min,
                    builder.y_min,
                    builder.x_max,
                    builder.y_max,
                );
            }
        }
        
        Ok(outline)
    }
    
    /// Extract signatures for multiple characters from a font file
    pub fn extract_signatures<P: AsRef<Path>>(&self, font_path: P, characters: &str) -> Result<Vec<(char, MicroSignature)>, GlyphError> {
        let font_data = std::fs::read(font_path.as_ref())
            .map_err(|e| GlyphError::IoError(e.to_string()))?;
        
        let mut signatures = Vec::new();
        
        for ch in characters.chars() {
            match self.extract_from_data(&font_data, ch) {
                Ok(outline) => {
                    let sig = MicroSignature::from_outline(&outline);
                    signatures.push((ch, sig));
                }
                Err(GlyphError::GlyphNotFound(_)) => {
                    // Skip characters not in the font
                    continue;
                }
                Err(e) => return Err(e),
            }
        }
        
        Ok(signatures)
    }
    
    /// Extract signatures for all alphanumeric characters (A-Z, a-z, 0-9)
    pub fn extract_alphanumeric_signatures<P: AsRef<Path>>(&self, font_path: P) -> Result<Vec<(char, MicroSignature)>, GlyphError> {
        const ALPHANUMERIC: &str = "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789";
        self.extract_signatures(font_path, ALPHANUMERIC)
    }
}

impl Default for GlyphExtractor {
    fn default() -> Self {
        Self::new()
    }
}

// =============================================================================
// ERRORS
// =============================================================================

/// Errors that can occur during glyph extraction
#[derive(Debug, Clone)]
pub enum GlyphError {
    /// Failed to read font file
    IoError(String),
    /// Failed to parse font data
    ParseError(String),
    /// Glyph not found in font
    GlyphNotFound(char),
}

impl std::fmt::Display for GlyphError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            GlyphError::IoError(msg) => write!(f, "IO error: {}", msg),
            GlyphError::ParseError(msg) => write!(f, "Parse error: {}", msg),
            GlyphError::GlyphNotFound(c) => write!(f, "Glyph not found: '{}'", c),
        }
    }
}

impl std::error::Error for GlyphError {}

// =============================================================================
// TESTS
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_micro_signature_bytes_roundtrip() {
        let sig = MicroSignature {
            aspect_ratio: 128,
            density: 200,
            quadrant_nw: 50,
            quadrant_ne: 60,
            quadrant_sw: 70,
            quadrant_se: 80,
            curve_ratio: 128,
            point_count: 42,
            x_balance: 120,
            y_balance: 130,
            stroke_width: 100,
            serif_score: 150,
            feature_hash: 0xABCD,
            reserved: 0,
        };
        
        let bytes = sig.to_bytes();
        let restored = MicroSignature::from_bytes(&bytes);
        
        assert_eq!(sig, restored);
    }
    
    #[test]
    fn test_signature_self_similarity() {
        let sig = MicroSignature {
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
        
        let similarity = sig.similarity(&sig);
        assert!((similarity - 1.0).abs() < 0.001, "Self-similarity should be 1.0");
    }
    
    #[test]
    fn test_signature_different_similarity() {
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
        
        let sig2 = MicroSignature {
            aspect_ratio: 200,
            density: 50,
            quadrant_nw: 200,
            quadrant_ne: 20,
            quadrant_sw: 200,
            quadrant_se: 20,
            curve_ratio: 50,
            point_count: 100,
            x_balance: 50,
            y_balance: 200,
            stroke_width: 200,
            serif_score: 200,
            feature_hash: 54321,
            reserved: 0,
        };
        
        let similarity = sig1.similarity(&sig2);
        assert!(similarity < 0.8, "Different signatures should have lower similarity, got: {}", similarity);
        assert!(similarity >= 0.0, "Similarity should be non-negative");
    }
    
    #[test]
    fn test_bounding_box() {
        let bbox = BoundingBox::new(0.0, 0.0, 100.0, 200.0);
        assert_eq!(bbox.width(), 100.0);
        assert_eq!(bbox.height(), 200.0);
        assert_eq!(bbox.aspect_ratio(), 0.5);
        assert_eq!(bbox.center_x(), 50.0);
        assert_eq!(bbox.center_y(), 100.0);
    }
    
    #[test]
    fn test_segment_stats() {
        let mut outline = GlyphOutline::new('A', 1000);
        outline.segments = vec![
            PathSegment::MoveTo(0.0, 0.0),
            PathSegment::LineTo(50.0, 100.0),
            PathSegment::LineTo(100.0, 0.0),
            PathSegment::QuadTo { cx: 75.0, cy: 50.0, x: 50.0, y: 0.0 },
            PathSegment::Close,
        ];
        
        let stats = outline.segment_stats();
        assert_eq!(stats.line_count, 2);
        assert_eq!(stats.quad_count, 1);
        assert_eq!(stats.cubic_count, 0);
        assert_eq!(stats.total_control_points, 1);
    }
}
