//! Distance metrics with a higher-is-better score convention.

use core::cmp::Ordering;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DistanceMetric {
    /// Cosine similarity (1 = identical direction, 0 = orthogonal, -1 = opposite).
    Cosine,
    /// Negative squared L2 distance (higher = closer). Avoids sqrt for ranking.
    L2,
    /// Raw dot product (higher = more similar). Meaningful only for normalized vectors.
    Dot,
}

impl DistanceMetric {
    /// Similarity score where **higher = more similar** for all metrics.
    ///
    /// Dimension mismatch panics in debug, returns `f32::NEG_INFINITY` in release.
    #[must_use]
    pub fn score(self, a: &[f32], b: &[f32]) -> f32 {
        debug_assert_eq!(a.len(), b.len(), "dimension mismatch: {} vs {}", a.len(), b.len());
        if a.len() != b.len() {
            return f32::NEG_INFINITY;
        }
        match self {
            Self::Cosine => cosine_similarity(a, b),
            Self::L2 => neg_squared_l2(a, b),
            Self::Dot => dot_product(a, b),
        }
    }

    /// Checked version of `score`.
    ///
    /// # Errors
    /// Returns `DimensionMismatch` if `a.len() != b.len()`.
    pub fn try_score(self, a: &[f32], b: &[f32]) -> Result<f32, DimensionMismatch> {
        if a.len() != b.len() {
            return Err(DimensionMismatch {
                expected: a.len(),
                got: b.len(),
            });
        }
        Ok(self.score(a, b))
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DimensionMismatch {
    pub expected: usize,
    pub got: usize,
}

impl core::fmt::Display for DimensionMismatch {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "dimension mismatch: expected {}, got {}", self.expected, self.got)
    }
}

fn dot_product(a: &[f32], b: &[f32]) -> f32 {
    a.iter().zip(b.iter()).map(|(x, y)| x * y).sum()
}

#[allow(clippy::similar_names)]
fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
    let dot = dot_product(a, b);
    let norm_a: f32 = a.iter().map(|x| x * x).sum();
    let norm_b: f32 = b.iter().map(|x| x * x).sum();
    if norm_a == 0.0 || norm_b == 0.0 {
        return f32::NEG_INFINITY;
    }
    let denom = libm::sqrtf(norm_a) * libm::sqrtf(norm_b);
    dot / denom
}

fn neg_squared_l2(a: &[f32], b: &[f32]) -> f32 {
    let sum: f32 = a.iter().zip(b.iter()).map(|(x, y)| {
        let d = x - y;
        d * d
    }).sum();
    -sum
}

// ---------------------------------------------------------------------------
// OrderedScore — f32 wrapper with Ord via total_cmp, NaN = least

#[derive(Debug, Clone, Copy)]
pub struct OrderedScore(pub f32);

impl OrderedScore {
    #[must_use]
    pub fn new(score: f32) -> Self {
        Self(score)
    }
}

impl PartialEq for OrderedScore {
    fn eq(&self, other: &Self) -> bool {
        self.0.total_cmp(&other.0) == Ordering::Equal
    }
}

impl Eq for OrderedScore {}

impl PartialOrd for OrderedScore {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for OrderedScore {
    fn cmp(&self, other: &Self) -> Ordering {
        match (self.0.is_nan(), other.0.is_nan()) {
            (true, true) => Ordering::Equal,
            (true, false) => Ordering::Less,
            (false, true) => Ordering::Greater,
            (false, false) => self.0.total_cmp(&other.0),
        }
    }
}
