//! Wall-clock conversions from the clock of the injected [`Ctx`].
//!
//! Vault rows store their timestamps as `chrono` `DateTime<Utc>` values. This
//! module converts `Clock::wall_now` to one, through the
//! `foundation_compact::time` helpers, so no handler repeats the epoch
//! arithmetic.

use chrono::{DateTime, Utc};
use foundation_compact::time;
use foundation_context::Ctx;

/// The current wall time as a UTC date and time.
///
/// A value outside the `chrono` range (never produced by a real clock) maps to
/// the UNIX epoch.
#[must_use]
pub fn wall_now_utc(ctx: &Ctx) -> DateTime<Utc> {
    DateTime::from_timestamp_millis(time::unix_millis(ctx.clock().wall_now()))
        .unwrap_or(DateTime::UNIX_EPOCH)
}
