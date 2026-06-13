//! # M4 — Focus management (initial / trap / restore) (spec-42 §3 M4, F4)
//!
//! WHY: A modal overlay that ISN'T a native `<dialog>` must keep Tab focus
//! inside it, move focus IN on open, and RESTORE focus to the trigger on close
//! (F4 `initialFocus`/`finalFocus`). `<dialog>.showModal()` traps natively;
//! the `popover`/M1 path needs this delivered.
//!
//! WHAT: [`FOCUS_TRAP_JS`] (the behavior) + [`focus_trap_behavior`] (the
//! `<script>` a modal popup embeds) + the data-attribute contract.
//!
//! HOW: Watches the popup's `data-open`. On open it records the active element,
//! then focuses `data-initial-focus` (a selector; `"none"` skips) or the first
//! tabbable. A `keydown` listener wraps Tab/Shift+Tab between the first and last
//! visible tabbables. On close it restores focus to the recorded element, or to
//! `primal:anchor` (the trigger) as a fallback.

use foundation_ui_traits::Html;

use super::scoped_script;

/// Popup marker: present on a modal popup whose focus should be trapped while
/// open (`data-focus-trap`).
pub const DATA_FOCUS_TRAP: &str = "data-focus-trap";
/// Optional initial-focus target: a CSS selector resolved within the popup, or
/// `"none"` to leave focus where it is (`data-initial-focus`).
pub const DATA_INITIAL_FOCUS: &str = "data-initial-focus";

/// The M4 focus-trap behavior. Moves focus in on open (initial-focus target or
/// first tabbable), wraps Tab within the popup, and restores focus on close.
pub const FOCUS_TRAP_JS: &str = r#"function(scope){
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
      var anchor = el.getAttribute('primal:anchor');
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
  mo.observe(el, { attributes: true, attributeFilter: ['data-open', 'data-closed'] });
}"#;

/// The `<script>` node a modal popup embeds to trap focus while open. The popup
/// root must carry [`DATA_FOCUS_TRAP`], toggle `data-open`, and (for restore)
/// carry `primal:anchor` pointing at its trigger.
#[must_use]
pub fn focus_trap_behavior() -> Html {
    scoped_script(FOCUS_TRAP_JS)
}
