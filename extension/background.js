'use strict';
/**
 * Intellifont — background service worker (Manifest V3)
 */

chrome.runtime.onInstalled.addListener(() => {
  // Remove stale items before re-creating (safe on both install and reload)
  chrome.contextMenus.removeAll(() => {
    chrome.contextMenus.create({
      id:       'intellifont-image',
      title:    'Identify font in image',
      contexts: ['image'],
    });

    chrome.contextMenus.create({
      id:       'intellifont-region',
      title:    'Identify font in image region…',
      contexts: ['image'],
    });

    chrome.contextMenus.create({
      id:       'intellifont-ai-detect',
      title:    'Is text in image AI-generated?',
      contexts: ['image'],
    });

    chrome.contextMenus.create({
      id:       'intellifont-forensics',
      title:    'Check document timeline (forgery detection)',
      contexts: ['image'],
    });

    chrome.contextMenus.create({
      id:       'intellifont-page',
      title:    'Identify font on page',
      contexts: ['page', 'selection', 'link'],
    });
  });
});

function _send(tabId, msg) {
  chrome.tabs.sendMessage(tabId, msg).catch(() => {
    chrome.scripting.executeScript({
      target: { tabId },
      files: ['lib/canvas-dna.js', 'lib/image-pipeline.js', 'lib/matcher.js', 'content.js'],
    }).then(() => {
      setTimeout(() => chrome.tabs.sendMessage(tabId, msg).catch(() => {}), 100);
    }).catch(() => {});
  });
}

chrome.runtime.onMessage.addListener((msg, sender, sendResponse) => {
  if (msg.action === 'capture-tab') {
    // Ping chrome API every 20s to prevent MV3 service worker idle termination
    // while captureVisibleTab is pending (normally < 100ms, but safety net).
    const keepAlive = setInterval(() => chrome.runtime.getPlatformInfo(() => {}), 20_000);
    chrome.tabs.captureVisibleTab(sender.tab.windowId, { format: 'png' }, (dataUrl) => {
      clearInterval(keepAlive);
      if (chrome.runtime.lastError) sendResponse({ error: chrome.runtime.lastError.message });
      else sendResponse({ dataUrl });
    });
    return true; // keep channel open for async response
  }
});

chrome.contextMenus.onClicked.addListener((info, tab) => {
  if (!tab?.id) return;

  if (info.menuItemId === 'intellifont-image') {
    _send(tab.id, { action: 'identify-image',  srcUrl: info.srcUrl });
  }
  if (info.menuItemId === 'intellifont-region') {
    _send(tab.id, { action: 'identify-region', srcUrl: info.srcUrl });
  }
  if (info.menuItemId === 'intellifont-ai-detect') {
    _send(tab.id, { action: 'detect-ai-text',  srcUrl: info.srcUrl });
  }
  if (info.menuItemId === 'intellifont-forensics') {
    _send(tab.id, { action: 'forensics-image', srcUrl: info.srcUrl });
  }
  if (info.menuItemId === 'intellifont-page') {
    _send(tab.id, { action: 'identify-page', selectionText: info.selectionText || null });
  }
});
