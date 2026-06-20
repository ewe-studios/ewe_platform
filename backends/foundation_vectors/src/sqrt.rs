//! Sqrt strategy selection for vector normalization and L2 distance.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum SqrtStrategy {
    /// Normalize vectors on insert so cosine == dot at query time. No sqrt
    /// needed per query.
    NormalizedVectors,
    /// `libm::sqrtf` — pure Rust, works everywhere including wasm.
    #[default]
    Libm,
    /// Fast inverse square root (`0x5f3759df` + 1 Newton iteration).
    /// Educational; not faster than hardware sqrt on modern targets.
    FastInvSqrt,
}

impl SqrtStrategy {
    #[must_use]
    pub fn sqrt(self, x: f32) -> f32 {
        match self {
            Self::NormalizedVectors | Self::Libm => libm::sqrtf(x),
            Self::FastInvSqrt => {
                if x <= 0.0 {
                    return 0.0;
                }
                x * fast_inv_sqrt(x)
            }
        }
    }

    #[must_use]
    pub fn inv_sqrt(self, x: f32) -> f32 {
        match self {
            Self::NormalizedVectors | Self::Libm => {
                let s = libm::sqrtf(x);
                if s == 0.0 { 0.0 } else { 1.0 / s }
            }
            Self::FastInvSqrt => {
                if x <= 0.0 {
                    return 0.0;
                }
                fast_inv_sqrt(x)
            }
        }
    }
}

fn fast_inv_sqrt(x: f32) -> f32 {
    let half = 0.5 * x;
    let i = f32::to_bits(x);
    let i = 0x5f37_59df - (i >> 1);
    let y = f32::from_bits(i);
    y * (1.5 - half * y * y) // one Newton iteration
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn libm_sqrt_accuracy() {
        let s = SqrtStrategy::Libm;
        assert!((s.sqrt(4.0) - 2.0).abs() < 1e-6);
        assert!((s.sqrt(9.0) - 3.0).abs() < 1e-6);
    }

    #[test]
    fn fast_inv_sqrt_reasonable() {
        let s = SqrtStrategy::FastInvSqrt;
        let result = s.sqrt(4.0);
        assert!((result - 2.0).abs() < 0.1, "fast sqrt(4) = {result}");
    }

    #[test]
    fn sqrt_zero() {
        for strat in [SqrtStrategy::Libm, SqrtStrategy::FastInvSqrt] {
            assert_eq!(strat.sqrt(0.0), 0.0);
        }
    }

    #[test]
    fn inv_sqrt_accuracy() {
        let s = SqrtStrategy::Libm;
        assert!((s.inv_sqrt(4.0) - 0.5).abs() < 1e-6);
    }
}
