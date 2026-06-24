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
//! NOTE: The wasi_host module uses process-global statics (`TIMERS`,
//! `SCHEDULED_CALLBACKS`, etc.). `#[serial_test]` serialises these tests
//! via a FairGate so each one sees clean global state.

#![cfg(feature = "wasi")]

use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

use foundation_macros::serial_test;
use foundation_wasm::{wasi_host, TickState};

#[serial_test]
fn schedule_fires_after_deadline() {
    let fired = Arc::new(Mutex::new(false));
    let fired_clone = fired.clone();

    wasi_host::register_schedule(50.0, move || {
        *fired_clone.lock().unwrap() = true;
    });

    assert!(!*fired.lock().unwrap());

    // tick immediately — too early, should not fire
    wasi_host::tick();
    assert!(!*fired.lock().unwrap());

    // wait past the deadline, then tick
    thread::sleep(Duration::from_millis(60));
    wasi_host::tick();

    assert!(*fired.lock().unwrap());
}

#[serial_test]
fn schedule_does_not_fire_before_deadline() {
    let fired = Arc::new(Mutex::new(false));
    let fired_clone = fired.clone();

    wasi_host::register_schedule(200.0, move || {
        *fired_clone.lock().unwrap() = true;
    });

    // tick immediately — well before the 200ms deadline
    wasi_host::tick();
    assert!(!*fired.lock().unwrap());

    // 50ms later — still before deadline
    thread::sleep(Duration::from_millis(50));
    wasi_host::tick();
    assert!(!*fired.lock().unwrap());

    // clean up so it doesn't leak into other tests via poll_blocking
    thread::sleep(Duration::from_millis(200));
    wasi_host::tick();
}

#[serial_test]
fn schedule_unregister_prevents_firing() {
    let fired = Arc::new(Mutex::new(false));
    let fired_clone = fired.clone();

    let id = wasi_host::register_schedule(50.0, move || {
        *fired_clone.lock().unwrap() = true;
    });

    wasi_host::unregister_schedule(id);

    thread::sleep(Duration::from_millis(60));
    wasi_host::tick();

    assert!(!*fired.lock().unwrap());
}

#[serial_test]
fn interval_fires_repeatedly() {
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
    });

    for _ in 0..3 {
        thread::sleep(Duration::from_millis(40));
        wasi_host::tick();
    }

    assert_eq!(*count.lock().unwrap(), 3);
}

#[serial_test]
fn interval_unregister_stops_firing() {
    let count = Arc::new(Mutex::new(0u32));
    let count_clone = count.clone();

    let id = wasi_host::register_interval(30.0, move || {
        *count_clone.lock().unwrap() += 1;
        TickState::REQUEUE
    });

    thread::sleep(Duration::from_millis(40));
    wasi_host::tick();
    assert_eq!(*count.lock().unwrap(), 1);

    wasi_host::unregister_interval(id);

    thread::sleep(Duration::from_millis(40));
    wasi_host::tick();
    assert_eq!(*count.lock().unwrap(), 1);
}

#[serial_test]
fn time_until_next_event_returns_some_when_timers_exist() {
    let id = wasi_host::register_schedule(500.0, || {});

    let remaining = wasi_host::time_until_next_event();
    assert!(remaining.is_some());
    let dur = remaining.unwrap();
    assert!(dur.as_millis() <= 500);
    assert!(dur.as_millis() > 0);

    wasi_host::unregister_schedule(id);
    assert!(wasi_host::time_until_next_event().is_none());
}

#[serial_test]
fn poll_blocking_sleeps_and_fires() {
    let fired = Arc::new(Mutex::new(false));
    let fired_clone = fired.clone();

    wasi_host::register_schedule(30.0, move || {
        *fired_clone.lock().unwrap() = true;
    });

    let start = std::time::Instant::now();
    wasi_host::poll_blocking();
    let elapsed = start.elapsed();

    assert!(*fired.lock().unwrap());
    assert!(elapsed.as_millis() >= 20);
}

#[serial_test]
fn tick_does_not_panic_when_idle() {
    wasi_host::tick();
}
