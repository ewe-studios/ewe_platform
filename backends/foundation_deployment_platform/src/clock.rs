//! Wall-clock conversions from the injected [`Clock`].
//!
//! The platform records wall time in three shapes: a `chrono` date and time, an
//! RFC 3339 string and a UNIX millisecond count. This module converts
//! `Clock::wall_now` to each shape, so no module repeats the epoch arithmetic.
//! The conversions use `foundation_compact::time`, which handles a time before
//! the UNIX epoch instead of panicking.

use chrono::{DateTime, Utc};
use foundation_compact::time;
use foundation_core::valtron::Clock;

/// The current wall time as a UTC date and time.
///
/// A value outside the `chrono` range (never produced by a real clock) maps to
/// the UNIX epoch.
pub(crate) fn wall_now_utc(clock: &dyn Clock) -> DateTime<Utc> {
    DateTime::from_timestamp_millis(time::unix_millis(clock.wall_now()))
        .unwrap_or(DateTime::UNIX_EPOCH)
}

/// The current wall time as an RFC 3339 string with a fractional second.
pub(crate) fn wall_now_rfc3339(clock: &dyn Clock) -> String {
    wall_now_utc(clock).to_rfc3339()
}

/// The current wall time as an RFC 3339 string with whole seconds and `Z`.
pub(crate) fn wall_now_rfc3339_secs(clock: &dyn Clock) -> String {
    wall_now_utc(clock).to_rfc3339_opts(chrono::SecondsFormat::Secs, true)
}

/// The current wall time in the `date`T`time` shape that a VM state file uses.
pub(crate) fn wall_now_date_time(clock: &dyn Clock) -> String {
    let now = wall_now_utc(clock);
    format!("{}T{}", now.date_naive(), now.time())
}

/// The current wall time in whole milliseconds since the UNIX epoch.
pub(crate) fn wall_now_millis(clock: &dyn Clock) -> i64 {
    time::unix_millis(clock.wall_now())
}
