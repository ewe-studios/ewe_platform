//! # Scroll-area / skeleton (F8 — Surfaces)
//!
//! WHY: Two PURE-presentation components — `scroll_area` (custom-styled
//! scrollbars over native scrolling, behavior entirely JS, NO signals — the
//! catalog's proof that "component" ≠ "signals") and `skeleton` (a static
//! loading placeholder; CSS does the shimmer) (spec-42 §F8).
//!
//! WHAT: [`scroll_area`] (markup + a JS module keyed off `data-scroll-area`)
//! and [`skeleton`] over a [`SkeletonShape`].
//!
//! HOW: `scroll_area` renders Root → Viewport (the native scroller) → Content +
//! overlay Scrollbars, and embeds a scoped script that flips the
//! `data-has-overflow-*`/`data-overflow-*-start|end`/`data-scrolling` attributes
//! and the `--scroll-area-thumb-*` vars from scroll/resize events.
//! `scrollbar-width`/`scrollbar-color` is the documented zero-JS recipe.
//! `skeleton` is pure markup with `aria-hidden` (real loading semantics ride
//! `aria-busy` on the replaced region).

use alloc::borrow::Cow;
use alloc::vec::Vec;

use foundation_ui_traits::Html;
use foundation_wasm_ui::html;

use crate::machinery::scoped_script;

/// Scroll-area presentation behavior (overflow/edge attrs + thumb size vars).
const SCROLL_AREA_JS: &str = r#"function(scope){
  var root = scope.parent();
  if (!root || root.__scrollArea) return; root.__scrollArea = true;
  var win = (root.ownerDocument && root.ownerDocument.defaultView) || window;
  var vp = root.querySelector('[data-scroll-viewport]');
  if (!vp) return;
  function update(){
    var hasX = vp.scrollWidth > vp.clientWidth, hasY = vp.scrollHeight > vp.clientHeight;
    root.toggleAttribute('data-has-overflow-x', hasX);
    root.toggleAttribute('data-has-overflow-y', hasY);
    var maxX = vp.scrollWidth - vp.clientWidth, maxY = vp.scrollHeight - vp.clientHeight;
    root.toggleAttribute('data-overflow-x-start', vp.scrollLeft > 0);
    root.toggleAttribute('data-overflow-x-end', vp.scrollLeft < maxX - 1);
    root.toggleAttribute('data-overflow-y-start', vp.scrollTop > 0);
    root.toggleAttribute('data-overflow-y-end', vp.scrollTop < maxY - 1);
    if (vp.scrollWidth > 0) root.style.setProperty('--scroll-area-thumb-width', (vp.clientWidth / vp.scrollWidth * 100) + '%');
    if (vp.scrollHeight > 0) root.style.setProperty('--scroll-area-thumb-height', (vp.clientHeight / vp.scrollHeight * 100) + '%');
  }
  var idle = null;
  scope.addEvent(vp, 'scroll', function(){
    update();
    root.setAttribute('data-scrolling', '');
    if (idle) win.clearTimeout(idle);
    idle = win.setTimeout(function(){ root.removeAttribute('data-scrolling'); }, 600);
  });
  scope.addEvent(root, 'pointerenter', function(){ root.setAttribute('data-hovering', ''); });
  scope.addEvent(root, 'pointerleave', function(){ root.removeAttribute('data-hovering'); });
  if (win.ResizeObserver) { var ro = new win.ResizeObserver(update); ro.observe(vp); }
  update();
}"#;

/// Static config for a scroll-area.
pub struct ScrollAreaConfig {
    /// Class override (default `"scroll-area"`).
    pub class: Option<Cow<'static, str>>,
}

impl Default for ScrollAreaConfig {
    fn default() -> Self {
        Self { class: None }
    }
}

/// Scroll-area — custom scrollbars over a native scroller. PURE (no signals).
#[must_use]
pub fn scroll_area(config: ScrollAreaConfig, children: Vec<Html>) -> Html {
    let class = config.class.unwrap_or(Cow::Borrowed("scroll-area"));
    html! {
        <div class={class} data-scroll-area="true">
            <div class="scroll-area-viewport" data-scroll-viewport="true" tabindex="0">
                <div class="scroll-area-content"><Fragment>{children.clone()}</Fragment></div>
            </div>
            <div class="scroll-area-scrollbar" data-orientation="vertical" aria-hidden="true">
                <div class="scroll-area-thumb"></div>
            </div>
            <div class="scroll-area-scrollbar" data-orientation="horizontal" aria-hidden="true">
                <div class="scroll-area-thumb"></div>
            </div>
            <div class="scroll-area-corner" aria-hidden="true"></div>
            <Fragment>{scoped_script(SCROLL_AREA_JS)}</Fragment>
        </div>
    }
}

/// A skeleton placeholder shape.
pub enum SkeletonShape {
    /// A text line of the given CSS width (e.g. `"60%"`).
    Line {
        /// CSS width token.
        width: Cow<'static, str>,
    },
    /// A circle of the given CSS size (avatar placeholder).
    Circle {
        /// CSS width/height token.
        size: Cow<'static, str>,
    },
    /// A rectangular block.
    Block {
        /// CSS width token.
        width: Cow<'static, str>,
        /// CSS height token.
        height: Cow<'static, str>,
    },
    /// Wrap real children with a loading shimmer.
    Wrap(Vec<Html>),
}

/// Skeleton — a static loading placeholder. PURE; CSS does the shimmer.
/// `aria-hidden`; the real loading semantics ride `aria-busy` on the region
/// this stands in for.
#[must_use]
pub fn skeleton(shape: SkeletonShape) -> Html {
    match shape {
        SkeletonShape::Line { width } => html! {
            <span class="skeleton skeleton-line" aria-hidden="true"
                  data-loading="true" style={alloc::format!("inline-size:{width}")} />
        },
        SkeletonShape::Circle { size } => html! {
            <span class="skeleton skeleton-circle" aria-hidden="true"
                  data-loading="true"
                  style={alloc::format!("inline-size:{size};block-size:{size}")} />
        },
        SkeletonShape::Block { width, height } => html! {
            <span class="skeleton skeleton-block" aria-hidden="true"
                  data-loading="true"
                  style={alloc::format!("inline-size:{width};block-size:{height}")} />
        },
        SkeletonShape::Wrap(children) => html! {
            <div class="skeleton" aria-hidden="true" data-loading="true">
                <Fragment>{children.clone()}</Fragment>
            </div>
        },
    }
}
