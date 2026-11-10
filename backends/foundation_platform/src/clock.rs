//! Wall-clock conversions from the clock of the injected [`Ctx`].
//!
//! WHY: the platform stamps cached entries, offline mutations, session ids and
//! OTA versions with a wall time. A direct wall-clock read makes a
//! deterministic simulation test impossible, so every stamp comes from the
//! clock the host injected.
//!
//! WHAT: the wall time as whole seconds or milliseconds since the UNIX epoch.
//! The conversions are the `foundation_compact::time` functions; no module
//! repeats the epoch arithmetic.

use foundation_core::valtron::Ctx;

/// The current wall time as whole seconds since the UNIX epoch.
///
/// A time before the epoch maps to `0`; a real clock never produces one.
pub(crate) fn wall_seconds(ctx: &Ctx) -> u64 {
    u64::try_from(foundation_compact::time::unix_seconds(
        ctx.clock().wall_now(),
    ))
    .unwrap_or_default()
}

/// The current wall time as milliseconds since the UNIX epoch.
///
/// A time before the epoch maps to `0`; a real clock never produces one.
pub(crate) fn wall_millis(ctx: &Ctx) -> u64 {
    u64::try_from(foundation_compact::time::unix_millis(
        ctx.clock().wall_now(),
    ))
    .unwrap_or_default()
}
