//! `StateStore::set_if_version`: write only if the stored state is still the
//! one the caller read (compare-and-set on `updated_at`).
//!
//! Two callers that read the same state and write at the same time must not
//! both win. The file store and the local libsql store guarantee this; the
//! tests run the same cases on both, and on a `NamespacedStore` over each.

#![recursion_limit = "256"]

use std::sync::{Arc, Barrier};
use std::thread;

use chrono::{Duration, Utc};
use foundation_core::valtron::{valtron_test, BlockingFutureExt};
use foundation_db::core::state::namespaced::NamespacedStore;
use foundation_db::state::{FileStateStore, ResourceState, StateStatus, StateStore};
use serde_json::json;

const THREADS: usize = 20;

fn state(id: &str, hash: &str) -> ResourceState {
    let now = Utc::now();
    ResourceState {
        id: id.to_string(),
        kind: "test::resource".to_string(),
        provider: "test".to_string(),
        status: StateStatus::Created,
        environment: None,
        config_hash: hash.to_string(),
        output: json!({}),
        config_snapshot: json!(null),
        created_at: now,
        updated_at: now,
    }
}

/// The contract, one thread.
async fn check_contract<S: StateStore>(store: &S) {
    let first = state("r1", "v1");

    // Absent: only "must be absent" (None) writes.
    assert!(!store
        .set_if_version("r1", &first, Some(first.updated_at))
        .await
        .unwrap());
    assert!(store.set_if_version("r1", &first, None).await.unwrap());
    // Now present: "must be absent" fails.
    assert!(!store.set_if_version("r1", &first, None).await.unwrap());

    // The version is the stored `updated_at`.
    let stored = store.get("r1").await.unwrap().unwrap();
    let mut second = state("r1", "v2");
    second.updated_at = stored.updated_at + Duration::seconds(1);
    assert!(!store
        .set_if_version(
            "r1",
            &second,
            Some(stored.updated_at - Duration::seconds(5))
        )
        .await
        .unwrap());
    assert_eq!(store.get("r1").await.unwrap().unwrap().config_hash, "v1");
    assert!(store
        .set_if_version("r1", &second, Some(stored.updated_at))
        .await
        .unwrap());
    assert_eq!(store.get("r1").await.unwrap().unwrap().config_hash, "v2");

    // A stale writer (it still holds the first version) loses.
    let mut stale = state("r1", "stale");
    stale.updated_at = stored.updated_at + Duration::seconds(2);
    assert!(!store
        .set_if_version("r1", &stale, Some(stored.updated_at))
        .await
        .unwrap());
    assert_eq!(store.get("r1").await.unwrap().unwrap().config_hash, "v2");
}

/// 20 writers read the same version and all try to write it: one wins.
fn check_race<S: StateStore + 'static>(store: S) {
    let store = Arc::new(store);
    let seed = {
        let store = Arc::clone(&store);
        async move {
            let seed = state("race", "seed");
            store.set("race", &seed).await.unwrap();
            store.get("race").await.unwrap().unwrap()
        }
        .block()
    };
    let barrier = Arc::new(Barrier::new(THREADS));
    let handles: Vec<_> = (0..THREADS)
        .map(|n| {
            let (store, barrier, seed) = (Arc::clone(&store), Arc::clone(&barrier), seed.clone());
            thread::spawn(move || {
                barrier.wait();
                async move {
                    let mut mine = state("race", &format!("writer-{n}"));
                    mine.updated_at = seed.updated_at + Duration::seconds(1);
                    store
                        .set_if_version("race", &mine, Some(seed.updated_at))
                        .await
                        .unwrap()
                }
                .block()
            })
        })
        .collect();
    let winners = handles
        .into_iter()
        .map(|h| h.join().unwrap())
        .filter(|won| *won)
        .count();
    assert_eq!(winners, 1, "exactly one writer wins, not {winners}");
}

/// Run the one-thread contract on `store`, then the race on `race_store`.
fn run<S: StateStore + 'static>(store: S, race_store: S) {
    let race_store = async move {
        store.init().await.unwrap();
        check_contract(&store).await;
        race_store.init().await.unwrap();
        race_store
    }
    .block();
    check_race(race_store);
}

#[valtron_test(threads = 8, timeout = 60000)]
fn file_store_set_if_version() {
    let dir = tempfile::tempdir().unwrap();
    run(
        FileStateStore::with_root(dir.path().join("state")),
        FileStateStore::with_root(dir.path().join("race")),
    );
}

#[valtron_test(threads = 8, timeout = 60000)]
fn namespaced_store_set_if_version() {
    let dir = tempfile::tempdir().unwrap();
    let inner = Arc::new(FileStateStore::with_root(dir.path().join("state")));
    let store = NamespacedStore::new(inner, "ns");
    async move {
        store.init().await.unwrap();
        check_contract(&store).await;
    }
    .block();
}

#[cfg(feature = "libsql")]
#[valtron_test(threads = 8, timeout = 60000)]
fn libsql_store_set_if_version() {
    use foundation_db::LibsqlStore;
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("state.db").to_string_lossy().into_owned();
    let make = |name: &'static str| {
        let path = path.clone();
        async move {
            LibsqlStore::new_state(&path, "proj", name, foundation_context::Ctx::host())
                .await
                .unwrap()
        }
        .block()
    };
    run(make("contract"), make("race"));
}
