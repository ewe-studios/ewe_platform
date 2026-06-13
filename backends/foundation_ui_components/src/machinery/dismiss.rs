//! # M3 — Dismiss (light dismiss: Escape + outside pointer) (spec-42 §3 M3)
//!
//! WHY: Non-native overlays (popover/menu/select/tooltip fallback) close when
//! the user presses Escape or points outside them — base-ui's "light dismiss"
//! (machinery.md §M3). Native `<dialog>`/`popover="auto"` get this for free;
//! everything on the M1/M3 fallback path needs it delivered as behavior.
//!
//! WHAT: [`DISMISS_JS`] (the behavior) + [`dismiss_behavior`] (the `<script>`
//! node a popup embeds) + the data-attribute contract constants. Hover-intent
//! open/close + the safe-polygon state machine are a documented follow-up
//! (machinery.md §M3) — this module ships the dismiss half the catalog needs
//! first.
//!
//! HOW: Mirrors M5's proven "synthesize a click the Rust callback already
//! handles" mechanism. The popup owns the open signal; it renders a hidden
//! `[data-dismiss-action]` element wired to `set_open(false)`. On Escape
//! (document keydown) or a pointer-down OUTSIDE both the popup and its trigger
//! (resolved via `primal:anchor`), the body stamps `data-dismiss-reason` and
//! `.click()`s that action — the signal flips, morph closes the popup, the
//! scoped script is removed and re-runs clean on the next open. Listeners use
//! `scope.addEvent` (auto-cleanup on disconnect), so it's idempotent.

use foundation_ui_traits::Html;

use super::scoped_script;

/// Popup marker: present on a light-dismissable popup root (`data-dismiss`).
pub const DATA_DISMISS: &str = "data-dismiss";
/// The element `.click()`ed to request close; the component wires its
/// `primal:onclick` to `set_open(false)` (`data-dismiss-action`).
pub const DATA_DISMISS_ACTION: &str = "data-dismiss-action";
/// Stamped on the action before the synthesized click so the close handler can
/// read WHY it dismissed (`escape-key` / `outside-press`) per the event-reason
/// taxonomy (`data-dismiss-reason`).
pub const DATA_DISMISS_REASON: &str = "data-dismiss-reason";
/// Opt-out of Escape dismissal: set `data-dismiss-escape="false"`.
pub const DATA_DISMISS_ESCAPE: &str = "data-dismiss-escape";
/// Opt-out of outside-pointer dismissal: set `data-dismiss-outside="false"`
/// (e.g. base-ui's `disablePointerDismissal`).
pub const DATA_DISMISS_OUTSIDE: &str = "data-dismiss-outside";

/// The M3 light-dismiss behavior (Escape + outside-pointer → click the
/// `[data-dismiss-action]`). Reads `primal:anchor` (trigger id, so clicks on
/// the trigger don't dismiss) and the `data-dismiss-escape/outside` opt-outs
/// off the popup root.
pub const DISMISS_JS: &str = r#"function(scope){
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
      var anchor = popup.getAttribute('primal:anchor');
      if (anchor) {
        var trig = doc.getElementById(anchor);
        if (trig && trig.contains(e.target)) return;
      }
      dismiss('outside-press');
    });
  }
}"#;

/// The `<script>` node a light-dismissable popup embeds. The popup root must
/// carry [`DATA_DISMISS`] (+ optional `primal:anchor` / `data-dismiss-escape` /
/// `data-dismiss-outside`) and render a hidden [`DATA_DISMISS_ACTION`] element
/// whose `primal:onclick` writes `set_open(false)`.
#[must_use]
pub fn dismiss_behavior() -> Html {
    scoped_script(DISMISS_JS)
}
