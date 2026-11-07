//! WHY: Verify the `wasi_host` module's timer-based scheduling (timeouts,
//! intervals, tick loop) against real elapsed time on native.
//!
//! WHAT: `wasi_host::register_schedule`, `register_interval`,
//! `unregister_schedule`, `unregister_interval`, `tick`, `poll_blocking`,
//! `time_until_next_event`, `has_pending_work`.
//!
//! HOW: Register callbacks with short timing, advance via `tick()` or
//! `poll_blocking()`, and assert side-effects fire at the right moments.
//!
//! NOTE: The `wasi_host` module uses process-global statics (`TIMERS`, the
//! installed `TimeSource`, etc.). `#[serial_test]` serialises these tests
//! via a `FairGate` so each one sees clean global state. Each test installs a
//! real-clock `TimeSource`, because the module has no default source.

#![cfg(feature = "wasi")]

use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use foundation_macros::serial_test;
use foundation_wasm::wasi_host::{self, TimeSource, WasiHost};
use foundation_wasm::TickState;

/// The real monotonic clock, supplied by the test host.
struct RealTimeSource;

impl TimeSource for RealTimeSource {
    fn now(&self) -> Instant {
        Instant::now()
    }
}

/// Install a real-clock source for the module. Each test starts with it.
fn install_real_time_source() {
    WasiHost::new()
        .with_time_source(Arc::new(RealTimeSource))
        .install();
}

#[serial_test]
fn schedule_fires_after_deadline() {
    install_real_time_source();
    let fired = Arc::new(Mutex::new(false));
    let fired_clone = fired.clone();

    wasi_host::register_schedule(50.0, move || {
        *fired_clone.lock().unwrap() = true;
    })
    .expect("time source installed");

    assert!(!*fired.lock().unwrap());

    // tick immediately — too early, should not fire
    wasi_host::tick().expect("time source installed");
    assert!(!*fired.lock().unwrap());

    // wait past the deadline, then tick
    thread::sleep(Duration::from_millis(60));
    wasi_host::tick().expect("time source installed");

    assert!(*fired.lock().unwrap());
}

#[serial_test]
fn schedule_does_not_fire_before_deadline() {
    install_real_time_source();
    let fired = Arc::new(Mutex::new(false));
    let fired_clone = fired.clone();

    wasi_host::register_schedule(200.0, move || {
        *fired_clone.lock().unwrap() = true;
    })
    .expect("time source installed");

    // tick immediately — well before the 200ms deadline
    wasi_host::tick().expect("time source installed");
    assert!(!*fired.lock().unwrap());

    // 50ms later — still before deadline
    thread::sleep(Duration::from_millis(50));
    wasi_host::tick().expect("time source installed");
    assert!(!*fired.lock().unwrap());

    // clean up so it doesn't leak into other tests via poll_blocking
    thread::sleep(Duration::from_millis(200));
    wasi_host::tick().expect("time source installed");
}

#[serial_test]
fn schedule_unregister_prevents_firing() {
    install_real_time_source();
    let fired = Arc::new(Mutex::new(false));
    let fired_clone = fired.clone();

    let id = wasi_host::register_schedule(50.0, move || {
        *fired_clone.lock().unwrap() = true;
    })
    .expect("time source installed");

    wasi_host::unregister_schedule(id);

    thread::sleep(Duration::from_millis(60));
    wasi_host::tick().expect("time source installed");

    assert!(!*fired.lock().unwrap());
}

#[serial_test]
fn interval_fires_repeatedly() {
    install_real_time_source();
    let count = Arc::new(Mutex::new(0u32));
    let count_clone = count.clone();

    wasi_host::register_interval(30.0, move || {
        let mut c = count_clone.lock().unwrap();
        *c += 1;
        if *c >= 3 {
            TickState::STOP
        } else {
            TickState::REQUEUE
        }
    })
    .expect("time source installed");

    for _ in 0..3 {
        thread::sleep(Duration::from_millis(40));
        wasi_host::tick().expect("time source installed");
    }

    assert_eq!(*count.lock().unwrap(), 3);
}

#[serial_test]
fn interval_unregister_stops_firing() {
    install_real_time_source();
    let count = Arc::new(Mutex::new(0u32));
    let count_clone = count.clone();

    let id = wasi_host::register_interval(30.0, move || {
        *count_clone.lock().unwrap() += 1;
        TickState::REQUEUE
    })
    .expect("time source installed");

    thread::sleep(Duration::from_millis(40));
    wasi_host::tick().expect("time source installed");
    assert_eq!(*count.lock().unwrap(), 1);

    wasi_host::unregister_interval(id);

    thread::sleep(Duration::from_millis(40));
    wasi_host::tick().expect("time source installed");
    assert_eq!(*count.lock().unwrap(), 1);
}

#[serial_test]
fn time_until_next_event_returns_some_when_timers_exist() {
    install_real_time_source();
    let id = wasi_host::register_schedule(500.0, || {}).expect("time source installed");

    let remaining = wasi_host::time_until_next_event().expect("time source installed");
    assert!(remaining.is_some());
    let dur = remaining.unwrap();
    assert!(dur.as_millis() <= 500);
    assert!(dur.as_millis() > 0);

    wasi_host::unregister_schedule(id);
    assert!(wasi_host::time_until_next_event()
        .expect("time source installed")
        .is_none());
}

#[serial_test]
fn poll_blocking_sleeps_and_fires() {
    install_real_time_source();
    let fired = Arc::new(Mutex::new(false));
    let fired_clone = fired.clone();

    wasi_host::register_schedule(30.0, move || {
        *fired_clone.lock().unwrap() = true;
    })
    .expect("time source installed");

    let start = std::time::Instant::now();
    wasi_host::poll_blocking().expect("time source installed");
    let elapsed = start.elapsed();

    assert!(*fired.lock().unwrap());
    assert!(elapsed.as_millis() >= 20);
}

#[serial_test]
fn tick_does_not_panic_when_idle() {
    install_real_time_source();
    wasi_host::tick().expect("time source installed");
}
