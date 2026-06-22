//! # Bézier curves — the math behind smooth scales & SVG paths
//!
//! WHY: Smooth, designer-pleasing progressions (color ramps, easing, hand-drawn
//! SVG strokes) all come from the same primitive — a cubic Bézier. Centralize
//! it so palettes (`palette`), animations, and vector art share one
//! implementation (spec-39, theme system).
//!
//! WHAT: [`Point`], cubic Bézier evaluation in 2D ([`cubic_bezier`]) and 1D
//! easing ([`ease`]), uniform sampling ([`sample`]), an SVG path builder
//! ([`svg_path`]), and the CSS [`cubic_bezier_css`] string.
//!
//! HOW: Pure polynomial arithmetic — `no_std`-safe (no `sqrt`/`powf`/trig). A
//! cubic Bézier is `B(t) = (1-t)³P₀ + 3(1-t)²t·P₁ + 3(1-t)t²·P₂ + t³P₃`.

use alloc::format;
use alloc::string::String;
use alloc::vec::Vec;
use core::fmt::Write as _;

/// A 2D point (also reused as a control value pair).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Point {
    /// X coordinate.
    pub x: f32,
    /// Y coordinate.
    pub y: f32,
}

impl Point {
    /// Construct a point.
    #[must_use]
    pub const fn new(x: f32, y: f32) -> Self {
        Self { x, y }
    }
}

/// Evaluate a cubic Bézier at `t ∈ [0, 1]`.
#[must_use]
pub fn cubic_bezier(p0: Point, p1: Point, p2: Point, p3: Point, t: f32) -> Point {
    let u = 1.0 - t;
    let w0 = u * u * u;
    let w1 = 3.0 * u * u * t;
    let w2 = 3.0 * u * t * t;
    let w3 = t * t * t;
    Point {
        x: w0 * p0.x + w1 * p1.x + w2 * p2.x + w3 * p3.x,
        y: w0 * p0.y + w1 * p1.y + w2 * p2.y + w3 * p3.y,
    }
}

/// A 1D ease over `t ∈ [0, 1]` with endpoints pinned to 0 and 1 and two
/// control values `c1`, `c2` (the y's of a CSS-style `cubic-bezier`). Returns
/// the eased value — use it to distribute lightness, spacing, etc. non-linearly.
#[must_use]
pub fn ease(t: f32, c1: f32, c2: f32) -> f32 {
    let u = 1.0 - t;
    // P0 = 0, P3 = 1.
    3.0 * u * u * t * c1 + 3.0 * u * t * t * c2 + t * t * t
}

/// Sample `n` points uniformly in `t` (`t = i/(n-1)`) along a cubic Bézier.
/// `n < 2` yields just the endpoint(s).
#[must_use]
pub fn sample(p0: Point, p1: Point, p2: Point, p3: Point, n: usize) -> Vec<Point> {
    if n == 0 {
        return Vec::new();
    }
    if n == 1 {
        return alloc::vec![p0];
    }
    #[allow(clippy::cast_precision_loss)]
    let last = (n - 1) as f32;
    (0..n)
        .map(|i| {
            #[allow(clippy::cast_precision_loss)]
            let t = i as f32 / last;
            cubic_bezier(p0, p1, p2, p3, t)
        })
        .collect()
}

/// Build an SVG path `d` string for a single cubic segment (`M … C …`).
#[must_use]
pub fn svg_path(p0: Point, p1: Point, p2: Point, p3: Point) -> String {
    format!(
        "M {} {} C {} {}, {} {}, {} {}",
        p0.x, p0.y, p1.x, p1.y, p2.x, p2.y, p3.x, p3.y
    )
}

/// Build a smooth SVG polyline `d` string through sampled curve points.
#[must_use]
pub fn svg_polyline(points: &[Point]) -> String {
    let mut d = String::new();
    for (i, p) in points.iter().enumerate() {
        let cmd = if i == 0 { 'M' } else { 'L' };
        let _ = write!(d, "{cmd} {} {} ", p.x, p.y);
    }
    let trimmed = d.trim_end();
    String::from(trimmed)
}

/// The CSS `cubic-bezier(x1, y1, x2, y2)` timing-function string.
#[must_use]
pub fn cubic_bezier_css(x1: f32, y1: f32, x2: f32, y2: f32) -> String {
    format!("cubic-bezier({x1}, {y1}, {x2}, {y2})")
}
