//! Re-implementation of [`std::time::SystemTime`] for wasm32 using `Date.now()`.

use std::error::Error;
use std::fmt::{self, Display, Formatter};
use std::ops::{Add, AddAssign, Sub, SubAssign};
use std::time::Duration;

/// See [`std::time::SystemTime`].
///
/// On wasm32-unknown-unknown, this uses `Date.now()` instead of the native
/// system clock, since `std::time::SystemTime::now()` panics on this target.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct SystemTime(pub(crate) Duration);

impl SystemTime {
    /// See [`std::time::SystemTime::UNIX_EPOCH`].
    pub const UNIX_EPOCH: Self = Self(Duration::ZERO);

    /// See [`std::time::SystemTime::now()`].
    #[must_use]
    pub fn now() -> Self {
        #[allow(clippy::as_conversions, clippy::cast_possible_truncation)]
        let ms = js_sys::Date::now() as i64;
        let ms = ms.try_into().expect("found negative timestamp");
        Self(Duration::from_millis(ms))
    }

    /// See [`std::time::SystemTime::duration_since()`].
    pub fn duration_since(&self, earlier: Self) -> Result<Duration, SystemTimeError> {
        if self.0 < earlier.0 {
            Err(SystemTimeError(earlier.0 - self.0))
        } else {
            Ok(self.0 - earlier.0)
        }
    }

    /// See [`std::time::SystemTime::elapsed()`].
    pub fn elapsed(&self) -> Result<Duration, SystemTimeError> {
        Self::now().duration_since(*self)
    }

    /// See [`std::time::SystemTime::checked_add()`].
    pub fn checked_add(&self, duration: Duration) -> Option<Self> {
        self.0.checked_add(duration).map(SystemTime)
    }

    /// See [`std::time::SystemTime::checked_sub()`].
    pub fn checked_sub(&self, duration: Duration) -> Option<Self> {
        self.0.checked_sub(duration).map(SystemTime)
    }
}

impl Add<Duration> for SystemTime {
    type Output = Self;

    fn add(self, dur: Duration) -> Self {
        self.checked_add(dur)
            .expect("overflow when adding duration to instant")
    }
}

impl AddAssign<Duration> for SystemTime {
    fn add_assign(&mut self, other: Duration) {
        *self = *self + other;
    }
}

impl Sub<Duration> for SystemTime {
    type Output = Self;

    fn sub(self, dur: Duration) -> Self {
        self.checked_sub(dur)
            .expect("overflow when subtracting duration from instant")
    }
}

impl SubAssign<Duration> for SystemTime {
    fn sub_assign(&mut self, other: Duration) {
        *self = *self - other;
    }
}

/// See [`std::time::SystemTimeError`].
#[derive(Clone, Debug)]
pub struct SystemTimeError(pub(crate) Duration);

impl SystemTimeError {
    /// See [`std::time::SystemTimeError::duration()`].
    #[must_use]
    pub fn duration(&self) -> Duration {
        self.0
    }
}

impl Display for SystemTimeError {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(f, "second time provided was later than self")
    }
}

impl Error for SystemTimeError {}

#[cfg(feature = "serde")]
mod serde_impl {
    use super::*;
    use serde::{Deserialize, Deserializer, Serialize, Serializer};

    impl Serialize for SystemTime {
        fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
        where
            S: Serializer,
        {
            self.0.serialize(serializer)
        }
    }

    impl<'de> Deserialize<'de> for SystemTime {
        fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
        where
            D: Deserializer<'de>,
        {
            let duration = Duration::deserialize(deserializer)?;
            Ok(SystemTime(duration))
        }
    }
}

#[cfg(feature = "serde")]
pub use serde_impl::*;
