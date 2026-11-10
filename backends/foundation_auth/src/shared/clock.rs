//! Wall-clock conversions from the injected [`Ctx`].
//!
//! The auth crate needs a wall time in three shapes: a `chrono`
//! `DateTime<Utc>`, a UNIX second and a UNIX millisecond. This module reads
//! `Clock::wall_now` through the `Ctx` and converts it to each shape, so no
//! module repeats the epoch arithmetic. The conversions are the
//! `foundation_compact::time` functions, which handle a time before the UNIX
//! epoch instead of panicking.

use chrono::{DateTime, Utc};
use foundation_compact::time;
use foundation_context::Ctx;

/// The current wall time as a UTC date and time.
///
/// A value outside the `chrono` range (never produced by a real clock) maps to
/// the UNIX epoch.
pub(crate) fn wall_now_utc(ctx: &Ctx) -> DateTime<Utc> {
    DateTime::from_timestamp_millis(time::unix_millis(ctx.clock().wall_now()))
        .unwrap_or(DateTime::UNIX_EPOCH)
}

/// The current wall time as whole seconds since the UNIX epoch.
pub(crate) fn now_seconds(ctx: &Ctx) -> i64 {
    time::unix_seconds(ctx.clock().wall_now())
}

/// The current wall time as milliseconds since the UNIX epoch.
///
/// Only the server module stores millisecond timestamps, so this helper exists
/// only when that feature is on.
#[cfg(feature = "server")]
pub(crate) fn now_millis(ctx: &Ctx) -> i64 {
    time::unix_millis(ctx.clock().wall_now())
}
