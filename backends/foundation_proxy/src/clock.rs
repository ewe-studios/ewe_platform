//! Wall-clock conversions from the injected [`Clock`].
//!
//! WHY: the proxy stamps persisted state with a `chrono` date and, for a
//! renewed certificate, a UNIX second. A direct wall-clock read made a
//! deterministic simulation test impossible, so every stored value comes from
//! the clock the host passed to [`ProxyServer::start`](crate::ProxyServer::start).
//!
//! WHAT: the conversions are the `foundation_compact::time` functions; no module
//! repeats the epoch arithmetic, and a time before the epoch is handled instead
//! of panicking.

use chrono::{DateTime, Utc};
use foundation_compact::time;
use foundation_core::valtron::Clock;

/// `clock`'s wall time as a UTC date and time.
///
/// A value outside the `chrono` range (never produced by a real clock) maps to
/// the UNIX epoch.
pub(crate) fn wall_now_utc(clock: &dyn Clock) -> DateTime<Utc> {
    DateTime::from_timestamp_millis(time::unix_millis(clock.wall_now()))
        .unwrap_or(DateTime::UNIX_EPOCH)
}

/// `clock`'s wall time as whole seconds since the UNIX epoch.
///
/// A time before the epoch maps to `0`; a real clock never produces one.
pub(crate) fn wall_seconds(clock: &dyn Clock) -> u64 {
    u64::try_from(time::unix_seconds(clock.wall_now())).unwrap_or_default()
}
