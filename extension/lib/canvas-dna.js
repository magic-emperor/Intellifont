/**
 * intelliFont Canvas DNA Analyzer
 * 
 * Computes visual font metrics (matching the Rust MicroSignature format)
 * by rendering characters to a hidden <canvas> and analyzing the pixel bitmap.
 * 
 * No file upload. No CORS. Works on any font currently loaded in the browser.
 * 
 * Metrics computed (all u8 range 0-255):
 *   aspect_ratio  — width ÷ height × 64
 *   density       — ink pixel density in bounding box
 *   quadrant_nw/ne/sw/se — ink distribution across quadrants
 *   curve_ratio   — estimated curviness of strokes
 *   point_count   — outline complexity (direction changes)
 *   x_balance     — horizontal center of mass
 *   y_balance     — vertical center of mass
 *   stroke_width  — estimated stroke thickness
 *   serif_score   — serif vs sans classification
 */

(() => {
if (window.CanvasDNA) return;
'use strict';

// =============================================================================
// RENDER SIZE — larger = more accurate, slower
// =============================================================================
const RENDER_SIZE = 256;
const DARK_THRESHOLD = 128; // pixel brightness below this = "ink"

// =============================================================================
// INTERNAL: Render a single character to pixel data
// =============================================================================
function _renderChar(fontFamily, character, size) {
  const canvas = document.createElement('canvas');
  canvas.width = size;
  canvas.height = size;
  const ctx = canvas.getContext('2d', { willReadFrequently: true });

  // White background
  ctx.fillStyle = '#ffffff';
  ctx.fillRect(0, 0, size, size);

  // Draw the character centered
  ctx.fillStyle = '#000000';
  ctx.font = `${Math.floor(size * 0.72)}px "${fontFamily}", serif`;
  ctx.textBaseline = 'middle';
  ctx.textAlign = 'center';
  ctx.fillText(character, size / 2, size / 2);

  return ctx.getImageData(0, 0, size, size);
}

// =============================================================================
// INTERNAL: Find bounding box and collect dark pixel list
// =============================================================================
function _collectDarkPixels(imageData, size) {
  const { data } = imageData;
  let minX = size, minY = size, maxX = -1, maxY = -1;
  const dark = [];

  for (let y = 0; y < size; y++) {
    for (let x = 0; x < size; x++) {
      const i = (y * size + x) * 4;
      const brightness = (data[i] + data[i + 1] + data[i + 2]) / 3;
      if (brightness < DARK_THRESHOLD) {
        dark.push({ x, y });
        if (x < minX) minX = x;
        if (y < minY) minY = y;
        if (x > maxX) maxX = x;
        if (y > maxY) maxY = y;
      }
    }
  }

  return { dark, minX, minY, maxX, maxY };
}

// =============================================================================
// INTERNAL: Get edge pixels (pixels with at least one white neighbour)
// =============================================================================
function _getEdgePixels(data, size, minX, minY, maxX, maxY) {
  const edges = [];
  for (let y = minY; y <= maxY; y++) {
    for (let x = minX; x <= maxX; x++) {
      const i = (y * size + x) * 4;
      const brightness = (data[i] + data[i + 1] + data[i + 2]) / 3;
      if (brightness >= DARK_THRESHOLD) continue; // not a dark pixel

      // Check 4-connected neighbours
      const neighbors = [
        [x - 1, y], [x + 1, y], [x, y - 1], [x, y + 1]
      ];
      for (const [nx, ny] of neighbors) {
        if (nx < 0 || ny < 0 || nx >= size || ny >= size) {
          edges.push({ x, y });
          break;
        }
        const ni = (ny * size + nx) * 4;
        const nb = (data[ni] + data[ni + 1] + data[ni + 2]) / 3;
        if (nb >= DARK_THRESHOLD) {
          edges.push({ x, y });
          break;
        }
      }
    }
  }
  return edges;
}

// =============================================================================
// INTERNAL: Estimate curve ratio from edge pixel direction changes
// A straight edge has consistent dx/dy; a curve has varying direction.
// =============================================================================
function _estimateCurveRatio(edgePixels) {
  if (edgePixels.length < 8) return 128;

  // Sort edge pixels into an approximate contour order by angle from centroid
  const cx = edgePixels.reduce((s, p) => s + p.x, 0) / edgePixels.length;
  const cy = edgePixels.reduce((s, p) => s + p.y, 0) / edgePixels.length;
  const sorted = [...edgePixels].sort((a, b) =>
    Math.atan2(a.y - cy, a.x - cx) - Math.atan2(b.y - cy, b.x - cx)
  );

  // Count direction changes between consecutive edge pixels
  let dirChanges = 0;
  const step = Math.max(1, Math.floor(sorted.length / 32)); // sample up to 32 points
  let prevAngle = null;

  for (let i = 0; i < sorted.length; i += step) {
    const next = sorted[(i + step) % sorted.length];
    const angle = Math.atan2(next.y - sorted[i].y, next.x - sorted[i].x);
    if (prevAngle !== null) {
      let delta = Math.abs(angle - prevAngle);
      if (delta > Math.PI) delta = 2 * Math.PI - delta;
      if (delta > 0.3) dirChanges++; // threshold ~17 degrees
    }
    prevAngle = angle;
  }

  const samples = Math.ceil(sorted.length / step);
  // More direction changes per sample → more curved
  return Math.min(255, Math.floor((dirChanges / samples) * 255 * 1.5));
}

// =============================================================================
// INTERNAL: Estimate stroke width from horizontal run lengths
// =============================================================================
function _estimateStrokeWidth(data, size, minX, minY, maxX, maxY) {
  const width = maxX - minX + 1;
  let totalRunLen = 0;
  let runCount = 0;

  for (let y = minY; y <= maxY; y++) {
    let runLen = 0;
    for (let x = minX; x <= maxX; x++) {
      const i = (y * size + x) * 4;
      const brightness = (data[i] + data[i + 1] + data[i + 2]) / 3;
      if (brightness < DARK_THRESHOLD) {
        runLen++;
      } else {
        // Only count short-to-medium runs (strokes, not large filled areas)
        if (runLen > 1 && runLen < width * 0.75) {
          totalRunLen += runLen;
          runCount++;
        }
        runLen = 0;
      }
    }
    if (runLen > 1 && runLen < width * 0.75) {
      totalRunLen += runLen;
      runCount++;
    }
  }

  const avgRun = runCount > 0 ? totalRunLen / runCount : width / 6;
  // Normalize so that a typical stroke (~12px at 256px render) maps to midrange
  return Math.min(255, Math.floor((avgRun / RENDER_SIZE) * 1400));
}

// =============================================================================
// INTERNAL: Estimate serif score from horizontal structure at baseline
// =============================================================================
function _estimateSerifScore(data, size, minX, minY, maxX, maxY) {
  const height = maxY - minY + 1;

  // Looking at the bottom 15% of the bounding box (where serifs appear)
  const bottomStart = Math.floor(minY + height * 0.80);
  const bottomEnd = Math.floor(minY + height * 0.95);
  const topStart = Math.floor(minY + height * 0.05);
  const topEnd = Math.floor(minY + height * 0.20);

  let bottomRuns = 0;
  let topRuns = 0;

  function countHorizRuns(yStart, yEnd) {
    let runs = 0;
    for (let y = yStart; y <= Math.min(yEnd, maxY); y++) {
      let runLen = 0;
      for (let x = minX; x <= maxX; x++) {
        const i = (y * size + x) * 4;
        const brightness = (data[i] + data[i + 1] + data[i + 2]) / 3;
        if (brightness < DARK_THRESHOLD) {
          runLen++;
        } else {
          if (runLen >= 2) runs++;
          runLen = 0;
        }
      }
      if (runLen >= 2) runs++;
    }
    return runs;
  }

  bottomRuns = countHorizRuns(bottomStart, bottomEnd);
  topRuns = countHorizRuns(topStart, topEnd);

  // Serifs add extra horizontal runs at the top and bottom of strokes
  const rows = (bottomEnd - bottomStart + 1) + (topEnd - topStart + 1);
  return Math.min(255, Math.floor(((bottomRuns + topRuns) / Math.max(1, rows)) * 200));
}

// =============================================================================
// INTERNAL: Sobel gradient — detect italic angle from a glyph image.
//
// Computes the gradient direction at every strong edge pixel.
// Near-vertical edges (±30° of 90°) are collected; their median deviation
// from true-vertical gives the slant angle.
// Returns degrees: 0 = upright, positive = slants right (italic), negative = back-slant.
// =============================================================================
function _detectItalicAngle(data, size) {
  const deviations = [];

  for (let y = 1; y < size - 1; y++) {
    for (let x = 1; x < size - 1; x++) {
      // Luminance for each of the 3×3 neighbourhood pixels
      const lum = (r, c) => {
        const i = ((y + r) * size + (x + c)) * 4;
        return (data[i] * 77 + data[i + 1] * 150 + data[i + 2] * 29) >> 8;
      };
      // Sobel
      const gx = -lum(-1,-1) + lum(-1,1) - 2*lum(0,-1) + 2*lum(0,1) - lum(1,-1) + lum(1,1);
      const gy =  lum(-1,-1) + 2*lum(-1,0) + lum(-1,1) - lum(1,-1) - 2*lum(1,0) - lum(1,1);

      const mag = Math.sqrt(gx * gx + gy * gy);
      if (mag < 25) continue; // ignore weak / noise edges

      const angleDeg = Math.atan2(gy, gx) * 180 / Math.PI;
      const absAngle = Math.abs(angleDeg);

      // Keep only near-vertical edges (60°–120° from horizontal axis)
      if (absAngle >= 60 && absAngle <= 120) {
        // Deviation from 90° (positive = slants right)
        deviations.push(angleDeg > 0 ? angleDeg - 90 : angleDeg + 90);
      }
    }
  }

  if (deviations.length < 20) return 0;

  // Median is more robust than mean against noise
  deviations.sort((a, b) => a - b);
  return deviations[Math.floor(deviations.length / 2)];
}

// =============================================================================
// INTERNAL: Map stroke_width metric (0-255) to CSS font-weight (100-900)
// Calibrated against rendered Google Fonts at RENDER_SIZE=256
// =============================================================================
function _mapToWeight(strokeWidth) {
  if (strokeWidth < 18)  return 100; // Thin
  if (strokeWidth < 27)  return 200; // ExtraLight
  if (strokeWidth < 38)  return 300; // Light
  if (strokeWidth < 52)  return 400; // Regular
  if (strokeWidth < 68)  return 500; // Medium
  if (strokeWidth < 86)  return 600; // SemiBold
  if (strokeWidth < 108) return 700; // Bold
  if (strokeWidth < 135) return 800; // ExtraBold
  return 900;                        // Black
}

// =============================================================================
// PUBLIC: Detect style properties (italic, weight) from raw pixel data.
// Call after analyzeImageData to get a full style profile for one glyph.
//
// Returns:
//   italic      — true if slant angle > 8°
//   italicAngle — exact measured slant in degrees
//   cssWeight   — one of 100/200/…/900
//   weightLabel — "Thin"|"Light"|"Regular"|"Bold"|etc.
// =============================================================================
function analyzeStyle(imageData, size) {
  const { data } = imageData;

  // Compute stroke width via the existing helper (operates on raw RGBA data)
  // We need bounding box first — reuse _collectDarkPixels
  const { dark, minX, minY, maxX, maxY } = _collectDarkPixels(imageData, size);
  if (!dark.length || maxX < minX) {
    return { italic: false, italicAngle: 0, cssWeight: 400, weightLabel: 'Regular' };
  }

  const strokeWidth = _estimateStrokeWidth(data, size, minX, minY, maxX, maxY);
  const cssWeight   = _mapToWeight(strokeWidth);
  const italicAngle = _detectItalicAngle(data, size);

  const WEIGHT_LABELS = {
    100: 'Thin', 200: 'ExtraLight', 300: 'Light', 400: 'Regular',
    500: 'Medium', 600: 'SemiBold', 700: 'Bold', 800: 'ExtraBold', 900: 'Black',
  };

  return {
    italic:      Math.abs(italicAngle) >= 8,
    italicAngle: Math.round(italicAngle * 10) / 10,
    cssWeight,
    weightLabel: WEIGHT_LABELS[cssWeight],
  };
}

// =============================================================================
// PUBLIC: Compute metrics directly from raw ImageData (no rendering).
// Used by image-pipeline.js to analyze cropped glyphs from real images.
// character — the label to attach (caller's responsibility to assign correctly)
// size      — the canvas dimension the imageData was drawn at
// =============================================================================
function analyzeImageData(imageData, size, character) {
  const { data } = imageData;
  const { dark, minX, minY, maxX, maxY } = _collectDarkPixels(imageData, size);

  if (dark.length === 0 || maxX < minX || maxY < minY) return null;

  const width  = maxX - minX + 1;
  const height = maxY - minY + 1;
  const total  = dark.length;

  const aspectRatio = Math.min(255, Math.floor((width / height) * 64));
  const boxArea     = Math.max(1, width * height);
  const density     = Math.min(255, Math.floor((total / boxArea) * 255));

  const cx = minX + width / 2;
  const cy = minY + height / 2;
  let nw = 0, ne = 0, sw = 0, se = 0;
  for (const { x, y } of dark) {
    if (x < cx && y < cy) nw++;
    else if (x >= cx && y < cy) ne++;
    else if (x < cx && y >= cy) sw++;
    else se++;
  }

  const sumX     = dark.reduce((s, p) => s + p.x, 0);
  const sumY     = dark.reduce((s, p) => s + p.y, 0);
  const xBalance = Math.min(255, Math.floor(((sumX / total) - minX) / width * 255));
  const yBalance = Math.min(255, Math.floor(((sumY / total) - minY) / height * 255));

  const edgePixels = _getEdgePixels(data, size, minX, minY, maxX, maxY);

  return {
    character,
    aspectRatio,
    density,
    quadrantNw: Math.floor((nw / total) * 255),
    quadrantNe: Math.floor((ne / total) * 255),
    quadrantSw: Math.floor((sw / total) * 255),
    quadrantSe: Math.floor((se / total) * 255),
    curveRatio:  _estimateCurveRatio(edgePixels),
    pointCount:  Math.min(255, Math.floor(edgePixels.length / 5)),
    xBalance,
    yBalance,
    strokeWidth: _estimateStrokeWidth(data, size, minX, minY, maxX, maxY),
    serifScore:  _estimateSerifScore(data, size, minX, minY, maxX, maxY),
  };
}

// =============================================================================
// PUBLIC: Analyze one character — returns a JsPixelMetrics-compatible object
// =============================================================================
function analyzeChar(fontFamily, character, size = RENDER_SIZE) {
  const imageData = _renderChar(fontFamily, character, size);
  const metrics   = analyzeImageData(imageData, size, character);
  return metrics; // null if font doesn't contain this glyph
}

// =============================================================================
// PUBLIC: Analyze multiple characters for a given font family name
// Returns array of JsPixelMetrics-compatible objects (one per char)
// =============================================================================
function analyzeFont(fontFamily, characters = 'RQWM', size = RENDER_SIZE) {
  const results = [];
  for (const char of characters) {
    const metrics = analyzeChar(fontFamily, char, size);
    if (metrics) results.push(metrics);
  }
  return results;
}

// =============================================================================
// PUBLIC: Analyze a DOM element's font
// Returns { fontFamily, metrics[] }
// =============================================================================
function analyzeElement(element, characters = 'RQWM') {
  const style = window.getComputedStyle(element);
  // fontFamily may be a comma-separated list like '"Inter", sans-serif'
  const raw = style.fontFamily || 'sans-serif';
  // Take the first family name, strip quotes
  const fontFamily = raw.split(',')[0].trim().replace(/['"]/g, '');
  const metrics = analyzeFont(fontFamily, characters);
  return { fontFamily, cssRaw: raw, metrics };
}

// =============================================================================
// Variable Font Axis Detection
//
// Detects whether a font is variable and estimates its current axis values
// from measured pixel metrics.
//
// Strategy:
//   1. Render the font at CSS weight 300 and 700 — if stroke widths differ
//      by more than a threshold the font is variable (responds to wght axis).
//   2. Binary-search the wght axis to find the value that best matches the
//      measured stroke_width from an image or DOM element.
//   3. Detect optical-size (opsz) axis by comparing x-height ratios at
//      different opsz values (requires font-variation-settings support).
//
// @param {string} fontFamily
// @param {Object} measuredMetrics  — JsPixelMetrics from analyzeImageData/analyzeFont
// @returns {{ variable: boolean, axes: Record<string, number> | null, note?: string }}
// =============================================================================

function _renderCharWithWeight(fontFamily, character, size, weight) {
  const canvas = document.createElement('canvas');
  canvas.width  = size;
  canvas.height = size;
  const ctx = canvas.getContext('2d', { willReadFrequently: true });
  ctx.fillStyle = '#ffffff';
  ctx.fillRect(0, 0, size, size);
  ctx.fillStyle = '#000000';
  ctx.font = `${weight} ${Math.floor(size * 0.72)}px "${fontFamily}", serif`;
  ctx.textBaseline = 'middle';
  ctx.textAlign    = 'center';
  ctx.fillText(character, size / 2, size / 2);
  return ctx.getImageData(0, 0, size, size);
}

function _avgStrokeWidth(metrics) {
  if (!metrics || !metrics.length) return 0;
  return metrics.reduce((s, m) => s + (m.strokeWidth ?? m.stroke_width ?? 0), 0) / metrics.length;
}

function detectVariableAxes(fontFamily, measuredMetrics) {
  if (typeof document === 'undefined') {
    return { variable: false, axes: null, note: 'detectVariableAxes requires a browser environment' };
  }

  const SIZE  = RENDER_SIZE;
  const CHARS = 'RHn';

  // Render at thin (300) and bold (700) to test wght response
  const thin = CHARS.split('').map(ch => {
    const id = _renderCharWithWeight(fontFamily, ch, SIZE, 300);
    return analyzeImageData(id, SIZE, ch);
  }).filter(Boolean);

  const bold = CHARS.split('').map(ch => {
    const id = _renderCharWithWeight(fontFamily, ch, SIZE, 700);
    return analyzeImageData(id, SIZE, ch);
  }).filter(Boolean);

  if (!thin.length || !bold.length) {
    return { variable: false, axes: null, note: 'Font did not render — may not be loaded' };
  }

  const swThin = _avgStrokeWidth(thin);
  const swBold = _avgStrokeWidth(bold);

  // Non-variable fonts: static weights snap to nearest defined weight.
  // Stroke width difference between 300 and 700 on a static font is typically < 8 units.
  // Variable fonts show smooth continuous change, typically > 15 units difference.
  if (Math.abs(swBold - swThin) < 8) {
    return { variable: false, axes: null };
  }

  // ── Variable detected — binary-search wght axis ───────────────────────────
  const targetSw = measuredMetrics
    ? (measuredMetrics.strokeWidth ?? measuredMetrics.stroke_width ?? (swThin + swBold) / 2)
    : (swThin + swBold) / 2;

  let lo = 100, hi = 900, wght = 400;

  for (let iter = 0; iter < 7; iter++) {
    const mid  = Math.round((lo + hi) / 2);
    const mids = CHARS.split('').map(ch => {
      const id = _renderCharWithWeight(fontFamily, ch, SIZE, mid);
      return analyzeImageData(id, SIZE, ch);
    }).filter(Boolean);

    const swMid = _avgStrokeWidth(mids);
    wght = mid;

    if (swMid < targetSw) lo = mid + 1;
    else if (swMid > targetSw) hi = mid - 1;
    else break;
  }

  // Round to nearest 50 (CSS weight scale)
  wght = Math.round(wght / 50) * 50;
  wght = Math.max(100, Math.min(900, wght));

  return { variable: true, axes: { wght } };
}

// =============================================================================
// EXPORTS (works as both ES module and CommonJS in browser/Node contexts)
// =============================================================================
const CanvasDNA = { analyzeChar, analyzeFont, analyzeElement, analyzeImageData, analyzeStyle, detectVariableAxes };

if (typeof module !== 'undefined' && module.exports) {
  module.exports = CanvasDNA;
} else if (typeof window !== 'undefined') {
  window.CanvasDNA = CanvasDNA;
}
})();
