//! `timestamp_to_duration`: the millisecond-`f64` → `Duration` conversion behind
//! the wasm32 `Instant`/`SystemTime` polyfill. Moved here from an inline
//! `#[cfg(test)]` module in `src/wasm/instant.rs` that never compiled (it used an
//! undeclared `rand` dependency and only built for wasm).
#![cfg(not(feature = "msrv"))]

use std::time::Duration;

use foundation_compact::rng::{Rng, SeedableRng, Xoshiro256PlusPlus};
use foundation_compact::timestamp::timestamp_to_duration;

/// Largest integer an `f64` represents exactly.
const MAXIMUM_ACCURATE_F64: u64 = u64::pow(2, f64::MANTISSA_DIGITS);

/// [`Duration`] wrapper that converts with perfect precision (slow), as the
/// reference to compare against.
#[derive(Debug)]
struct ControlDuration(Duration);

impl ControlDuration {
    fn new(time_stamp: f64) -> Self {
        // Inspired by https://github.com/rust-lang/rust/blob/1.83.0/library/core/src/time.rs#L822-L833
        const NANOS_PER_SEC: u64 = 1_000_000_000;
        let rhs: u32 = 1000;
        let time_stamp = Duration::from_secs_f64(time_stamp);
        let (secs, extra_secs) = (
            time_stamp.as_secs() / u64::from(rhs),
            time_stamp.as_secs() % u64::from(rhs),
        );
        let (mut nanos, extra_nanos) = (
            time_stamp.subsec_nanos() / rhs,
            time_stamp.subsec_nanos() % rhs,
        );
        let extra = extra_secs * NANOS_PER_SEC + u64::from(extra_nanos);
        nanos += u32::try_from(extra / u64::from(rhs)).unwrap();
        nanos += u32::from(extra % u64::from(rhs) >= u64::from(rhs / 2));
        Self(Duration::new(secs, nanos))
    }
}

impl PartialEq<Duration> for ControlDuration {
    fn eq(&self, other: &Duration) -> bool {
        if self.0 == *other {
            true
        } else if let Some(diff) = self.0.checked_sub(*other) {
            diff == Duration::from_nanos(1)
        } else {
            false
        }
    }
}

#[track_caller]
fn check(time_stamp: f64, result: Duration) {
    let control = ControlDuration::new(time_stamp);
    let duration = timestamp_to_duration(time_stamp);

    assert_eq!(control, result, "control and expected result are different");
    assert_eq!(control, duration);
}

/// Known timestamps around nanosecond, millisecond and second boundaries.
#[test]
fn sanity() {
    check(0.000_000, Duration::ZERO);
    check(0.000_000_4, Duration::ZERO);
    check(0.000_000_5, Duration::from_nanos(1));
    check(0.000_001, Duration::from_nanos(1));
    check(0.000_001_4, Duration::from_nanos(1));
    check(0.000_001_5, Duration::from_nanos(2));
    check(0.999_999, Duration::from_nanos(999_999));
    check(0.999_999_4, Duration::from_nanos(999_999));
    check(0.999_999_5, Duration::from_millis(1));
    check(1., Duration::from_millis(1));
    check(1.000_000_4, Duration::from_millis(1));
    check(1.000_000_5, Duration::from_nanos(1_000_001));
    check(1.000_001, Duration::from_nanos(1_000_001));
    check(1.000_001_4, Duration::from_nanos(1_000_001));
    check(1.000_001_5, Duration::from_nanos(1_000_002));
    check(999.999_999, Duration::from_nanos(999_999_999));
    check(999.999_999_4, Duration::from_nanos(999_999_999));
    check(999.999_999_5, Duration::from_secs(1));
    check(1000., Duration::from_secs(1));
    check(1_000.000_000_4, Duration::from_secs(1));
    check(1_000.000_000_5, Duration::from_nanos(1_000_000_001));
    check(1_000.000_001, Duration::from_nanos(1_000_000_001));
    check(1_000.000_001_4, Duration::from_nanos(1_000_000_001));
    check(1_000.000_001_5, Duration::from_nanos(1_000_000_002));
    #[allow(clippy::cast_precision_loss)]
    check(
        MAXIMUM_ACCURATE_F64 as f64,
        Duration::from_secs(MAXIMUM_ACCURATE_F64) / 1000,
    );
}

/// Negative and NaN timestamps saturate to zero instead of panicking.
#[test]
fn negative_and_nan_saturate_to_zero() {
    assert_eq!(timestamp_to_duration(-1.5), Duration::ZERO);
    assert_eq!(timestamp_to_duration(f64::NAN), Duration::ZERO);
}

/// Random timestamps across the exactly representable range, seeded so a
/// failure reproduces.
#[test]
fn fuzzing() {
    let mut rng = Xoshiro256PlusPlus::seed_from_u64(0x5eed);
    #[allow(clippy::cast_precision_loss)]
    let max = (MAXIMUM_ACCURATE_F64 / 1000) as f64;

    for _ in 0..1_000_000 {
        // 53 random bits → a uniform f64 in [0, 1], scaled to [0, max].
        #[allow(clippy::cast_precision_loss)]
        let unit = (rng.next_u64() >> 11) as f64 / (1u64 << 53) as f64;
        let time_stamp = unit * max;

        let control = ControlDuration::new(time_stamp);
        let duration = timestamp_to_duration(time_stamp);

        assert_eq!(control, duration, "time_stamp = {time_stamp}");
    }
}
