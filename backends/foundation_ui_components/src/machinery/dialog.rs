//! # Native `<dialog>` open/close sync (M2 layering for dialogs) (spec-42 §F4)
//!
//! WHY: `<dialog>.showModal()` gives the top layer, focus trap, scroll lock,
//! `::backdrop` and page inertness for free — but the open state is a Rust
//! signal, and `showModal()`/`close()` are imperative. This behavior keeps the
//! native element in sync with the signal-driven `data-open` and routes the
//! native `cancel` (Escape) back through the signal so it stays the source of
//! truth (F4 dialog verdict: "native `<dialog>`, fully").
//!
//! WHAT: [`DIALOG_JS`] (the behavior) + [`dialog_behavior`] (the `<script>` a
//! dialog embeds) + the attribute contract.
//!
//! HOW: Reads `data-dialog-mode` (`modal` default / `nonmodal`) and watches
//! `data-open`: present → `showModal()`/`show()`, absent → `close()`. The
//! `cancel` event is prevented and instead `.click()`s `[data-dialog-close]`
//! (wired to `set_open(false)`), stamping the dismiss reason.

use foundation_ui_traits::Html;

use super::scoped_script;

/// Dialog mode attribute (`data-dialog-mode`): `modal` (default, `showModal()`) or
/// `nonmodal` (`show()`).
pub const DATA_DIALOG_MODE: &str = "data-dialog-mode";
/// The element `.click()`ed to request close (wired to `set_open(false)`) —
/// `data-dialog-close`.
pub const DATA_DIALOG_CLOSE: &str = "data-dialog-close";
/// Opt out of backdrop (light) dismissal with `data-dialog-dismissable="false"`
/// (alert-dialog fixes this).
pub const DATA_DIALOG_DISMISSABLE: &str = "data-dialog-dismissable";

/// The native-dialog sync behavior: `data-open` ⇄ `showModal()`/`close()`, with
/// `cancel` (Escape) routed back through `[data-dialog-close]`.
pub const DIALOG_JS: &str = r#"function(scope){
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
  mo.observe(dlg, { attributes: true, attributeFilter: ['data-open', 'data-closed'] });
}"#;

/// The `<script>` a `<dialog>` embeds to sync its open state with the signal.
/// The dialog must carry `data-dialog-mode`, toggle `data-open`, and render a
/// hidden [`DATA_DIALOG_CLOSE`] element wired to `set_open(false)`.
#[must_use]
pub fn dialog_behavior() -> Html {
    scoped_script(DIALOG_JS)
}
