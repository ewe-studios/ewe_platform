//! `NoThreadController::yield_for` really waits (native, single-threaded
//! executor).
//!
//! WHY a separate test binary: since the idle-wake change, a wake anywhere in
//! the process interrupts a sleeping controller early (by design). In the big
//! `mod` binary, other tests' wakes would cut these waits short; in its own
//! process nothing else runs, so the waits are deterministic.

#![cfg(all(not(feature = "multi"), not(target_family = "wasm")))]

use std::time::{Duration, Instant};

use foundation_core::valtron::{single::NoThreadController, ProcessController};

#[test]
fn nothreadcontroller_waits_for_the_requested_time() {
    let controller = NoThreadController::new();
    let start = Instant::now();
    controller.yield_for(Duration::from_millis(10));
    assert!(start.elapsed() >= Duration::from_millis(5), "returned after {:?}", start.elapsed());
}

#[test]
fn nothreadcontroller_has_default_and_waits() {
    let controller = NoThreadController::default();
    let start = Instant::now();
    controller.yield_for(Duration::from_millis(5));
    assert!(start.elapsed() >= Duration::from_millis(2), "returned after {:?}", start.elapsed());
}
