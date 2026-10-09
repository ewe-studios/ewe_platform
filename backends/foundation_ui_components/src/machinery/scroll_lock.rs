//! # M2 — Scroll lock for non-dialog modals (spec-42 §3 M2)
//!
//! WHY: A modal `popover`/menu/select that ISN'T a native `<dialog>` must lock
//! page scroll while open WITHOUT the layout shift a naive `overflow:hidden`
//! causes (machinery.md §M2). Native `<dialog>.showModal()` locks scroll for
//! free; everything on the `popover`/M1 path needs this delivered.
//!
//! WHAT: [`SCROLL_LOCK_JS`] (the behavior) + [`scroll_lock_behavior`] (the
//! `<script>` a modal popup embeds) + the data-attribute contract.
//!
//! HOW: The behavior watches the popup's `data-open` (toggled by the open
//! signal). On open it locks the scroll container — `<html>` unless `<body>`
//! carries its own overflow styles. When `scrollbar-gutter: stable` is
//! supported it sets `overflow-y: hidden` + `scrollbar-gutter: stable` (no
//! width jump); otherwise it sets `overflow-y: hidden` and pads the gutter
//! width onto `padding-right`. All originals are restored on close.

use foundation_ui_traits::Html;

use super::scoped_script;

/// Popup marker: present on a modal popup that should lock page scroll while
/// open (`data-scroll-lock`). The behavior engages when the popup also has
/// `data-open`.
pub const DATA_SCROLL_LOCK: &str = "data-scroll-lock";

/// The M2 scroll-lock behavior. Locks the scroll container while the popup is
/// open and restores it on close, preserving the gutter so the page doesn't
/// shift.
pub const SCROLL_LOCK_JS: &str = r#"function(scope){
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
  function sync(){ if (el.hasAttribute('data-open')) lock(); else unlock(); }
  sync();
  var mo = new MutationObserver(sync);
  mo.observe(el, { attributes: true, attributeFilter: ['data-open', 'data-closed'] });
}"#;

/// The `<script>` node a modal popup embeds to lock page scroll while open.
/// The popup root must carry [`DATA_SCROLL_LOCK`] and toggle `data-open`.
#[must_use]
pub fn scroll_lock_behavior() -> Html {
    scoped_script(SCROLL_LOCK_JS)
}
