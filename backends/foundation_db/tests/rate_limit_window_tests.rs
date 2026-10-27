//! Fixed-window rate limits: the same checks over every local
//! `RateLimiterStore` (in-memory, in-memory JSON, Turso, libsql).
//!
//! The regression these pin: a record used to restart the window on every
//! hit, so a key hit at least once per window never reset and its count
//! grew for good.

use std::time::Duration;

use foundation_core::valtron::valtron_test;
use foundation_db::{RateLimiterStore, MemoryJsonStore, MemoryStorage, TursoStorage};

#[cfg(feature = "libsql")]
use foundation_db::LibsqlStore;

/// Counts within one window, the limit, and reset.
async fn counts_and_limits<S: RateLimiterStore>(store: &S) {
    assert!(store.check_rate_limit("k", 2, 60).await.unwrap());
    assert_eq!(store.record_rate_limit("k", 60).await.unwrap(), 1);
    assert_eq!(store.record_rate_limit("k", 60).await.unwrap(), 2);
    assert!(!store.check_rate_limit("k", 2, 60).await.unwrap(), "limit reached");
    assert!(store.check_rate_limit("other", 2, 60).await.unwrap(), "keys are independent");

    store.reset_rate_limit("k").await.unwrap();
    assert!(store.check_rate_limit("k", 2, 60).await.unwrap());
    assert_eq!(store.record_rate_limit("k", 60).await.unwrap(), 1);
}

/// A zero-length window has always elapsed: every record starts a new one.
async fn zero_window_never_accumulates<S: RateLimiterStore>(store: &S) {
    for _ in 0..3 {
        assert_eq!(store.record_rate_limit("z", 0).await.unwrap(), 1);
    }
    assert!(store.check_rate_limit("z", 1, 0).await.unwrap());
}

/// A key hit inside its window keeps the window's start, so it still resets
/// once the window has elapsed. Window: 2s; hits at ~0s, ~0.5s and ~3.1s.
/// (The old code moved the start on every hit and answered 3 here.)
async fn busy_key_resets_after_the_window<S: RateLimiterStore>(store: &S) {
    assert_eq!(store.record_rate_limit("busy", 2).await.unwrap(), 1);
    std::thread::sleep(Duration::from_millis(500));
    assert_eq!(store.record_rate_limit("busy", 2).await.unwrap(), 2);
    assert!(!store.check_rate_limit("busy", 2, 2).await.unwrap());

    std::thread::sleep(Duration::from_millis(2600));
    assert!(store.check_rate_limit("busy", 2, 2).await.unwrap(), "window elapsed");
    assert_eq!(store.record_rate_limit("busy", 2).await.unwrap(), 1, "new window");
}

async fn all<S: RateLimiterStore>(store: &S) {
    counts_and_limits(store).await;
    zero_window_never_accumulates(store).await;
    busy_key_resets_after_the_window(store).await;
}

#[valtron_test]
async fn memory_storage() {
    all(&MemoryStorage::new()).await;
}

#[valtron_test]
async fn memory_json_store() {
    all(&MemoryJsonStore::new()).await;
}

#[valtron_test]
async fn turso_storage() {
    all(&TursoStorage::in_memory().await.unwrap()).await;
}

#[cfg(feature = "libsql")]
#[valtron_test]
async fn libsql_store() {
    all(&LibsqlStore::in_memory().await.unwrap()).await;
}

/// A store without the core migrations creates the same `rate_limits` table
/// itself.
#[valtron_test]
async fn turso_storage_without_migrations() {
    let store = TursoStorage::new(":memory:").await.unwrap();
    counts_and_limits(&store).await;
    zero_window_never_accumulates(&store).await;
}
