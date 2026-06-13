//! # M1 — Anchored positioning (placement + collision) (spec-42 §3 M1)
//!
//! WHY: Popover/tooltip/preview-card/menus on the JS fallback path (where CSS
//! Anchor Positioning is unavailable) must place a popup relative to its
//! trigger, flip/shift it to stay on-screen, and publish the headless styling
//! contract — `data-side`/`data-align` (arrow + origin hooks) and the
//! `--anchor-*`/`--available-*`/`--positioner-*`/`--popup-*`/`--transform-origin`
//! CSS vars (machinery.md §M1, F4 Positioner table).
//!
//! WHAT: [`POSITION_JS`] (the behavior) + [`position_behavior`] (the `<script>`
//! a positioner embeds) + the data-attribute contract constants. The
//! [`crate::positioning`] config types ([`PlacementSide`]/[`PlacementAlign`]/
//! [`CollisionBehavior`]) describe the same contract on the Rust side.
//!
//! HOW: Reads its options off `scope.parent()` (the positioner element):
//! `data-anchor` (trigger id — else the previous sibling), `data-prefer-side`,
//! `data-prefer-align`, `data-offset`, `data-collision-side|align` (`flip|shift|
//! none`), `data-collision-padding`, `data-position-method`,
//! `data-disable-anchor-tracking`. It measures both rects, applies side flip +
//! align flip/shift within the viewport, writes `style.left/top` and the full
//! var/attr contract, then re-runs on scroll/resize (unless tracking is
//! disabled). The full three-knob `fallbackAxisSide`, `sticky`, and arrow
//! centering are documented follow-ups (machinery.md §M1).
//!
//! [`PlacementSide`]: crate::positioning::PlacementSide
//! [`PlacementAlign`]: crate::positioning::PlacementAlign
//! [`CollisionBehavior`]: crate::positioning::CollisionBehavior

use foundation_ui_traits::Html;

use super::scoped_script;

/// Positioner attribute: the trigger/anchor element id (`data-anchor`).
/// Absent → the positioner's previous element sibling is the anchor.
pub const DATA_ANCHOR: &str = "data-anchor";
/// Preferred side token (`data-prefer-side`): `top|bottom|left|right`.
pub const DATA_PREFER_SIDE: &str = "data-prefer-side";
/// Preferred alignment token (`data-prefer-align`): `start|center|end`.
pub const DATA_PREFER_ALIGN: &str = "data-prefer-align";
/// Final post-collision side, reflected for CSS (`data-side`).
pub const DATA_SIDE: &str = "data-side";
/// Final post-collision alignment, reflected for CSS (`data-align`).
pub const DATA_ALIGN: &str = "data-align";
/// Present when the anchor scrolled fully out of view (`data-anchor-hidden`).
pub const DATA_ANCHOR_HIDDEN: &str = "data-anchor-hidden";

/// The M1 anchored-positioning behavior. Places the positioner against its
/// anchor with side flip + align flip/shift, and emits the `data-side`/
/// `data-align` + CSS-var styling contract; re-runs on scroll/resize.
pub const POSITION_JS: &str = r#"function(scope){
  var pos = scope.parent();
  if (!pos) return;
  var doc = pos.ownerDocument || document;
  var win = doc.defaultView || window;
  var anchorId = pos.getAttribute('data-anchor');
  var prefSide = pos.getAttribute('data-prefer-side') || 'bottom';
  var prefAlign = pos.getAttribute('data-prefer-align') || 'center';
  var offset = parseFloat(pos.getAttribute('data-offset')) || 0;
  var pad = parseFloat(pos.getAttribute('data-collision-padding'));
  if (isNaN(pad)) pad = 5;
  var sideMode = pos.getAttribute('data-collision-side') || 'flip';
  var alignMode = pos.getAttribute('data-collision-align') || 'flip';
  var track = pos.getAttribute('data-disable-anchor-tracking') !== 'true';
  function opposite(s){ return {top:'bottom',bottom:'top',left:'right',right:'left'}[s]; }
  function vertical(s){ return s === 'top' || s === 'bottom'; }
  function anchorEl(){ return anchorId ? doc.getElementById(anchorId) : pos.previousElementSibling; }
  function place(){
    var anchor = anchorEl();
    if (!anchor) return;
    var a = anchor.getBoundingClientRect();
    var p = pos.getBoundingClientRect();
    var w = p.width, h = p.height;
    var vw = doc.documentElement.clientWidth;
    var vh = doc.documentElement.clientHeight;
    var space = { top: a.top - pad, bottom: vh - a.bottom - pad, left: a.left - pad, right: vw - a.right - pad };
    var side = prefSide, align = prefAlign;
    var need = (vertical(side) ? h : w) + offset;
    if (sideMode === 'flip' && space[side] < need && space[opposite(side)] > space[side]) side = opposite(side);
    var left, top;
    if (side === 'top') top = a.top - h - offset;
    else if (side === 'bottom') top = a.bottom + offset;
    else if (side === 'left') left = a.left - w - offset;
    else left = a.right + offset;
    function cross(al){
      if (vertical(side)) return al === 'start' ? a.left : al === 'end' ? a.right - w : a.left + (a.width - w) / 2;
      return al === 'start' ? a.top : al === 'end' ? a.bottom - h : a.top + (a.height - h) / 2;
    }
    var lo = pad;
    var hi = vertical(side) ? vw - w - pad : vh - h - pad;
    var c = cross(align);
    if (alignMode === 'flip' && (align === 'start' || align === 'end')) {
      if (c < lo || c > hi) {
        var alt = align === 'start' ? 'end' : 'start', c2 = cross(alt);
        if (c2 >= lo && c2 <= hi) { align = alt; c = c2; }
      }
    } else if (alignMode === 'shift') {
      if (c > hi) c = hi;
      if (c < lo) c = lo;
    }
    if (vertical(side)) left = c; else top = c;
    var fixed = pos.getAttribute('data-position-method') === 'fixed';
    pos.style.position = fixed ? 'fixed' : 'absolute';
    var sx = fixed ? 0 : (win.scrollX || 0), sy = fixed ? 0 : (win.scrollY || 0);
    pos.style.left = (left + sx) + 'px';
    pos.style.top = (top + sy) + 'px';
    pos.setAttribute('data-side', side);
    pos.setAttribute('data-align', align);
    if (a.bottom < 0 || a.right < 0 || a.top > vh || a.left > vw) pos.setAttribute('data-anchor-hidden', '');
    else pos.removeAttribute('data-anchor-hidden');
    var st = pos.style;
    st.setProperty('--anchor-width', a.width + 'px');
    st.setProperty('--anchor-height', a.height + 'px');
    st.setProperty('--available-width', Math.max(0, vertical(side) ? vw - 2 * pad : space[side]) + 'px');
    st.setProperty('--available-height', Math.max(0, vertical(side) ? space[side] : vh - 2 * pad) + 'px');
    st.setProperty('--positioner-width', w + 'px');
    st.setProperty('--positioner-height', h + 'px');
    st.setProperty('--popup-width', w + 'px');
    st.setProperty('--popup-height', h + 'px');
    var ox = align === 'start' ? '0%' : align === 'end' ? '100%' : '50%';
    st.setProperty('--transform-origin', vertical(side)
      ? ox + ' ' + (side === 'top' ? '100%' : '0%')
      : (side === 'left' ? '100%' : '0%') + ' ' + ox);
  }
  place();
  if (track) {
    scope.addEvent(doc, 'scroll', place, true);
    scope.addEvent(win, 'resize', place);
  }
}"#;

/// The `<script>` node a positioner embeds to get M1 anchored positioning.
/// The positioner root must carry [`DATA_ANCHOR`] (or sit immediately after
/// its anchor) plus the optional `data-prefer-side`/`data-prefer-align`/
/// `data-offset`/`data-collision-*` config; the body writes [`DATA_SIDE`]/
/// [`DATA_ALIGN`] + the CSS-var contract.
#[must_use]
pub fn position_behavior() -> Html {
    scoped_script(POSITION_JS)
}
