//! `KeyValueStore::insert_if_absent_async`: first writer wins.
//!
//! The key store of `foundation_auth` uses this method to make one certificate
//! authority when two processes start at the same time. Every backend that can
//! do it atomically is tested here with 16 threads on one key.

#![recursion_limit = "256"]

use std::sync::{Arc, Barrier};
use std::thread;

use foundation_core::valtron::{valtron_test, BlockingFutureExt};
use foundation_db::{JsonFileStorage, KeyValueStore, MemoryJsonStore, MemoryStorage, TursoStorage};
use tempfile::TempDir;

const THREADS: usize = 16;

/// Insert `key` from `THREADS` threads at once; return how many inserted it.
fn concurrent_inserts<S: KeyValueStore + 'static>(store: &Arc<S>, key: &str) -> usize {
    let barrier = Arc::new(Barrier::new(THREADS));
    let handles: Vec<_> = (0..THREADS)
        .map(|_| {
            let (store, barrier, key) = (Arc::clone(store), Arc::clone(&barrier), key.to_string());
            thread::spawn(move || {
                barrier.wait();
                async move { store.insert_if_absent_async(&key, "value").await }
                    .block()
                    .expect("insert")
            })
        })
        .collect();
    handles
        .into_iter()
        .map(|handle| handle.join().expect("thread"))
        .filter(|inserted| *inserted)
        .count()
}

/// The contract on any backend: the first insert wins, and the value stays.
fn check_insert_if_absent<S: KeyValueStore + 'static>(store: S) {
    let store = Arc::new(store);
    {
        let store = Arc::clone(&store);
        async move {
            assert!(
                store.insert_if_absent_async("first", "one").await.unwrap(),
                "the first insert stores the value"
            );
            assert!(
                !store.insert_if_absent_async("first", "two").await.unwrap(),
                "the second insert leaves the value alone"
            );
            assert_eq!(
                store.get::<String>("first").await.unwrap().as_deref(),
                Some("one")
            );
            assert!(!store.exists("absent").await.unwrap());
        }
        .block();
    }
    assert_eq!(
        concurrent_inserts(&store, "race"),
        1,
        "exactly one writer inserts the key"
    );
}

#[valtron_test(threads = 8, timeout = 60000)]
fn memory_insert_if_absent_is_atomic() {
    check_insert_if_absent(MemoryStorage::new());
}

#[valtron_test(threads = 8, timeout = 60000)]
fn memory_json_insert_if_absent_is_atomic() {
    check_insert_if_absent(MemoryJsonStore::new());
}

#[valtron_test(threads = 8, timeout = 60000)]
fn json_file_insert_if_absent_is_atomic() {
    let dir = TempDir::new().unwrap();
    check_insert_if_absent(JsonFileStorage::new(dir.path().join("kv.json")).unwrap());
}

#[valtron_test(threads = 8, timeout = 60000)]
fn turso_insert_if_absent_is_atomic() {
    let store = TursoStorage::in_memory(foundation_context::Ctx::host())
        .block()
        .expect("in-memory Turso store");
    check_insert_if_absent(store);
}

/// The libsql backend is an optional feature; the test runs when it is on.
#[cfg(feature = "libsql")]
#[valtron_test(threads = 8, timeout = 60000)]
fn libsql_insert_if_absent_is_atomic() {
    let store = foundation_db::LibsqlStore::in_memory(foundation_context::Ctx::host())
        .block()
        .expect("in-memory libsql store");
    check_insert_if_absent(store);
}
