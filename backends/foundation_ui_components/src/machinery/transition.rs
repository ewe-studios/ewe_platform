//! # M7 — Enter/leave transitions (starting/ending style) (spec-42 §3 M7)
//!
//! WHY: Overlays stay MOUNTED in our visibility model (consistent with F3); to
//! animate open/close, base-ui's vendored stylesheets target
//! `data-starting-style` (the pre-enter baseline, removed one frame later so
//! the element transitions INTO its open styles) and `data-ending-style` (held
//! during exit until `transitionend`), and disable transitions under
//! `data-instant` (machinery.md §M7). That attribute choreography is the
//! behavior this module delivers.
//!
//! WHAT: [`TRANSITION_JS`] (the behavior) + [`transition_behavior`] (the
//! `<script>` a popup embeds) + the data-attribute contract constants.
//!
//! HOW: The Rust open signal toggles `data-open`/`data-closed` on the popup
//! (the component's effect). This body watches those via a `MutationObserver`:
//! on open it stamps `data-starting-style` and clears it after two frames; on
//! close it stamps `data-ending-style` and clears it on `transitionend`, then
//! `.click()`s an optional `[data-transition-complete]` action (the
//! `onOpenChangeComplete` hook — same proven mechanism as M3/M5). When
//! `data-instant` is present the choreography is skipped (no animation), per
//! the M7 contract.

use foundation_ui_traits::Html;

use super::scoped_script;

/// Present on the popup while logically open (`data-open`) — toggled by the
/// component's open-signal effect; this behavior watches it.
pub const DATA_OPEN: &str = "data-open";
/// Present on the popup while logically closed (`data-closed`).
pub const DATA_CLOSED: &str = "data-closed";
/// Stamped for one frame at the start of an enter transition (`data-starting-style`).
pub const DATA_STARTING_STYLE: &str = "data-starting-style";
/// Held during an exit transition until `transitionend` (`data-ending-style`).
pub const DATA_ENDING_STYLE: &str = "data-ending-style";
/// When present, suppress the enter/leave choreography (no animation) —
/// emitted for `click`/`dismiss`/`focus`/`trigger-change` causes (`data-instant`).
pub const DATA_INSTANT: &str = "data-instant";
/// Optional element `.click()`ed after an exit transition completes — the
/// `onOpenChangeComplete` hook (`data-transition-complete`).
pub const DATA_TRANSITION_COMPLETE: &str = "data-transition-complete";

/// The M7 transition behavior. Observes `data-open` toggling and manages the
/// `data-starting-style`/`data-ending-style` attributes + `transitionend`
/// completion, honoring `data-instant`.
pub const TRANSITION_JS: &str = r#"function(scope){
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
    el.addEventListener('transitionend', onEnd);
  }
  var isOpen = el.hasAttribute('data-open');
  if (isOpen) enter();
  var mo = new MutationObserver(function(){
    var now = el.hasAttribute('data-open');
    if (now === isOpen) return;
    isOpen = now;
    if (now) enter(); else leave();
  });
  mo.observe(el, { attributes: true, attributeFilter: ['data-open', 'data-closed'] });
}"#;

/// The `<script>` node a popup embeds to get M7 enter/leave transitions. The
/// popup root's open signal must toggle [`DATA_OPEN`]/[`DATA_CLOSED`]; the body
/// manages the starting/ending styles. Optionally render a hidden
/// [`DATA_TRANSITION_COMPLETE`] element to receive the post-exit callback.
#[must_use]
pub fn transition_behavior() -> Html {
    scoped_script(TRANSITION_JS)
}
