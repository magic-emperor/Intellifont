(() => {
if (window.IntellifontMatcher) return;
'use strict';
/**
 * Intellifont — browser-compatible glyph database loader + font matcher.
 *
 * Loads glyph_signatures.gz from the extension assets, decompresses it
 * using the native DecompressionStream API (Chrome 80+, no WASM needed),
 * parses the bincode format, and runs weighted L1 distance matching.
 *
 * Exposes: window.IntellifontMatcher = { identify }
 */

// ── Database loader ──────────────────────────────────────────────────────────

let _dbCache = null;
let _dbLoading = null;

async function _loadDatabase() {
  if (_dbCache) return _dbCache;
  if (_dbLoading) return _dbLoading;

  _dbLoading = (async () => {
    // Prefer pixel-rendered DB (apples-to-apples with CanvasDNA queries)
    const pixelDb = await _tryLoadPixelDb();
    if (pixelDb) {
      _dbCache = pixelDb;
      _dbLoading = null;
      return _dbCache;
    }

    // Fall back to vector-outline DB
    const url      = chrome.runtime.getURL('data/glyph_signatures.gz');
    const response = await fetch(url);
    if (!response.ok) throw new Error(`Failed to fetch database: ${response.status}`);

    const compressed = await response.arrayBuffer();
    const raw        = new Uint8Array(compressed);

    const magic = String.fromCharCode(...raw.slice(0, 8));
    if (magic !== 'GLYPHDB1') throw new Error('Invalid database format');

    const gzipPayload = raw.slice(8);
    const decompressed = await _decompressGzip(gzipPayload.buffer);

    _dbCache = _parseBincode(decompressed);
    _dbLoading = null;
    return _dbCache;
  })();

  return _dbLoading;
}

async function _decompressGzip(compressedBuffer) {
  const ds     = new DecompressionStream('gzip');
  const writer = ds.writable.getWriter();
  writer.write(new Uint8Array(compressedBuffer));
  writer.close();

  const chunks = [];
  const reader = ds.readable.getReader();
  for (;;) {
    const { done, value } = await reader.read();
    if (done) break;
    chunks.push(value);
  }

  const totalLen = chunks.reduce((s, c) => s + c.length, 0);
  const out      = new Uint8Array(totalLen);
  let off        = 0;
  for (const chunk of chunks) { out.set(chunk, off); off += chunk.length; }
  return out.buffer;
}

// ── IMGDB1 loader — 16×16 thumbnail database (preferred — direct visual comparison) ─

let _imgDbCache = null;
let _imgDbLoading = null;

async function _tryLoadImgDb() {
  if (_imgDbCache) return _imgDbCache;
  if (_imgDbLoading) return _imgDbLoading;
  _imgDbLoading = (async () => {
    try {
      const url      = chrome.runtime.getURL('data/img_signatures.gz');
      const response = await fetch(url);
      if (!response.ok) { _imgDbLoading = null; return null; }
      const compressed = await response.arrayBuffer();
      const raw        = new Uint8Array(compressed);
      const magic      = String.fromCharCode(...raw.slice(0, 8));
      if (magic !== 'IMGDB2\0\0') { _imgDbLoading = null; return null; }
      const decompressed = await _decompressGzip(raw.slice(8).buffer);
      _imgDbCache   = _parseImgDb(decompressed);
      _imgDbLoading = null;
      return _imgDbCache;
    } catch (_) {
      _imgDbLoading = null;
      return null;
    }
  })();
  return _imgDbLoading;
}

function _parseImgDb(buffer) {
  const view = new DataView(buffer);
  let off = 0;
  const u8  = () => view.getUint8(off++);
  const u16 = () => { const v = view.getUint16(off, true); off += 2; return v; };
  const u32 = () => { const v = view.getUint32(off, true); off += 4; return v; };
  const str = (len) => { const b = new Uint8Array(buffer, off, len); off += len; return new TextDecoder().decode(b); };

  const fontCount = u32();
  const fonts = [];
  for (let i = 0; i < fontCount; i++) {
    const family    = str(u16());
    const hasSub    = u8();
    const subfamily = hasSub ? str(u8()) : null;
    const charCount = u8();
    const sigs      = [];
    for (let s = 0; s < charCount; s++) {
      const ch    = String.fromCharCode(u8());
      const thumb = new Uint8Array(buffer, off, 4096); // zero-copy view
      off += 4096;
      sigs.push({ ch, thumb });
    }
    fonts.push({ family, subfamily, sigs });
  }
  return fonts;
}

// ── PIXELDB1 loader — pixel-rendered signatures (preferred over bincode DB) ───

async function _tryLoadPixelDb() {
  try {
    const url      = chrome.runtime.getURL('data/pixel_signatures.gz');
    const response = await fetch(url);
    if (!response.ok) return null;
    const compressed = await response.arrayBuffer();
    const raw        = new Uint8Array(compressed);
    const magic      = String.fromCharCode(...raw.slice(0, 8));
    if (magic !== 'PIXELDB1') return null;
    const gzipPayload   = raw.slice(8);
    const decompressed  = await _decompressGzip(gzipPayload.buffer);
    return _parsePixelDb(decompressed);
  } catch (_) {
    return null;
  }
}

function _parsePixelDb(buffer) {
  const view = new DataView(buffer);
  let off = 0;
  const u8  = () => view.getUint8(off++);
  const u16 = () => { const v = view.getUint16(off, true); off += 2; return v; };
  const u32 = () => { const v = view.getUint32(off, true); off += 4; return v; };
  const str = (len) => {
    const bytes = new Uint8Array(buffer, off, len);
    off += len;
    return new TextDecoder().decode(bytes);
  };

  const fontCount = u32();
  const fonts = [];
  for (let i = 0; i < fontCount; i++) {
    const family    = str(u16());
    const hasSub    = u8();
    const subfamily = hasSub ? str(u8()) : null;
    const sigCount  = u8();
    const sigs      = [];
    for (let s = 0; s < sigCount; s++) {
      const ch = String.fromCharCode(u8());
      sigs.push({
        ch,
        aspect_ratio: u8(), density:      u8(),
        quadrant_nw:  u8(), quadrant_ne:  u8(),
        quadrant_sw:  u8(), quadrant_se:  u8(),
        curve_ratio:  u8(), point_count:  u8(),
        x_balance:    u8(), y_balance:    u8(),
        stroke_width: u8(), serif_score:  u8(),
        feature_hash: 0,    reserved:     0,
      });
    }
    fonts.push({ family, subfamily, sigs });
  }
  return fonts;
}

// ── Bincode parser ───────────────────────────────────────────────────────────

function _parseBincode(buffer) {
  const view = new DataView(buffer);
  let off    = 0;

  const u8  = ()   => view.getUint8(off++);
  const u16 = ()   => { const v = view.getUint16(off, true); off += 2; return v; };
  const u32 = ()   => { const v = view.getUint32(off, true); off += 4; return v; };
  const u64 = ()   => {
    const lo = view.getUint32(off, true);
    const hi = view.getUint32(off + 4, true);
    off += 8;
    return lo + hi * 0x100000000;
  };
  const str = ()   => {
    const len   = u64();
    const bytes = new Uint8Array(buffer, off, len);
    off += len;
    return new TextDecoder().decode(bytes);
  };
  const opt = (fn) => u8() ? fn() : null;

  // DatabaseHeader
  u32(); u32(); u8(); u8(); u64(); u64(); u64(); // version, counts, offsets — skip

  // LshIndex — skip entirely (brute-force on 2k fonts is fast enough)
  const ntables = u64();
  for (let t = 0; t < ntables; t++) {
    const nbuckets = u64();
    for (let b = 0; b < nbuckets; b++) {
      const nids = u64();
      off += nids * 2;
    }
  }

  // FontEntry[]
  const nfonts = u64();
  const fonts  = [];
  for (let i = 0; i < nfonts; i++) {
    const family    = str();
    const subfamily = opt(str);
    const nsigs     = u64();
    const sigs      = [];
    for (let s = 0; s < nsigs; s++) {
      const ch = String.fromCharCode(u8());
      sigs.push({
        ch,
        aspect_ratio: u8(), density:      u8(),
        quadrant_nw:  u8(), quadrant_ne:  u8(),
        quadrant_sw:  u8(), quadrant_se:  u8(),
        curve_ratio:  u8(), point_count:  u8(),
        x_balance:    u8(), y_balance:    u8(),
        stroke_width: u8(), serif_score:  u8(),
        feature_hash: u16(),
        reserved:     u16(),
      });
    }
    fonts.push({ family, subfamily, sigs });
  }
  return fonts;
}

// ── Image similarity (IMGDB1 matching) ───────────────────────────────────────

function _imgSimilarity(queryThumb, dbThumb) {
  let diff = 0;
  for (let i = 0; i < 4096; i++) diff += Math.abs(queryThumb[i] - dbThumb[i]);
  return 1 - diff / (4096 * 255);
}

function _identifyByThumbnail(metrics, imgDb, limit) {
  const scores = new Map();

  for (const font of imgDb) {
    const key = `${font.family}||${font.subfamily || ''}`;
    let totalScore = 0, matched = 0;
    const matchedChars = [];

    for (const m of metrics) {
      const queryThumb = m.thumbnail;
      if (!queryThumb || queryThumb.length !== 4096) continue;

      // Always use best visual match across all chars — character labels from
      // _guessCharacter() are often wrong (H→I, m→h, b→f), so exact-label
      // lookup compares the query glyph against the wrong DB character and
      // artificially clusters all font scores at ~70%.
      let bestSc = -1, dbSig = null;
      for (const s of font.sigs) {
        const sc = _imgSimilarity(queryThumb, s.thumb);
        if (sc > bestSc) { bestSc = sc; dbSig = s; }
      }
      if (!dbSig) continue;

      totalScore += bestSc;
      matched++;
      if (bestSc > 0.70) matchedChars.push(m.character);
    }

    if (!matched) continue;
    const avg = totalScore / matched;
    const existing = scores.get(key);
    if (!existing || avg > existing.score) {
      scores.set(key, { family: font.family, subfamily: font.subfamily, score: avg, matchedChars });
    }
  }

  return [...scores.values()]
    .sort((a, b) => b.score - a.score)
    .slice(0, limit)
    .map(r => ({
      family:       r.family,
      subfamily:    r.subfamily || null,
      confidence:   Math.round(r.score * 1000) / 1000,
      matchedChars: r.matchedChars,
    }));
}

// ── Matching (PIXELDB1 / GLYPHDB1 fallback) ───────────────────────────────────

const FIELD_WEIGHTS = [
  ['aspect_ratio', 0.15], ['density',      0.15],
  ['quadrant_nw',  0.08], ['quadrant_ne',  0.08],
  ['quadrant_sw',  0.08], ['quadrant_se',  0.08],
  ['curve_ratio',  0.10], ['point_count',  0.05],
  ['x_balance',    0.06], ['y_balance',    0.06],
  ['stroke_width', 0.10], ['serif_score',  0.11],
];

function _sigSimilarity(query, dbSig) {
  let dist = 0;
  for (const [field, w] of FIELD_WEIGHTS) {
    dist += w * Math.abs((query[field] || 0) - (dbSig[field] || 0)) / 255;
  }
  return Math.max(0, 1 - dist);
}

/**
 * Identify fonts from pixel metrics.
 *
 * @param {Array<{character: string, aspectRatio: number, thumbnail?: number[], ...}>} metrics
 *   JsPixelMetrics[] — one per analyzed glyph. thumbnail is a 256-element array (16×16).
 * @param {number} [limit=8]
 * @returns {Promise<Array<{family, subfamily, confidence, matchedChars}>>}
 */
async function identify(metrics, limit = 8) {
  if (!metrics.length) return [];

  // Prefer IMGDB2 (direct visual comparison — 64×64 pixels per glyph, character-agnostic)
  const hasThumbnails = metrics.some(m => Array.isArray(m.thumbnail) && m.thumbnail.length === 4096);
  if (hasThumbnails) {
    const imgDb = await _tryLoadImgDb();
    if (imgDb && imgDb.length > 0) {
      const results = _identifyByThumbnail(metrics, imgDb, limit);
      if (results.length > 0) return results;
    }
  }

  // Fallback: 12-feature PIXELDB1 / GLYPHDB1 matching
  const db = await _loadDatabase();
  if (!db || !metrics.length) return [];

  // Convert camelCase metric fields to snake_case for comparison
  const queries = metrics.map(m => ({
    ch:           m.character,
    aspect_ratio: m.aspectRatio,
    density:      m.density,
    quadrant_nw:  m.quadrantNw,
    quadrant_ne:  m.quadrantNe,
    quadrant_sw:  m.quadrantSw,
    quadrant_se:  m.quadrantSe,
    curve_ratio:  m.curveRatio,
    point_count:  m.pointCount,
    x_balance:    m.xBalance,
    y_balance:    m.yBalance,
    stroke_width: m.strokeWidth,
    serif_score:  m.serifScore,
  }));

  const scores = new Map(); // family+subfamily → { score, count, chars }

  for (const font of db) {
    const key = `${font.family}||${font.subfamily || ''}`;
    let totalScore = 0, matched = 0;
    const matchedChars = [];

    for (const q of queries) {
      // Prefer exact character match; fall back to best score across all glyphs
      // (avoids silent miss when _guessCharacter returns a char not in DB)
      const exactSig = font.sigs.find(s => s.ch === q.ch);
      const dbSig    = exactSig ?? font.sigs.reduce((best, s) => {
        const sc = _sigSimilarity(q, s);
        return (!best || sc > _sigSimilarity(q, best)) ? s : best;
      }, null);
      if (!dbSig) continue;
      const s = _sigSimilarity(q, dbSig);
      totalScore += s;
      matched++;
      if (s > 0.7) matchedChars.push(q.ch);
    }

    if (!matched) continue;
    const avg = totalScore / matched;

    const existing = scores.get(key);
    if (!existing || avg > existing.score) {
      scores.set(key, { family: font.family, subfamily: font.subfamily, score: avg, matchedChars });
    }
  }

  return [...scores.values()]
    .sort((a, b) => b.score - a.score)
    .slice(0, limit)
    .map(r => ({
      family:       r.family,
      subfamily:    r.subfamily || null,
      confidence:   Math.round(r.score * 1000) / 1000,
      matchedChars: r.matchedChars,
    }));
}

// ── Export ────────────────────────────────────────────────────────────────────
window.IntellifontMatcher = { identify };
})();
