// Platform scheme interceptor — foundation_wasm_ui (spec-52).
//
// Android WebView has no API for custom URI scheme handlers. wry works around
// this by converting custom-scheme URLs to HTTP URLs during initial load and
// intercepting them in shouldInterceptRequest. But link clicks and
// programmatic navigations to ewe:// URLs are NOT converted — Android's
// shouldOverrideUrlLoading lets them fall through to the OS, which has no
// handler for them.
//
// This script intercepts clicks and navigations to platform schemes
// (ewe://, foundation://, platform://) and rewrites them to
// http://{scheme}.localhost/... — the format wry's shouldInterceptRequest
// recognizes and routes to the registered protocol handler.
//
// Desktop engines (WebKit, WebView2) support custom schemes natively through
// register_uri_scheme_protocol — this script is a no-op on those platforms.
//
// Usage: include this script BEFORE any other application code.
// foundation_platform's builder injects it automatically; foundation_wasm_ui
// embeds it in the core JS runtime for standalone WASM apps.

;(function () {
  'use strict';

  var PLATFORM_SCHEMES = ['ewe', 'foundation', 'platform'];
  var PREFIX = 'http://';

  function isPlatformScheme(href) {
    if (!href) return false;
    for (var i = 0; i < PLATFORM_SCHEMES.length; i++) {
      var prefix = PLATFORM_SCHEMES[i] + '://';
      if (href.indexOf(prefix) === 0) return PLATFORM_SCHEMES[i];
    }
    return null;
  }

  function toWorkaroundUrl(href) {
    var scheme = isPlatformScheme(href);
    if (!scheme) return null;
    return href.replace(scheme + '://', PREFIX + scheme + '.');
  }

  function isAndroid() {
    return /android/i.test(navigator.userAgent);
  }

  // Only activate on Android — desktop engines handle custom schemes natively.
  if (!isAndroid()) return;

  // ── Intercept <a> clicks ──────────────────────────────────────────
  document.addEventListener('click', function (e) {
    var a = e.target.closest('a');
    if (!a) return;
    var href = a.getAttribute('href') || a.href;
    var newUrl = toWorkaroundUrl(href);
    if (!newUrl) return;
    e.preventDefault();
    e.stopImmediatePropagation();
    location.href = newUrl;
  }, true); // capture phase — beats user-land listeners

  // ── Intercept programmatic navigations ────────────────────────────
  var _origAssign = location.assign;
  var _origReplace = location.replace;
  var _origSetHref = Object.getOwnPropertyDescriptor(Location.prototype, 'href');

  function wrap(fn) {
    return function (url) {
      var rewritten = toWorkaroundUrl(url);
      if (rewritten) {
        url = rewritten;
      }
      return fn.call(this, url);
    };
  }

  location.assign = wrap(_origAssign);
  location.replace = wrap(_origReplace);

  if (_origSetHref && _origSetHref.set) {
    var _set = _origSetHref.set;
    Object.defineProperty(location, 'href', {
      get: _origSetHref.get,
      set: function (url) {
        var rewritten = toWorkaroundUrl(url);
        _set.call(this, rewritten || url);
      },
      configurable: true,
      enumerable: true,
    });
  }

  // ── Intercept window.open ─────────────────────────────────────────
  var _origOpen = window.open;
  window.open = function (url) {
    var rewritten = toWorkaroundUrl(url);
    if (rewritten) {
      arguments[0] = rewritten;
    }
    return _origOpen.apply(this, arguments);
  };
})();
