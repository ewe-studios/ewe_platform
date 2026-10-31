//! `TimerReadiness` fallback tests (moved from `src/websocket/native/readiness.rs`).

#![cfg(all(feature = "multi", not(target_family = "wasm")))]

use std::sync::Arc;
use std::thread;
use std::time::Duration;

use foundation_core::valtron::EventReadiness;
use foundation_netio::websocket::TimerReadiness;

#[test]
fn timer_readiness_fires_after_interval() {
    let timer = TimerReadiness::new(Duration::from_millis(50));
    // First call: last_ready is now(), elapsed ≈ 0. Not ready yet.
    assert!(!timer.is_ready(None));

    // After the interval: should fire.
    thread::sleep(Duration::from_millis(60));
    assert!(timer.is_ready(None));

    // Resets on fire — next immediate call should be false.
    assert!(!timer.is_ready(None));
}

#[test]
fn timer_readiness_implements_event_readiness() {
    let timer = Arc::new(TimerReadiness::new(Duration::from_millis(0)));
    // 0ms interval — always fires immediately.
    let ready: Arc<dyn EventReadiness + Send + Sync> = timer;
    assert!(ready.is_ready(None));
    // Also fires on second call — 0ms interval is always ready.
    assert!(ready.is_ready(None));
}
