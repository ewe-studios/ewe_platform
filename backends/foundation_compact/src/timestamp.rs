//! Millisecond timestamps (`DOMHighResTimeStamp`) to [`Duration`].
//!
//! The wasm32 time polyfill reads clocks as `f64` milliseconds (`performance.now()`,
//! `Date.now()`); this is the conversion it uses. It is plain arithmetic, so it is
//! available, and tested, on every target.

use core::time::Duration;

/// Convert a non-negative millisecond timestamp to a [`Duration`], rounding the
/// sub-millisecond part to the nearest nanosecond.
///
/// With the `msrv` feature it rounds to the nearest whole millisecond instead.
/// Negative and NaN inputs saturate to [`Duration::ZERO`] (float-to-int `as`
/// conversions saturate).
#[must_use]
#[allow(
    clippy::as_conversions,
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss
)]
pub fn timestamp_to_duration(timestamp: f64) -> Duration {
    #[cfg(feature = "msrv")]
    {
        Duration::from_millis(timestamp.round() as u64)
    }

    #[cfg(not(feature = "msrv"))]
    {
        Duration::from_millis(timestamp.trunc() as u64)
            + Duration::from_nanos((timestamp.fract() * 1_000_000.0).round() as u64)
    }
}
