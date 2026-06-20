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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cosine_identical_vectors() {
        let a = [1.0, 0.0, 0.0];
        let s = DistanceMetric::Cosine.score(&a, &a);
        assert!((s - 1.0).abs() < 1e-6);
    }

    #[test]
    fn cosine_orthogonal() {
        let a = [1.0, 0.0];
        let b = [0.0, 1.0];
        let s = DistanceMetric::Cosine.score(&a, &b);
        assert!(s.abs() < 1e-6);
    }

    #[test]
    fn cosine_opposite() {
        let a = [1.0, 0.0];
        let b = [-1.0, 0.0];
        let s = DistanceMetric::Cosine.score(&a, &b);
        assert!((s - (-1.0)).abs() < 1e-6);
    }

    #[test]
    fn cosine_zero_vector_returns_neg_inf() {
        let a = [0.0, 0.0];
        let b = [1.0, 0.0];
        assert_eq!(DistanceMetric::Cosine.score(&a, &b), f32::NEG_INFINITY);
    }

    #[test]
    fn dot_product_known() {
        let a = [1.0, 2.0, 3.0];
        let b = [4.0, 5.0, 6.0];
        let s = DistanceMetric::Dot.score(&a, &b);
        assert!((s - 32.0).abs() < 1e-6);
    }

    #[test]
    fn l2_identical_is_zero() {
        let a = [1.0, 2.0];
        let s = DistanceMetric::L2.score(&a, &a);
        assert!((s - 0.0).abs() < 1e-6);
    }

    #[test]
    fn l2_higher_means_closer() {
        let q = [0.0, 0.0];
        let near = [1.0, 0.0];
        let far = [3.0, 4.0];
        let s_near = DistanceMetric::L2.score(&q, &near);
        let s_far = DistanceMetric::L2.score(&q, &far);
        assert!(s_near > s_far, "near={s_near} should be > far={s_far}");
    }

    #[test]
    fn try_score_dimension_mismatch() {
        let a = [1.0, 2.0];
        let b = [1.0, 2.0, 3.0];
        let err = DistanceMetric::Cosine.try_score(&a, &b).unwrap_err();
        assert_eq!(err.expected, 2);
        assert_eq!(err.got, 3);
    }

    #[test]
    fn ordered_score_nan_is_least() {
        let nan = OrderedScore(f32::NAN);
        let neg = OrderedScore(f32::NEG_INFINITY);
        let zero = OrderedScore(0.0);
        assert!(nan < neg);
        assert!(nan < zero);
    }

    #[test]
    fn ordered_score_sorts_correctly() {
        let mut scores = vec![
            OrderedScore(0.5),
            OrderedScore(f32::NAN),
            OrderedScore(0.9),
            OrderedScore(-0.1),
        ];
        scores.sort();
        assert!(scores[0].0.is_nan());
        assert!((scores[1].0 - (-0.1)).abs() < 1e-6);
        assert!((scores[2].0 - 0.5).abs() < 1e-6);
        assert!((scores[3].0 - 0.9).abs() < 1e-6);
    }
}
