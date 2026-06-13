//! # M5 — Virtual highlight + typeahead (menu/listbox variant) (spec-42 §3 M5, F5/F6)
//!
//! WHY: Menus, selects and comboboxes keep DOM focus on the POPUP and move a
//! "virtual" highlight (`data-highlighted` + `aria-activedescendant`) rather
//! than roving real focus (the composite/`tabindex` variant). They also do
//! typeahead — the algorithm pinned in machinery.md §M5 (750 ms buffer reset,
//! locale-lowercased prefix match, same-letter cycling).
//!
//! WHAT: [`LISTBOX_JS`] (the behavior) + [`listbox_behavior`] (the `<script>` a
//! popup embeds) + the attribute contract.
//!
//! HOW: Attaches to the popup (`scope.parent()`, which must be focusable —
//! `tabindex=0`). Items are `[data-list-item]`; the initially-active item is
//! `[data-active-item]`. ↑/↓/Home/End move the highlight (loop per
//! `data-loop`), Enter/Space `.click()` the highlighted item, printable keys
//! drive typeahead over `data-label` (or text), pointer move highlights the
//! hovered item. The highlight reflects `data-highlighted` on the item and
//! `aria-activedescendant` (the item id) on the popup.

use foundation_ui_traits::Html;

use super::scoped_script;

/// Item marker: a navigable list member (`data-list-item`).
pub const DATA_LIST_ITEM: &str = "data-list-item";
/// Item marker: the initially-highlighted member (`data-active-item`).
pub const DATA_ACTIVE_ITEM: &str = "data-active-item";
/// Item reflection: the currently-highlighted member (`data-highlighted`).
pub const DATA_HIGHLIGHTED: &str = "data-highlighted";
/// Optional per-item typeahead text override (`data-label`); falls back to text.
pub const DATA_LABEL: &str = "data-label";

/// The M5 virtual-highlight + typeahead behavior for menus/listboxes.
pub const LISTBOX_JS: &str = r#"function(scope){
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
}"#;

/// The `<script>` a menu/listbox popup embeds for virtual highlight + typeahead.
/// The popup must be focusable (`tabindex=0`) and mark members `data-list-item`.
#[must_use]
pub fn listbox_behavior() -> Html {
    scoped_script(LISTBOX_JS)
}
