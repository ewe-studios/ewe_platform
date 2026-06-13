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
