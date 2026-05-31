'use strict';

// ── Settings persistence ─────────────────────────────────────────────────────
(async () => {
  const { serverUrl = '', apiKey = '' } = await chrome.storage.sync.get(['serverUrl', 'apiKey']);
  document.getElementById('inp-url').value = serverUrl;
  document.getElementById('inp-key').value = apiKey;
})();

document.getElementById('btn-save').addEventListener('click', async () => {
  const url = document.getElementById('inp-url').value.trim().replace(/\/$/, '');
  const key = document.getElementById('inp-key').value.trim();
  await chrome.storage.sync.set({ serverUrl: url, apiKey: key });
  const el = document.getElementById('save-status');
  el.textContent = 'Saved ✓';
  setTimeout(() => { el.textContent = ''; }, 2000);
});


document.getElementById('btn-page').addEventListener('click', async () => {
  const [tab] = await chrome.tabs.query({ active: true, currentWindow: true });
  if (!tab?.id) return;
  chrome.tabs.sendMessage(tab.id, { action: 'identify-page' }).catch(() => {
    document.getElementById('status').textContent =
      'Cannot run on this page. Navigate to a regular website first.';
  });
  window.close();
});

document.getElementById('btn-hover').addEventListener('click', async () => {
  const [tab] = await chrome.tabs.query({ active: true, currentWindow: true });
  if (!tab?.id) return;
  chrome.tabs.sendMessage(tab.id, { action: 'toggle-hover-mode' }).catch(() => {
    document.getElementById('status').textContent =
      'Cannot run on this page. Navigate to a regular website first.';
  });
  window.close();
});

document.getElementById('btn-help').addEventListener('click', () => {
  document.getElementById('status').textContent =
    'Right-click any image on a webpage and choose "Identify font in image".';
});

// Show database stats by attempting to load it
(async () => {
  const statusEl = document.getElementById('stat-status');
  const fontsEl  = document.getElementById('stat-fonts');
  try {
    const url  = chrome.runtime.getURL('data/glyph_signatures.gz');
    const resp = await fetch(url, { method: 'HEAD' });
    if (resp.ok) {
      const size = resp.headers.get('content-length');
      statusEl.textContent = 'Ready';
      statusEl.style.color = '#86efac';
      fontsEl.textContent  = size ? `${(size / 1e6).toFixed(1)} MB loaded` : '2000+';
    } else {
      throw new Error('not found');
    }
  } catch {
    statusEl.textContent = 'Database missing — run build:browser-db';
    statusEl.style.color = '#fca5a5';
    fontsEl.textContent  = '—';
  }
})();
