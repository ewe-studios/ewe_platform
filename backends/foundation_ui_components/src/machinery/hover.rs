//! # M3 — Hover intent + safe polygon (spec-42 §3 M3)
//!
//! WHY: Tooltip / preview-card / hover-popover / submenus open on pointer/focus
//! and close on leave — but moving the cursor diagonally from the trigger to the
//! popup must NOT close it. base-ui's `safePolygon` keeps the popup open while
//! the cursor stays inside a triangle/quadrilateral between its exit point and
//! the popup's near edges (machinery.md §M3). Ported here 1:1.
//!
//! WHAT: [`HOVER_JS`] (timer open/close + the full safe-polygon close path) +
//! [`hover_behavior`] (the `<script>` a hover TRIGGER embeds) + the attribute
//! contract.
//!
//! HOW: Embedded INSIDE the trigger (`scope.parent()` = trigger). pointerenter/
//! focus open after `data-hover-delay`; pointerleave starts the safe polygon:
//! a document `mousemove` handler closes (after `data-hover-close-delay`) only
//! when the cursor exits the polygon, the slow-cursor heuristic fires, or it
//! leaves having landed on the popup. The popup is found via
//! `[data-hover-popup]` in the shared parent; placement side from its
//! `data-side`. Open/close drive the wired `[data-hover-open]`/
//! `[data-hover-close]` actions.

use foundation_ui_traits::Html;

use super::scoped_script;

/// Open delay in ms before the trigger opens on hover/focus (`data-hover-delay`).
pub const DATA_HOVER_DELAY: &str = "data-hover-delay";
/// Close delay in ms before the trigger closes on leave/blur (`data-hover-close-delay`).
pub const DATA_HOVER_CLOSE_DELAY: &str = "data-hover-close-delay";
/// Popup marker the hover behavior resolves as the floating element
/// (`data-hover-popup`).
pub const DATA_HOVER_POPUP: &str = "data-hover-popup";

/// The M3 hover-intent + safe-polygon behavior (port of floating-ui-react
/// `safePolygon.ts` + the hover timer model).
pub const HOVER_JS: &str = r#"function(scope){
  var trig = scope.parent();
  if (!trig || trig.__hover) return; trig.__hover = true;
  var doc = trig.ownerDocument || document;
  var win = doc.defaultView || window;
  var root = trig.parentNode;
  var openDelay = parseInt(trig.getAttribute('data-hover-delay'), 10) || 0;
  var closeDelay = parseInt(trig.getAttribute('data-hover-close-delay'), 10) || 0;
  function floating(){ return root && root.querySelector('[data-hover-popup]'); }
  function fire(sel){ var a = root && root.querySelector(sel); if (a) a.click(); }
  var openT = null, closeT = null, polyT = null, moveHandler = null;
  function clearOpen(){ if (openT) { win.clearTimeout(openT); openT = null; } }
  function clearClose(){ if (closeT) { win.clearTimeout(closeT); closeT = null; } }
  function clearPoly(){ if (polyT) { win.clearTimeout(polyT); polyT = null; } }
  function removeMove(){ if (moveHandler) { doc.removeEventListener('mousemove', moveHandler); moveHandler = null; } clearPoly(); }
  function open(){ clearClose(); removeMove(); clearOpen(); openT = win.setTimeout(function(){ fire('[data-hover-open]'); }, openDelay); }
  function scheduleClose(){ removeMove(); clearClose(); closeT = win.setTimeout(function(){ fire('[data-hover-close]'); }, closeDelay); }

  var BUF = 0.5, SPEED = 0.1;
  function edge(px, py, xi, yi, xj, yj){ return (yi >= py) !== (yj >= py) && px <= ((xj - xi) * (py - yi)) / (yj - yi) + xi; }
  function inQuad(px, py, x1, y1, x2, y2, x3, y3, x4, y4){
    var s = false;
    if (edge(px, py, x1, y1, x2, y2)) s = !s;
    if (edge(px, py, x2, y2, x3, y3)) s = !s;
    if (edge(px, py, x3, y3, x4, y4)) s = !s;
    if (edge(px, py, x4, y4, x1, y1)) s = !s;
    return s;
  }
  function inRect(px, py, r){ return px >= r.left && px <= r.right && py >= r.top && py <= r.bottom; }
  function inAA(px, py, x1, y1, x2, y2){
    return px >= Math.min(x1, x2) && px <= Math.max(x1, x2) && py >= Math.min(y1, y2) && py <= Math.max(y1, y2);
  }
  function startPolygon(x, y){
    var fl = floating();
    if (!fl) { scheduleClose(); return; }
    var side = fl.getAttribute('data-side') || 'bottom';
    var landed = false, lastX = null, lastY = null, lastT = (win.performance ? win.performance.now() : Date.now());
    function slow(nx, ny){
      var t = (win.performance ? win.performance.now() : Date.now()), dt = t - lastT;
      if (lastX === null || dt === 0) { lastX = nx; lastY = ny; lastT = t; return false; }
      var dx = nx - lastX, dy = ny - lastY, d2 = dx * dx + dy * dy, th = dt * dt * SPEED * SPEED;
      lastX = nx; lastY = ny; lastT = t;
      return d2 < th;
    }
    clearClose();
    moveHandler = function(e){
      clearPoly();
      var refRect = trig.getBoundingClientRect(), rect = fl.getBoundingClientRect();
      var cx = e.clientX, cy = e.clientY, target = e.target;
      if (fl.contains(target)) { landed = true; return; }
      if (trig.contains(target)) { landed = true; return; }
      var leaveRight = x > rect.right - rect.width / 2, leaveBottom = y > rect.bottom - rect.height / 2;
      var wider = rect.width > refRect.width, taller = rect.height > refRect.height;
      var left = (wider ? refRect : rect).left, right = (wider ? refRect : rect).right;
      var top = (taller ? refRect : rect).top, bottom = (taller ? refRect : rect).bottom;
      if ((side === 'top' && y >= refRect.bottom - 1) || (side === 'bottom' && y <= refRect.top + 1) ||
          (side === 'left' && x >= refRect.right - 1) || (side === 'right' && x <= refRect.left + 1)) { scheduleClose(); return; }
      var trough = false;
      if (side === 'top') trough = inAA(cx, cy, left, refRect.top + 1, right, rect.bottom - 1);
      else if (side === 'bottom') trough = inAA(cx, cy, left, rect.top + 1, right, refRect.bottom - 1);
      else if (side === 'left') trough = inAA(cx, cy, rect.right - 1, bottom, refRect.left + 1, top);
      else if (side === 'right') trough = inAA(cx, cy, refRect.right - 1, bottom, rect.left + 1, top);
      if (trough) return;
      if (landed && !inRect(cx, cy, refRect)) { scheduleClose(); return; }
      if (slow(cx, cy)) { scheduleClose(); return; }
      var inside = false;
      if (side === 'top' || side === 'bottom') {
        var off = wider ? BUF / 2 : BUF * 4;
        var p1x = wider ? x + off : (leaveRight ? x + off : x - off);
        var p2x = wider ? x - off : (leaveRight ? x + off : x - off);
        var py = side === 'top' ? y + BUF + 1 : y - BUF;
        var nearY = side === 'top' ? rect.bottom - BUF : rect.top + BUF;
        var farY = side === 'top' ? rect.top : rect.bottom;
        var cyL = leaveRight ? nearY : (wider ? nearY : farY);
        var cyR = leaveRight ? (wider ? nearY : farY) : nearY;
        inside = inQuad(cx, cy, p1x, py, p2x, py, rect.left, cyL, rect.right, cyR);
      } else {
        var offY = taller ? BUF / 2 : BUF * 4;
        var p1y = taller ? y + offY : (leaveBottom ? y + offY : y - offY);
        var p2y = taller ? y - offY : (leaveBottom ? y + offY : y - offY);
        var px = side === 'left' ? x + BUF + 1 : x - BUF;
        var nearX = side === 'left' ? rect.right - BUF : rect.left + BUF;
        var farX = side === 'left' ? rect.left : rect.right;
        var cxT = leaveBottom ? nearX : (taller ? nearX : farX);
        var cxB = leaveBottom ? (taller ? nearX : farX) : nearX;
        if (side === 'left') inside = inQuad(cx, cy, cxT, rect.top, cxB, rect.bottom, px, p1y, px, p2y);
        else inside = inQuad(cx, cy, px, p1y, px, p2y, cxT, rect.top, cxB, rect.bottom);
      }
      if (!inside) { scheduleClose(); }
      else if (!landed) { polyT = win.setTimeout(function(){ fire('[data-hover-close]'); }, 40); }
    };
    doc.addEventListener('mousemove', moveHandler);
  }

  scope.addEvent(trig, 'pointerenter', open);
  scope.addEvent(trig, 'pointerleave', function(e){ clearOpen(); startPolygon(e.clientX, e.clientY); });
  scope.addEvent(trig, 'focus', function(){ clearClose(); removeMove(); clearOpen(); fire('[data-hover-open]'); });
  scope.addEvent(trig, 'blur', function(){ scheduleClose(); });
  var fl0 = floating();
  if (fl0) {
    scope.addEvent(fl0, 'pointerenter', function(){ clearClose(); removeMove(); });
    scope.addEvent(fl0, 'pointerleave', function(e){ startPolygon(e.clientX, e.clientY); });
  }
}"#;

/// The `<script>` a hover trigger embeds (INSIDE the trigger element). The
/// trigger carries the optional `data-hover-delay`/`data-hover-close-delay`; the
/// popup carries [`DATA_HOVER_POPUP`]; the overlay renders sibling
/// `[data-hover-open]`/`[data-hover-close]` actions wired to the open signal.
#[must_use]
pub fn hover_behavior() -> Html {
    scoped_script(HOVER_JS)
}
