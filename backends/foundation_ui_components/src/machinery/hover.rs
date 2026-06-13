//! # M3 — Hover intent (timer model) (spec-42 §3 M3)
//!
//! WHY: Tooltip / preview-card / hover-popover open on pointer/focus and close
//! on leave, with per-component open/close DELAYS (machinery.md §M3). This
//! module ships the timer half of M3 hover intent; the safe-polygon /
//! grouping refinements remain the documented follow-up.
//!
//! WHAT: [`HOVER_JS`] (the behavior) + [`hover_behavior`] (the `<script>` a
//! hover trigger embeds) + the attribute contract.
//!
//! HOW: Attaches to the trigger (`scope.parent()`). pointerenter/focus start an
//! OPEN timer (`data-hover-delay` ms); pointerleave/blur start a CLOSE timer
//! (`data-hover-close-delay` ms). On fire it `.click()`s a sibling
//! `[data-hover-open]` / `[data-hover-close]` (wired to `set_open(true/false)`)
//! — the same proven JS→signal bridge. Re-entering before the close timer fires
//! cancels it (no flicker).

use foundation_ui_traits::Html;

use super::scoped_script;

/// Open delay in ms before the trigger opens on hover/focus (`data-hover-delay`).
pub const DATA_HOVER_DELAY: &str = "data-hover-delay";
/// Close delay in ms before the trigger closes on leave/blur (`data-hover-close-delay`).
pub const DATA_HOVER_CLOSE_DELAY: &str = "data-hover-close-delay";

/// The M3 hover-intent (timer) behavior. Opens/closes via sibling
/// `[data-hover-open]`/`[data-hover-close]` actions after the configured delays.
pub const HOVER_JS: &str = r#"function(scope){
  var trig = scope.parent();
  if (!trig || trig.__hover) return; trig.__hover = true;
  var win = (trig.ownerDocument && trig.ownerDocument.defaultView) || window;
  var openDelay = parseInt(trig.getAttribute('data-hover-delay'), 10) || 0;
  var closeDelay = parseInt(trig.getAttribute('data-hover-close-delay'), 10) || 0;
  var openT = null, closeT = null, root = trig.parentNode;
  function clearTimers(){
    if (openT) { win.clearTimeout(openT); openT = null; }
    if (closeT) { win.clearTimeout(closeT); closeT = null; }
  }
  function fire(sel){ var a = root && root.querySelector(sel); if (a) a.click(); }
  function open(){ clearTimers(); openT = win.setTimeout(function(){ fire('[data-hover-open]'); }, openDelay); }
  function close(){ clearTimers(); closeT = win.setTimeout(function(){ fire('[data-hover-close]'); }, closeDelay); }
  scope.addEvent(trig, 'pointerenter', open);
  scope.addEvent(trig, 'pointerleave', close);
  scope.addEvent(trig, 'focus', function(){ clearTimers(); fire('[data-hover-open]'); });
  scope.addEvent(trig, 'blur', close);
}"#;

/// The `<script>` a hover trigger embeds. The trigger must carry the optional
/// `data-hover-delay`/`data-hover-close-delay`, and the overlay must render
/// sibling `[data-hover-open]`/`[data-hover-close]` actions wired to the open
/// signal.
#[must_use]
pub fn hover_behavior() -> Html {
    scoped_script(HOVER_JS)
}
