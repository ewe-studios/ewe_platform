//! WHY: The generative palette/curve helpers are designer-facing API; their
//! math (HSL→hex, Bézier evaluation, eased ramps, paired light/dark) must be
//! correct and `no_std`-safe (spec-39 theme system).
//!
//! WHAT: known-value HSL conversions, Bézier endpoints/easing, and the shape of
//! the pastel/gaming/ramp/dual_theme outputs.

use foundation_theme::curve::{cubic_bezier, ease, sample, svg_path, Point};
use foundation_theme::palette::{dual_theme, gaming, pastel, ramp, Hsl};

fn is_hex(s: &str) -> bool {
    s.len() == 7 && s.starts_with('#') && s[1..].chars().all(|c| c.is_ascii_hexdigit())
}

#[test]
fn hsl_to_hex_known_values() {
    assert_eq!(Hsl::new(0.0, 1.0, 0.5).to_hex(), "#ff0000", "red");
    assert_eq!(Hsl::new(120.0, 1.0, 0.5).to_hex(), "#00ff00", "green");
    assert_eq!(Hsl::new(240.0, 1.0, 0.5).to_hex(), "#0000ff", "blue");
    assert_eq!(Hsl::new(0.0, 0.0, 1.0).to_hex(), "#ffffff", "white");
    assert_eq!(Hsl::new(0.0, 0.0, 0.0).to_hex(), "#000000", "black");
    // Hue wraps and negative hues normalize.
    assert_eq!(Hsl::new(360.0, 1.0, 0.5).to_hex(), "#ff0000");
    assert_eq!(Hsl::new(-120.0, 1.0, 0.5).to_hex(), "#0000ff");
}

#[test]
fn cubic_bezier_hits_endpoints() {
    let (p0, p1, p2, p3) = (
        Point::new(0.0, 0.0),
        Point::new(0.0, 1.0),
        Point::new(1.0, 1.0),
        Point::new(1.0, 0.0),
    );
    assert_eq!(cubic_bezier(p0, p1, p2, p3, 0.0), p0);
    assert_eq!(cubic_bezier(p0, p1, p2, p3, 1.0), p3);
    // Midpoint stays within the convex hull (x in [0,1]).
    let mid = cubic_bezier(p0, p1, p2, p3, 0.5);
    assert!(mid.x > 0.0 && mid.x < 1.0);

    assert_eq!(sample(p0, p1, p2, p3, 3).len(), 3);
    assert!(svg_path(p0, p1, p2, p3).starts_with("M 0 0 C"));
}

#[test]
fn ease_is_pinned_and_monotonic() {
    assert!((ease(0.0, 0.42, 1.0) - 0.0).abs() < 1e-6, "ease(0)=0");
    assert!((ease(1.0, 0.42, 1.0) - 1.0).abs() < 1e-6, "ease(1)=1");
    let (a, b, c) = (ease(0.25, 0.42, 1.0), ease(0.5, 0.42, 1.0), ease(0.75, 0.42, 1.0));
    assert!(a < b && b < c, "monotonic increasing: {a} {b} {c}");
}

#[test]
fn palettes_have_shape_and_valid_hex() {
    let p = pastel(20.0, 5);
    assert_eq!(p.len(), 5);
    assert!(p.iter().all(|c| is_hex(c)), "pastel hex: {p:?}");

    let g = gaming(0.0, 6);
    assert_eq!(g.len(), 6);
    assert!(g.iter().all(|c| is_hex(c)));

    // Ramp: light → dark (first lighter than last by summed channels).
    let r = ramp(210.0, 0.6, 5, 0.42, 1.0);
    assert_eq!(r.len(), 5);
    assert!(r.iter().all(|c| is_hex(c)));
    assert!(channel_sum(&r[0]) > channel_sum(&r[4]), "ramp goes light→dark");

    // Dual theme: two mirrored ramps, opposite lightness direction.
    let (light, dark) = dual_theme(210.0, 0.6, 4);
    assert_eq!(light.len(), 4);
    assert_eq!(dark.len(), 4);
    assert!(channel_sum(&light[0]) > channel_sum(&light[3]), "light: bright→mid");
    assert!(channel_sum(&dark[0]) < channel_sum(&dark[3]), "dark: dim→bright");
}

fn channel_sum(hex: &str) -> u32 {
    let n = |i: usize| u32::from_str_radix(&hex[i..i + 2], 16).unwrap();
    n(1) + n(3) + n(5)
}
