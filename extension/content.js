'use strict';
/**
 * Intellifont — content script
 *
 * Handles messages from the background service worker, runs the font
 * identification pipeline, and injects an overlay with results.
 *
 * Dependencies (loaded before this file by the manifest):
 *   lib/canvas-dna.js      → window.CanvasDNA
 *   lib/image-pipeline.js  → window.IntellifontImagePipeline
 *   lib/matcher.js         → window.IntellifontMatcher
 */

console.log('[Intellifont] Content script initialized');
console.log('[Intellifont] Checking dependencies...');
console.log('[Intellifont] window.CanvasDNA:', typeof window.CanvasDNA !== 'undefined' ? 'loaded ✓' : 'MISSING ✗');
console.log('[Intellifont] window.IntellifontImagePipeline:', typeof window.IntellifontImagePipeline !== 'undefined' ? 'loaded ✓' : 'MISSING ✗');
console.log('[Intellifont] window.IntellifontMatcher:', typeof window.IntellifontMatcher !== 'undefined' ? 'loaded ✓' : 'MISSING ✗');

// ── Overlay singleton ────────────────────────────────────────────────────────

let _overlayHost = null;

const _OVERLAY_CSS = `:host{all:initial;position:fixed;top:20px;right:20px;z-index:2147483647;font-family:-apple-system,BlinkMacSystemFont,'Segoe UI',sans-serif;max-width:min(360px,calc(100vw - 40px))}.if-overlay{background:#1e1b4b;border:1px solid #4f46e5;border-radius:12px;box-shadow:0 8px 32px rgba(0,0,0,.6);color:#e0e7ff;min-width:260px;width:100%;max-height:calc(100vh - 40px);overflow-y:auto;overflow-x:hidden;font-size:13px;line-height:1.4;animation:if-slide-in .18s ease}@keyframes if-slide-in{from{opacity:0;transform:translateY(-8px)}to{opacity:1;transform:translateY(0)}}.if-header{display:flex;align-items:center;gap:8px;padding:10px 14px;background:#312e81;border-bottom:1px solid #4f46e5}.if-logo{font-weight:700;font-size:14px;color:#a5b4fc;flex:0 0 auto}.if-source{font-size:11px;color:#818cf8;flex:1;overflow:hidden;text-overflow:ellipsis;white-space:nowrap}.if-close{all:unset;cursor:pointer;color:#818cf8;font-size:15px;line-height:1;padding:2px 4px;border-radius:4px;flex:0 0 auto}.if-close:hover{background:rgba(255,255,255,.1);color:#fff}.if-style-badge{padding:5px 14px;font-size:11px;color:#a5b4fc;background:rgba(79,70,229,.2);border-bottom:1px solid rgba(79,70,229,.3)}.if-style-badge.if-italic{font-style:italic}.if-results{list-style:none;margin:0;padding:6px 0}.if-result{display:flex;align-items:center;gap:8px;padding:7px 14px;transition:background .1s}.if-result:hover{background:rgba(255,255,255,.05)}.if-result.if-top{background:rgba(79,70,229,.15)}.if-rank{font-size:11px;font-weight:700;color:#6366f1;width:16px;flex:0 0 auto;text-align:center}.if-top .if-rank{color:#a5b4fc}.if-info{flex:1;min-width:0}.if-family{display:block;font-weight:600;color:#e0e7ff;white-space:nowrap;overflow:hidden;text-overflow:ellipsis}.if-top .if-family{color:#fff;font-size:14px}.if-sub{display:block;font-size:11px;color:#818cf8}.if-bar-wrap{height:3px;background:rgba(255,255,255,.1);border-radius:2px;margin-top:4px}.if-bar{height:100%;background:#6366f1;border-radius:2px;transition:width .4s ease}.if-top .if-bar{background:#a5b4fc}.if-pct{font-size:12px;font-weight:600;color:#818cf8;width:36px;text-align:right;flex:0 0 auto}.if-copy{all:unset;cursor:pointer;color:#6366f1;font-size:15px;padding:2px 5px;border-radius:4px;flex:0 0 auto}.if-copy:hover{background:rgba(255,255,255,.1);color:#a5b4fc}.if-similar-section{border-top:1px solid rgba(79,70,229,.3)}.if-similar-btn{all:unset;display:block;width:100%;box-sizing:border-box;cursor:pointer;padding:8px 14px;font-size:11px;font-weight:600;color:#818cf8;text-align:left;transition:background .1s,color .1s}.if-similar-btn:hover{background:rgba(255,255,255,.05);color:#a5b4fc}.if-similar-panel{padding:0 14px 10px}.if-similar-cat{font-size:10px;color:#4f46e5;text-transform:uppercase;letter-spacing:.06em;margin-bottom:6px;padding-top:2px}.if-similar-list{list-style:none;margin:0;padding:0;display:flex;flex-direction:column;gap:5px}.if-similar-item{display:flex;align-items:flex-start;gap:6px}.if-similar-info{flex:1;min-width:0;display:flex;flex-wrap:wrap;align-items:center;gap:4px}.if-similar-name{font-size:12px;font-weight:600;color:#e0e7ff;white-space:nowrap}.if-similar-note{display:block;width:100%;font-size:10px;color:#6366f1;white-space:nowrap;overflow:hidden;text-overflow:ellipsis}.if-badge{font-size:9px;font-weight:700;padding:1px 5px;border-radius:3px;text-transform:uppercase;letter-spacing:.04em;flex:0 0 auto}.if-badge-free{background:rgba(34,197,94,.2);color:#86efac;border:1px solid rgba(34,197,94,.4)}.if-badge-paid{background:rgba(251,146,60,.2);color:#fcd34d;border:1px solid rgba(251,146,60,.4)}.if-copy-import{all:unset;cursor:pointer;color:#6366f1;font-size:14px;padding:2px 4px;border-radius:4px;flex:0 0 auto;line-height:1;margin-top:1px}.if-copy-import:hover{background:rgba(255,255,255,.1);color:#a5b4fc}.if-similar-hint{margin:0;font-size:11px;color:#4f46e5;padding:4px 0}.if-footer{padding:7px 14px;font-size:10px;color:#4f46e5;border-top:1px solid rgba(79,70,229,.3);text-align:center}.if-loading{display:flex;align-items:center;gap:10px;padding:16px 18px}.if-spinner{width:16px;height:16px;border:2px solid rgba(99,102,241,.3);border-top-color:#6366f1;border-radius:50%;animation:if-spin .7s linear infinite;flex:0 0 auto}@keyframes if-spin{to{transform:rotate(360deg)}}.if-error{display:flex;align-items:center;gap:10px;padding:12px 14px}.if-err-msg{flex:1;color:#fca5a5;font-size:12px}.if-empty{padding:16px 14px;color:#818cf8;font-size:12px;text-align:center}`;

function _injectStyles(shadow) {
  const style = document.createElement('style');
  style.textContent = _OVERLAY_CSS;
  shadow.appendChild(style);
}

function _removeOverlay() {
  if (_overlayHost) { _overlayHost.remove(); _overlayHost = null; }
}

function _showOverlay(results, style, source) {
  _removeOverlay();

  _overlayHost = document.createElement('div');
  _overlayHost.id = 'intellifont-overlay-host';
  document.documentElement.appendChild(_overlayHost);

  const shadow = _overlayHost.attachShadow({ mode: 'open' });
  _injectStyles(shadow);

  // ── Build overlay HTML ────────────────────────────────────────────────────
  const root = document.createElement('div');
  root.className = 'if-overlay';

  // Header
  const hdr = document.createElement('div');
  hdr.className = 'if-header';
  hdr.innerHTML = `
    <span class="if-logo">Intellifont</span>
    <span class="if-source">${_esc(source)}</span>
    <button class="if-close" title="Close">✕</button>
  `;
  root.appendChild(hdr);

  // Style badge
  if (style) {
    const badge = document.createElement('div');
    badge.className = 'if-style-badge';
    const parts = [style.weightLabel || 'Regular'];
    if (style.italic) parts.push('Italic');
    badge.textContent = `Detected style: ${parts.join(' ')}`;
    if (style.italic) badge.classList.add('if-italic');
    root.appendChild(badge);
  }

  // Results list
  if (!results.length) {
    const empty = document.createElement('div');
    empty.className = 'if-empty';
    empty.textContent = 'No font matches found. Try a higher-quality image.';
    root.appendChild(empty);
  } else {
    const list = document.createElement('ul');
    list.className = 'if-results';

    results.forEach((r, i) => {
      const pct = Math.round(r.confidence * 100);
      const li  = document.createElement('li');
      li.className = i === 0 ? 'if-result if-top' : 'if-result';
      const cssBadge = r._fromCss
        ? `<span style="font-size:9px;background:#4f46e5;color:#e0e7ff;padding:1px 5px;border-radius:3px;margin-left:5px;vertical-align:middle">CSS</span>`
        : '';
      li.innerHTML = `
        <div class="if-rank">${i + 1}</div>
        <div class="if-info">
          <span class="if-family">${_esc(r.family)}${cssBadge}</span>
          ${r.subfamily ? `<span class="if-sub">${_esc(r.subfamily)}</span>` : ''}
          <div class="if-bar-wrap">
            <div class="if-bar" style="width:${pct}%"></div>
          </div>
        </div>
        <div class="if-pct">${r._fromCss ? 'CSS' : pct + '%'}</div>
        <button class="if-copy" data-font="${_esc(r.family)}" title="Copy font name">⎘</button>
      `;
      list.appendChild(li);
    });
    root.appendChild(list);
  }

  // Similar fonts section (shown only when results exist)
  if (results.length) {
    const topFont = results[0].family;
    const similarSection = document.createElement('div');
    similarSection.className = 'if-similar-section';

    const btn = document.createElement('button');
    btn.className = 'if-similar-btn';
    btn.textContent = 'Similar fonts ▾';
    similarSection.appendChild(btn);

    const panel = document.createElement('div');
    panel.className = 'if-similar-panel';
    panel.hidden = true;
    similarSection.appendChild(panel);

    btn.addEventListener('click', async () => {
      if (!panel.hidden) {
        panel.hidden = true;
        btn.textContent = 'Similar fonts ▾';
        return;
      }

      btn.textContent = 'Loading…';
      const cfg = await chrome.storage.sync.get(['serverUrl']);
      const serverUrl = (cfg.serverUrl || '').trim();

      if (!serverUrl) {
        panel.innerHTML = `<p class="if-similar-hint">Configure the server URL in the extension popup to see alternatives.</p>`;
        panel.hidden = false;
        btn.textContent = 'Similar fonts ▴';
        return;
      }

      try {
        const res = await fetch(`${serverUrl}/api/similar?font=${encodeURIComponent(topFont)}&limit=5`);
        if (!res.ok) throw new Error(`HTTP ${res.status}`);
        const data = await res.json();

        if (!data.similar || !data.similar.length) {
          panel.innerHTML = `<p class="if-similar-hint">No alternatives found for "${_esc(topFont)}".</p>`;
        } else {
          const catLine = data.category
            ? `<div class="if-similar-cat">${_esc(data.category)}</div>`
            : '';
          const items = data.similar.map(s => {
            const badgeClass = s.license === 'OFL' || s.license === 'Apache' ? 'if-badge-free' : 'if-badge-paid';
            const badgeLabel = s.source === 'google' ? 'Free' : s.source === 'system' ? 'System' : 'Commercial';
            const copyBtn = s.googleImport
              ? `<button class="if-copy-import" data-import="${_esc(s.googleImport)}" title="Copy @import">⎘</button>`
              : '';
            return `
              <li class="if-similar-item">
                <div class="if-similar-info">
                  <span class="if-similar-name">${_esc(s.family)}</span>
                  <span class="if-badge ${badgeClass}">${badgeLabel}</span>
                  <span class="if-similar-note">${_esc(s.note || '')}</span>
                </div>
                ${copyBtn}
              </li>`;
          }).join('');
          panel.innerHTML = `${catLine}<ul class="if-similar-list">${items}</ul>`;

          panel.querySelectorAll('.if-copy-import').forEach(b => {
            b.addEventListener('click', () => {
              const imp = `@import url('${b.dataset.import}');`;
              navigator.clipboard.writeText(imp).then(() => {
                b.textContent = '✓';
                setTimeout(() => { b.textContent = '⎘'; }, 1500);
              });
            });
          });
        }
      } catch (e) {
        panel.innerHTML = `<p class="if-similar-hint">Could not load alternatives: ${_esc(e.message)}</p>`;
      }

      panel.hidden = false;
      btn.textContent = 'Similar fonts ▴';
    });

    root.appendChild(similarSection);
  }

  // Footer
  const footer = document.createElement('div');
  footer.className = 'if-footer';
  footer.textContent = source.includes('server')
    ? 'Matched via server — only glyph metrics were sent, never image pixels.'
    : 'Identified locally — nothing was uploaded.';
  root.appendChild(footer);

  shadow.appendChild(root);

  // ── Event handlers ────────────────────────────────────────────────────────
  shadow.querySelector('.if-close').addEventListener('click', _removeOverlay);

  shadow.querySelectorAll('.if-copy').forEach(btn => {
    btn.addEventListener('click', () => {
      const font = btn.dataset.font;
      navigator.clipboard.writeText(font).then(() => {
        btn.textContent = '✓';
        setTimeout(() => { btn.textContent = '⎘'; }, 1500);
      });
    });
  });

  // Click outside overlay to dismiss
  document.addEventListener('click', function handler(e) {
    if (!_overlayHost) { document.removeEventListener('click', handler); return; }
    if (!_overlayHost.contains(e.target)) {
      _removeOverlay();
      document.removeEventListener('click', handler);
    }
  }, { capture: false });
}

function _showLoading() {
  _removeOverlay();
  _overlayHost = document.createElement('div');
  _overlayHost.id = 'intellifont-overlay-host';
  document.documentElement.appendChild(_overlayHost);

  const shadow = _overlayHost.attachShadow({ mode: 'open' });
  _injectStyles(shadow);

  const root = document.createElement('div');
  root.className = 'if-overlay if-loading';
  root.innerHTML = `<span class="if-spinner"></span><span>Identifying font…</span>`;
  shadow.appendChild(root);
}

function _showError(msg) {
  _removeOverlay();
  _overlayHost = document.createElement('div');
  _overlayHost.id = 'intellifont-overlay-host';
  document.documentElement.appendChild(_overlayHost);

  const shadow = _overlayHost.attachShadow({ mode: 'open' });
  _injectStyles(shadow);

  const root = document.createElement('div');
  root.className = 'if-overlay if-error';
  root.innerHTML = `
    <span class="if-logo">Intellifont</span>
    <span class="if-err-msg">${_esc(msg)}</span>
    <button class="if-close">✕</button>
  `;
  shadow.appendChild(root);
  shadow.querySelector('.if-close').addEventListener('click', _removeOverlay);
}

function _esc(s) {
  return String(s).replace(/[&<>"']/g, c => ({ '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;', "'": '&#39;' }[c]));
}

// ── Identify via server (preferred — avoids loading 2 MB glyph DB in the tab) ─

async function _identifyViaServer(metrics, limit = 8) {
  const cfg = await chrome.storage.sync.get(['serverUrl', 'apiKey']);
  const url = (cfg.serverUrl || '').trim();
  const key = (cfg.apiKey   || '').trim();
  if (!url || !key) return null;

  try {
    const res = await fetch(`${url}/api/identify`, {
      method:  'POST',
      headers: { 'Content-Type': 'application/json', 'X-Api-Key': key },
      body:    JSON.stringify({ metrics }),
    });
    if (!res.ok) return null;
    const data = await res.json();
    return (data.results || []).slice(0, limit);
  } catch {
    return null;
  }
}

/**
 * Deduplicate metrics by character label — keep the glyph with the highest
 * ink density for each character. Prevents 3× 'W' queries (e.g. w/m/v all
 * mapped to 'W' by _guessCharacter) from wasting the budget on duplicates.
 */
function _dedupeMetrics(metrics) {
  const best = new Map();
  for (const m of metrics) {
    const ch  = m.character;
    const den = m.density ?? m.aspectRatio ?? 0;
    if (!best.has(ch) || den > (best.get(ch).density ?? 0)) {
      best.set(ch, m);
    }
  }
  return [...best.values()];
}

/** Server first when configured; local DB only as offline fallback. */
async function _identifyMetrics(metrics, limit = 8) {
  // IMGDB1 visual matching benefits from all thumbnails — each 256-pixel fingerprint
  // provides independent evidence regardless of char label. Only deduplicate for
  // PIXELDB1 scalar fallback (where duplicate labels add no signal).
  const hasThumbs = metrics.some(m => Array.isArray(m.thumbnail) && m.thumbnail.length === 256);
  const queryMetrics = hasThumbs ? metrics : _dedupeMetrics(metrics);
  const serverMatches = await _identifyViaServer(queryMetrics, limit);
  if (serverMatches && serverMatches.length) {
    return { matches: serverMatches, via: 'server' };
  }
  const matches = await window.IntellifontMatcher.identify(queryMetrics, limit);
  return { matches, via: 'local' };
}

// ── Message handler ──────────────────────────────────────────────────────────

chrome.runtime.onMessage.addListener((msg) => {
  console.log('[Intellifont] Message received:', msg?.action);
  if (msg.action === 'identify-image')  { console.log('[Intellifont] Handling identify-image'); _handleImage(msg.srcUrl).catch(e  => { console.error('[Intellifont] identify-image error:', e); _showError(`Error: ${e.message}`); }); }
  if (msg.action === 'identify-region') { console.log('[Intellifont] Handling identify-region'); _handleRegion(msg.srcUrl).catch(e => { console.error('[Intellifont] identify-region error:', e); _showError(`Error: ${e.message}`); }); }
  if (msg.action === 'identify-page')   { console.log('[Intellifont] Handling identify-page'); _handlePage().catch(e             => { console.error('[Intellifont] identify-page error:', e); _showError(`Error: ${e.message}`); }); }
  if (msg.action === 'detect-ai-text')  { console.log('[Intellifont] Handling detect-ai-text'); _handleAiDetect(msg.srcUrl).catch(e => { console.error('[Intellifont] detect-ai-text error:', e); _showError(`Error: ${e.message}`); }); }
  if (msg.action === 'forensics-image') { console.log('[Intellifont] Handling forensics-image'); _handleForensics(msg.srcUrl).catch(e => { console.error('[Intellifont] forensics-image error:', e); _showError(`Error: ${e.message}`); }); }
});

// ── Identify font from an image URL ─────────────────────────────────────────

async function _analyzeViaScreenshot(srcUrl) {
  const imgEl = [...document.querySelectorAll('img')].find(
    el => el.src === srcUrl || el.currentSrc === srcUrl
  );
  if (!imgEl) throw new Error('Could not locate image on page. Use "Identify font in image region…" to draw a selection instead.');

  const rect = imgEl.getBoundingClientRect();
  if (rect.width < 4 || rect.height < 4) throw new Error('Image element is too small to capture.');

  const { dataUrl, error } = await new Promise((resolve) => {
    chrome.runtime.sendMessage({ action: 'capture-tab' }, (resp) => {
      if (chrome.runtime.lastError) resolve({ error: chrome.runtime.lastError.message });
      else resolve(resp || { error: 'No response from background' });
    });
  });
  if (error) throw new Error(`Screenshot failed: ${error}`);

  const screenshot = await new Promise((resolve, reject) => {
    const img = new Image();
    img.onload  = () => resolve(img);
    img.onerror = () => reject(new Error('Failed to load tab screenshot'));
    img.src = dataUrl;
  });

  const dpr   = window.devicePixelRatio || 1;
  const cropX = Math.round(rect.left   * dpr);
  const cropY = Math.round(rect.top    * dpr);
  const cropW = Math.max(1, Math.round(rect.width  * dpr));
  const cropH = Math.max(1, Math.round(rect.height * dpr));

  // Upscale if the crop is too small for reliable glyph analysis (target min 120px tall)
  const MIN_H = 120;
  const scale = cropH < MIN_H ? MIN_H / cropH : 1;
  const outW  = Math.round(cropW * scale);
  const outH  = Math.round(cropH * scale);
  console.log(`[Intellifont] Screenshot crop: ${cropX},${cropY} ${cropW}×${cropH} (dpr=${dpr}) → rendered ${outW}×${outH}`);

  const cropped = document.createElement('canvas');
  cropped.width  = outW;
  cropped.height = outH;
  // imageSmoothingQuality 'high' gives better results when upscaling text
  const ctx = cropped.getContext('2d');
  ctx.imageSmoothingEnabled = true;
  ctx.imageSmoothingQuality = 'high';
  ctx.drawImage(screenshot, cropX, cropY, cropW, cropH, 0, 0, outW, outH);

  const { ImagePipeline } = window.IntellifontImagePipeline;
  return await new ImagePipeline().analyze(cropped);
}

async function _handleImage(srcUrl) {
  if (!srcUrl) return _showError('Could not determine image URL.');
  console.log('[Intellifont] Analyzing image:', srcUrl);
  _showLoading();

  try {
    const { ImagePipeline } = window.IntellifontImagePipeline;
    const pipeline = new ImagePipeline();

    let result;
    try {
      result = await pipeline.analyze(srcUrl);
    } catch (loadErr) {
      // CORS or network error — fall back to tab screenshot cropped to the image's page rect
      console.log('[Intellifont] Image URL blocked, falling back to screenshot:', loadErr.message);
      result = await _analyzeViaScreenshot(srcUrl);
    }

    console.log('[Intellifont] Pipeline result:', {
      metricCount: result.metrics?.length,
      regionCount: result.regionCount,
    });
    const { metrics, style } = result;

    if (!metrics.length) {
      console.log('[Intellifont] No text in image — falling back to page font detection');
      return _handlePage(true);
    }

    const { matches, via } = await _identifyMetrics(metrics, 8);
    console.log('[Intellifont] Matches:', matches.slice(0, 3), `(${via})`);
    _showOverlay(matches, style, `from image · ${via}`);
  } catch (e) {
    console.error('[Intellifont] Error:', e);
    _showError(`Error: ${e.message}`);
  }
}

// ── Identify font from the current page's visible text ──────────────────────

async function _handlePage(autoFallback = false) {
  _showLoading();

  try {
    // Find the most prominent text element (largest font-size visible in viewport)
    const el = _findProminentTextElement();
    if (!el) return _showError('No text element found on page.');

    const style    = window.getComputedStyle(el);
    const rawFont  = style.fontFamily || 'sans-serif';
    const family   = rawFont.split(',')[0].trim().replace(/['"]/g, '');
    const tag      = el.tagName.toLowerCase();
    const fontSize = style.fontSize;
    console.log(`[Intellifont] Detected element: <${tag}> "${el.textContent.trim().slice(0,40)}" font-family: ${family} font-size: ${fontSize}`);

    // Use CanvasDNA to get pixel metrics from the rendered CSS font
    const metrics = window.CanvasDNA.analyzeFont(family, 'RQWMag');

    if (!metrics.length) return _showError(`Could not render font: "${family}"`);

    const { matches: idMatches, via } = await _identifyMetrics(metrics, 8);
    let matches = idMatches;
    const modeLabel = autoFallback ? 'page CSS (image had no text)' : 'page CSS';
    let source  = `"${family}" · ${modeLabel} · ${via}`;

    // CSS gave us the definitive font name — pin it as #1 so the user isn't
    // confused by recognition results for fonts that aren't in the DB
    const cssNameNorm = family.toLowerCase().replace(/\s+/g, '');
    const alreadyTop  = matches.length &&
      matches[0].family.toLowerCase().replace(/\s+/g, '') === cssNameNorm;
    if (!alreadyTop) {
      // Remove any existing entry for this family (case-insensitive) to avoid duplication
      const filtered = matches.filter(m =>
        m.family.toLowerCase().replace(/\s+/g, '') !== cssNameNorm
      ).slice(0, 7);
      matches = [
        { family, subfamily: null, confidence: 1.0, matchedChars: [], _fromCss: true },
        ...filtered,
      ];
    }

    _showOverlay(matches, null, source);
  } catch (e) {
    _showError(`Error: ${e.message}`);
  }
}

// ── AI-generated text detection ──────────────────────────────────────────────

async function _handleAiDetect(srcUrl) {
  if (!srcUrl) return _showError('Could not determine image URL.');
  _showLoading();

  try {
    const { ImagePipeline, analyzeTextAuthenticity } = window.IntellifontImagePipeline;
    const { metrics } = await new ImagePipeline().analyze(srcUrl);

    if (!metrics.length) return _showError('No text found in image.');

    let fontMatches = null;
    try {
      const id = await _identifyMetrics(metrics, 3);
      fontMatches = id.matches;
    } catch (_) {}

    const result = analyzeTextAuthenticity(metrics, fontMatches);

    _showAiResult(result, srcUrl);
  } catch (e) {
    _showError(`Error: ${e.message}`);
  }
}

function _showAiResult(result, _srcUrl) {
  _removeOverlay();
  _overlayHost = document.createElement('div');
  _overlayHost.id = 'intellifont-overlay-host';
  document.documentElement.appendChild(_overlayHost);

  const shadow = _overlayHost.attachShadow({ mode: 'open' });
  _injectStyles(shadow);

  const root = document.createElement('div');
  root.className = 'if-overlay';

  const verdict   = result.isAiGenerated;
  const pct       = Math.round(result.confidence * 100);
  const color     = verdict ? '#f87171' : '#86efac';
  const label     = verdict ? 'AI-Generated' : 'Real Font';
  const sublabel  = verdict
    ? 'Text shows signs of AI image generation'
    : 'Text metrics match a real font engine';

  root.innerHTML = `
    <div class="if-header">
      <span class="if-logo">Intellifont</span>
      <span class="if-source">AI Text Detection</span>
      <button class="if-close" title="Close">✕</button>
    </div>
    <div style="padding:14px 16px;text-align:center">
      <div style="font-size:32px;font-weight:800;color:${color};margin-bottom:4px">${label}</div>
      <div style="font-size:13px;color:#818cf8;margin-bottom:12px">${sublabel}</div>
      <div style="background:#312e81;border-radius:8px;height:10px;overflow:hidden;margin-bottom:8px">
        <div style="background:${color};width:${pct}%;height:100%;transition:width 0.4s"></div>
      </div>
      <div style="font-size:12px;color:#a5b4fc">${pct}% confidence</div>
      ${result.nearestFont
        ? `<div style="margin-top:10px;font-size:11px;color:#6366f1">Nearest real font: <strong style="color:#a5b4fc">${_esc(result.nearestFont)}</strong></div>`
        : ''}
      <div style="margin-top:8px;font-size:10px;color:#4f46e5">
        ${result.indicators.map(i => `<span style="display:inline-block;background:#1e1b4b;border:1px solid #4f46e5;border-radius:4px;padding:1px 6px;margin:2px">${_esc(i.replace(/_/g,' '))}</span>`).join('')}
      </div>
    </div>
    <div class="if-footer">Analysis runs locally — image was not uploaded.</div>
  `;

  shadow.appendChild(root);
  shadow.querySelector('.if-close').addEventListener('click', _removeOverlay);
}

// ── Document forgery timeline check ─────────────────────────────────────────

async function _handleForensics(srcUrl) {
  if (!srcUrl) return _showError('Could not determine image URL.');

  // Ask user for the claimed document date
  const yearStr = window.prompt('Enter the year the document claims to be from (e.g. 1998):');
  if (!yearStr) return;
  const claimedYear = parseInt(yearStr, 10);
  if (!claimedYear || claimedYear < 1400 || claimedYear > 2100) {
    return _showError('Enter a valid year (1400–2100).');
  }

  _showLoading();

  try {
    const { ImagePipeline } = window.IntellifontImagePipeline;
    const { metrics } = await new ImagePipeline().analyze(srcUrl);
    if (!metrics.length) return _showError('No text found in image.');

    const cfg = await chrome.storage.sync.get(['serverUrl', 'apiKey']);
    const url = (cfg.serverUrl || '').trim();
    const key = (cfg.apiKey   || '').trim();

    if (!url || !key) {
      return _showError('Server URL and API key required for forensics check.\nConfigure in the extension popup.');
    }

    const res = await fetch(`${url}/api/forensics/timeline`, {
      method:  'POST',
      headers: { 'Content-Type': 'application/json', 'X-Api-Key': key },
      body:    JSON.stringify({ metrics, claimedYear }),
    });

    if (!res.ok) {
      const err = await res.json().catch(() => ({}));
      return _showError(`Server error: ${err.error || res.statusText}`);
    }

    const data = await res.json();
    _showForensicsResult(data);
  } catch (e) {
    _showError(`Error: ${e.message}`);
  }
}

function _showForensicsResult(data) {
  _removeOverlay();
  _overlayHost = document.createElement('div');
  _overlayHost.id = 'intellifont-overlay-host';
  document.documentElement.appendChild(_overlayHost);

  const shadow = _overlayHost.attachShadow({ mode: 'open' });
  _injectStyles(shadow);

  const root = document.createElement('div');
  root.className = 'if-overlay';

  const plausible = data.plausible;
  const color  = plausible === null ? '#fbbf24' : plausible ? '#86efac' : '#f87171';
  const verdict = plausible === null ? 'Inconclusive' : plausible ? 'Plausible' : 'Suspicious';

  const warningHtml = data.warnings?.length
    ? data.warnings.map(w =>
        `<div style="background:#1e1b4b;border-left:3px solid ${color};padding:8px 10px;margin:6px 0;font-size:11px;border-radius:0 4px 4px 0;color:#e0e7ff">${_esc(w)}</div>`
      ).join('')
    : '<div style="color:#6366f1;font-size:11px">No anachronisms detected.</div>';

  root.innerHTML = `
    <div class="if-header">
      <span class="if-logo">Intellifont</span>
      <span class="if-source">Document Forensics</span>
      <button class="if-close" title="Close">✕</button>
    </div>
    <div style="padding:14px 16px">
      <div style="font-size:26px;font-weight:800;color:${color};margin-bottom:2px">${verdict}</div>
      <div style="font-size:12px;color:#818cf8;margin-bottom:12px">
        Claimed year: <strong style="color:#a5b4fc">${data.claimedYear}</strong>
        ${data.identifiedFont ? ` · Identified font: <strong style="color:#a5b4fc">${_esc(data.identifiedFont)}</strong>` : ''}
        ${data.fontReleaseYear ? ` (released ${data.fontReleaseYear})` : ''}
      </div>
      ${warningHtml}
    </div>
    <div class="if-footer">Font timeline analysis — powered by Intellifont forensics DB.</div>
  `;

  shadow.appendChild(root);
  shadow.querySelector('.if-close').addEventListener('click', _removeOverlay);
}

// ── Region selector — returns {x,y,w,h} in viewport coords, or null if cancelled ─

function _showRegionSelector() {
  console.log('[Intellifont] _showRegionSelector creating overlay...');
  return new Promise((resolve) => {
    try {
      const overlay = document.createElement('div');
      Object.assign(overlay.style, {
        position: 'fixed', inset: '0',
        zIndex: '2147483646',
        cursor: 'crosshair',
        userSelect: 'none',
        WebkitUserSelect: 'none',
        backgroundColor: 'transparent',
      });
      console.log('[Intellifont] Overlay div created');

      const banner = document.createElement('div');
      Object.assign(banner.style, {
        position: 'absolute', top: '0', left: '0', right: '0',
        background: 'rgba(30,27,75,0.92)', color: '#a5b4fc',
        fontFamily: "-apple-system, BlinkMacSystemFont, 'Segoe UI', sans-serif",
        fontSize: '13px', padding: '10px 16px', textAlign: 'center',
        borderBottom: '1px solid #4f46e5', pointerEvents: 'none', zIndex: '999',
      });
      banner.textContent = 'Draw a rectangle around the text you want to identify — Esc to cancel';
      banner.id = 'intellifont-region-banner';
      overlay.appendChild(banner);
      console.log('[Intellifont] Banner created and appended');

      // Selection box
      const box = document.createElement('div');
      Object.assign(box.style, {
        position: 'absolute', border: '2px solid #6366f1',
        background: 'rgba(99,102,241,0.08)',
        boxShadow: '0 0 0 9999px rgba(0,0,0,0.50)',
        pointerEvents: 'none', display: 'none', zIndex: '998',
      });
      box.id = 'intellifont-region-box';
      overlay.appendChild(box);
      console.log('[Intellifont] Selection box created and appended');

      document.documentElement.appendChild(overlay);
      console.log('[Intellifont] Overlay appended to DOM, checking if visible...');
      console.log('[Intellifont] Overlay visibility check: display=' + window.getComputedStyle(overlay).display + ', zIndex=' + window.getComputedStyle(overlay).zIndex);

      let startX = 0, startY = 0, dragging = false;

      const cleanup = () => {
        console.log('[Intellifont] Cleaning up region selector');
        overlay.remove();
        document.removeEventListener('keydown', onKey);
      };

      const onKey = (e) => {
        console.log('[Intellifont] Key pressed:', e.key);
        if (e.key === 'Escape') {
          console.log('[Intellifont] Escape pressed, cancelling region selection');
          cleanup();
          resolve(null);
        }
      };
      document.addEventListener('keydown', onKey, true);
      console.log('[Intellifont] Keyboard listener registered');

      overlay.addEventListener('mousedown', (e) => {
        console.log('[Intellifont] Mousedown at', e.clientX, e.clientY);
        e.preventDefault();
        e.stopPropagation();
        dragging = true;
        startX = e.clientX; startY = e.clientY;
        Object.assign(box.style, {
          display: 'block', left: startX + 'px', top: startY + 'px',
          width: '0', height: '0',
        });
      });

      overlay.addEventListener('mousemove', (e) => {
        if (!dragging) return;
        const x = Math.min(e.clientX, startX);
        const y = Math.min(e.clientY, startY);
        Object.assign(box.style, {
          left: x + 'px', top: y + 'px',
          width:  Math.abs(e.clientX - startX) + 'px',
          height: Math.abs(e.clientY - startY) + 'px',
        });
      }, false);

      overlay.addEventListener('mouseup', (e) => {
        console.log('[Intellifont] Mouseup at', e.clientX, e.clientY);
        if (!dragging) return;
        dragging = false;
        const w = Math.abs(e.clientX - startX);
        const h = Math.abs(e.clientY - startY);
        console.log('[Intellifont] Selection complete: w=' + w + ', h=' + h);
        cleanup();
        if (w < 10 || h < 10) {
          console.log('[Intellifont] Selection too small, cancelled');
          resolve(null);
          return;
        }
        const result = {
          x: Math.min(e.clientX, startX),
          y: Math.min(e.clientY, startY),
          w, h,
        };
        console.log('[Intellifont] Selection result:', result);
        resolve(result);
      });

      console.log('[Intellifont] All event listeners registered, overlay ready for interaction');
    } catch (err) {
      console.error('[Intellifont] Error in _showRegionSelector:', err);
      resolve(null);
    }
  });
}

// ── Identify font in a user-drawn region of an image ────────────────────────

async function _handleRegion(_srcUrl) {
  console.log('[Intellifont] _handleRegion called with srcUrl:', _srcUrl);
  // Show crosshair selection overlay first (before any loading UI)
  console.log('[Intellifont] About to show region selector...');
  const sel = await _showRegionSelector();
  console.log('[Intellifont] Region selector returned:', sel);
  if (!sel) { console.log('[Intellifont] User cancelled region selection (pressed Esc)'); return; } // user pressed Esc

  console.log('[Intellifont] User selected region, showing loading UI...');
  _showLoading();

  try {
    // Capture the visible tab as a screenshot — bypasses CORS entirely.
    // Works for Google Lens URLs, authenticated images, canvas-rendered text, anything.
    console.log('[Intellifont] Requesting tab screenshot from background...');
    const { dataUrl, error } = await new Promise((resolve) => {
      chrome.runtime.sendMessage({ action: 'capture-tab' }, (resp) => {
        if (chrome.runtime.lastError) { console.error('[Intellifont] Screenshot request error:', chrome.runtime.lastError); resolve({ error: chrome.runtime.lastError.message }); }
        else { console.log('[Intellifont] Screenshot received, size:', resp?.dataUrl?.length); resolve(resp || { error: 'No response from background' }); }
      });
    });

    if (error) { console.error('[Intellifont] Screenshot error:', error); return _showError(`Screenshot failed: ${error}`); }

    // Load screenshot into an Image element
    const screenshot = await new Promise((resolve, reject) => {
      const img = new Image();
      img.onload  = () => resolve(img);
      img.onerror = () => reject(new Error('Failed to load tab screenshot'));
      img.src = dataUrl;
    });

    // sel coords are CSS viewport pixels; captureVisibleTab captures at device pixel ratio
    const dpr = window.devicePixelRatio || 1;
    const cropX = Math.round(sel.x * dpr);
    const cropY = Math.round(sel.y * dpr);
    const cropW = Math.max(1, Math.round(sel.w * dpr));
    const cropH = Math.max(1, Math.round(sel.h * dpr));

    const MIN_H = 120;
    const scale = cropH < MIN_H ? MIN_H / cropH : 1;
    const outW  = Math.round(cropW * scale);
    const outH  = Math.round(cropH * scale);
    console.log(`[Intellifont] Region crop: ${cropX},${cropY} ${cropW}×${cropH} (dpr=${dpr}) → rendered ${outW}×${outH}`);

    const cropped = document.createElement('canvas');
    cropped.width  = outW;
    cropped.height = outH;
    const rCtx = cropped.getContext('2d');
    rCtx.imageSmoothingEnabled = true;
    rCtx.imageSmoothingQuality = 'high';
    rCtx.drawImage(screenshot, cropX, cropY, cropW, cropH, 0, 0, outW, outH);

    const { ImagePipeline } = window.IntellifontImagePipeline;
    const pipeline = new ImagePipeline();
    const result   = await pipeline.analyze(cropped);
    console.log('[Intellifont] Region result:', { metricCount: result.metrics?.length, regionCount: result.regionCount });

    const { metrics, style } = result;
    if (!metrics.length) {
      return _showError('No text found in the selected region.\nTry selecting a larger area that tightly frames the text.');
    }

    const { matches, via } = await _identifyMetrics(metrics, 8);
    _showOverlay(matches, style, `screen region · ${via}`);
  } catch (e) {
    console.error('[Intellifont] Region error:', e);
    _showError(`Error: ${e.message}`);
  }
}

function _findProminentTextElement() {
  const vw = window.innerWidth, vh = window.innerHeight;
  // Heading priority multipliers — strongly prefer h1 > h2 > h3 > others
  const TAG_BOOST = { H1: 8, H2: 4, H3: 2 };
  let best = null, bestScore = -1;

  document.querySelectorAll('h1, h2, h3, p, span, div, a, li').forEach(el => {
    const rect = el.getBoundingClientRect();
    if (rect.width < 10 || rect.height < 5) return;
    if (rect.bottom < 0 || rect.top > vh || rect.right < 0 || rect.left > vw) return;

    // Must have own visible text (not just child elements)
    const ownText = Array.from(el.childNodes)
      .filter(n => n.nodeType === Node.TEXT_NODE)
      .map(n => n.textContent.trim())
      .join('');
    if (!ownText && !el.textContent.trim()) return;

    const st = window.getComputedStyle(el);
    // Skip hidden elements
    if (st.display === 'none' || st.visibility === 'hidden' || parseFloat(st.opacity) < 0.1) return;

    const size   = parseFloat(st.fontSize) || 0;
    const weight = parseFloat(st.fontWeight) || 400;
    const area   = rect.width * rect.height;
    const boost  = TAG_BOOST[el.tagName] || 1;
    // Score: font-size × weight-bonus × heading-boost, area used only as tiebreaker
    const score  = size * (weight >= 600 ? 1.5 : 1) * boost * Math.pow(area, 0.25);

    if (score > bestScore) { bestScore = score; best = el; }
  });

  return best;
}

// ════════════════════════════════════════════════════════════════════════════
// HOVER-TO-IDENTIFY (Phase 5)
//
// Toggle with Ctrl+Shift+F. While active, hovering any text element shows a
// mini tooltip with the *actually-rendered* font family + size. Clicking the
// element runs full identification (CanvasDNA + optional @font-face visual ID).
// ════════════════════════════════════════════════════════════════════════════

let _hoverEnabled = false;
let _hoverTimer   = null;
let _hoverTip     = null;
let _lastHoverEl  = null;

/** Split a CSS font-family value into individual families, respecting quotes. */
function _parseFamilyStack(raw) {
  const out = []; let cur = '', q = null;
  for (const ch of raw) {
    if (q) { if (ch === q) q = null; else cur += ch; }
    else if (ch === '"' || ch === "'") q = ch;
    else if (ch === ',') { out.push(cur); cur = ''; }
    else cur += ch;
  }
  if (cur.trim()) out.push(cur);
  return out.map(s => s.trim().replace(/^["']|["']$/g, '').trim()).filter(Boolean);
}

/**
 * Resolve the font family actually rendering for an element.
 * getComputedStyle already resolves CSS var() references, so we just parse the
 * resolved stack and find the first family the browser reports as loaded.
 */
function _getActiveFontFamily(el) {
  const cs    = window.getComputedStyle(el);
  const stack = _parseFamilyStack(cs.fontFamily || '');
  const size  = cs.fontSize || '16px';
  for (const fam of stack) {
    try { if (document.fonts.check(`${size} "${fam}"`)) return { family: fam, stack, resolved: true }; }
    catch { /* invalid font shorthand — ignore */ }
  }
  return { family: stack[0] || null, stack, resolved: false };
}

/** Find the @font-face src URL for a family by scanning same-origin stylesheets. */
function _findFontFaceSrc(family) {
  const want = family.toLowerCase();
  for (const sheet of document.styleSheets) {
    let rules;
    try { rules = sheet.cssRules; } catch { continue; } // cross-origin sheet — inaccessible
    if (!rules) continue;
    for (const rule of rules) {
      const isFontFace = (typeof CSSFontFaceRule !== 'undefined' && rule instanceof CSSFontFaceRule) || rule.type === 5;
      if (!isFontFace) continue;
      const fam = (rule.style.getPropertyValue('font-family') || '').trim().replace(/^["']|["']$/g, '');
      if (fam.toLowerCase() !== want) continue;
      const src = rule.style.getPropertyValue('src') || '';
      // Prefer ttf/otf/woff (parseable) over woff2
      const urls = [...src.matchAll(/url\(\s*["']?([^"')]+)["']?\s*\)/gi)].map(m => m[1]);
      const ranked = urls.sort((a, b) => _srcRank(a) - _srcRank(b));
      if (ranked[0]) { try { return new URL(ranked[0], sheet.href || location.href).href; } catch { return ranked[0]; } }
    }
  }
  return null;
}
function _srcRank(u) {
  const x = u.toLowerCase();
  if (x.includes('.ttf'))  return 0;
  if (x.includes('.otf'))  return 1;
  if (x.includes('.woff2')) return 3;
  if (x.includes('.woff')) return 2;
  return 5;
}

function _ensureTip() {
  if (_hoverTip) return _hoverTip;
  const host = document.createElement('div');
  host.id = 'intellifont-hover-host';
  host.style.cssText = 'position:fixed;z-index:2147483646;pointer-events:none;top:0;left:0';
  document.documentElement.appendChild(host);
  const shadow = host.attachShadow({ mode: 'open' });
  const tip = document.createElement('div');
  tip.style.cssText =
    "all:initial;font-family:-apple-system,BlinkMacSystemFont,'Segoe UI',sans-serif;" +
    'background:#1e1b4b;color:#e0e7ff;border:1px solid #4f46e5;border-radius:8px;' +
    'padding:6px 10px;font-size:12px;line-height:1.35;box-shadow:0 4px 16px rgba(0,0,0,.5);' +
    'max-width:260px;white-space:nowrap;overflow:hidden;text-overflow:ellipsis';
  shadow.appendChild(tip);
  _hoverTip = { host, tip };
  return _hoverTip;
}

function _positionTip(x, y) {
  const { host } = _ensureTip();
  const ox = Math.min(x + 14, window.innerWidth - 270);
  const oy = Math.min(y + 16, window.innerHeight - 60);
  host.style.transform = `translate(${Math.max(4, ox)}px, ${Math.max(4, oy)}px)`;
}

function _hideTip() {
  if (_hoverTip) { _hoverTip.host.remove(); _hoverTip = null; }
}

function _onHoverMove(e) {
  if (!_hoverEnabled) return;
  const el = e.target;
  if (!(el instanceof Element)) return;
  // Ignore our own overlay/tooltip
  if (el.id && (el.id.startsWith('intellifont-'))) return;

  clearTimeout(_hoverTimer);
  _hoverTimer = setTimeout(() => {
    // Only show for elements with their own visible text
    const hasText = Array.from(el.childNodes).some(n => n.nodeType === Node.TEXT_NODE && n.textContent.trim());
    if (!hasText) { _hideTip(); return; }

    const { family, resolved, stack } = _getActiveFontFamily(el);
    if (!family) { _hideTip(); return; }
    const cs = window.getComputedStyle(el);
    const px = parseFloat(cs.fontSize) || 0;
    const wlabel = _weightLabel(parseInt(cs.fontWeight, 10) || 400);
    const italic = cs.fontStyle && cs.fontStyle !== 'normal';
    const faceSrc = _findFontFaceSrc(family);

    const { tip } = _ensureTip();
    tip.innerHTML =
      `<b style="color:#a5b4fc">${_esc(family)}</b>` +
      `<span style="color:#818cf8"> · ${Math.round(px)}px · ${wlabel}${italic ? ' Italic' : ''}</span>` +
      (faceSrc ? `<br><span style="color:#6366f1;font-size:10px">@font-face web font · click to identify</span>`
               : (resolved ? '' : `<br><span style="color:#6366f1;font-size:10px">fallback: ${_esc(stack[0] || '')}</span>`));
    _positionTip(e.clientX, e.clientY);
  }, 600);
}

function _weightLabel(w) {
  return ({ 100: 'Thin', 200: 'ExtraLight', 300: 'Light', 400: 'Regular',
            500: 'Medium', 600: 'SemiBold', 700: 'Bold', 800: 'ExtraBold', 900: 'Black' })[Math.round(w / 100) * 100] || 'Regular';
}

async function _onHoverClick(e) {
  if (!_hoverEnabled) return;
  const el = e.target;
  if (!(el instanceof Element) || (el.id && el.id.startsWith('intellifont-'))) return;
  const hasText = Array.from(el.childNodes).some(n => n.nodeType === Node.TEXT_NODE && n.textContent.trim());
  if (!hasText) return;

  e.preventDefault();
  e.stopPropagation();
  _hideTip();
  _setHoverMode(false); // exit hover mode once an element is chosen
  await _identifyElement(el);
}

/** Full identification for a specific element (CanvasDNA + @font-face visual ID). */
async function _identifyElement(el) {
  _showLoading();
  try {
    const { family } = _getActiveFontFamily(el);
    if (!family) return _showError('No font family on this element.');

    const metrics = window.CanvasDNA.analyzeFont(family, 'RQWMag');
    if (!metrics.length) return _showError(`Could not render font: "${family}"`);

    const { matches: idMatches, via } = await _identifyMetrics(metrics, 8);
    let matches = idMatches;
    let source  = `"${family}" · element · ${via}`;

    // Try @font-face download → server visual ID (definitive when reachable)
    let webConfirmed = null;
    const faceSrc = _findFontFaceSrc(family);
    if (faceSrc) {
      webConfirmed = await _identifyWebFont(faceSrc).catch(() => null);
      if (webConfirmed) source = `"${family}" · @font-face · visual ID`;
    }

    // Pin the CSS-declared name as #1
    const norm = s => s.toLowerCase().replace(/\s+/g, '');
    const filtered = matches.filter(m => norm(m.family) !== norm(family)).slice(0, 7);
    matches = [
      { family, subfamily: webConfirmed ? `confirmed: ${webConfirmed.family} (${Math.round(webConfirmed.confidence*100)}%)` : null,
        confidence: 1.0, matchedChars: [], _fromCss: true },
      ...filtered,
    ];

    _showOverlay(matches, null, source);
  } catch (e) {
    _showError(`Error: ${e.message}`);
  }
}

/** Fetch a font file and identify it via the server's visual engine. */
async function _identifyWebFont(url) {
  const cfg = await chrome.storage.sync.get(['serverUrl']);
  const server = cfg.serverUrl;
  if (!server) return null; // need server for buffer visual ID

  const resp = await fetch(url);                 // same-origin or CORS-enabled CDN
  if (!resp.ok) return null;
  const buf = await resp.arrayBuffer();

  const r = await fetch(`${server.replace(/\/$/, '')}/api/identify-document`, {
    method: 'POST',
    headers: { 'Content-Type': 'application/octet-stream' },
    body: buf,
  });
  if (!r.ok) return null;
  const data = await r.json();
  const top = data.fonts && data.fonts[0];
  return top && top.confidence != null ? { family: top.family, confidence: top.confidence } : null;
}

function _toast(msg) {
  const host = document.createElement('div');
  host.style.cssText = 'position:fixed;z-index:2147483647;bottom:24px;left:50%;pointer-events:none';
  document.documentElement.appendChild(host);
  const sh = host.attachShadow({ mode: 'open' });
  const t = document.createElement('div');
  t.textContent = msg;
  t.style.cssText =
    "all:initial;font-family:-apple-system,sans-serif;transform:translateX(-50%);" +
    'background:#312e81;color:#e0e7ff;border:1px solid #4f46e5;border-radius:20px;' +
    'padding:8px 16px;font-size:13px;box-shadow:0 4px 16px rgba(0,0,0,.5);white-space:nowrap';
  sh.appendChild(t);
  setTimeout(() => host.remove(), 1800);
}

function _setHoverMode(on) {
  _hoverEnabled = on;
  if (on) {
    document.addEventListener('mousemove', _onHoverMove, true);
    document.addEventListener('click', _onHoverClick, true);
    _toast('Intellifont: hover a text element to identify its font  ·  Esc to exit');
  } else {
    document.removeEventListener('mousemove', _onHoverMove, true);
    document.removeEventListener('click', _onHoverClick, true);
    clearTimeout(_hoverTimer);
    _hideTip();
  }
}

// Ctrl+Shift+F toggles hover mode; Esc exits it.
document.addEventListener('keydown', (e) => {
  if (e.ctrlKey && e.shiftKey && (e.key === 'F' || e.key === 'f')) {
    e.preventDefault();
    _setHoverMode(!_hoverEnabled);
  } else if (e.key === 'Escape' && _hoverEnabled) {
    _setHoverMode(false);
    _toast('Intellifont: hover mode off');
  }
}, true);

// Allow the popup/background to toggle hover mode too.
chrome.runtime.onMessage.addListener((msg) => {
  if (msg && msg.action === 'toggle-hover-mode') _setHoverMode(!_hoverEnabled);
});
