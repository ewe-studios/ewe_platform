//! Geometry types (spec-43 phase-1).

use serde_json::Value;

/// An axis-aligned rectangle in CSS pixels (viewport coordinates).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Rect {
    /// Left edge.
    pub x: f64,
    /// Top edge.
    pub y: f64,
    /// Width.
    pub width: f64,
    /// Height.
    pub height: f64,
}

impl Rect {
    /// The centre point (used as the click/hover target).
    #[must_use]
    pub fn center(&self) -> (f64, f64) {
        (self.x + self.width / 2.0, self.y + self.height / 2.0)
    }

    /// Whether the rect has non-zero area.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.width <= 0.0 || self.height <= 0.0
    }

    /// Build a `Rect` from a CDP box-model content quad (8 numbers: four
    /// `(x, y)` corners). Returns `None` if the quad is malformed.
    #[must_use]
    pub fn from_quad(quad: &Value) -> Option<Self> {
        let arr = quad.as_array()?;
        if arr.len() < 8 {
            return None;
        }
        let n = |i: usize| arr.get(i).and_then(Value::as_f64);
        let xs = [n(0)?, n(2)?, n(4)?, n(6)?];
        let ys = [n(1)?, n(3)?, n(5)?, n(7)?];
        let min_x = xs.iter().cloned().fold(f64::INFINITY, f64::min);
        let max_x = xs.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
        let min_y = ys.iter().cloned().fold(f64::INFINITY, f64::min);
        let max_y = ys.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
        Some(Self { x: min_x, y: min_y, width: max_x - min_x, height: max_y - min_y })
    }
}
