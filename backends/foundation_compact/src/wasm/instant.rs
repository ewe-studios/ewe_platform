//! Re-implementation of [`std::time::Instant`] for wasm32 using `Performance.now()`.

use std::ops::{Add, AddAssign, Sub, SubAssign};
use std::time::Duration;

use super::clock;
use crate::timestamp::timestamp_to_duration;

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
    /// With the `wasm-bindgen` backend, panics if the [`Performance` object] is not
    /// available (e.g. in a worklet). The `host-abi` backend leaves that to the host.
    ///
    /// [`Performance` object]: https://developer.mozilla.org/en-US/docs/Web/API/performance_property
    #[must_use]
    pub fn now() -> Self {
        Self(timestamp_to_duration(clock::monotonic_now_ms()))
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
