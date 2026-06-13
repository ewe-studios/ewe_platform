//! # M5 — Roving focus / composite (spec-42 feature 05 §3 M5)
//!
//! WHY: Menus, tabs, toolbars, radio-groups and toggle-groups are a single
//! tab stop whose members are navigated with arrows (WAI-ARIA composite
//! pattern), not Tab. One member is `tabindex=0`, the rest `-1`; arrows move
//! the roving focus.
//!
//! WHAT: [`COMPOSITE_JS`] (the behavior) + [`composite_behavior`] (the
//! `<script>` node a composite container embeds) + the data-attribute contract
//! constants the container/items emit.
//!
//! HOW: The body reads the container's `data-orientation` (`horizontal`
//! default / `vertical`) and `data-loop` off `scope.parent()`, finds items by
//! [`DATA_COMPOSITE_ITEM`], seeds the roving `tabindex`, and on `keydown`
//! (orientation-aware arrows + Home/End, optional loop, skipping disabled)
//! moves focus. Typeahead is a documented follow-up.

use foundation_ui_traits::Html;

use super::scoped_script;

/// Container marker: present on the composite root (`data-composite`).
pub const DATA_COMPOSITE: &str = "data-composite";
/// Item marker: present on each navigable member (`data-composite-item`).
pub const DATA_COMPOSITE_ITEM: &str = "data-composite-item";
/// Optional per-item marker for the initially-active member
/// (`data-composite-active`); defaults to the first item.
pub const DATA_COMPOSITE_ACTIVE: &str = "data-composite-active";
/// Optional container marker (`data-composite-select`): arrows MOVE AND SELECT
/// (the WAI-ARIA radio-group pattern — clicks the focused member). Absent =
/// arrows only move focus (toolbar/toggle-group/tabs-manual pattern).
pub const DATA_COMPOSITE_SELECT: &str = "data-composite-select";

/// The M5 roving-focus behavior (orientation-aware arrows, Home/End, loop,
/// roving `tabindex`, disabled-skipping). Reads `data-orientation`/`data-loop`
/// off the container.
pub const COMPOSITE_JS: &str = r#"function(scope){
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
  root.addEventListener('keydown', function(e){
    var list = items();
    var cur = list.indexOf(document.activeElement);
    if (cur < 0) return;
    var next = null;
    if (e.key === nextKey) next = cur + 1;
    else if (e.key === prevKey) next = cur - 1;
    else if (e.key === 'Home') next = 0;
    else if (e.key === 'End') next = list.length - 1;
    else return;
    e.preventDefault();
    move(next);
  });
  seed();
}"#;

/// The `<script>` node a composite container embeds to get roving focus.
/// The container must also carry `data-composite` + `data-orientation`
/// (+ optional `data-loop="false"`), and mark each member with
/// `data-composite-item`.
#[must_use]
pub fn composite_behavior() -> Html {
    scoped_script(COMPOSITE_JS)
}
