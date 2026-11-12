//! `KeyValueStore::take`: read and remove a key in one step.
//!
//! Two callers that take the same key at the same time must not both get the
//! value. Single-use codes (`ToS` codes, MFA challenges) rely on this. Every
//! backend that can guarantee it is tested here with 20 threads on one key.

#![recursion_limit = "256"]

use std::sync::{Arc, Barrier};
use std::thread;

use foundation_core::valtron::{valtron_test, BlockingFutureExt};
use foundation_db::{JsonFileStorage, KeyValueStore, MemoryJsonStore, MemoryStorage, TursoStorage};
use tempfile::TempDir;

const THREADS: usize = 20;

/// Take `key` from `THREADS` threads at once; return how many got the value.
fn concurrent_takes<S: KeyValueStore + 'static>(store: &Arc<S>, key: &str) -> usize {
    let barrier = Arc::new(Barrier::new(THREADS));
    let handles: Vec<_> = (0..THREADS)
        .map(|_| {
            let (store, barrier, key) = (Arc::clone(store), Arc::clone(&barrier), key.to_string());
            thread::spawn(move || {
                barrier.wait();
                async move { store.take::<String>(&key).await }
                    .block()
                    .expect("take")
                    .is_some()
            })
        })
        .collect();
    handles
        .into_iter()
        .map(|h| h.join().expect("thread"))
        .filter(|got| *got)
        .count()
}

/// The contract on any backend: the value comes back once, then it is gone.
fn check_take<S: KeyValueStore + 'static>(store: S) {
    let store = Arc::new(store);
    {
        let store = Arc::clone(&store);
        async move {
            store.set("one", "v".to_string()).await.unwrap();
            assert_eq!(
                store.take::<String>("one").await.unwrap().as_deref(),
                Some("v")
            );
            assert_eq!(store.take::<String>("one").await.unwrap(), None);
            assert!(!store.exists("one").await.unwrap());
            assert_eq!(store.take::<String>("never-set").await.unwrap(), None);
            store.set("many", "v".to_string()).await.unwrap();
        }
        .block();
    }
    assert_eq!(
        concurrent_takes(&store, "many"),
        1,
        "exactly one taker gets the value"
    );
}

#[valtron_test(threads = 8, timeout = 60000)]
fn memory_take_is_atomic() {
    check_take(MemoryStorage::new());
}

#[valtron_test(threads = 8, timeout = 60000)]
fn memory_json_take_is_atomic() {
    check_take(MemoryJsonStore::new());
}

#[valtron_test(threads = 8, timeout = 60000)]
fn json_file_take_is_atomic() {
    let dir = TempDir::new().unwrap();
    check_take(JsonFileStorage::new(dir.path().join("kv.json")).unwrap());
}

#[valtron_test(threads = 8, timeout = 60000)]
fn turso_take_is_atomic() {
    let dir = TempDir::new().unwrap();
    let url = dir.path().join("take.db").to_str().unwrap().to_owned();
    let store = async move {
        let s = TursoStorage::new(&url, foundation_context::Ctx::host())
            .await
            .unwrap();
        s.init_schema().await.unwrap();
        s
    }
    .block();
    check_take(store);
}

#[cfg(feature = "libsql")]
#[valtron_test(threads = 8, timeout = 60000)]
fn libsql_take_is_atomic() {
    let dir = TempDir::new().unwrap();
    let url = dir.path().join("take.db").to_str().unwrap().to_owned();
    let store = async move {
        let s = foundation_db::LibsqlStore::new_kv(&url, None, foundation_context::Ctx::host())
            .await
            .unwrap();
        s.init_kv().await.unwrap();
        s
    }
    .block();
    check_take(store);
}
