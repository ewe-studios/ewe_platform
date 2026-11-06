//! A stored row's `created_at` and `updated_at` come from the injected clock,
//! never from the real time.

use std::sync::Arc;
use std::time::Duration;

use foundation_compact::time::unix_millis;
use foundation_core::valtron::{valtron_test, Clock, SimulatedClock};
use foundation_db::state::namespaced::NamespacedStore;
use foundation_db::state::{FileStateStore, StateStore};

/// 2026-01-01T00:00:00Z in unix seconds.
const SIMULATED_SECS: u64 = 1_767_225_600;

#[valtron_test]
async fn store_typed_uses_the_injected_wall_clock() {
    let dir = tempfile::tempdir().expect("temp dir");
    let store = FileStateStore::with_root(dir.path().to_path_buf());
    store.init().await.expect("init store");
    let ns = NamespacedStore::new(Arc::new(store), "clock-test");

    // A simulated clock parked at a known wall time.
    let clock = SimulatedClock::new(Duration::ZERO);
    clock.advance_wall(Duration::from_secs(SIMULATED_SECS));
    let expected_ms = unix_millis(clock.wall_now());
    assert_eq!(expected_ms, i64::try_from(SIMULATED_SECS * 1_000).unwrap());

    ns.store_typed("resource", &42, &clock)
        .await
        .expect("store a typed value");
    let state = ns
        .get("resource")
        .await
        .expect("read the row")
        .expect("the row exists");
    assert_eq!(state.created_at.timestamp_millis(), expected_ms);
    assert_eq!(state.updated_at.timestamp_millis(), expected_ms);

    // The simulated time is January 2026. A real-clock value is far from it.
    let real_ms = unix_millis(std::time::SystemTime::now());
    assert!(
        (state.created_at.timestamp_millis() - real_ms).abs() > 1_000,
        "stored timestamp {} is within one second of real time {real_ms}",
        state.created_at.timestamp_millis()
    );
}
