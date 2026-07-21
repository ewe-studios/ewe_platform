/**
 * floating-nav.js — Platform overlay toolbar (F29 Stage 4).
 *
 * Injected by the ScriptInjector into every WebView. Renders a floating
 * navigation bar at the bottom of the screen with back, home, app-switcher,
 * and refresh buttons. Styled via CSS — zero native bridge overhead.
 *
 * The toolbar auto-hides when scrolling down and reappears on scroll-up.
 * On desktop, it renders at the top; on mobile, at the bottom.
 *
 * Configurable via CSS custom properties:
 *   --fn-bg: toolbar background (default: #112240)
 *   --fn-color: text/icon color (default: #64ffda)
 *   --fn-position: 'top' or 'bottom' (default: bottom on mobile, top on desktop)
 */

(function () {
  'use strict';

  if (window.__eweFloatingNav) return; // singleton guard
  window.__eweFloatingNav = true;

  // ── Config ───────────────────────────────────────────────────────────

  var isMobile = /Android|iPhone|iPad|iPod/i.test(navigator.userAgent);
  var position = isMobile ? 'bottom' : 'top';
  var visible = true;
  var lastScrollY = 0;
  var scrollThreshold = 40;
  var hideTimer = null;

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
    '  background: var(--fn-bg, #112240);' +
    '  color: var(--fn-color, #64ffda);' +
    '  border-' + (position === 'bottom' ? 'top' : 'bottom') +
    ': 1px solid #233554;' +
    '  font-family: system-ui, sans-serif;' +
    '  font-size: 14px;' +
    '  transition: transform 0.25s ease;' +
    '  min-height: 44px;' +
    '  box-shadow: 0 -2px 8px rgba(0,0,0,0.3);' +
    '}' +
    '#__ewe_floating_nav.hidden {' +
    '  transform: ' + (position === 'bottom' ? 'translateY(100%)' : 'translateY(-100%)') + ';' +
    '}' +
    '#__ewe_floating_nav button {' +
    '  background: transparent;' +
    '  border: 1px solid #233554;' +
    '  border-radius: 6px;' +
    '  color: var(--fn-color, #64ffda);' +
    '  padding: 6px 12px;' +
    '  font-size: 16px;' +
    '  cursor: pointer;' +
    '  min-width: 36px;' +
    '  min-height: 32px;' +
    '}' +
    '#__ewe_floating_nav button:hover {' +
    '  background: #233554;' +
    '}' +
    '#__ewe_floating_nav button:active {' +
    '  background: #1a2a4a;' +
    '  transform: scale(0.95);' +
    '}' +
    '#__ewe_nav_title {' +
    '  flex: 1;' +
    '  text-align: center;' +
    '  font-size: 13px;' +
    '  font-weight: 500;' +
    '  overflow: hidden;' +
    '  text-overflow: ellipsis;' +
    '  white-space: nowrap;' +
    '  padding: 0 8px;' +
    '  color: #ccd6f6;' +
    '}' +
    'body.ewe-floating-nav-bottom {' +
    '  padding-bottom: 52px;' +
    '}' +
    'body.ewe-floating-nav-top {' +
    '  padding-top: 52px;' +
    '}';
  document.head.appendChild(style);

  // ── Button handlers ──────────────────────────────────────────────────

  document.getElementById('__ewe_nav_back').onclick = function () {
    // Dispatch a custom event that the platform shell can listen for,
    // or use history.back() as fallback.
    var ev = new CustomEvent('ewe:nav:back', { bubbles: true });
    document.dispatchEvent(ev);
    if (!ev.defaultPrevented) {
      history.back();
    }
  };

  document.getElementById('__ewe_nav_home').onclick = function () {
    var ev = new CustomEvent('ewe:nav:home', { bubbles: true, detail: { route: '/' } });
    document.dispatchEvent(ev);
    if (!ev.defaultPrevented) {
      location.href = '/';
    }
  };

  document.getElementById('__ewe_nav_refresh').onclick = function () {
    var ev = new CustomEvent('ewe:nav:refresh', { bubbles: true });
    document.dispatchEvent(ev);
    if (!ev.defaultPrevented) {
      location.reload();
    }
  };

  document.getElementById('__ewe_nav_apps').onclick = function () {
    var ev = new CustomEvent('ewe:nav:apps', { bubbles: true });
    document.dispatchEvent(ev);
  };

  // ── Auto-hide on scroll ──────────────────────────────────────────────

  window.addEventListener('scroll', function () {
    var currentScrollY = window.scrollY;
    if (Math.abs(currentScrollY - lastScrollY) < scrollThreshold) return;

    if (currentScrollY > lastScrollY && visible) {
      // Scrolling down — hide
      hide();
    } else if (currentScrollY < lastScrollY && !visible) {
      // Scrolling up — show
      show();
    }
    lastScrollY = currentScrollY;
  }, { passive: true });

  function show() {
    toolbar.classList.remove('hidden');
    visible = true;
    clearTimeout(hideTimer);
    // Auto-hide again after 3 seconds of no scroll
    hideTimer = setTimeout(function () {
      if (visible && window.scrollY > 200) hide();
    }, 3000);
  }

  function hide() {
    toolbar.classList.add('hidden');
    visible = false;
  }

  // ── Public API ───────────────────────────────────────────────────────

  window.__eweNav = {
    /** Set the title in the nav bar. */
    setTitle: function (title) {
      document.getElementById('__ewe_nav_title').textContent = title || '';
    },
    /** Show the toolbar. */
    show: show,
    /** Hide the toolbar. */
    hide: function () {
      toolbar.classList.add('hidden');
      visible = false;
      clearTimeout(hideTimer);
    },
    /** Whether the toolbar is currently visible. */
    isVisible: function () { return visible; },
    /** Set the position ('top' or 'bottom'). */
    setPosition: function (pos) {
      position = pos;
      document.body.classList.remove('ewe-floating-nav-bottom', 'ewe-floating-nav-top');
      document.body.classList.add('ewe-floating-nav-' + pos);
      toolbar.style.top = pos === 'top' ? '0' : '';
      toolbar.style.bottom = pos === 'bottom' ? '0' : '';
    },
  };
})();
