//! WHY: Issue #219. `wasi_host` read `Instant::now()` directly in `host_now`,
//! so a deterministic test could not control the instant the clock-reading
//! functions used. The host now installs its own `TimeSource` before the
//! module runs; no clock read stays in the module.
//!
//! WHAT: `WasiHost::with_time_source`, `WasiHost::install`,
//! `WasiHost::uninstall`, `wasi_host::now`, and `WasiHostError`.
//!
//! HOW: Install a source that always returns a fixed instant, then assert the
//! module returns that instant and arms a timer from it. With no source
//! installed, assert the typed error.
//!
//! NOTE: The installed source is process-global, like the timer state, so
//! `#[serial_test]` serialises these tests and each one clears the source
//! before it runs.

#![cfg(feature = "wasi")]

use std::sync::Arc;
use std::time::{Duration, Instant};

use foundation_macros::serial_test;
use foundation_wasm::wasi_host::{self, TimeSource, WasiHost, WasiHostError};

/// A time source that always returns the same instant.
struct FixedSource(Instant);

impl TimeSource for FixedSource {
    fn now(&self) -> Instant {
        self.0
    }
}

#[serial_test]
fn the_wasi_clock_reads_the_installed_source() {
    // The source is process-global; start from a clean state.
    let _ = WasiHost::uninstall();

    // One day in the past, so a real-clock read can never equal it.
    let base = Instant::now()
        .checked_sub(Duration::from_secs(86_400))
        .expect("one day before now");

    WasiHost::new()
        .with_time_source(Arc::new(FixedSource(base)))
        .install();

    assert_eq!(
        wasi_host::now(),
        Ok(base),
        "the module must read the installed time source, not the real clock"
    );

    // The convenience functions arm from the same source: the timer is due
    // 50 ms after the simulated base, not 50 ms after the real now.
    let id = wasi_host::register_schedule(50.0, || {}).expect("installed time source");
    let remaining = wasi_host::time_until_next_event()
        .expect("installed time source")
        .expect("armed timer");
    assert_eq!(
        remaining,
        Duration::from_millis(50),
        "a timer must arm from the installed source's instant"
    );
    wasi_host::unregister_schedule(id);

    let _ = WasiHost::uninstall();
}

#[serial_test]
fn a_missing_source_is_a_typed_error_or_the_documented_default() {
    let _ = WasiHost::uninstall();

    assert_eq!(
        wasi_host::now(),
        Err(WasiHostError::TimeSourceNotInstalled),
        "a host that installs no source gets the typed error"
    );
    assert_eq!(
        wasi_host::tick(),
        Err(WasiHostError::TimeSourceNotInstalled),
        "tick must report the missing source instead of reading the real clock"
    );
    assert_eq!(
        wasi_host::register_schedule(10.0, || {}),
        Err(WasiHostError::TimeSourceNotInstalled),
        "a timer must not register when the host installed no source"
    );

    // An empty host installs no source, so the module stays in the error state.
    WasiHost::new().install();
    assert_eq!(wasi_host::now(), Err(WasiHostError::TimeSourceNotInstalled));

    // The explicit-instant variants need no installed source.
    let base = Instant::now();
    let id = wasi_host::register_schedule_at(10.0, || {}, base);
    assert_eq!(
        wasi_host::time_until_next_event_at(base),
        Some(Duration::from_millis(10))
    );
    wasi_host::unregister_schedule(id);

    let _ = WasiHost::uninstall();
}
