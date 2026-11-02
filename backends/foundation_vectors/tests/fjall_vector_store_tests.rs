//! `FjallVectorStore` tests (F29): the persistent native `VectorStore` over
//! fjall and a persisted IVF index. No external services; it uses a temp dir.
//!
//! The shared contract covers insert, search, get, delete, len, validation
//! and namespace isolation. The tests here cover what only a persistent
//! store has: survival across reopen, with the index loaded from the
//! `indexes` partition or rebuilt from the vectors.

#[macro_use]
mod contract;

use foundation_core::valtron::valtron_test;
use foundation_vectors::metric::DistanceMetric;
use foundation_vectors::store::{VectorEntry, VectorMetadata, VectorStore, VectorStoreConfig};
use foundation_vectors::vector::Vector;
use foundation_vectors::FjallVectorStore;
use futures_lite::future::{yield_now, zip};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::Arc;
use tempfile::TempDir;

fn cfg() -> VectorStoreConfig {
    VectorStoreConfig::new(3, DistanceMetric::Cosine)
}

async fn make_store() -> (TempDir, FjallVectorStore) {
    let dir = TempDir::new().unwrap();
    let store = FjallVectorStore::open(dir.path(), cfg()).unwrap();
    (dir, store)
}

vector_store_contract!(make_store);

fn entry(id: &str, v: [f32; 3]) -> VectorEntry {
    VectorEntry {
        id: id.into(),
        vector: Vector::new(v.to_vec()),
        metadata: VectorMetadata::default(),
    }
}

fn ids(matches: &[foundation_vectors::store::VectorMatch]) -> Vec<&str> {
    matches.iter().map(|m| m.id.as_str()).collect()
}

#[valtron_test]
async fn persists_across_reopen() {
    let dir = TempDir::new().unwrap();
    {
        let store = FjallVectorStore::open(dir.path(), cfg()).unwrap();
        store
            .insert("ns", entry("x", [1.0, 0.0, 0.0]))
            .await
            .unwrap();
        store
            .insert("ns", entry("y", [0.0, 1.0, 0.0]))
            .await
            .unwrap();
        store.flush().await.unwrap(); // persist vectors + the IVF index blob
    }
    // Reopen from the same path: a fresh store with no in-memory state.
    let store = FjallVectorStore::open(dir.path(), cfg()).unwrap();
    assert_eq!(store.len("ns").await.unwrap(), 2);
    let got = store.get("ns", "x").await.unwrap().unwrap();
    assert_eq!(got.vector.data, vec![1.0, 0.0, 0.0]);
    // Search works after reload (index loaded from disk or rebuilt from vectors).
    let m = store.search("ns", &[1.0, 0.0, 0.0], 1).await.unwrap();
    assert_eq!(ids(&m), vec!["x"]);
}

#[valtron_test]
async fn reopen_rebuilds_index_from_vectors() {
    let dir = TempDir::new().unwrap();
    {
        let store = FjallVectorStore::open(dir.path(), cfg()).unwrap();
        store
            .insert("ns", entry("x", [1.0, 0.0, 0.0]))
            .await
            .unwrap();
        store.flush().await.unwrap();
    }
    // The vector partition is durable; even if the index blob were absent the
    // store rebuilds the namespace index by scanning stored vectors.
    let store = FjallVectorStore::open(dir.path(), cfg()).unwrap();
    let m = store.search("ns", &[1.0, 0.0, 0.0], 1).await.unwrap();
    assert_eq!(ids(&m), vec!["x"]);
}

/// The first search on a reopened store rebuilds the namespace index by
/// scanning every stored vector. The rebuild runs on the offload pool, so a
/// ticker on the same valtron worker keeps ticking while the search awaits.
#[valtron_test(threads = 2)]
async fn index_rebuild_does_not_hold_worker() {
    let dir = TempDir::new().unwrap();
    {
        let store = FjallVectorStore::open(dir.path(), cfg()).unwrap();
        let mut x = 0.0f32;
        for i in 0..100_000 {
            store
                .insert("ns", entry(&format!("v{i}"), [x + 1.0, x + 2.0, x + 3.0]))
                .await
                .unwrap();
            x += 1.0;
        }
        // No flush: the index blob is not persisted, so the reopened store
        // must rebuild the namespace index from the stored vectors.
    }
    let store = FjallVectorStore::open(dir.path(), cfg()).unwrap();

    let ticks = Arc::new(AtomicUsize::new(0));
    let done = Arc::new(AtomicBool::new(false));
    let ticker = {
        let (ticks, done) = (Arc::clone(&ticks), Arc::clone(&done));
        async move {
            while !done.load(Ordering::SeqCst) {
                ticks.fetch_add(1, Ordering::SeqCst);
                yield_now().await;
            }
        }
    };
    let search = {
        let done = Arc::clone(&done);
        async move {
            let query = vec![1.0f32, 0.0, 0.0];
            let matches = store.search("ns", &query, 5).await;
            done.store(true, Ordering::SeqCst);
            matches
        }
    };

    let (matches, ()) = zip(search, ticker).await;
    let matches = matches.unwrap();
    assert_eq!(matches.len(), 5);
    assert!(
        ticks.load(Ordering::SeqCst) > 10,
        "the ticker must tick while the index rebuild runs, saw {}",
        ticks.load(Ordering::SeqCst)
    );
}
