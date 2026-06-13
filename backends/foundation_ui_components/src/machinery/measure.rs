//! # Measurement behaviors (the M7 measuring seam) (spec-42 §F3)
//!
//! WHY: Headless height/underline animation needs the browser to MEASURE a
//! panel's natural size or a tab's box and publish it as CSS vars the
//! stylesheet transitions against (F3: `--collapsible-panel-height`,
//! `--accordion-panel-height`, the tabs `--active-tab-*` set). That measurement
//! is genuinely JS — delivered here as scoped scripts.
//!
//! WHAT: [`panel_size_behavior`] (publishes `<prefix>-height/-width` from a
//! panel's scroll size) + [`tab_indicator_behavior`] (publishes the active
//! tab's box as `--active-tab-*` on the tablist) + their builders.
//!
//! HOW: Both read their target off `scope.parent()`, write the vars once, then
//! re-measure on `ResizeObserver` + window resize. The panel behavior reads its
//! var prefix from `data-size-var`; the indicator behavior finds the
//! `[data-active]` tab within the list and also re-measures when the active tab
//! changes (a `MutationObserver` on the list subtree's `data-active`).

use foundation_ui_traits::Html;

use super::scoped_script;

/// Panel marker: the CSS-var prefix to publish (`data-size-var`, e.g.
/// `--collapsible-panel`). The behavior emits `<prefix>-height`/`<prefix>-width`.
pub const DATA_SIZE_VAR: &str = "data-size-var";

/// Measures a panel's natural scroll size and publishes `<prefix>-height` /
/// `<prefix>-width` (prefix from `data-size-var`); re-measures on resize.
pub const PANEL_SIZE_JS: &str = r#"function(scope){
  var el = scope.parent();
  if (!el || el.__panelSize) return; el.__panelSize = true;
  var win = (el.ownerDocument && el.ownerDocument.defaultView) || window;
  var prefix = el.getAttribute('data-size-var') || '--panel';
  function measure(){
    el.style.setProperty(prefix + '-height', el.scrollHeight + 'px');
    el.style.setProperty(prefix + '-width', el.scrollWidth + 'px');
  }
  measure();
  if (win.ResizeObserver) { var ro = new win.ResizeObserver(measure); ro.observe(el); }
  scope.addEvent(win, 'resize', measure);
}"#;

/// Publishes the active tab's box (relative to the tablist) as
/// `--active-tab-left/right/top/bottom/width/height` for the animated
/// indicator; re-measures on activation + resize.
pub const TAB_INDICATOR_JS: &str = r#"function(scope){
  var list = scope.parent();
  if (!list || list.__tabIndicator) return; list.__tabIndicator = true;
  var win = (list.ownerDocument && list.ownerDocument.defaultView) || window;
  function measure(){
    var tab = list.querySelector('[role="tab"][data-active]');
    if (!tab) return;
    var l = list.getBoundingClientRect(), t = tab.getBoundingClientRect();
    var s = list.style;
    s.setProperty('--active-tab-left', (t.left - l.left) + 'px');
    s.setProperty('--active-tab-right', (l.right - t.right) + 'px');
    s.setProperty('--active-tab-top', (t.top - l.top) + 'px');
    s.setProperty('--active-tab-bottom', (l.bottom - t.bottom) + 'px');
    s.setProperty('--active-tab-width', t.width + 'px');
    s.setProperty('--active-tab-height', t.height + 'px');
  }
  measure();
  var mo = new MutationObserver(measure);
  mo.observe(list, { attributes: true, subtree: true, attributeFilter: ['data-active'] });
  if (win.ResizeObserver) { var ro = new win.ResizeObserver(measure); ro.observe(list); }
  scope.addEvent(win, 'resize', measure);
}"#;

/// The `<script>` a collapsible/accordion panel embeds to publish its measured
/// size vars. The panel must carry [`DATA_SIZE_VAR`].
#[must_use]
pub fn panel_size_behavior() -> Html {
    scoped_script(PANEL_SIZE_JS)
}

/// The `<script>` a tablist embeds to publish the active tab's box as the
/// `--active-tab-*` indicator vars.
#[must_use]
pub fn tab_indicator_behavior() -> Html {
    scoped_script(TAB_INDICATOR_JS)
}
