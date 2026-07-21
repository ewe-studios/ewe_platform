/**
 * floating-nav.js — Platform overlay toolbar (F29 Stage 4).
 *
 * Renders a floating navigation bar with back, home, and refresh buttons.
 * Styled via CSS — zero native bridge overhead. Waits for DOMContentLoaded
 * so it works whether placed in <head> or <body>.
 */

(function () {
  'use strict';

  if (window.__eweFloatingNav) return;
  window.__eweFloatingNav = true;

  function init() {
    var isMobile = /Android|iPhone|iPad|iPod/i.test(navigator.userAgent);
    var position = isMobile ? 'bottom' : 'top';

    // ── Toolbar DOM ──────────────────────────────────────────────────────
    var toolbar = document.createElement('div');
    toolbar.id = '__ewe_floating_nav';
    toolbar.innerHTML =
      '<button id="__ewe_nav_back" title="Back">←</button>' +
      '<button id="__ewe_nav_home" title="Home">⌂</button>' +
      '<span id="__ewe_nav_title"></span>' +
      '<button id="__ewe_nav_refresh" title="Refresh">↻</button>' +
      '<button id="__ewe_nav_apps" title="Apps">≡</button>';

    document.body.appendChild(toolbar);
    document.body.classList.add('ewe-floating-nav-' + position);

    // ── Styles ───────────────────────────────────────────────────────────
    var style = document.createElement('style');
    style.textContent =
      '#__ewe_floating_nav {' +
      '  position: fixed; ' + (position === 'bottom' ? 'bottom:0;' : 'top:0;') +
      '  left:0; right:0; z-index:99990;' +
      '  display:flex; align-items:center; justify-content:space-between;' +
      '  padding:6px 12px;' +
      '  background: #f5f5f5;' +
      '  color: #333;' +
      '  border-' + (position === 'bottom' ? 'top' : 'bottom') + ': 1px solid #ddd;' +
      '  font-family: system-ui, sans-serif;' +
      '  font-size: 14px;' +
      '  min-height: 44px;' +
      '  box-shadow: 0 1px 4px rgba(0,0,0,0.1);' +
      '}' +
      '#__ewe_floating_nav.hidden { display: none; }' +
      '#__ewe_floating_nav button {' +
      '  background: transparent;' +
      '  border: 1px solid #ccc;' +
      '  border-radius: 6px;' +
      '  color: #333;' +
      '  padding: 6px 12px;' +
      '  font-size: 16px;' +
      '  cursor: pointer;' +
      '  min-width: 36px;' +
      '  min-height: 32px;' +
      '}' +
      '#__ewe_floating_nav button:hover { background: #e0e0e0; }' +
      '#__ewe_floating_nav button:active { background: #ccc; }' +
      '#__ewe_nav_title {' +
      '  flex:1; text-align:center; font-size:13px; font-weight:500;' +
      '  overflow:hidden; text-overflow:ellipsis; white-space:nowrap;' +
      '  padding:0 8px; color:#333;' +
      '}' +
      'body.ewe-floating-nav-bottom { padding-bottom: 52px; }' +
      'body.ewe-floating-nav-top { padding-top: 52px; }';
    document.head.appendChild(style);

    // ── Button handlers ──────────────────────────────────────────────────
    document.getElementById('__ewe_nav_back').onclick = function () {
      window.history.back();
    };

    document.getElementById('__ewe_nav_home').onclick = function () {
      window.location.href = 'ewe://localhost/app/';
    };

    document.getElementById('__ewe_nav_refresh').onclick = function () {
      window.location.reload();
    };

    document.getElementById('__ewe_nav_apps').onclick = function () {
      window.location.href = 'ewe://localhost/';
    };

    // ── Public API ───────────────────────────────────────────────────────
    window.__eweNav = {
      setTitle: function (t) { document.getElementById('__ewe_nav_title').textContent = t || ''; },
      show: function () { toolbar.classList.remove('hidden'); },
      hide: function () { toolbar.classList.add('hidden'); },
      isVisible: function () { return !toolbar.classList.contains('hidden'); },
      setPosition: function (pos) {
        toolbar.style.top = pos === 'top' ? '0' : '';
        toolbar.style.bottom = pos === 'bottom' ? '0' : '';
      },
    };
  }

  if (document.readyState === 'loading') {
    document.addEventListener('DOMContentLoaded', init);
  } else {
    init();
  }
})();
