//! Re-implementation of [`std::time::Instant`] for wasm32 using `Performance.now()`.

use std::ops::{Add, AddAssign, Sub, SubAssign};
use std::time::Duration;

use super::js::PERFORMANCE;

#[cfg(target_feature = "atomics")]
thread_local! {
    static ORIGIN: f64 = PERFORMANCE.with(super::js::Performance::time_origin);
}

/// See [`std::time::Instant`].
///
/// On wasm32-unknown-unknown, this uses `Performance.now()` instead of the
/// native platform clock, since `std::time::Instant::now()` panics on this target.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct Instant(Duration);

impl Instant {
    /// See [`std::time::Instant::now()`].
    ///
    /// # Panics
    ///
    /// Panics if the [`Performance` object] is not available (e.g. in a worklet).
    ///
    /// [`Performance` object]: https://developer.mozilla.org/en-US/docs/Web/API/performance_property
    #[must_use]
    pub fn now() -> Self {
        let now = PERFORMANCE.with(|performance| {
            #[cfg(target_feature = "atomics")]
            return ORIGIN.with(|origin| performance.now() + origin);

            #[cfg(not(target_feature = "atomics"))]
            performance.now()
        });

        Self(timestamp_to_duration(now))
    }

    /// See [`std::time::Instant::duration_since()`].
    #[must_use]
    pub fn duration_since(&self, earlier: Self) -> Duration {
        self.checked_duration_since(earlier).unwrap_or_default()
    }

    /// See [`std::time::Instant::checked_duration_since()`].
    #[must_use]
    pub fn checked_duration_since(&self, earlier: Self) -> Option<Duration> {
        self.0.checked_sub(earlier.0)
    }

    /// See [`std::time::Instant::saturating_duration_since()`].
    #[must_use]
    pub fn saturating_duration_since(&self, earlier: Self) -> Duration {
        self.checked_duration_since(earlier).unwrap_or_default()
    }

    /// See [`std::time::Instant::elapsed()`].
    #[must_use]
    pub fn elapsed(&self) -> Duration {
        Self::now() - *self
    }

    /// See [`std::time::Instant::checked_add()`].
    pub fn checked_add(&self, duration: Duration) -> Option<Self> {
        self.0.checked_add(duration).map(Instant)
    }

    /// See [`std::time::Instant::checked_sub()`].
    pub fn checked_sub(&self, duration: Duration) -> Option<Self> {
        self.0.checked_sub(duration).map(Instant)
    }
}

impl Add<Duration> for Instant {
    type Output = Self;

    fn add(self, other: Duration) -> Self {
        self.checked_add(other)
            .expect("overflow when adding duration to instant")
    }
}

impl AddAssign<Duration> for Instant {
    fn add_assign(&mut self, other: Duration) {
        *self = *self + other;
    }
}

impl Sub<Duration> for Instant {
    type Output = Self;

    fn sub(self, other: Duration) -> Self {
        self.checked_sub(other)
            .expect("overflow when subtracting duration from instant")
    }
}

impl Sub<Self> for Instant {
    type Output = Duration;

    fn sub(self, other: Self) -> Duration {
        self.duration_since(other)
    }
}

impl SubAssign<Duration> for Instant {
    fn sub_assign(&mut self, other: Duration) {
        *self = *self - other;
    }
}

/// Converts a `DOMHighResTimeStamp` (milliseconds as f64) to a [`Duration`].
///
/// When the `msrv` feature is enabled and Rust >= 1.77, uses `f64.nearest`
/// for better performance.
#[allow(clippy::as_conversions, clippy::cast_possible_truncation, clippy::cast_sign_loss)]
fn timestamp_to_duration(timestamp: f64) -> Duration {
    #[cfg(feature = "msrv")]
    {
        // f64.nearest is available since Rust 1.77 with std feature
        let millis = timestamp.round() as u64;
        Duration::from_millis(millis)
    }

    #[cfg(not(feature = "msrv"))]
    {
        Duration::from_millis(timestamp.trunc() as u64)
            + Duration::from_nanos((timestamp.fract() * 1_000_000.0).round() as u64)
    }
}

#[cfg(feature = "serde")]
mod serde_impl {
    use super::*;
    use serde::{Deserialize, Deserializer, Serialize, Serializer};

    impl Serialize for Instant {
        fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
        where
            S: Serializer,
        {
            self.0.serialize(serializer)
        }
    }

    impl<'de> Deserialize<'de> for Instant {
        fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
        where
            D: Deserializer<'de>,
        {
            let duration = Duration::deserialize(deserializer)?;
            Ok(Instant(duration))
        }
    }
}

#[cfg(feature = "serde")]
pub use serde_impl::*;

#[cfg(test)]
mod test {
    use std::time::Duration;

    use rand::distributions::Uniform;
    use rand::rngs::{OsRng, StdRng};
    use rand::{Rng, SeedableRng};
    use wasm_bindgen_test::wasm_bindgen_test;

    /// Range to maximum accurately representable integer.
    const MAXIMUM_ACCURATE_F64: u64 = u64::pow(2, f64::MANTISSA_DIGITS);

    /// [`Duration`] wrapper to simulate perfect-precision conversion.
    #[derive(Debug)]
    struct ControlDuration(Duration);

    impl ControlDuration {
        /// Implements perfect but expensive conversion from
        /// `DOMHighResTimeStamp` to [`Duration`].
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

    /// Compare [`super::timestamp_to_duration()`] against a pre-determined set
    /// of [`Duration`]s.
    #[wasm_bindgen_test]
    fn sanity() {
        #[track_caller]
        fn assert(time_stamp: f64, result: Duration) {
            let control = ControlDuration::new(time_stamp);
            let duration = super::timestamp_to_duration(time_stamp);

            assert_eq!(control, result, "control and expected result are different");
            assert_eq!(control, duration);
        }

        assert(0.000_000, Duration::ZERO);
        assert(0.000_000_4, Duration::ZERO);
        assert(0.000_000_5, Duration::from_nanos(1));
        assert(0.000_001, Duration::from_nanos(1));
        assert(0.000_001_4, Duration::from_nanos(1));
        assert(0.000_001_5, Duration::from_nanos(2));
        assert(0.999_999, Duration::from_nanos(999_999));
        assert(0.999_999_4, Duration::from_nanos(999_999));
        assert(0.999_999_5, Duration::from_millis(1));
        assert(1., Duration::from_millis(1));
        assert(1.000_000_4, Duration::from_millis(1));
        assert(1.000_000_5, Duration::from_nanos(1_000_001));
        assert(1.000_001, Duration::from_nanos(1_000_001));
        assert(1.000_001_4, Duration::from_nanos(1_000_001));
        assert(1.000_001_5, Duration::from_nanos(1_000_002));
        assert(999.999_999, Duration::from_nanos(999_999_999));
        assert(999.999_999_4, Duration::from_nanos(999_999_999));
        assert(999.999_999_5, Duration::from_secs(1));
        assert(1000., Duration::from_secs(1));
        assert(1_000.000_000_4, Duration::from_secs(1));
        assert(1_000.000_000_5, Duration::from_nanos(1_000_000_001));
        assert(1_000.000_001, Duration::from_nanos(1_000_000_001));
        assert(1_000.000_001_4, Duration::from_nanos(1_000_000_001));
        assert(1_000.000_001_5, Duration::from_nanos(1_000_000_002));
        #[allow(clippy::as_conversions, clippy::cast_precision_loss)]
        assert(
            MAXIMUM_ACCURATE_F64 as f64,
            Duration::from_secs(MAXIMUM_ACCURATE_F64) / 1000,
        );
    }

    /// Compare [`super::timestamp_to_duration()`] against random
    /// [`Duration`]s.
    #[wasm_bindgen_test]
    fn fuzzing() {
        #[allow(clippy::as_conversions, clippy::cast_precision_loss)]
        let mut random = StdRng::from_rng(OsRng)
            .unwrap()
            .sample_iter(Uniform::new_inclusive(
                0.,
                (MAXIMUM_ACCURATE_F64 / 1000) as f64,
            ));

        for _ in 0..10_000_000 {
            let time_stamp = random.next().unwrap();

            let control = ControlDuration::new(time_stamp);
            let duration = super::timestamp_to_duration(time_stamp);

            assert_eq!(control, duration);
        }
    }
}
