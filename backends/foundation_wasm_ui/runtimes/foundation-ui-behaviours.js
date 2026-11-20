// Browser implementations of the component machinery in foundation_ui_components.
import { BehaviourId } from "./foundation-ui-wire.js";

function composite(scope){
  var root = scope.parent();
  if (!root || root.__composite) return; root.__composite = true;
  var orientation = root.getAttribute('data-orientation') || 'horizontal';
  var loop = root.getAttribute('data-loop') !== 'false';
  function items(){
    return Array.prototype.slice.call(root.querySelectorAll('[data-composite-item]'))
      .filter(function(el){ return !el.hasAttribute('disabled') && el.getAttribute('aria-disabled') !== 'true'; });
  }
  function seed(){
    var list = items();
    var active = list.findIndex(function(el){ return el.hasAttribute('data-composite-active'); });
    if (active < 0) active = 0;
    list.forEach(function(el,i){ el.tabIndex = i === active ? 0 : -1; });
  }
  function move(idx){
    var list = items();
    if (idx < 0) idx = loop ? list.length - 1 : 0;
    if (idx >= list.length) idx = loop ? 0 : list.length - 1;
    list.forEach(function(el,i){ el.tabIndex = i === idx ? 0 : -1; });
    if (list[idx]) {
      list[idx].focus();
      if (root.hasAttribute('data-composite-select')) list[idx].click();
    }
  }
  var nextKey = orientation === 'vertical' ? 'ArrowDown' : 'ArrowRight';
  var prevKey = orientation === 'vertical' ? 'ArrowUp' : 'ArrowLeft';
  // Typeahead (M5; port of floating-ui useTypeahead): 750ms buffer reset,
  // locale-lowercased prefix match, same-letter cycling.
  var buf = '', bufAt = 0, matchIdx = -1;
  function label(el){ return (el.getAttribute('data-label') || el.textContent || '').trim().toLowerCase(); }
  function typeahead(ch){
    var list = items();
    if (!list.length) return;
    var now = Date.now();
    if (now - bufAt > 750) { buf = ''; }
    bufAt = now;
    var cur = list.indexOf(document.activeElement);
    if (buf === '') matchIdx = cur;
    var allowRapid = list.every(function(el){ var t = label(el); return t.length < 2 || t[0] !== t[1]; });
    if (allowRapid && buf === ch.toLowerCase()) { buf = ''; }
    buf += ch.toLowerCase();
    var start = (matchIdx >= 0 ? matchIdx : cur) + 1;
    for (var k = 0; k < list.length; k++) {
      var i = (((start + k) % list.length) + list.length) % list.length;
      if (label(list[i]).indexOf(buf) === 0) { matchIdx = i; move(i); return; }
    }
    buf = '';
  }
  scope.addEvent(root, 'keydown', function(e){
    var list = items();
    var cur = list.indexOf(document.activeElement);
    var next = null;
    if (e.key === nextKey) next = cur + 1;
    else if (e.key === prevKey) next = cur - 1;
    else if (e.key === 'Home') next = 0;
    else if (e.key === 'End') next = list.length - 1;
    else if (e.key.length === 1 && !e.ctrlKey && !e.metaKey && !e.altKey) { e.preventDefault(); typeahead(e.key); return; }
    else return;
    if (cur < 0 && e.key !== 'Home' && e.key !== 'End') return;
    e.preventDefault();
    move(next);
  });
  seed();
}

function dialog(scope){
  var dlg = scope.parent();
  if (!dlg || dlg.__dialog || !dlg.showModal) return; dlg.__dialog = true;
  var modal = dlg.getAttribute('data-dialog-mode') !== 'nonmodal';
  function sync(){
    var want = dlg.hasAttribute('data-open');
    if (want && !dlg.open) { try { modal ? dlg.showModal() : dlg.show(); } catch (e) {} }
    else if (!want && dlg.open) { dlg.close(); }
  }
  function requestClose(reason){
    var act = dlg.querySelector('[data-dialog-close]');
    if (act) { act.setAttribute('data-dismiss-reason', reason); act.click(); }
  }
  scope.addEvent(dlg, 'cancel', function(e){ e.preventDefault(); requestClose('escape-key'); });
  // Backdrop (light) dismiss: a click landing on the dialog box itself (not its
  // content) is the backdrop. Opt out with data-dialog-dismissable="false".
  scope.addEvent(dlg, 'click', function(e){
    if (e.target === dlg && dlg.getAttribute('data-dialog-dismissable') !== 'false') {
      requestClose('outside-press');
    }
  });
  sync();
  var mo = new MutationObserver(sync);
  scope.onDispose(() => mo.disconnect());
  mo.observe(dlg, { attributes: true, attributeFilter: ['data-open', 'data-closed'] });
}

function dismiss(scope){
  var popup = scope.parent();
  if (!popup || popup.__dismiss) return; popup.__dismiss = true;
  var doc = popup.ownerDocument || document;
  function dismiss(reason){
    var act = popup.querySelector('[data-dismiss-action]');
    if (!act) return;
    act.setAttribute('data-dismiss-reason', reason);
    act.click();
  }
  if (popup.getAttribute('data-dismiss-escape') !== 'false') {
    scope.addEvent(doc, 'keydown', function(e){
      if (e.key !== 'Escape' && e.key !== 'Esc') return;
      e.preventDefault();
      dismiss('escape-key');
    });
  }
  if (popup.getAttribute('data-dismiss-outside') !== 'false') {
    scope.addEvent(doc, 'pointerdown', function(e){
      if (popup.contains(e.target)) return;
      var anchor = popup.getAttribute('data-anchor');
      if (anchor) {
        var trig = doc.getElementById(anchor);
        if (trig && trig.contains(e.target)) return;
      }
      dismiss('outside-press');
    });
  }
}

function focus_trap(scope){
  var el = scope.parent();
  if (!el || el.__focusTrap) return; el.__focusTrap = true;
  var doc = el.ownerDocument || document;
  var SEL = 'a[href],button:not([disabled]),input:not([disabled]),select:not([disabled]),textarea:not([disabled]),[tabindex]:not([tabindex="-1"])';
  function tabbables(){
    return Array.prototype.slice.call(el.querySelectorAll(SEL)).filter(function(n){
      return n.offsetWidth || n.offsetHeight || n.getClientRects().length;
    });
  }
  var restore = null;
  function activate(){
    restore = doc.activeElement;
    var init = el.getAttribute('data-initial-focus');
    if (init === 'none') return;
    var node = init ? el.querySelector(init) : (tabbables()[0] || el);
    if (node && node.focus) node.focus();
  }
  function deactivate(){
    var node = (restore && restore.focus) ? restore : null;
    if (!node) {
      var anchor = el.getAttribute('data-anchor');
      if (anchor) { var t = doc.getElementById(anchor); if (t) node = t; }
    }
    if (node && node.focus) node.focus();
    restore = null;
  }
  scope.addEvent(el, 'keydown', function(e){
    if (e.key !== 'Tab') return;
    var list = tabbables();
    if (!list.length) { e.preventDefault(); return; }
    var first = list[0], last = list[list.length - 1];
    if (e.shiftKey && doc.activeElement === first) { e.preventDefault(); last.focus(); }
    else if (!e.shiftKey && doc.activeElement === last) { e.preventDefault(); first.focus(); }
  });
  var isOpen = el.hasAttribute('data-open');
  if (isOpen) activate();
  var mo = new MutationObserver(function(){
    var now = el.hasAttribute('data-open');
    if (now === isOpen) return;
    isOpen = now;
    if (now) activate(); else deactivate();
  });
  scope.onDispose(() => mo.disconnect());
  mo.observe(el, { attributes: true, attributeFilter: ['data-open', 'data-closed'] });
}

function slider_drag(scope){
  var root = scope.parent();
  if (!root || root.__slider) return; root.__slider = true;
  var win = (root.ownerDocument && root.ownerDocument.defaultView) || window;
  var control = root.querySelector('[data-slider-control]');
  var input = root.querySelector('[data-slider-input]');
  if (!control || !input) return;
  function num(a){ var v = parseFloat(root.getAttribute(a)); return isNaN(v) ? null : v; }
  var min = num('data-min'); if (min == null) min = 0;
  var max = num('data-max'); if (max == null) max = 100;
  var step = num('data-step') || 1;
  var largeStep = num('data-large-step') || 10;
  var vertical = root.getAttribute('data-orientation') === 'vertical';
  var rtl = root.getAttribute('dir') === 'rtl';
  function roundStep(v){ return Math.round((v - min) / step) * step + min; }
  function clampV(v){ return Math.max(min, Math.min(max, v)); }
  function fingerValue(e){
    var rect = control.getBoundingClientRect(), pct;
    if (vertical) pct = (rect.bottom - e.clientY) / rect.height;
    else pct = rtl ? (rect.right - e.clientX) / rect.width : (e.clientX - rect.left) / rect.width;
    pct = Math.max(0, Math.min(1, pct));
    return clampV(roundStep((max - min) * pct + min));
  }
  function commit(v){ input.value = String(v); input.dispatchEvent(new Event('change', { bubbles: true })); }
  var dragging = false;
  scope.addEvent(control, 'pointerdown', function(e){
    if (e.button) return;
    e.preventDefault(); dragging = true;
    root.setAttribute('data-dragging', '');
    if (control.setPointerCapture) { try { control.setPointerCapture(e.pointerId); } catch (_) {} }
    if (input.focus) input.focus();
    commit(fingerValue(e));
  });
  scope.addEvent(win, 'pointermove', function(e){ if (dragging) commit(fingerValue(e)); });
  scope.addEvent(win, 'pointerup', function(){ if (!dragging) return; dragging = false; root.removeAttribute('data-dragging'); });
  scope.addEvent(input, 'keydown', function(e){
    var cur = parseFloat(input.value); if (isNaN(cur)) cur = min;
    if (e.key === 'PageUp') { e.preventDefault(); commit(clampV(cur + largeStep)); }
    else if (e.key === 'PageDown') { e.preventDefault(); commit(clampV(cur - largeStep)); }
  });
}

function swipe(scope){
  var el = scope.parent();
  if (!el || el.__swipe) return; el.__swipe = true;
  var win = (el.ownerDocument && el.ownerDocument.defaultView) || window;
  var dir = el.getAttribute('data-swipe-direction') || 'down';
  var prefix = el.getAttribute('data-swipe-prefix') || '--swipe';
  var threshold = parseFloat(el.getAttribute('data-swipe-threshold')) || 50;
  var horizontal = dir === 'left' || dir === 'right';
  var sign = (dir === 'up' || dir === 'left') ? -1 : 1;
  var startX = 0, startY = 0, swiping = false, mx = 0, my = 0;
  function setVars(){
    el.style.setProperty(prefix + '-movement-x', mx + 'px');
    el.style.setProperty(prefix + '-movement-y', my + 'px');
    var along = horizontal ? mx : my;
    var progress = Math.max(0, Math.min(1, (along * sign) / Math.max(1, el.offsetHeight || el.offsetWidth)));
    el.style.setProperty('--drawer-swipe-progress', String(progress));
  }
  function reset(){ mx = 0; my = 0; setVars(); }
  reset();
  scope.addEvent(el, 'pointerdown', function(e){
    if (e.button) return;
    swiping = true; startX = e.clientX; startY = e.clientY;
    el.setAttribute('data-swiping', '');
    el.setAttribute('data-swipe-direction', dir);
    if (el.setPointerCapture) { try { el.setPointerCapture(e.pointerId); } catch (_) {} }
  });
  scope.addEvent(el, 'pointermove', function(e){
    if (!swiping) return;
    var dx = e.clientX - startX, dy = e.clientY - startY;
    // Only allow movement in the dismiss direction.
    mx = horizontal ? (sign > 0 ? Math.max(0, dx) : Math.min(0, dx)) : 0;
    my = horizontal ? 0 : (sign > 0 ? Math.max(0, dy) : Math.min(0, dy));
    setVars();
  });
  scope.addEvent(el, 'pointerup', function(){
    if (!swiping) return; swiping = false;
    el.removeAttribute('data-swiping');
    var along = (horizontal ? mx : my) * sign;
    if (along >= threshold) {
      var act = el.querySelector('[data-swipe-dismiss]');
      if (act) act.click();
    }
    reset();
  });
}

function snap(scope){
  var el = scope.parent();
  if (!el || el.__snap) return; el.__snap = true;
  var dir = el.getAttribute('data-swipe-direction') || 'bottom';
  function sign(){ return (dir === 'up' || dir === 'left') ? -1 : 1; }
  var horizontal = dir === 'left' || dir === 'right';
  var points = (el.getAttribute('data-snap-points') || '1').split(',').map(parseFloat)
    .filter(function(n){ return !isNaN(n); }).sort(function(a, b){ return a - b; });
  if (!points.length) points = [1];
  var sequential = el.getAttribute('data-snap-sequential') === 'true';
  var snapInput = el.querySelector('[data-snap-input]');
  var full = horizontal ? el.offsetWidth : el.offsetHeight;
  var startCoord = 0, dragging = false, curIdx = points.length - 1, base = 0;
  function toOffset(f){ return (1 - f) * full; }
  function setOffset(px){
    el.style.setProperty('--drawer-offset', px + 'px');
    el.style.setProperty('--drawer-snap-progress', String(full ? 1 - px / full : 1));
  }
  setOffset(toOffset(points[curIdx]));
  scope.addEvent(el, 'pointerdown', function(e){
    if (e.button) return;
    dragging = true; startCoord = horizontal ? e.clientX : e.clientY;
    base = toOffset(points[curIdx]);
    el.setAttribute('data-swiping', '');
    if (el.setPointerCapture) { try { el.setPointerCapture(e.pointerId); } catch (_) {} }
  });
  scope.addEvent(el, 'pointermove', function(e){
    if (!dragging) return;
    var d = (horizontal ? e.clientX : e.clientY) - startCoord;
    var off = Math.max(0, Math.min(base + sign() * d, full));
    setOffset(off);
  });
  scope.addEvent(el, 'pointerup', function(){
    if (!dragging) return; dragging = false; el.removeAttribute('data-swiping');
    var off = parseFloat(el.style.getPropertyValue('--drawer-offset')) || 0;
    var frac = full ? 1 - off / full : 1;
    if (frac < points[0] / 2) { var act = el.querySelector('[data-swipe-dismiss]'); if (act) { act.click(); return; } }
    var best = 0, bestD = Infinity;
    for (var i = 0; i < points.length; i++) { var dd = Math.abs(points[i] - frac); if (dd < bestD) { bestD = dd; best = i; } }
    if (sequential) best = Math.max(curIdx - 1, Math.min(curIdx + 1, best));
    curIdx = best;
    setOffset(toOffset(points[curIdx]));
    if (snapInput) { snapInput.value = String(curIdx); snapInput.dispatchEvent(new Event('change', { bubbles: true })); }
  });
}

function hover(scope){
  var trig = scope.parent();
  if (!trig || trig.__hover) return; trig.__hover = true;
  var doc = trig.ownerDocument || document;
  var win = doc.defaultView || window;
  var root = trig.parentNode;
  var openDelay = parseInt(trig.getAttribute('data-hover-delay'), 10) || 0;
  var closeDelay = parseInt(trig.getAttribute('data-hover-close-delay'), 10) || 0;
  function floating(){ return root && root.querySelector('[data-hover-popup]'); }
  function fire(sel){ var a = root && root.querySelector(sel); if (a) a.click(); }
  var openT = null, closeT = null, polyT = null, moveHandler = null;
  function clearOpen(){ if (openT) { win.clearTimeout(openT); openT = null; } }
  function clearClose(){ if (closeT) { win.clearTimeout(closeT); closeT = null; } }
  function clearPoly(){ if (polyT) { win.clearTimeout(polyT); polyT = null; } }
  function removeMove(){ if (moveHandler) { doc.removeEventListener('mousemove', moveHandler); moveHandler = null; } clearPoly(); }
  scope.onDispose(() => { clearOpen(); clearClose(); removeMove(); });
  function open(){ clearClose(); removeMove(); clearOpen(); openT = win.setTimeout(function(){ fire('[data-hover-open]'); }, openDelay); }
  function scheduleClose(){ removeMove(); clearClose(); closeT = win.setTimeout(function(){ fire('[data-hover-close]'); }, closeDelay); }

  var BUF = 0.5, SPEED = 0.1;
  function edge(px, py, xi, yi, xj, yj){ return (yi >= py) !== (yj >= py) && px <= ((xj - xi) * (py - yi)) / (yj - yi) + xi; }
  function inQuad(px, py, x1, y1, x2, y2, x3, y3, x4, y4){
    var s = false;
    if (edge(px, py, x1, y1, x2, y2)) s = !s;
    if (edge(px, py, x2, y2, x3, y3)) s = !s;
    if (edge(px, py, x3, y3, x4, y4)) s = !s;
    if (edge(px, py, x4, y4, x1, y1)) s = !s;
    return s;
  }
  function inRect(px, py, r){ return px >= r.left && px <= r.right && py >= r.top && py <= r.bottom; }
  function inAA(px, py, x1, y1, x2, y2){
    return px >= Math.min(x1, x2) && px <= Math.max(x1, x2) && py >= Math.min(y1, y2) && py <= Math.max(y1, y2);
  }
  function startPolygon(x, y){
    var fl = floating();
    if (!fl) { scheduleClose(); return; }
    var side = fl.getAttribute('data-side') || 'bottom';
    var landed = false, lastX = null, lastY = null, lastT = (win.performance ? win.performance.now() : Date.now());
    function slow(nx, ny){
      var t = (win.performance ? win.performance.now() : Date.now()), dt = t - lastT;
      if (lastX === null || dt === 0) { lastX = nx; lastY = ny; lastT = t; return false; }
      var dx = nx - lastX, dy = ny - lastY, d2 = dx * dx + dy * dy, th = dt * dt * SPEED * SPEED;
      lastX = nx; lastY = ny; lastT = t;
      return d2 < th;
    }
    clearClose();
    moveHandler = function(e){
      clearPoly();
      var refRect = trig.getBoundingClientRect(), rect = fl.getBoundingClientRect();
      var cx = e.clientX, cy = e.clientY, target = e.target;
      if (fl.contains(target)) { landed = true; return; }
      if (trig.contains(target)) { landed = true; return; }
      var leaveRight = x > rect.right - rect.width / 2, leaveBottom = y > rect.bottom - rect.height / 2;
      var wider = rect.width > refRect.width, taller = rect.height > refRect.height;
      var left = (wider ? refRect : rect).left, right = (wider ? refRect : rect).right;
      var top = (taller ? refRect : rect).top, bottom = (taller ? refRect : rect).bottom;
      if ((side === 'top' && y >= refRect.bottom - 1) || (side === 'bottom' && y <= refRect.top + 1) ||
          (side === 'left' && x >= refRect.right - 1) || (side === 'right' && x <= refRect.left + 1)) { scheduleClose(); return; }
      var trough = false;
      if (side === 'top') trough = inAA(cx, cy, left, refRect.top + 1, right, rect.bottom - 1);
      else if (side === 'bottom') trough = inAA(cx, cy, left, rect.top + 1, right, refRect.bottom - 1);
      else if (side === 'left') trough = inAA(cx, cy, rect.right - 1, bottom, refRect.left + 1, top);
      else if (side === 'right') trough = inAA(cx, cy, refRect.right - 1, bottom, rect.left + 1, top);
      if (trough) return;
      if (landed && !inRect(cx, cy, refRect)) { scheduleClose(); return; }
      if (slow(cx, cy)) { scheduleClose(); return; }
      var inside = false;
      if (side === 'top' || side === 'bottom') {
        var off = wider ? BUF / 2 : BUF * 4;
        var p1x = wider ? x + off : (leaveRight ? x + off : x - off);
        var p2x = wider ? x - off : (leaveRight ? x + off : x - off);
        var py = side === 'top' ? y + BUF + 1 : y - BUF;
        var nearY = side === 'top' ? rect.bottom - BUF : rect.top + BUF;
        var farY = side === 'top' ? rect.top : rect.bottom;
        var cyL = leaveRight ? nearY : (wider ? nearY : farY);
        var cyR = leaveRight ? (wider ? nearY : farY) : nearY;
        inside = inQuad(cx, cy, p1x, py, p2x, py, rect.left, cyL, rect.right, cyR);
      } else {
        var offY = taller ? BUF / 2 : BUF * 4;
        var p1y = taller ? y + offY : (leaveBottom ? y + offY : y - offY);
        var p2y = taller ? y - offY : (leaveBottom ? y + offY : y - offY);
        var px = side === 'left' ? x + BUF + 1 : x - BUF;
        var nearX = side === 'left' ? rect.right - BUF : rect.left + BUF;
        var farX = side === 'left' ? rect.left : rect.right;
        var cxT = leaveBottom ? nearX : (taller ? nearX : farX);
        var cxB = leaveBottom ? (taller ? nearX : farX) : nearX;
        if (side === 'left') inside = inQuad(cx, cy, cxT, rect.top, cxB, rect.bottom, px, p1y, px, p2y);
        else inside = inQuad(cx, cy, px, p1y, px, p2y, cxT, rect.top, cxB, rect.bottom);
      }
      if (!inside) { scheduleClose(); }
      else if (!landed) { polyT = win.setTimeout(function(){ fire('[data-hover-close]'); }, 40); }
    };
    doc.addEventListener('mousemove', moveHandler);
  }

  scope.addEvent(trig, 'pointerenter', open);
  scope.addEvent(trig, 'pointerleave', function(e){ clearOpen(); startPolygon(e.clientX, e.clientY); });
  scope.addEvent(trig, 'focus', function(){ clearClose(); removeMove(); clearOpen(); fire('[data-hover-open]'); });
  scope.addEvent(trig, 'blur', function(){ scheduleClose(); });
  var fl0 = floating();
  if (fl0) {
    scope.addEvent(fl0, 'pointerenter', function(){ clearClose(); removeMove(); });
    scope.addEvent(fl0, 'pointerleave', function(e){ startPolygon(e.clientX, e.clientY); });
  }
}

function listbox(scope){
  var pop = scope.parent();
  if (!pop || pop.__listbox) return; pop.__listbox = true;
  var loop = pop.getAttribute('data-loop') !== 'false';
  var buf = '', bufAt = 0;
  function items(){
    return Array.prototype.slice.call(pop.querySelectorAll('[data-list-item]')).filter(function(el){
      return !el.hasAttribute('disabled') && el.getAttribute('aria-disabled') !== 'true' && el.offsetParent !== null;
    });
  }
  function current(){ var l = items(); for (var i = 0; i < l.length; i++) if (l[i].hasAttribute('data-highlighted')) return i; return -1; }
  function setHL(idx){
    var l = items();
    l.forEach(function(el){ el.removeAttribute('data-highlighted'); });
    if (idx >= 0 && idx < l.length){
      var el = l[idx];
      el.setAttribute('data-highlighted', '');
      if (el.id) pop.setAttribute('aria-activedescendant', el.id);
      if (el.scrollIntoView) el.scrollIntoView({ block: 'nearest' });
    } else {
      pop.removeAttribute('aria-activedescendant');
    }
  }
  function move(delta){
    var l = items(); if (!l.length) return;
    var idx = current() + delta;
    if (idx < 0) idx = loop ? l.length - 1 : 0;
    if (idx >= l.length) idx = loop ? 0 : l.length - 1;
    setHL(idx);
  }
  function typeahead(ch){
    var now = Date.now();
    if (now - bufAt > 750) buf = '';
    bufAt = now; buf += ch.toLowerCase();
    var l = items(); if (!l.length) return;
    var start = (current() + 1) % l.length;
    for (var k = 0; k < l.length; k++){
      var i = (start + k) % l.length;
      var label = (l[i].getAttribute('data-label') || l[i].textContent || '').trim().toLowerCase();
      if (label.indexOf(buf) === 0){ setHL(i); return; }
    }
  }
  scope.addEvent(pop, 'keydown', function(e){
    if (e.key === 'ArrowDown'){ e.preventDefault(); move(1); }
    else if (e.key === 'ArrowUp'){ e.preventDefault(); move(-1); }
    else if (e.key === 'Home'){ e.preventDefault(); setHL(0); }
    else if (e.key === 'End'){ e.preventDefault(); setHL(items().length - 1); }
    else if (e.key === 'Enter' || e.key === ' '){ var c = current(); var l = items(); if (c >= 0){ e.preventDefault(); l[c].click(); } }
    else if (e.key.length === 1 && !e.ctrlKey && !e.metaKey && !e.altKey){ typeahead(e.key); }
  });
  scope.addEvent(pop, 'pointermove', function(e){
    var item = e.target.closest ? e.target.closest('[data-list-item]') : null;
    if (item){ var i = items().indexOf(item); if (i >= 0) setHL(i); }
  });
  var l = items();
  var active = -1;
  for (var i = 0; i < l.length; i++) if (l[i].hasAttribute('data-active-item')) { active = i; break; }
  setHL(active);
}

function panel_size(scope){
  var el = scope.parent();
  if (!el || el.__panelSize) return; el.__panelSize = true;
  var win = (el.ownerDocument && el.ownerDocument.defaultView) || window;
  var prefix = el.getAttribute('data-size-var') || '--panel';
  function measure(){
    el.style.setProperty(prefix + '-height', el.scrollHeight + 'px');
    el.style.setProperty(prefix + '-width', el.scrollWidth + 'px');
  }
  measure();
  if (win.ResizeObserver) { var ro = new win.ResizeObserver(measure); ro.observe(el); scope.onDispose(() => ro.disconnect()); }
  scope.addEvent(win, 'resize', measure);
}

function tab_indicator(scope){
  var list = scope.parent();
  if (!list || list.__tabIndicator) return; list.__tabIndicator = true;
  var win = (list.ownerDocument && list.ownerDocument.defaultView) || window;
  function measure(){
    var tab = list.querySelector('[role="tab"][data-active]');
    if (!tab) return;
    var l = list.getBoundingClientRect(), t = tab.getBoundingClientRect();
    var s = list.style;
    s.setProperty('--active-tab-left', (t.left - l.left) + 'px');
    s.setProperty('--active-tab-right', (l.right - t.right) + 'px');
    s.setProperty('--active-tab-top', (t.top - l.top) + 'px');
    s.setProperty('--active-tab-bottom', (l.bottom - t.bottom) + 'px');
    s.setProperty('--active-tab-width', t.width + 'px');
    s.setProperty('--active-tab-height', t.height + 'px');
  }
  measure();
  var mo = new MutationObserver(measure);
  scope.onDispose(() => mo.disconnect());
  mo.observe(list, { attributes: true, subtree: true, attributeFilter: ['data-active'] });
  if (win.ResizeObserver) { var ro = new win.ResizeObserver(measure); ro.observe(list); scope.onDispose(() => ro.disconnect()); }
  scope.addEvent(win, 'resize', measure);
}

function position(scope){
  var pos = scope.parent();
  if (!pos) return;
  var doc = pos.ownerDocument || document;
  var win = doc.defaultView || window;
  var anchorId = pos.getAttribute('data-anchor');
  var prefSide = pos.getAttribute('data-prefer-side') || 'bottom';
  var prefAlign = pos.getAttribute('data-prefer-align') || 'center';
  var offset = parseFloat(pos.getAttribute('data-offset')) || 0;
  var pad = parseFloat(pos.getAttribute('data-collision-padding'));
  if (isNaN(pad)) pad = 5;
  var sideMode = pos.getAttribute('data-collision-side') || 'flip';
  var alignMode = pos.getAttribute('data-collision-align') || 'flip';
  var fallbackAxis = pos.getAttribute('data-fallback-axis') || 'none';
  var sticky = pos.getAttribute('data-sticky') === 'true';
  var arrowPad = parseFloat(pos.getAttribute('data-arrow-padding')); if (isNaN(arrowPad)) arrowPad = 5;
  var rtl = pos.getAttribute('dir') === 'rtl';
  var track = pos.getAttribute('data-disable-anchor-tracking') !== 'true';
  function opposite(s){ return {top:'bottom',bottom:'top',left:'right',right:'left'}[s]; }
  function vertical(s){ return s === 'top' || s === 'bottom'; }
  function anchorEl(){ return anchorId ? doc.getElementById(anchorId) : pos.previousElementSibling; }
  function place(){
    var anchor = anchorEl();
    if (!anchor) return;
    var a = anchor.getBoundingClientRect();
    var p = pos.getBoundingClientRect();
    var w = p.width, h = p.height;
    var vw = doc.documentElement.clientWidth;
    var vh = doc.documentElement.clientHeight;
    var space = { top: a.top - pad, bottom: vh - a.bottom - pad, left: a.left - pad, right: vw - a.right - pad };
    var side = prefSide, align = prefAlign;
    var need = (vertical(side) ? h : w) + offset;
    if (sideMode === 'flip' && space[side] < need && space[opposite(side)] > space[side]) side = opposite(side);
    // fallbackAxisSide: preferred axis cannot fit either way → perpendicular axis.
    if (fallbackAxis !== 'none' && space[side] < need && space[opposite(side)] < need) {
      var perpNeed = (vertical(side) ? w : h) + offset;
      var perp = vertical(side) ? ['left', 'right'] : ['top', 'bottom'];
      if (vertical(side) && rtl) perp = ['right', 'left'];           // logical start in RTL
      if (fallbackAxis === 'end') perp = [perp[1], perp[0]];          // logical end first
      var chosen = null;
      for (var pi = 0; pi < perp.length; pi++) { if (space[perp[pi]] >= perpNeed) { chosen = perp[pi]; break; } }
      if (!chosen) chosen = space[perp[0]] >= space[perp[1]] ? perp[0] : perp[1];
      side = chosen;
    }
    var left, top;
    if (side === 'top') top = a.top - h - offset;
    else if (side === 'bottom') top = a.bottom + offset;
    else if (side === 'left') left = a.left - w - offset;
    else left = a.right + offset;
    function cross(al){
      if (vertical(side)) return al === 'start' ? a.left : al === 'end' ? a.right - w : a.left + (a.width - w) / 2;
      return al === 'start' ? a.top : al === 'end' ? a.bottom - h : a.top + (a.height - h) / 2;
    }
    var lo = pad;
    var hi = vertical(side) ? vw - w - pad : vh - h - pad;
    var c = cross(align);
    if (alignMode === 'flip' && (align === 'start' || align === 'end')) {
      if (c < lo || c > hi) {
        var alt = align === 'start' ? 'end' : 'start', c2 = cross(alt);
        if (c2 >= lo && c2 <= hi) { align = alt; c = c2; }
      }
    } else if (alignMode === 'shift') {
      if (c > hi) c = hi;
      if (c < lo) c = lo;
    }
    if (vertical(side)) left = c; else top = c;
    // sticky: keep the popup fully on-screen even as the anchor scrolls out.
    if (sticky) {
      left = Math.max(pad, Math.min(left, vw - w - pad));
      top = Math.max(pad, Math.min(top, vh - h - pad));
    }
    var fixed = pos.getAttribute('data-position-method') === 'fixed';
    pos.style.position = fixed ? 'fixed' : 'absolute';
    var sx = fixed ? 0 : (win.scrollX || 0), sy = fixed ? 0 : (win.scrollY || 0);
    pos.style.left = (left + sx) + 'px';
    pos.style.top = (top + sy) + 'px';
    pos.setAttribute('data-side', side);
    pos.setAttribute('data-align', align);
    var hidden = a.bottom < 0 || a.right < 0 || a.top > vh || a.left > vw;
    if (hidden && !sticky) pos.setAttribute('data-anchor-hidden', '');
    else pos.removeAttribute('data-anchor-hidden');
    // Arrow centering: align a [data-arrow] to the anchor centre on the cross
    // axis, clamped by arrowPadding; data-uncentered when it had to be clamped.
    var arrow = pos.querySelector('[data-arrow]');
    if (arrow) {
      var as = arrow.style;
      if (vertical(side)) {
        var center = a.left + a.width / 2 - left;
        var clamped = Math.max(arrowPad, Math.min(center, w - arrowPad));
        as.setProperty('--arrow-x', clamped + 'px'); as.removeProperty('--arrow-y');
        if (clamped !== center) arrow.setAttribute('data-uncentered', ''); else arrow.removeAttribute('data-uncentered');
      } else {
        var centerY = a.top + a.height / 2 - top;
        var clampedY = Math.max(arrowPad, Math.min(centerY, h - arrowPad));
        as.setProperty('--arrow-y', clampedY + 'px'); as.removeProperty('--arrow-x');
        if (clampedY !== centerY) arrow.setAttribute('data-uncentered', ''); else arrow.removeAttribute('data-uncentered');
      }
      arrow.setAttribute('data-side', side);
    }
    var st = pos.style;
    st.setProperty('--anchor-width', a.width + 'px');
    st.setProperty('--anchor-height', a.height + 'px');
    st.setProperty('--available-width', Math.max(0, vertical(side) ? vw - 2 * pad : space[side]) + 'px');
    st.setProperty('--available-height', Math.max(0, vertical(side) ? space[side] : vh - 2 * pad) + 'px');
    st.setProperty('--positioner-width', w + 'px');
    st.setProperty('--positioner-height', h + 'px');
    st.setProperty('--popup-width', w + 'px');
    st.setProperty('--popup-height', h + 'px');
    var ox = align === 'start' ? '0%' : align === 'end' ? '100%' : '50%';
    st.setProperty('--transform-origin', vertical(side)
      ? ox + ' ' + (side === 'top' ? '100%' : '0%')
      : (side === 'left' ? '100%' : '0%') + ' ' + ox);
  }
  place();
  if (track) {
    scope.addEvent(doc, 'scroll', place, true);
    scope.addEvent(win, 'resize', place);
  }
}

function scroll_lock(scope){
  var el = scope.parent();
  if (!el || el.__scrollLock) return; el.__scrollLock = true;
  var doc = el.ownerDocument || document;
  var win = doc.defaultView || window;
  var html = doc.documentElement, body = doc.body;
  function targetEl(){
    var bs = win.getComputedStyle(body), oy = bs.overflowY;
    return (oy === 'scroll' || oy === 'hidden' || oy === 'auto') ? body : html;
  }
  var saved = null;
  function lock(){
    if (saved) return;
    var t = targetEl();
    var stable = win.CSS && win.CSS.supports && win.CSS.supports('scrollbar-gutter', 'stable');
    saved = { t: t, overflowY: t.style.overflowY, gutter: t.style.scrollbarGutter, padding: t.style.paddingRight };
    t.style.overflowY = 'hidden';
    if (stable) {
      t.style.scrollbarGutter = 'stable';
    } else {
      var sbw = win.innerWidth - html.clientWidth;
      if (sbw > 0) t.style.paddingRight = (parseFloat(win.getComputedStyle(t).paddingRight) + sbw) + 'px';
    }
  }
  function unlock(){
    if (!saved) return;
    saved.t.style.overflowY = saved.overflowY;
    saved.t.style.scrollbarGutter = saved.gutter;
    saved.t.style.paddingRight = saved.padding;
    saved = null;
  }
  scope.onDispose(unlock);
  function sync(){ if (el.hasAttribute('data-open')) lock(); else unlock(); }
  sync();
  var mo = new MutationObserver(sync);
  scope.onDispose(() => mo.disconnect());
  mo.observe(el, { attributes: true, attributeFilter: ['data-open', 'data-closed'] });
}

function transition(scope){
  var el = scope.parent();
  if (!el || el.__transition) return; el.__transition = true;
  var win = (el.ownerDocument && el.ownerDocument.defaultView) || window;
  function twoFrames(fn){ win.requestAnimationFrame(function(){ win.requestAnimationFrame(fn); }); }
  function onEnd(e){
    if (e.target !== el) return;
    el.removeEventListener('transitionend', onEnd);
    el.removeAttribute('data-ending-style');
    var act = el.querySelector('[data-transition-complete]');
    if (act) act.click();
  }
  function enter(){
    el.removeAttribute('data-ending-style');
    el.removeEventListener('transitionend', onEnd);
    if (el.hasAttribute('data-instant')) return;
    el.setAttribute('data-starting-style', '');
    twoFrames(function(){ el.removeAttribute('data-starting-style'); });
  }
  function leave(){
    el.removeAttribute('data-starting-style');
    if (el.hasAttribute('data-instant')) { el.removeAttribute('data-ending-style'); return; }
    el.setAttribute('data-ending-style', '');
    scope.addEvent(el, 'transitionend', onEnd);
  }
  var isOpen = el.hasAttribute('data-open');
  if (isOpen) enter();
  var mo = new MutationObserver(function(){
    var now = el.hasAttribute('data-open');
    if (now === isOpen) return;
    isOpen = now;
    if (now) enter(); else leave();
  });
  scope.onDispose(() => mo.disconnect());
  mo.observe(el, { attributes: true, attributeFilter: ['data-open', 'data-closed'] });
}

const active = new WeakMap();
const custom = new Map();

function machinery(fn) {
  return (node, args) => {
    const removals = [];
    const scope = {
      parent: () => node,
      onDispose: cleanup => removals.push(cleanup),
      args,
      addEvent: (target, name, callback, options) => {
        if (typeof target === "string") target = node.querySelector(target);
        if (!target) return;
        target.addEventListener(name, callback, options);
        removals.push(() => target.removeEventListener(name, callback, options));
      },
    };
    fn(scope);
    return () => { for (const remove of removals) remove(); };
  };
}

export const SHARED = Object.freeze({
  DIALOG: machinery(dialog), DISMISS: machinery(dismiss),
  SCROLL_LOCK: machinery(scroll_lock), TRANSITION: machinery(transition),
});
export const WEB_ONLY = Object.freeze({
  composite: machinery(composite), focus_trap: machinery(focus_trap),
  gestures: machinery(scope => {
    const mode = scope.args.find(arg => arg.name === "mode")?.value?.v;
    ({ slider: slider_drag, swipe, snap }[mode] ?? slider_drag)(scope);
  }),
  hover: machinery(hover), listbox: machinery(listbox),
  measure: machinery(scope => {
    const mode = scope.args.find(arg => arg.name === "mode")?.value?.v;
    (mode === "tab_indicator" ? tab_indicator : panel_size)(scope);
  }),
  position: machinery(position),
});

export function registerWebBehaviour(name, fn) {
  if (typeof fn !== "function") throw new TypeError("Expected a behaviour function");
  custom.set(name, fn);
}

function apply(node, name, args, run) {
  if (!run) return false;
  let entries = active.get(node);
  if (!entries) active.set(node, entries = new Map());
  if (!entries.has(name)) entries.set(name, run(node, args));
  return true;
}

export function applyBehaviour(node, id, args = []) {
  const name = BehaviourId[id];
  return apply(node, name, args, SHARED[name]);
}

export function applyWebBehaviour(node, name, args = []) {
  const mode = (name === "gestures" || name === "measure")
    ? args?.find(arg => arg.name === "mode")?.value?.v : undefined;
  const key = mode === undefined ? name : name + ":" + mode;
  return apply(node, key, args, Object.hasOwn(WEB_ONLY, name) ? WEB_ONLY[name] : custom.get(name));
}

export function disposeBehaviours(node) {
  const entries = active.get(node);
  active.delete(node);
  if (entries) for (const cleanup of entries.values()) if (typeof cleanup === "function") cleanup();
}

// First-page markup uses the same parent binding as a frame.
export function hydrateMarkers(root) {
  for (const marker of root.querySelectorAll?.("span[data-ewe-behaviour]") ?? []) {
    const parent = marker.parentElement ?? marker.parentNode;
    if (!parent) continue;
    const name = marker.getAttribute("data-ewe-behaviour");
    const mode = marker.getAttribute("data-ewe-mode");
    const args = mode === null ? [] : [{name:"mode",value:{v:mode}}];
    const shared = BehaviourId[name.toUpperCase()];
    const applied = shared ? applyBehaviour(parent,shared,args) : applyWebBehaviour(parent,name,args);
    if (!applied) console.error("Unknown component behaviour", name);
    marker.remove();
  }
}
