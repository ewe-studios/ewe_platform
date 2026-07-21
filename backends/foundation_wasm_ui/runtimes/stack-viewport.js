/**
 * stack-viewport.js — WebView stack screenshot-swap (F35).
 *
 * Injected by ScriptInjector. Provides window.__eweStack with:
 *   captureScreenshot() → PNG dataURL
 *   showOverlay(slotIndex, dataURL) → shows screenshot for back nav
 *   hideOverlay() → removes screenshot overlay, shows live page
 *
 * On presentation::Push, the platform calls captureScreenshot() before
 * navigating away. On pop(), it calls showOverlay() to flash the previous
 * page's screenshot instantly, then navigates in the background.
 */
(function() {
  'use strict';
  if (window.__eweStack) return;
  window.__eweStack = true;

  var overlay = null;
  var overlayImg = null;

  function ensureOverlay() {
    if (overlay) return;
    overlay = document.createElement('div');
    overlay.id = '__ewe_screenshot_overlay';
    overlay.style.cssText =
      'position:fixed;top:0;left:0;width:100%;height:100%;z-index:99999;' +
      'background:#000;display:flex;align-items:center;justify-content:center;' +
      'transition:opacity 0.15s ease';
    overlayImg = document.createElement('img');
    overlayImg.style.cssText = 'max-width:100%;max-height:100%;object-fit:contain';
    overlay.appendChild(overlayImg);
    overlay.onclick = function() { hideOverlay(); };
  }

  window.__eweViewport = {
    /**
     * Capture the current page as a PNG dataURL via canvas.
     * Falls back to html2canvas-style if native capture not available.
     */
    captureScreenshot: function() {
      // Use Tauri's built-in screenshot if available, else html2canvas
      if (window.__TAURI_INTERNALS__ && window.__TAURI_INTERNALS__.invoke) {
        // Tauri provides captureScreenshot in 2.4+
        return null; // host will call it via Rust
      }
      // Browser fallback: use html2canvas if loaded
      if (window.html2canvas) {
        return new Promise(function(resolve) {
          window.html2canvas(document.body).then(function(canvas) {
            resolve(canvas.toDataURL('image/png'));
          });
        });
      }
      return null;
    },

    /**
     * Show a screenshot overlay. Used on back-navigation to flash
     * the previous page's screenshot before the WebView finishes loading.
     */
    showOverlay: function(dataUrl) {
      ensureOverlay();
      if (dataUrl) overlayImg.src = dataUrl;
      overlay.style.opacity = '1';
      overlay.style.display = 'flex';
      document.body.appendChild(overlay);
    },

    /**
     * Hide the screenshot overlay, revealing the live WebView beneath.
     */
    hideOverlay: function() {
      if (!overlay) return;
      overlay.style.opacity = '0';
      setTimeout(function() {
        if (overlay) overlay.style.display = 'none';
      }, 150);
    },

    /** Stack depth for JS consumption */
    depth: 0,
    activeRoute: '',
    lastPresentation: 'Morph',
  };
})();
