//! # Palette — generative, designer-friendly color scales
//!
//! WHY: Hand-picking harmonious colors is tedious; good palettes follow
//! math (even hue spacing, eased lightness ramps, paired light/dark). Give
//! designers one call for a beautiful set (spec-39, theme system) — feed the
//! results straight into [`Theme`](crate::Theme).
//!
//! WHAT: [`Hsl`] + hex conversion, even-hue [`pastel`]/[`gaming`] sets, a
//! Bézier-eased shade [`ramp`], and [`dual_theme`] — two MIRRORED lightness
//! curves that yield a light palette and a dark palette designed to pair.
//!
//! HOW: HSL→RGB by pure arithmetic (`no_std`-safe — no trig); lightness
//! progressions come from [`crate::curve::ease`].

use alloc::format;
use alloc::string::String;
use alloc::vec::Vec;

use crate::curve::ease;

/// An HSL color: `h ∈ [0, 360)`, `s`/`l ∈ [0, 1]`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Hsl {
    /// Hue in degrees.
    pub h: f32,
    /// Saturation `0..=1`.
    pub s: f32,
    /// Lightness `0..=1`.
    pub l: f32,
}

impl Hsl {
    /// Construct an HSL color.
    #[must_use]
    pub const fn new(h: f32, s: f32, l: f32) -> Self {
        Self { h, s, l }
    }

    /// Convert to `#rrggbb`.
    #[must_use]
    pub fn to_hex(self) -> String {
        let (r, g, b) = self.to_rgb();
        format!("#{r:02x}{g:02x}{b:02x}")
    }

    /// Convert to 8-bit RGB.
    #[must_use]
    pub fn to_rgb(self) -> (u8, u8, u8) {
        let h = norm_deg(self.h);
        let s = clamp01(self.s);
        let l = clamp01(self.l);
        let c = (1.0 - abs(2.0 * l - 1.0)) * s;
        let hp = h / 60.0;
        let x = c * (1.0 - abs(hp % 2.0 - 1.0));
        let (r1, g1, b1) = if hp < 1.0 {
            (c, x, 0.0)
        } else if hp < 2.0 {
            (x, c, 0.0)
        } else if hp < 3.0 {
            (0.0, c, x)
        } else if hp < 4.0 {
            (0.0, x, c)
        } else if hp < 5.0 {
            (x, 0.0, c)
        } else {
            (c, 0.0, x)
        };
        let m = l - c / 2.0;
        (to_u8(r1 + m), to_u8(g1 + m), to_u8(b1 + m))
    }
}

/// `count` soft PASTEL colors: high lightness, gentle saturation, hues evenly
/// spread around the wheel from `start_hue`.
#[must_use]
pub fn pastel(start_hue: f32, count: usize) -> Vec<String> {
    even_hues(start_hue, count, 0.70, 0.85)
}

/// `count` vivid GAMING/neon colors: punchy saturation, mid lightness.
#[must_use]
pub fn gaming(start_hue: f32, count: usize) -> Vec<String> {
    even_hues(start_hue, count, 0.95, 0.55)
}

/// A single-hue shade RAMP of `count` steps from light to dark, the lightness
/// progression eased by a cubic Bézier (`ease_c1`, `ease_c2` — try `0.42`,
/// `1.0` for a soft ease-out). Great for `50…900`-style scales.
#[must_use]
pub fn ramp(hue: f32, sat: f32, count: usize, ease_c1: f32, ease_c2: f32) -> Vec<String> {
    let denom = count.saturating_sub(1).max(1) as f32;
    (0..count)
        .map(|i| {
            let t = i as f32 / denom;
            let e = ease(t, ease_c1, ease_c2); // 0..1
            // Light (0.95) → dark (0.12).
            let l = 0.95 - e * 0.83;
            Hsl::new(hue, sat, l).to_hex()
        })
        .collect()
}

/// TWO mirrored ramps for paired light & dark themes: the light palette runs
/// lightness high→low, the dark palette runs the OPPOSITE way (low→high) over
/// the same hue/saturation — so a token's light and dark values are visual
/// counterparts. Returns `(light, dark)`, each `count` long.
#[must_use]
pub fn dual_theme(hue: f32, sat: f32, count: usize) -> (Vec<String>, Vec<String>) {
    let denom = count.saturating_sub(1).max(1) as f32;
    let mut light = Vec::with_capacity(count);
    let mut dark = Vec::with_capacity(count);
    for i in 0..count {
        let t = i as f32 / denom;
        let e = ease(t, 0.42, 1.0);
        // Light theme: bright → mid.   Dark theme: the mirror (dim → bright).
        light.push(Hsl::new(hue, sat, 0.92 - e * 0.55).to_hex());
        dark.push(Hsl::new(hue, sat * 0.9, 0.18 + e * 0.55).to_hex());
    }
    (light, dark)
}

/// `count` colors at one (s, l) with hues evenly spaced from `start_hue`.
fn even_hues(start_hue: f32, count: usize, s: f32, l: f32) -> Vec<String> {
    if count == 0 {
        return Vec::new();
    }
    let step = 360.0 / count as f32;
    (0..count)
        .map(|i| Hsl::new(start_hue + step * i as f32, s, l).to_hex())
        .collect()
}

#[inline]
fn abs(x: f32) -> f32 {
    if x < 0.0 {
        -x
    } else {
        x
    }
}

#[inline]
fn clamp01(x: f32) -> f32 {
    if x < 0.0 {
        0.0
    } else if x > 1.0 {
        1.0
    } else {
        x
    }
}

/// Normalize degrees into `[0, 360)`.
#[inline]
fn norm_deg(h: f32) -> f32 {
    let m = h % 360.0;
    if m < 0.0 {
        m + 360.0
    } else {
        m
    }
}

/// Scale a `0..=1` channel to a rounded `u8`.
#[inline]
fn to_u8(v: f32) -> u8 {
    let scaled = v * 255.0 + 0.5;
    if scaled <= 0.0 {
        0
    } else if scaled >= 255.0 {
        255
    } else {
        scaled as u8
    }
}
