//! `f64` rounding helpers that work with and without `std`.
//!
//! WHY: `f64::fract` and `f64::round` are inherent methods provided by `std`
//! (they lower to libm calls), so they do not exist in `no_std + alloc`
//! builds. Numeric keywords (`multipleOf`, integer detection, `maxLength`
//! style limits) still need them.
//!
//! WHAT: `fract` and `round` with the exact semantics of the `std` methods.
//!
//! HOW: Delegates to the pure-Rust `libm` crate, which is what `std` itself
//! uses on targets without a system libm, so results are identical in both
//! configurations.

/// Fractional part of `x`; same definition as `std`'s `f64::fract`
/// (`x - x.trunc()`), so non-finite inputs yield `NaN`.
#[inline]
pub(crate) fn fract(x: f64) -> f64 {
    x - libm::trunc(x)
}

/// Rounds half-way cases away from zero, like `std`'s `f64::round`.
#[inline]
pub(crate) fn round(x: f64) -> f64 {
    libm::round(x)
}

#[cfg(test)]
mod tests {
    use super::{fract, round};

    #[test]
    fn fract_matches_std() {
        for x in [
            0.0,
            -0.0,
            1.0,
            1.5,
            -1.5,
            2.25,
            -7.75,
            1e300,
            f64::MIN_POSITIVE,
        ] {
            assert_eq!(fract(x).to_bits(), x.fract().to_bits(), "fract({x})");
        }
        assert!(fract(f64::INFINITY).is_nan());
        assert!(fract(f64::NAN).is_nan());
    }

    #[test]
    fn round_matches_std() {
        for x in [
            0.0,
            -0.0,
            0.5,
            -0.5,
            1.5,
            -1.5,
            2.4999,
            2.5,
            -2.5,
            1e300,
            f64::INFINITY,
        ] {
            assert_eq!(round(x).to_bits(), x.round().to_bits(), "round({x})");
        }
        assert!(round(f64::NAN).is_nan());
    }
}
