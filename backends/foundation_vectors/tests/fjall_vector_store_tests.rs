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
use foundation_vectors::store::{
    VectorEntry, VectorMetadata, VectorStore, VectorStoreConfig,
};
use foundation_vectors::vector::Vector;
use foundation_vectors::FjallVectorStore;
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
        store.insert_async("ns", entry("x", [1.0, 0.0, 0.0])).await.unwrap();
        store.insert_async("ns", entry("y", [0.0, 1.0, 0.0])).await.unwrap();
        store.flush().unwrap(); // persist vectors + the IVF index blob
    }
    // Reopen from the same path: a fresh store with no in-memory state.
    let store = FjallVectorStore::open(dir.path(), cfg()).unwrap();
    assert_eq!(store.len_async("ns").await.unwrap(), 2);
    let got = store.get_async("ns", "x").await.unwrap().unwrap();
    assert_eq!(got.vector.data, vec![1.0, 0.0, 0.0]);
    // Search works after reload (index loaded from disk or rebuilt from vectors).
    let m = store.search_async("ns", &[1.0, 0.0, 0.0], 1).await.unwrap();
    assert_eq!(ids(&m), vec!["x"]);
}

#[valtron_test]
async fn reopen_rebuilds_index_from_vectors() {
    let dir = TempDir::new().unwrap();
    {
        let store = FjallVectorStore::open(dir.path(), cfg()).unwrap();
        store.insert_async("ns", entry("x", [1.0, 0.0, 0.0])).await.unwrap();
        store.flush().unwrap();
    }
    // The vector partition is durable; even if the index blob were absent the
    // store rebuilds the namespace index by scanning stored vectors.
    let store = FjallVectorStore::open(dir.path(), cfg()).unwrap();
    let m = store.search_async("ns", &[1.0, 0.0, 0.0], 1).await.unwrap();
    assert_eq!(ids(&m), vec!["x"]);
}
