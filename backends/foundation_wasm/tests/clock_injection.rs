//! WASI timers fire on the instant their caller supplies (spec 70 feature 134).
//!
//! WHY: `wasi_host` read the real monotonic clock inside its timer functions,
//! so a deterministic test could not control when a timer fired.
//!
//! WHAT: `register_schedule_at` and `tick_at` take the instant explicitly. A
//! timer armed at `base` does not fire at `base + 10ms`, and does fire at
//! `base + 60ms`, with no real wait.
//!
//! HOW: before this feature every timer function read the real clock and had no
//! injection point. The host-boundary read that feature 134 left is gone:
//! issue #219 makes `WasiHost` install a `TimeSource`, and the module reads
//! only that source (see `tests/wasi_host_time_source.rs`).

#![cfg(feature = "wasi")]

use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use foundation_macros::serial_test;
use foundation_wasm::wasi_host;

#[serial_test]
fn tick_at_fires_the_timer_registered_at_the_injected_instant() {
    let fired = Arc::new(Mutex::new(false));
    let signal = Arc::clone(&fired);
    let base = Instant::now();

    wasi_host::register_schedule_at(
        50.0,
        move || {
            *signal.lock().expect("timer flag") = true;
        },
        base,
    );

    let _ = wasi_host::tick_at(base + Duration::from_millis(10));
    assert!(
        !*fired.lock().expect("timer flag"),
        "a timer must not fire before its deadline on the injected instant"
    );

    let _ = wasi_host::tick_at(base + Duration::from_millis(60));
    assert!(
        *fired.lock().expect("timer flag"),
        "a timer must fire once the injected instant passes its deadline"
    );
}
