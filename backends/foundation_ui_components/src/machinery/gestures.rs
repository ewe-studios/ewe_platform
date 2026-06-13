//! # M8 — Pointer gestures (shared module) (spec-42 §3 M8)
//!
//! WHY: Slider drag, drawer/toast swipe-to-dismiss and number-field scrub are
//! one family of pointer gestures (base-ui's gesture layer). This module is the
//! shared home for the cross-component ones — slider drag + swipe; number-field
//! scrub/hold-repeat lives with its component.
//!
//! WHAT: [`slider_drag_behavior`] (pointer drag on the slider Control →
//! value) and [`swipe_behavior`] (drawer/toast swipe-to-dismiss with the
//! movement CSS vars + `data-swiping`/`data-swipe-direction`).
//!
//! HOW: Each reads its config off `scope.parent()` data-attributes, drives the
//! same hidden-input/`.click()` bridges the rest of the catalog uses, and emits
//! the documented data-attribute + CSS-var contract.

use foundation_ui_traits::Html;

use super::scoped_script;

/// Slider drag: pointer-down on `[data-slider-control]` computes the value from
/// the finger position and writes the hidden `[data-slider-input]` (→ change →
/// the Rust setter); PageUp/PageDown step by `data-large-step`. Single-thumb
/// (range swap/stop is the multi-thumb follow-up). Ported from base-ui
/// `SliderControl.getFingerState`.
pub const SLIDER_DRAG_JS: &str = r#"function(scope){
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
}"#;

/// Swipe-to-dismiss for drawer/toast: tracks pointer movement along the swipe
/// axis, publishing `--<prefix>-swipe-movement-x/y` (+ `--drawer-swipe-progress`
/// for drawers) and `data-swiping`; releasing past the threshold `.click()`s
/// `[data-swipe-dismiss]` (the wired close), otherwise it snaps back. Reads
/// `data-swipe-direction` (`down|up|left|right`) and `data-swipe-prefix`. Ported
/// from base-ui's drawer/toast swipe handling.
pub const SWIPE_JS: &str = r#"function(scope){
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
}"#;

/// Drawer snap points (OUR design — base-ui has no drawer; vaul-inspired). The
/// drawer drags along its swipe axis and rests at fractional-open heights
/// (`data-snap-points="0.5,1"`, 1 = fully open); release snaps to the nearest
/// point (or the next/prev with `data-snap-sequential`), dragging below the
/// smallest point dismisses. Publishes `--drawer-offset` + `--drawer-snap-progress`
/// and reports the resting index via `[data-snap-input]` (→ a `usize` signal).
pub const SNAP_JS: &str = r#"function(scope){
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
}"#;

/// The `<script>` a slider Root embeds for M8 pointer drag. The Root must carry
/// `data-min`/`data-max`/`data-step`/`data-large-step`/`data-orientation`, mark
/// the track `[data-slider-control]` and the hidden range `[data-slider-input]`.
#[must_use]
pub fn slider_drag_behavior() -> Html {
    scoped_script(SLIDER_DRAG_JS)
}

/// The `<script>` a drawer/toast embeds for M8 swipe-to-dismiss. The root must
/// carry `data-swipe-direction`/`data-swipe-prefix` and a hidden
/// `[data-swipe-dismiss]` wired to its close.
#[must_use]
pub fn swipe_behavior() -> Html {
    scoped_script(SWIPE_JS)
}

/// The `<script>` a drawer with snap points embeds. The drawer must carry
/// `data-swipe-direction`/`data-snap-points` (+ optional `data-snap-sequential`)
/// and a hidden `[data-snap-input]` wired to a `usize` snap-index signal +
/// `[data-swipe-dismiss]` for the dismiss-below-lowest path.
#[must_use]
pub fn snap_behavior() -> Html {
    scoped_script(SNAP_JS)
}
