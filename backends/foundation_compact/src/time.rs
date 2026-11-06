//! UNIX timestamp conversions for [`SystemTime`].
//!
//! Every crate converts a wall-clock time to a UNIX timestamp through these
//! functions. No crate repeats the epoch arithmetic. The functions are pure and
//! never panic.
//!
//! A time before the UNIX epoch maps to a negative value. [`SystemTime::duration_since`]
//! returns an error there, so the functions handle the error instead of
//! unwrapping it.

use core::time::Duration;

use crate::{SystemTime, UNIX_EPOCH};

/// The number of milliseconds from the UNIX epoch to `time`.
///
/// A time before the epoch returns a negative value. A value outside the `i64`
/// range saturates at the nearest bound.
#[must_use]
pub fn unix_millis(time: SystemTime) -> i64 {
    match time.duration_since(UNIX_EPOCH) {
        Ok(after) => duration_millis(after),
        Err(before) => -duration_millis(before.duration()),
    }
}

/// The [`SystemTime`] that is `millis` milliseconds after the UNIX epoch.
///
/// A negative `millis` returns a time before the epoch. [`unix_millis`] maps the
/// result back to `millis`, except where the value saturates.
#[must_use]
pub fn from_unix_millis(millis: i64) -> SystemTime {
    let magnitude = Duration::from_millis(millis.unsigned_abs());
    if millis >= 0 {
        UNIX_EPOCH + magnitude
    } else {
        UNIX_EPOCH - magnitude
    }
}

/// The number of whole seconds from the UNIX epoch to `time`.
///
/// A time before the epoch returns a negative value, rounded down: half a
/// second before the epoch is `-1`. A value outside the `i64` range saturates at
/// the nearest bound.
#[must_use]
pub fn unix_seconds(time: SystemTime) -> i64 {
    match time.duration_since(UNIX_EPOCH) {
        Ok(after) => whole_seconds(after, false),
        Err(before) => whole_seconds(before.duration(), true),
    }
}

/// `duration` in whole milliseconds, saturated to `i64::MAX`.
fn duration_millis(duration: Duration) -> i64 {
    let millis = u128::from(duration.as_secs()) * 1_000 + u128::from(duration.subsec_millis());
    i64::try_from(millis).unwrap_or(i64::MAX)
}

/// `duration` in whole seconds.
///
/// A positive duration truncates. A negative one rounds down on the number
/// line, so a sub-second part adds one to the magnitude.
fn whole_seconds(duration: Duration, negative: bool) -> i64 {
    let whole = i64::try_from(duration.as_secs()).unwrap_or(i64::MAX);
    if !negative {
        return whole;
    }
    let magnitude = if duration.subsec_nanos() == 0 {
        whole
    } else {
        whole.saturating_add(1)
    };
    -magnitude
}
