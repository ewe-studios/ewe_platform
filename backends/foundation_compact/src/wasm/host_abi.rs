//! Host-ABI clock: `performance.now()` and `Date.now()` as plain `abi` host imports.
//!
//! No wasm-bindgen glue, so a module built this way loads under
//! `foundation_wasm`'s `foundation-wasm.js` runtime, which implements these imports
//! in its `web_abi` object. Any other host must supply them too.

#[link(wasm_import_module = "abi")]
unsafe extern "C" {
    /// Monotonic milliseconds since the host's time origin (`performance.now()`).
    fn host_monotonic_now_ms() -> f64;

    /// The host's time origin in milliseconds since the Unix epoch
    /// (`performance.timeOrigin`). Only threaded builds need it.
    #[cfg(target_feature = "atomics")]
    fn host_time_origin_ms() -> f64;

    /// Wall-clock milliseconds since the Unix epoch (`Date.now()`).
    fn host_wall_clock_ms() -> f64;
}

#[cfg(target_feature = "atomics")]
thread_local! {
    // SAFETY: a host import with no arguments and no memory access.
    static ORIGIN: f64 = unsafe { host_time_origin_ms() };
}

/// Monotonic milliseconds for [`super::Instant`], plus the time origin on threaded
/// builds so instants agree across workers.
pub(super) fn monotonic_now_ms() -> f64 {
    // SAFETY: a host import with no arguments and no memory access.
    let now = unsafe { host_monotonic_now_ms() };

    #[cfg(target_feature = "atomics")]
    return ORIGIN.with(|origin| now + origin);

    #[cfg(not(target_feature = "atomics"))]
    now
}

/// Wall-clock milliseconds since the Unix epoch for [`super::SystemTime`].
pub(super) fn wall_clock_ms() -> f64 {
    // SAFETY: a host import with no arguments and no memory access.
    unsafe { host_wall_clock_ms() }
}
