(() => {
if (window.IntellifontImagePipeline) return;
'use strict';
/**
 * Intellifont Image Pipeline (browser)
 *
 * Identifies fonts from arbitrary raster images: screenshots, photos, PDFs,
 * scanned documents. Runs entirely client-side — no uploads, no server needed
 * for the analysis step.
 *
 * Pipeline stages:
 *   [1] Load image onto canvas
 *   [2] Quality check (Laplacian variance) → optional bicubic upscale
 *   [3] Deskew (projection profile method)
 *   [4] Text region detection (connected-component heuristic)
 *   [5] Crop + normalize each character region to GLYPH_SIZE × GLYPH_SIZE
 *   [6] Extract pixel metrics via CanvasDNA.analyzeImageData()
 *       Assign character label heuristically from shape
 *
 * Output: JsPixelMetrics[] ready for identifyFromPixelMetrics()
 *
 * Usage:
 *   const pipeline = new ImagePipeline();
 *   const { metrics, quality, deskewAngle } = await pipeline.analyze(imageSource);
 *   const matches = identifyFromPixelMetrics(metrics, 8);
 */

// Size to normalize each glyph crop to before metric extraction
const GLYPH_SIZE       = 128;
// Pixel brightness threshold for "ink" (0 = black, 255 = white)
const DARK_THRESHOLD   = 140;
// Minimum and maximum character height in pixels (on the deskewed image)
const MIN_GLYPH_H      = 12;
const MAX_GLYPH_H      = 600;
// Acceptable aspect ratio range for a character crop (w/h)
const MIN_CHAR_RATIO   = 0.15;
const MAX_CHAR_RATIO   = 3.5;
// How many glyph crops to extract and analyze per image
const MAX_GLYPHS       = 12;
// Sharpness score below this triggers a 2× upscale attempt
const SHARP_THRESHOLD  = 80;

// =============================================================================
// Helpers
// =============================================================================

function _createCanvas(w, h) {
  const c = document.createElement('canvas');
  c.width  = w;
  c.height = h;
  return c;
}

function _toGrayscaleData(rgba, w, h) {
  const gray = new Uint8Array(w * h);
  const total = w * h;

  // Classify image: B&W, clean colored, or glow colored.
  //
  // Glow images have a halo of medium-saturation pixels (sat 50–160) around
  // each letter. Clean colored images are bimodal: near-zero sat (dark bg) or
  // very high sat (colored text). We count medium-sat pixels to distinguish.
  let saturatedCount = 0, glowCount = 0, highlightCount = 0;
  for (let i = 0; i < total; i++) {
    const r = rgba[i * 4], g = rgba[i * 4 + 1], b = rgba[i * 4 + 2];
    const max = Math.max(r, g, b), min = Math.min(r, g, b);
    const s = (max - min);
    if (max > 60 && s > 80) saturatedCount++;
    if (max > 40 && s > 40 && s < 130) glowCount++;  // medium-sat = halo pixels
    // High-luminance + medium-saturation = highlighter fill (yellow/green/pink bg)
    if (max > 160 && s > 40 && s < 160) highlightCount++;
  }
  const useSaturation  = saturatedCount   > total * 0.05;
  const hasHighlight   = highlightCount   > total * 0.25;
  const hasGlow        = glowCount        > total * 0.20;   // >20% medium-sat pixels = glow (8% was too low — AA edges triggered it)

  if (!useSaturation) {
    // Standard ITU-R BT.601 luminance for B&W / non-colored images
    for (let i = 0; i < total; i++)
      gray[i] = (rgba[i*4]*77 + rgba[i*4+1]*150 + rgba[i*4+2]*29) >> 8;
    console.log(`[Intellifont ALSC] path=luminance imageSize=${w}×${h}`);
    return gray;
  }

  // Compute raw saturation array (needed by multiple paths)
  const sat = new Uint8Array(total);
  for (let i = 0; i < total; i++) {
    const r = rgba[i*4], g = rgba[i*4+1], b = rgba[i*4+2];
    const max = Math.max(r, g, b), min = Math.min(r, g, b);
    sat[i] = max === 0 ? 0 : Math.round(((max - min) / max) * 255);
  }

  if (hasHighlight) {
    // Highlighter bg (yellow/green/pink): dark text ink on bright colored background.
    // ALSC fails here because the BACKGROUND is the high-saturation region — it would
    // amplify background pixels as ink. Instead: suppress high-luminance medium-saturation
    // pixels to white (background), keep dark pixels as-is via standard luminance.
    for (let i = 0; i < total; i++) {
      const r = rgba[i*4], g = rgba[i*4+1], b = rgba[i*4+2];
      const max = Math.max(r, g, b), min = Math.min(r, g, b);
      const s   = max - min;
      gray[i] = (max > 160 && s > 40 && s < 160)
        ? 255   // suppress highlighter fill → white (background)
        : (r*77 + g*150 + b*29) >> 8;  // keep ink pixels via luminance
    }
    console.log(`[Intellifont ALSC] path=highlight-suppress imageSize=${w}×${h} highlight=${((highlightCount/total)*100).toFixed(1)}%`);
    return gray;
  }

  if (!hasGlow) {
    // Clean colored text — squared saturation: gray = 255 - sat²/255.
    // Squaring heavily penalises JPEG artifacts (sat≈80-130 → gray≈215+, white)
    // while keeping true colored text dark (sat≈200 → gray≈98).
    // Linear 255-sat was too sensitive: artifact pixels (gray≈127) fell below
    // DARK_THRESHOLD=140 and were mistaken for ink.
    for (let i = 0; i < total; i++)
      gray[i] = 255 - Math.round((sat[i] * sat[i]) / 255);
    console.log(`[Intellifont ALSC] path=clean-color imageSize=${w}×${h} saturated=${((saturatedCount/total)*100).toFixed(1)}%`);
    return gray;
  }

  // Glow colored text — ALSC (Adaptive Local Saturation Contrast).
  //
  // Global thresholds fail because the glow halo is the same hue as the letter
  // core — just less saturated. ALSC measures each pixel's saturation RELATIVE
  // to its local neighbourhood via an integral image:
  //   excess = max(0, S_pixel − 0.80 × S_localMean)
  // Letter cores stand above their local mean → dark. Glow pixels are at or
  // below their local mean → white.
  console.log(`[Intellifont ALSC] path=glow-alsc imageSize=${w}×${h} glowFrac=${((glowCount/total)*100).toFixed(1)}%`);

  // Integral image (summed area table) over saturation
  const intg = new Float64Array((w + 1) * (h + 1));
  for (let y = 0; y < h; y++) {
    for (let x = 0; x < w; x++) {
      intg[(y+1)*(w+1)+(x+1)] =
        sat[y*w+x]
        + intg[y*(w+1)+(x+1)]
        + intg[(y+1)*(w+1)+x]
        - intg[y*(w+1)+x];
    }
  }

  const R = Math.min(60, Math.max(20, Math.round(h * 0.25)));
  for (let y = 0; y < h; y++) {
    for (let x = 0; x < w; x++) {
      const x0 = Math.max(0, x - R), y0 = Math.max(0, y - R);
      const x1 = Math.min(w, x + R + 1), y1 = Math.min(h, y + R + 1);
      const area = (x1 - x0) * (y1 - y0);
      const sum  = intg[y1*(w+1)+x1] - intg[y0*(w+1)+x1]
                 - intg[y1*(w+1)+x0] + intg[y0*(w+1)+x0];
      const localMean = sum / area;
      const excess    = Math.max(0, sat[y*w+x] - 0.80 * localMean);
      gray[y*w+x] = Math.max(0, 255 - Math.round(excess * 3.0));
    }
  }
  return gray;
}

// Laplacian variance — high = sharp, low = blurry
function _computeSharpness(gray, w, h) {
  let sum = 0, sumSq = 0, n = 0;
  for (let y = 1; y < h - 1; y++) {
    for (let x = 1; x < w - 1; x++) {
      const lap =
        Math.abs(gray[y * w + x] * 4
          - gray[(y - 1) * w + x]
          - gray[(y + 1) * w + x]
          - gray[y * w + (x - 1)]
          - gray[y * w + (x + 1)]);
      sum   += lap;
      sumSq += lap * lap;
      n++;
    }
  }
  const mean = sum / n;
  return sumSq / n - mean * mean; // variance
}

// Otsu's method — returns the optimal binary threshold
function _otsuThreshold(gray) {
  const hist = new Int32Array(256);
  for (const v of gray) hist[v]++;
  const total = gray.length;
  let sum = 0;
  for (let i = 0; i < 256; i++) sum += i * hist[i];

  let sumB = 0, wB = 0, best = 0, thresh = DARK_THRESHOLD;
  for (let t = 0; t < 256; t++) {
    wB += hist[t];
    if (!wB) continue;
    const wF = total - wB;
    if (!wF) break;
    sumB += t * hist[t];
    const mB = sumB / wB;
    const mF = (sum - sumB) / wF;
    const between = wB * wF * (mB - mF) ** 2;
    if (between > best) { best = between; thresh = t; }
  }
  return thresh;
}

// Tile-based Otsu binarization — applies per-tile thresholds for images where
// global Otsu fails (colored text on colored backgrounds, mixed lighting).
// Returns a binary gray image (0 = ink, 255 = background).
function _tileOtsuGray(gray, w, h, tileSize = 64) {
  const out    = new Uint8Array(w * h);
  const tilesX = Math.ceil(w / tileSize);
  const tilesY = Math.ceil(h / tileSize);
  for (let ty = 0; ty < tilesY; ty++) {
    for (let tx = 0; tx < tilesX; tx++) {
      const x0 = tx * tileSize, x1 = Math.min(w, x0 + tileSize);
      const y0 = ty * tileSize, y1 = Math.min(h, y0 + tileSize);
      const tileLen = (x1 - x0) * (y1 - y0);
      if (!tileLen) continue;
      const tile = new Uint8Array(tileLen);
      let idx = 0;
      for (let y = y0; y < y1; y++)
        for (let x = x0; x < x1; x++)
          tile[idx++] = gray[y * w + x];
      const thresh = _otsuThreshold(tile);
      for (let y = y0; y < y1; y++)
        for (let x = x0; x < x1; x++)
          out[y * w + x] = gray[y * w + x] <= thresh ? 0 : 255;
    }
  }
  return out;
}

// =============================================================================
// Stage 3 — Deskew via projection profile
// Searches angles [-15°, +15°] in 1° steps on a downsampled image.
// Returns the angle (degrees) that maximises horizontal line variance.
// =============================================================================
function _computeDeskewAngle(gray, w, h) {
  // Downsample to speed up search
  const SCALE = 0.25;
  const sw = Math.max(1, Math.round(w * SCALE));
  const sh = Math.max(1, Math.round(h * SCALE));
  const small = new Uint8Array(sw * sh);
  for (let y = 0; y < sh; y++) {
    for (let x = 0; x < sw; x++) {
      small[y * sw + x] = gray[Math.round(y / SCALE) * w + Math.round(x / SCALE)];
    }
  }

  const thresh = _otsuThreshold(small);
  let bestVariance = -1, bestAngle = 0;

  for (let deg = -15; deg <= 15; deg++) {
    const rad = (deg * Math.PI) / 180;
    const cos = Math.cos(rad), sin = Math.sin(rad);
    const cx = sw / 2, cy = sh / 2;
    const profile = new Int32Array(sh);

    for (let y = 0; y < sh; y++) {
      for (let x = 0; x < sw; x++) {
        if (small[y * sw + x] > thresh) continue; // skip light pixels
        const ry = Math.round((x - cx) * sin + (y - cy) * cos + cy);
        if (ry >= 0 && ry < sh) profile[ry]++;
      }
    }

    const mean = profile.reduce((a, b) => a + b, 0) / sh;
    const variance = profile.reduce((s, v) => s + (v - mean) ** 2, 0) / sh;
    if (variance > bestVariance) { bestVariance = variance; bestAngle = deg; }
  }

  return bestAngle;
}

function _rotateCanvas(src, angleDeg) {
  if (angleDeg === 0) return src;
  const rad = (angleDeg * Math.PI) / 180;
  const cos = Math.abs(Math.cos(rad)), sin = Math.abs(Math.sin(rad));
  const nw  = Math.ceil(src.width * cos + src.height * sin);
  const nh  = Math.ceil(src.width * sin + src.height * cos);
  const dst = _createCanvas(nw, nh);
  const ctx = dst.getContext('2d');
  ctx.translate(nw / 2, nh / 2);
  ctx.rotate(rad);
  ctx.drawImage(src, -src.width / 2, -src.height / 2);
  return dst;
}

// =============================================================================
// BFS connected-component labeler for binarized (0=ink, 255=bg) images.
// Returns [{minX, maxX, minY, maxY, pixels}] — one entry per ink blob.
// Used by _detectTextRegions to isolate whole character shapes; avoids the
// column-projection split that breaks round letters (o, e, a, n) at counters.
// =============================================================================
function _labelComponents(binary, w, h) {
  const visited = new Uint8Array(w * h);
  const comps   = [];
  const queue   = new Int32Array(w * h); // pre-allocated — avoids GC churn

  for (let start = 0; start < binary.length; start++) {
    if (binary[start] !== 0 || visited[start]) continue; // bg or already labeled

    let head = 0, tail = 0;
    queue[tail++] = start;
    visited[start] = 1;
    let minX = w, maxX = 0, minY = h, maxY = 0, pixels = 0;

    while (head < tail) {
      const idx = queue[head++];
      const x = idx % w, y = (idx / w) | 0;
      if (x < minX) minX = x; if (x > maxX) maxX = x;
      if (y < minY) minY = y; if (y > maxY) maxY = y;
      pixels++;
      if (x > 0   && binary[idx-1] === 0 && !visited[idx-1]) { visited[idx-1]=1; queue[tail++]=idx-1; }
      if (x < w-1 && binary[idx+1] === 0 && !visited[idx+1]) { visited[idx+1]=1; queue[tail++]=idx+1; }
      if (y > 0   && binary[idx-w] === 0 && !visited[idx-w]) { visited[idx-w]=1; queue[tail++]=idx-w; }
      if (y < h-1 && binary[idx+w] === 0 && !visited[idx+w]) { visited[idx+w]=1; queue[tail++]=idx+w; }
    }
    comps.push({ minX, maxX, minY, maxY, pixels });
  }
  return comps;
}

// Split a region that is too wide to be a single character (likely 2+ glyphs merged).
// Finds local minima of the vertical projection and slices there.
function _splitWideRegion(region, binary, w) {
  const rW = region.x1 - region.x0;
  const rH = region.y1 - region.y0;
  if (rW / rH <= 1.3) return [region];  // CHANGED from 2.5

  // ── x-height zone projection (avoids baseline serif touching) ──
  const yStart = Math.floor(region.y0 + rH * 0.25);
  const yEnd   = Math.floor(region.y1 - rH * 0.25);
  
  const vProj = new Int32Array(rW);
  for (let y = yStart; y < yEnd; y++)
    for (let x = region.x0; x < region.x1; x++)
      if (binary[y * w + x] === 0) vProj[x - region.x0]++;

  if (vProj.length === 0) return [region];

  // ── Adaptive threshold: median-based, not peak-based ──
  const sorted = [...vProj].sort((a, b) => a - b);
  const median = sorted[Math.floor(sorted.length / 2)];
  const valleyThresh = Math.max(1, median * 0.5);  // 50% of median

  // ── Find continuous valley regions ──
  const valleys = [];
  let inValley = false, vStart = 0;
  for (let x = 0; x < rW; x++) {
    if (vProj[x] <= valleyThresh) {
      if (!inValley) { inValley = true; vStart = x; }
    } else {
      if (inValley) {
        inValley = false;
        if (x - vStart >= 2) valleys.push({ start: vStart, end: x });
      }
    }
  }
  if (inValley && rW - vStart >= 2) valleys.push({ start: vStart, end: rW });

  if (!valleys.length) return [region];

  // ── Split at valley centers ──
  const cuts = [region.x0];
  for (const v of valleys) {
    const cutX = region.x0 + Math.round((v.start + v.end) / 2);
    // Minimum char width: 20% of height
    if (cutX > cuts[cuts.length - 1] + rH * 0.2) cuts.push(cutX);
  }
  cuts.push(region.x1);

  if (cuts.length <= 2) return [region];

  const parts = [];
  for (let i = 0; i < cuts.length - 1; i++) {
    const x0 = cuts[i], x1 = cuts[i + 1];
    const subW = x1 - x0;
    if (subW < rH * 0.18) continue;

    let dark = 0;
    for (let y = region.y0; y < region.y1; y++)
      for (let x = x0; x < x1; x++)
        if (binary[y * w + x] === 0) dark++;
    const density = dark / (subW * rH);

    parts.push({ x0, y0: region.y0, x1, y1: region.y1, density });
  }
  return parts.length ? parts : [region];
}

// =============================================================================
// Stage 4 — Text region detection (connected-component based)
//
// Algorithm:
//   a. Binarise with Otsu threshold
//   b. Horizontal projection profile → locate text line bands
//   c. Within each band, BFS connected-component labeling → one blob per char
//   d. Filter blobs by aspect ratio and density; split wide (merged) blobs
//   e. Return up to MAX_GLYPHS bounding boxes, sorted by quality score
// =============================================================================
function _detectTextRegions(gray, w, h, threshMult = 1.0) {
  // ── VERSION MARKER — confirms CC-v3 code is running ──────────────────────
  console.log(`[CC-v3] _detectTextRegions mult=${threshMult} imageSize=${w}×${h}`);

  const otsu   = _otsuThreshold(gray);
  const thresh = Math.min(254, Math.max(1, Math.round(otsu * threshMult)));
  let darkCount = 0;
  for (let i = 0; i < gray.length; i++) if (gray[i] < thresh) darkCount++;
  const invert = darkCount > gray.length * 0.5;
  console.log(`[CC-v3] otsu=${otsu} thresh=${thresh} invert=${invert} darkFrac=${(darkCount/gray.length).toFixed(3)}`);

  const binary = new Uint8Array(w * h);
  for (let i = 0; i < gray.length; i++) {
    const isDark = invert ? gray[i] >= thresh : gray[i] < thresh;
    binary[i] = isDark ? 0 : 255;
  }

  // Horizontal projection: count dark pixels per row
  const hProj = new Int32Array(h);
  for (let y = 0; y < h; y++)
    for (let x = 0; x < w; x++)
      if (binary[y * w + x] === 0) hProj[y]++;

  // Find text line bands (rows with ≥ 1% dark pixels)
  const darkPerRow = Math.max(1, w * 0.01);
  const bands = [];
  let inBand = false, bandStart = 0;
  for (let y = 0; y < h; y++) {
    const isDark = hProj[y] >= darkPerRow;
    if (isDark && !inBand)  { inBand = true; bandStart = y; }
    if (!isDark && inBand)  {
      inBand = false;
      const bandH = y - bandStart;
      const kept = bandH >= MIN_GLYPH_H && bandH <= MAX_GLYPH_H;
      console.log(`[CC-v3] band y=${bandStart}-${y} h=${bandH} ${kept ? '✓ KEPT' : `✗ SKIP (${bandH < MIN_GLYPH_H ? 'too short' : 'too tall'})`}`);
      if (kept) bands.push({ y0: bandStart, y1: y });
    }
  }
  if (inBand) {
    const bandH = h - bandStart;
    const kept = bandH >= MIN_GLYPH_H && bandH <= MAX_GLYPH_H;
    console.log(`[CC-v3] band y=${bandStart}-${h} h=${bandH} ${kept ? '✓ KEPT (edge)' : '✗ SKIP (edge)'}`);
    if (kept) bands.push({ y0: bandStart, y1: h });
  }
  console.log(`[CC-v3] total bands: ${bands.length}  MIN_GLYPH_H=${MIN_GLYPH_H} MAX_GLYPH_H=${MAX_GLYPH_H} darkPerRow=${darkPerRow}`);

  const regions = [];

  for (let bi = 0; bi < bands.length; bi++) {
    const band  = bands[bi];
    const bandH = band.y1 - band.y0;

    const bandBin = new Uint8Array(w * bandH);
    for (let y = 0; y < bandH; y++)
      for (let x = 0; x < w; x++)
        bandBin[y * w + x] = binary[(band.y0 + y) * w + x];

    const comps = _labelComponents(bandBin, w, bandH);
    console.log(`[CC-v3] band[${bi}] y=${band.y0}-${band.y1} h=${bandH}: ${comps.length} CC components`);

    let passedInBand = 0;
    for (const c of comps) {
      const cW = c.maxX - c.minX + 1;
      const cH = c.maxY - c.minY + 1;
      const region = {
        x0: c.minX,
        y0: band.y0 + c.minY,
        x1: c.minX + cW,
        y1: band.y0 + c.minY + cH,
        density: c.pixels / (cW * cH),
      };

      for (const r of _splitWideRegion(region, binary, w)) {
        const rW = r.x1 - r.x0, rH = r.y1 - r.y0;
        const nat = rW / rH;
        let skipReason = null;
        if (nat < MIN_CHAR_RATIO)          skipReason = `nat=${nat.toFixed(3)}<MIN(${MIN_CHAR_RATIO})`;
        else if (nat > MAX_CHAR_RATIO)     skipReason = `nat=${nat.toFixed(3)}>MAX(${MAX_CHAR_RATIO})`;
        else if (r.density < 0.10)         skipReason = `density=${r.density.toFixed(3)}<0.10`;
        else if (r.density > 0.85)         skipReason = `density=${r.density.toFixed(3)}>0.85`;

        if (skipReason) {
          console.log(`  [CC-v3] ✗ SKIP [${r.x0},${r.y0}→${r.x1},${r.y1}] w=${rW} h=${rH} ${skipReason}`);
        } else {
          console.log(`  [CC-v3] ✓ PASS [${r.x0},${r.y0}→${r.x1},${r.y1}] w=${rW} h=${rH} nat=${nat.toFixed(3)} density=${r.density.toFixed(3)}`);
          regions.push(r);
          passedInBand++;
        }
      }
    }
    console.log(`[CC-v3] band[${bi}] passed filter: ${passedInBand}/${comps.length} components`);
  }

  console.log(`[CC-v3] total regions before dedup: ${regions.length}`);

  // Sort by bounding-box area (largest = most likely a real character).
  // Area-based sort is weight-agnostic: Bold (density 0.65) and Light (density 0.20) both
  // win over sub-character fragments regardless of weight. Old density-proximity sort
  // penalised Bold/Black by always ranking them behind Regular-weight noise.
  regions.sort((a, b) => {
    const aArea = (a.x1 - a.x0) * (a.y1 - a.y0);
    const bArea = (b.x1 - b.x0) * (b.y1 - b.y0);
    return bArea - aArea;
  });
  const picked = [];
  outer: for (const r of regions) {
    for (const p of picked) {
      const ox = Math.min(r.x1, p.x1) - Math.max(r.x0, p.x0);
      const oy = Math.min(r.y1, p.y1) - Math.max(r.y0, p.y0);
      if (ox > 0 && oy > 0) {
        console.log(`  [CC-v3] dedup skip [${r.x0},${r.y0}→${r.x1},${r.y1}] overlaps [${p.x0},${p.y0}→${p.x1},${p.y1}]`);
        continue outer;
      }
    }
    picked.push(r);
    if (picked.length >= MAX_GLYPHS) break;
  }

  console.log(`[CC-v3] final picked: ${picked.length} regions (MAX_GLYPHS=${MAX_GLYPHS})`);
  for (const r of picked) {
    const nat = (r.x1-r.x0)/(r.y1-r.y0);
    console.log(`  [CC-v3] → [${r.x0},${r.y0}→${r.x1},${r.y1}] w=${r.x1-r.x0} h=${r.y1-r.y0} nat=${nat.toFixed(3)} density=${r.density.toFixed(3)}`);
  }

  const avgDensity = picked.length
    ? picked.reduce((s, r) => s + r.density, 0) / picked.length
    : 0;
  return { regions: picked, invert, avgDensity, binary };
}

// =============================================================================
// Stage 5 — Crop region from canvas, normalize to GLYPH_SIZE × GLYPH_SIZE
// Returns an ImageData at GLYPH_SIZE resolution (always dark-ink on white bg)
// =============================================================================
function _cropAndNormalize(region, binary, srcW) {
  const { x0, y0, x1, y1 } = region;
  const rw = x1 - x0, rh = y1 - y0;

  // Render binary mask as clean black-on-white at native resolution.
  // This strips ALL source image artifacts (color, glow, shadow, compression)
  // so CanvasDNA receives the same kind of image the database was built from.
  const tmp = _createCanvas(rw, rh);
  const tCtx = tmp.getContext('2d', { willReadFrequently: true });
  tCtx.fillStyle = '#ffffff';
  tCtx.fillRect(0, 0, rw, rh);
  const tId = tCtx.getImageData(0, 0, rw, rh);
  for (let y = 0; y < rh; y++) {
    for (let x = 0; x < rw; x++) {
      if (binary[(y0 + y) * srcW + (x0 + x)] === 0) {
        const i = (y * rw + x) * 4;
        tId.data[i] = tId.data[i + 1] = tId.data[i + 2] = 0;
        tId.data[i + 3] = 255;
      }
    }
  }
  tCtx.putImageData(tId, 0, 0);

  // Scale to GLYPH_SIZE with padding, using canvas smoothing for antialiasing
  const dst = _createCanvas(GLYPH_SIZE, GLYPH_SIZE);
  const ctx = dst.getContext('2d', { willReadFrequently: true });
  ctx.fillStyle = '#ffffff';
  ctx.fillRect(0, 0, GLYPH_SIZE, GLYPH_SIZE);
  const pad   = Math.floor(GLYPH_SIZE * 0.08);
  const inner = GLYPH_SIZE - pad * 2;
  ctx.imageSmoothingEnabled = true;
  ctx.imageSmoothingQuality = 'high';
  ctx.drawImage(tmp, pad, pad, inner, inner);
  return ctx.getImageData(0, 0, GLYPH_SIZE, GLYPH_SIZE);
}

// =============================================================================
// Stage 6 — Heuristic character label assignment
// Assigns a character label to a glyph based on its shape metrics.
// Imperfect but sufficient to route into the right LSH bucket for matching.
// =============================================================================
// Extract a 64×64 binarized thumbnail from a glyph region of the binary image.
// region is already a tight bounding box (from CC labeling); we scale it directly to 64×64.
// Returns Uint8Array(4096): 0=ink, 255=background — same convention as IMGDB2.
function _glyphToThumbnail(binary, region, w) {
  const rW = region.x1 - region.x0;
  const rH = region.y1 - region.y0;
  const thumb = new Uint8Array(4096).fill(255);
  if (rW <= 0 || rH <= 0) return thumb;
  for (let ty = 0; ty < 64; ty++) {
    for (let tx = 0; tx < 64; tx++) {
      const sx = region.x0 + Math.round((tx / 63) * (rW - 1));
      const sy = region.y0 + Math.round((ty / 63) * (rH - 1));
      thumb[ty * 64 + tx] = binary[sy * w + sx] === 0 ? 0 : 255;
    }
  }
  return thumb;
}

function _guessCharacter(preMetrics, region) {
  const { yBalance, density, quadrantSw, quadrantSe,
          quadrantNw, quadrantNe } = preMetrics;

  // nat = natural region aspect ratio (width/height BEFORE 128×128 normalisation).
  // inkRatio = aspect of ink bbox INSIDE the normalised square — distorted by stretch,
  // so it cannot be used as a primary shape discriminator.
  const natW = region ? (region.x1 - region.x0) : 0;
  const natH = region ? (region.y1 - region.y0) : 1;
  const nat  = natW / natH;

  const symH = Math.abs((quadrantNw + quadrantSw) - (quadrantNe + quadrantSe));
  const symV = Math.abs((quadrantNw + quadrantNe) - (quadrantSw + quadrantSe));

  // ── Stems: I, l, 1 ──────────────────────────────────────────────────────
  if (nat < 0.22) return 'I';

  // ── Narrow (t, f, i, r) ─────────────────────────────────────────────────
  if (nat < 0.45) {
    if (yBalance < 100) return 'f';    // top-heavy narrow → f or t
    return 'I';                         // i, r, or thin serif stem
  }

  // ── Wide: W, M ───────────────────────────────────────────────────────────
  if (nat > 1.60) return 'W';
  if (nat > 1.25) return 'M';

  // ── Medium-narrow (0.45–0.75): c, e, s, n, d, b, h ─────────────────────

  // Descender below baseline → g, y, p, q
  if (yBalance > 155) return 'g';

  // Upper-only strokes: top-heavy, empty lower half → P, F, E
  if (yBalance < 108 && quadrantSe < 25 && nat < 0.80) return 'P';

  // Strong bilateral asymmetry + narrow → stem+bowl pair (d, b, h)
  if (symH > 55 && nat < 0.85) {
    if (quadrantNw < quadrantNe) return 'd';
    // b: bowl sits in lower half → ink centroid below midpoint (yBalance > 128)
    // h: arch connects at mid-height → centroid near/above midpoint (yBalance ≤ 128)
    return yBalance > 128 ? 'b' : 'h';
  }

  // Two-legged arch (h, n) — ceiling raised to 70 to catch bold strokes with higher symH
  if (symH < 70 && symV < 45 && yBalance > 112 && yBalance < 155 && quadrantNw > quadrantNe) {
    return nat < 0.68 ? 'h' : 'n';
  }

  // ── Squarish (0.45–1.25): O, C, a, e, R ─────────────────────────────────

  // Bilaterally symmetric → round bowl
  if (symH < 45 && symV < 50 && nat > 0.55) {
    // Fully closed (O, 0) vs open on right (C, c, e)
    return (quadrantNe > 45 && quadrantSe > 35) ? 'O' : 'C';
  }

  // Dense compact strokes → a
  if (density > 130 && nat < 0.90) return 'a';

  // ── Additional discriminators to reduce the 'R' catch-all ────────────────

  // Top-bar only + wide → T (strong top, thin bottom stem)
  if (symH < 40 && symV > 55 && yBalance < 120 && nat > 0.70) return 'T';

  // Bottom-convergence → V (more ink in top half than bottom)
  if (symH < 35 && symV > 40 && yBalance < 118 && nat > 0.80 && nat < 1.40) return 'V';

  // Strong left-heavy asymmetry + squarish → E or F (horizontal bars left side)
  if (symH > 50 && nat > 0.55 && nat < 1.1 && quadrantNe < 30 && quadrantSe < 30) return 'E';

  // Rightward lean with crossing diagonal → X or K
  if (symH > 35 && symV > 35 && nat > 0.65 && nat < 1.15 && density > 80) return 'X';

  return 'R';
}

// =============================================================================
// Aggregate style results across multiple glyph crops.
// Majority vote for italic; median CSS weight; median italic angle.
// =============================================================================
function _aggregateStyle(styles) {
  if (!styles.length) return { italic: false, italicAngle: 0, cssWeight: 400, weightLabel: 'Regular' };

  const italicVotes = styles.filter(s => s.italic).length;
  const italic      = italicVotes > styles.length / 2;

  const angles  = styles.map(s => s.italicAngle).sort((a, b) => a - b);
  const weights = styles.map(s => s.cssWeight).sort((a, b) => a - b);
  const mid     = Math.floor(styles.length / 2);

  const italicAngle = angles[mid];
  const cssWeight   = weights[mid];

  const WEIGHT_LABELS = {
    100: 'Thin', 200: 'ExtraLight', 300: 'Light', 400: 'Regular',
    500: 'Medium', 600: 'SemiBold', 700: 'Bold', 800: 'ExtraBold', 900: 'Black',
  };

  return { italic, italicAngle, cssWeight, weightLabel: WEIGHT_LABELS[cssWeight] };
}

// =============================================================================
// Main pipeline class
// =============================================================================
class ImagePipeline {
  /**
   * Analyze an image and extract font identification metrics.
   *
   * @param {string|File|Blob|HTMLImageElement|HTMLCanvasElement} imageSource
   * @returns {Promise<{
   *   metrics: JsPixelMetrics[],   // feed into identifyFromPixelMetrics()
   *   quality: number,             // sharpness score (higher = better)
   *   deskewAngle: number,         // degrees the image was rotated
   *   regionCount: number,         // text regions found before capping
   *   warning: string|null         // user-facing quality warning if any
   * }>}
   */
  async analyze(imageSource) {
    // ── Stage 1: load image ──────────────────────────────────────────────
    const canvas = await this._loadToCanvas(imageSource);
    const w = canvas.width, h = canvas.height;
    const ctx = canvas.getContext('2d', { willReadFrequently: true });

    // ── Stage 2: quality check + upscale if needed ───────────────────────
    let imgData   = ctx.getImageData(0, 0, w, h);
    let gray      = _toGrayscaleData(imgData.data, w, h);
    const quality = _computeSharpness(gray, w, h);
    let warning   = null;

    let workCanvas = canvas;
    let workW = w, workH = h;

    if (quality < SHARP_THRESHOLD && (w < 800 || h < 600)) {
      // Bicubic 2× upscale via canvas (no model needed, fast)
      workCanvas = _createCanvas(w * 2, h * 2);
      const upCtx = workCanvas.getContext('2d', { willReadFrequently: true });
      upCtx.imageSmoothingEnabled  = true;
      upCtx.imageSmoothingQuality  = 'high';
      upCtx.drawImage(canvas, 0, 0, w * 2, h * 2);
      workW = w * 2; workH = h * 2;
      imgData = upCtx.getImageData(0, 0, workW, workH);
      gray    = _toGrayscaleData(imgData.data, workW, workH);
      const qualityAfter = _computeSharpness(gray, workW, workH);
      if (qualityAfter < SHARP_THRESHOLD * 0.5) {
        warning = 'Image quality is low — results may be inaccurate. Try a higher-resolution source.';
      }
    }

    // ── Stage 3: deskew ──────────────────────────────────────────────────
    const deskewAngle = _computeDeskewAngle(gray, workW, workH);
    let straightCanvas = workCanvas;
    if (Math.abs(deskewAngle) >= 1) {
      straightCanvas = _rotateCanvas(workCanvas, -deskewAngle);
      workW = straightCanvas.width;
      workH = straightCanvas.height;
      const sCtx = straightCanvas.getContext('2d', { willReadFrequently: true });
      imgData = sCtx.getImageData(0, 0, workW, workH);
      gray    = _toGrayscaleData(imgData.data, workW, workH);
    }

    // ── Stage 4: multi-threshold text region detection ───────────────────
    // Two-pass selection:
    //   Pass 1 (preferred): levels where avgDensity ≤ 0.52 (glow not included).
    //                       Score = goodAR fraction × 10 − |density−0.35|.
    //   Pass 2 (fallback):  if pass 1 finds nothing, use pass-1 formula without cap.
    // "goodAR" = regions whose width/height ratio is in [0.35, 1.50] (single char).
    const THRESH_LEVELS   = [0.20, 0.28, 0.38, 0.50, 0.65, 0.80, 1.00, 1.20];
    const DENSITY_CAP     = 0.96; // ink > 88% of bbox = solid block, not a glyph (ExtraBold ≈ 45–60%, Black ≈ 65–82%)
    let bestRegions = null, bestBinary = null, bestScore = -Infinity, bestMult = 1.00;
    let _bestDensity = 0;

    const _otsuVal  = _otsuThreshold(gray);
    const _attempts = [];

    for (const mult of THRESH_LEVELS) {
      const attempt = _detectTextRegions(gray, workW, workH, mult);
      _attempts.push({ mult, attempt });
      let goodAR = 0;
      for (const r of attempt.regions) {
        const ar = (r.x1 - r.x0) / (r.y1 - r.y0);
        if (ar >= 0.12 && ar <= 3.0) goodAR++;
      }
      console.log(`[Intellifont SWEEP] mult=${mult} otsu=${_otsuVal} thresh=${Math.round(_otsuVal*mult)} regions=${attempt.regions.length} density=${attempt.avgDensity.toFixed(3)} goodAR=${goodAR}/${attempt.regions.length}`);
    }

    // Pass 1: density-capped selection (preferred — glow excluded).
    // Score = goodAR fraction × 10 only (no density penalty — bold fonts have density 0.45–0.55
    // and were previously penalised by |density−0.35|, causing wrong threshold selection).
    for (const { mult, attempt } of _attempts) {
      if (!attempt.regions.length || attempt.avgDensity > DENSITY_CAP) continue;
      let goodAR = 0;
      for (const r of attempt.regions) {
        const ar = (r.x1 - r.x0) / (r.y1 - r.y0);
        if (ar >= 0.12 && ar <= 3.0) goodAR++;
      }
      const goodFrac  = goodAR / attempt.regions.length;
      const qualScore = goodFrac * 10 + attempt.avgDensity;
      if (qualScore > bestScore) {
        bestScore = qualScore; bestRegions = attempt.regions;
        bestBinary = attempt.binary; bestMult = mult; _bestDensity = attempt.avgDensity;
      }
    }

    // Pass 2 fallback: if all levels exceeded density cap, use uncapped scoring
    if (!bestRegions) {
      for (const { mult, attempt } of _attempts) {
        if (!attempt.regions.length) continue;
        let goodAR = 0;
        for (const r of attempt.regions) {
          const ar = (r.x1 - r.x0) / (r.y1 - r.y0);
          if (ar >= 0.35 && ar <= 1.50) goodAR++;
        }
        const goodFrac  = goodAR / attempt.regions.length;
        const qualScore = goodFrac * 10 + attempt.avgDensity;
        if (qualScore > bestScore) {
          bestScore = qualScore; bestRegions = attempt.regions;
          bestBinary = attempt.binary; bestMult = mult; _bestDensity = attempt.avgDensity;
        }
      }
    }

    // Pass 3 fallback: tile-based Otsu for colored-bg / mixed-lighting images
    // where global Otsu produces a single-class histogram (no ink/bg separation).
    if (!bestRegions) {
      const adaptGray = _tileOtsuGray(gray, workW, workH, 64);
      const attempt   = _detectTextRegions(adaptGray, workW, workH, 1.0);
      if (attempt.regions.length > 0) {
        bestRegions = attempt.regions;
        bestBinary  = attempt.binary;
        bestMult    = 1.0;
        _bestDensity = attempt.avgDensity;
        console.log(`[Intellifont] Tile-Otsu fallback: ${bestRegions.length} regions found`);
      }
    }

    if (!bestRegions) {
      return { metrics: [], quality, deskewAngle, regionCount: 0,
        warning: 'No text regions detected. Ensure the image contains printed text.' };
    }

    console.log(`[Intellifont] Best threshold: Otsu×${bestMult} (thresh=${Math.round(_otsuVal*bestMult)}), density=${_bestDensity.toFixed(3)}, score=${bestScore.toFixed(3)}`);
    const regions = bestRegions;
    const binary  = bestBinary;

    // ── Stage 4b: split merged multi-character regions ───────────────────
    function _splitRegion(region, bin, imgW) {
      const rW = region.x1 - region.x0;
      const rH = region.y1 - region.y0;
    
      if (rW <= rH * 1.3) return [region];  // CHANGED from 1.5
    
      // x-height zone projection
      const yStart = Math.floor(region.y0 + rH * 0.25);
      const yEnd   = Math.floor(region.y1 - rH * 0.25);
      
      const vProj = new Int32Array(rW);
      for (let y = yStart; y < yEnd; y++)
        for (let x = region.x0; x < region.x1; x++)
          if (bin[y * imgW + x] === 0) vProj[x - region.x0]++;
    
      let peak = 0;
      for (let i = 0; i < rW; i++) if (vProj[i] > peak) peak = vProj[i];
      if (peak < 3) return [region];
    
      // Adaptive: median-based threshold
      const sorted = [...vProj].sort((a, b) => a - b);
      const median = sorted[Math.floor(sorted.length / 2)];
      const valleyThresh = Math.max(1, median * 0.5);
    
      const splits = [];
      let inValley = false, valleyStart = 0;
      for (let i = 0; i <= rW; i++) {
        const isValley = i < rW && vProj[i] <= valleyThresh;
        if (isValley && !inValley)  { inValley = true; valleyStart = i; }
        if (!isValley && inValley)  {
          inValley = false;
          if (i - valleyStart >= 2)
            splits.push(region.x0 + Math.round((valleyStart + i - 1) / 2));
        }
      }
    
      if (!splits.length) return [region];
    
      const bounds = [region.x0, ...splits, region.x1];
      const result = [];
      for (let i = 0; i < bounds.length - 1; i++) {
        const x0 = bounds[i], x1 = bounds[i + 1];
        const subW = x1 - x0;
        if (subW < rH * 0.18) continue;
        let dc = 0;
        for (let y = region.y0; y < region.y1; y++)
          for (let x = x0; x < x1; x++)
            if (bin[y * imgW + x] === 0) dc++;
        result.push({ x0, y0: region.y0, x1, y1: region.y1, density: dc / (subW * rH) });
      }
      return result.length ? result : [region];
    }

    const splitRegions = [];
    for (const r of regions) {
      const parts = _splitRegion(r, binary, workW);
      if (parts.length > 1)
        console.log(`[Intellifont SPLIT] region w=${r.x1-r.x0} h=${r.y1-r.y0} → ${parts.length} parts`);
      for (const p of parts) splitRegions.push(p);
    }
    const finalRegions = splitRegions.slice(0, MAX_GLYPHS);

    // ── Stage 5 + 6: crop → normalize → extract metrics + style ─────────
    const { analyzeImageData, analyzeStyle } = CanvasDNA;
    const metrics = [];
    const styles  = [];

    for (const region of finalRegions) {
      const croppedData = _cropAndNormalize(region, binary, workW);
      // Quick pre-pass to guess character label from shape
      const pre = analyzeImageData(croppedData, GLYPH_SIZE, '?');
      if (!pre) continue;
      const character = _guessCharacter(pre, region);
      // const character = '?';
      const m = analyzeImageData(croppedData, GLYPH_SIZE, character);
      if (!m) continue;

      // ── DEBUG: show extracted glyph in console ──
      const _dbgC = _createCanvas(GLYPH_SIZE, GLYPH_SIZE);
      _dbgC.getContext('2d').putImageData(croppedData, 0, 0);
      const _dbgUrl = _dbgC.toDataURL();
      const _qSymH = Math.abs((pre.quadrantNw + pre.quadrantSw) - (pre.quadrantNe + pre.quadrantSe));
      const _qSymV = Math.abs((pre.quadrantNw + pre.quadrantNe) - (pre.quadrantSw + pre.quadrantSe));
      const _natW = region.x1 - region.x0, _natH = region.y1 - region.y0;
      console.log(
        `[Intellifont DBG] glyph#${metrics.length} → char='${character}' | ` +
        `nat=${(_natW/_natH).toFixed(3)} (${_natW}×${_natH}px) | ` +
        `cdnaAR=${(pre.aspectRatio/64).toFixed(2)} density=${pre.density} yBal=${pre.yBalance} | ` +
        `NW=${pre.quadrantNw} NE=${pre.quadrantNe} SW=${pre.quadrantSw} SE=${pre.quadrantSe} | ` +
        `symH=${_qSymH} symV=${_qSymV} | region=[${region.x0},${region.y0}→${region.x1},${region.y1}]`
      );
      console.log('%c      ', `background:url(${_dbgUrl}) no-repeat center/contain;padding:40px 55px;outline:1px solid #555`);
      // ── END DEBUG ──

      // Attach 16×16 thumbnail for IMGDB1 visual matching
      m.thumbnail = Array.from(_glyphToThumbnail(binary, region, workW));
      metrics.push(m);
      styles.push(analyzeStyle(croppedData, GLYPH_SIZE));
    }

    // Aggregate style across all glyphs (majority vote for italic, median for weight)
    const style = _aggregateStyle(styles);

    return { metrics, style, quality, deskewAngle, regionCount: regions.length, warning };
  }

  // ── Load any image source onto a canvas ─────────────────────────────────
  async _loadToCanvas(src) {
    if (src instanceof HTMLCanvasElement) return src;

    const img = await this._toImage(src);
    const c   = _createCanvas(img.naturalWidth || img.width, img.naturalHeight || img.height);
    c.getContext('2d').drawImage(img, 0, 0);
    return c;
  }

  _toImage(src) {
    return new Promise((resolve, reject) => {
      if (src instanceof HTMLImageElement && src.complete) return resolve(src);
      const img = new Image();
      img.crossOrigin = 'anonymous';
      img.onload  = () => resolve(img);
      img.onerror = () => {
        // CORS load failed. Check if the image is accessible without CORS
        // (e.g. Google Lens, authenticated URLs) to give a helpful message.
        const probe = new Image();
        probe.onload  = () => reject(new Error(
          'Image is blocked by CORS policy. Right-click the image → "Save image as…", open the saved file in a browser tab, then try again.'
        ));
        probe.onerror = () => reject(new Error(
          'Could not load image. The URL may require a login or the image no longer exists.'
        ));
        probe.src = typeof src === 'string' ? src : (src.src || '');
      };
      if (typeof src === 'string') {
        img.src = src;
      } else if (src instanceof File || src instanceof Blob) {
        img.src = URL.createObjectURL(src);
      } else if (src instanceof HTMLImageElement) {
        img.src = src.src;
      } else {
        reject(new Error('Unsupported image source type'));
      }
    });
  }
}

// =============================================================================
// AI Text Authenticity Analysis
//
// Determines whether text in an image was rendered by a real font engine or
// generated by an AI image model (Midjourney, DALL-E, Stable Diffusion, etc.).
//
// Core insight: real fonts are deterministically consistent — stroke widths,
// serif structure, and curve ratios are nearly identical across characters in
// the same typeface. AI-generated text varies chaotically at the pixel level
// even when it looks correct at a glance.
//
// @param {JsPixelMetrics[]} metrics - from ImagePipeline.analyze()
// @param {Array<{name|family, confidence}>} [fontMatches] - from identifyFromPixelMetrics()
// @returns {{ isAiGenerated, confidence, deviationScore, indicators, nearestFont }}
// =============================================================================

function _stdev(arr) {
  if (arr.length < 2) return 0;
  const mean = arr.reduce((a, b) => a + b, 0) / arr.length;
  return Math.sqrt(arr.reduce((s, v) => s + (v - mean) ** 2, 0) / arr.length);
}

function analyzeTextAuthenticity(metrics, fontMatches = null) {
  if (!metrics || metrics.length < 2) {
    return {
      isAiGenerated: false, confidence: 0, deviationScore: 0,
      indicators: ['insufficient_glyphs'],
      nearestFont: null, nearestFontConfidence: null,
    };
  }

  const indicators = [];
  let aiScore = 0;

  // ── Signal 1: Stroke width consistency ─────────────────────────────────────
  // Real fonts: stdev < 12  |  AI text: stdev often > 20
  const strokeWidths = metrics.map(m => m.strokeWidth ?? m.stroke_width ?? 0);
  const strokeStdev  = _stdev(strokeWidths);
  if (strokeStdev > 22)      { aiScore += 0.35; indicators.push('inconsistent_stroke_width'); }
  else if (strokeStdev > 15) { aiScore += 0.15; }

  // ── Signal 2: Serif structure consistency ──────────────────────────────────
  // Real fonts are uniformly serif OR sans.
  // AI text mixes serif/sans-like letterforms within a "word".
  const serifScores = metrics.map(m => m.serifScore ?? m.serif_score ?? 0);
  const serifStdev  = _stdev(serifScores);
  if (serifStdev > 30)      { aiScore += 0.30; indicators.push('inconsistent_serif_structure'); }
  else if (serifStdev > 20) { aiScore += 0.10; }

  // ── Signal 3: Ink density variance ─────────────────────────────────────────
  // Real fonts within the same weight: density variance is moderate.
  // AI text can have extreme density swings across neighbouring characters.
  const densities    = metrics.map(m => m.density ?? 0);
  const densityStdev = _stdev(densities);
  if (densityStdev > 40)    { aiScore += 0.20; indicators.push('inconsistent_density'); }

  // ── Signal 4: Curve ratio consistency ──────────────────────────────────────
  const curveRatios = metrics.map(m => m.curveRatio ?? m.curve_ratio ?? 0);
  const curveStdev  = _stdev(curveRatios);
  if (curveStdev > 35)      { aiScore += 0.15; indicators.push('inconsistent_curves'); }

  // ── Signal 5: Font DB match confidence (bonus, if available) ───────────────
  // Real rendered text matches a known font at > 0.70 confidence.
  // AI text often < 0.55 — it looks like a font but isn't one.
  if (fontMatches && fontMatches.length > 0) {
    const top = fontMatches[0];
    const topConf = top.confidence ?? 0;
    if (topConf < 0.50)      { aiScore += 0.35; indicators.push('no_font_match'); }
    else if (topConf < 0.65) { aiScore += 0.15; indicators.push('weak_font_match'); }
  }

  aiScore = Math.min(1.0, aiScore);
  const isAiGenerated = aiScore > 0.45;

  const top = fontMatches?.[0];
  return {
    isAiGenerated,
    confidence:            isAiGenerated ? aiScore : 1 - aiScore,
    deviationScore:        aiScore,
    indicators:            indicators.length ? indicators : ['consistent_metrics'],
    nearestFont:           top?.family ?? top?.name ?? null,
    nearestFontConfidence: top?.confidence ?? null,
  };
}

// =============================================================================
// Convenience wrapper — one-call API
// =============================================================================
async function analyzeImage(imageSource) {
  return new ImagePipeline().analyze(imageSource);
}

// =============================================================================
// Exports
// =============================================================================
const ImagePipelineModule = { ImagePipeline, analyzeImage, analyzeTextAuthenticity };

if (typeof module !== 'undefined' && module.exports) {
  module.exports = ImagePipelineModule;
} else if (typeof window !== 'undefined') {
  window.IntellifontImagePipeline = ImagePipelineModule;
}
})();
