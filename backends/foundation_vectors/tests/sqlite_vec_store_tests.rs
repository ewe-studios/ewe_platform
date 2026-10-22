//! `SqliteVecVectorStore` tests (F29): native sqlite-vec k-nearest neighbour
//! (KNN) search with the bundled `.so`.
//!
//! The shared contract covers the `VectorStore` behaviour. The tests here
//! cover the sqlite-vec limits: no dot product, safe namespace names, and the
//! L2 score.

#[macro_use]
mod contract;

use foundation_core::valtron::valtron_test;
use foundation_vectors::metric::DistanceMetric;
use foundation_vectors::store::{
    VectorEntry, VectorMetadata, VectorStore, VectorStoreConfig, VectorStoreError,
};
use foundation_vectors::vector::Vector;
use foundation_vectors::SqliteVecVectorStore;
use tempfile::TempDir;

async fn make_store() -> (TempDir, SqliteVecVectorStore) {
    let dir = TempDir::new().unwrap();
    let cfg = VectorStoreConfig::new(3, DistanceMetric::Cosine);
    let store = SqliteVecVectorStore::open(cfg, dir.path().join("v.db"))
        .await
        .unwrap();
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

#[valtron_test]
async fn dot_metric_is_unsupported() {
    let dir = TempDir::new().unwrap();
    let cfg = VectorStoreConfig::new(3, DistanceMetric::Dot);
    let err = SqliteVecVectorStore::open(cfg, dir.path().join("v.db"))
        .await
        .err()
        .expect("dot is refused");
    assert!(matches!(err, VectorStoreError::Unsupported(_)));
}

#[valtron_test]
async fn unsafe_namespace_is_refused() {
    let (_dir, store) = make_store().await;
    let err = store
        .insert_async("x; DROP TABLE t", entry("a", [1.0, 0.0, 0.0]))
        .await
        .unwrap_err();
    assert!(matches!(err, VectorStoreError::Unsupported(_)));
}

#[valtron_test]
async fn l2_scores_are_negative_squared_distance() {
    let dir = TempDir::new().unwrap();
    let cfg = VectorStoreConfig::new(3, DistanceMetric::L2);
    let store = SqliteVecVectorStore::open(cfg, dir.path().join("v.db"))
        .await
        .unwrap();
    store.insert_async("ns", entry("near", [1.0, 0.0, 0.0])).await.unwrap();
    store.insert_async("ns", entry("far", [3.0, 0.0, 0.0])).await.unwrap();
    let m = store.search_async("ns", &[0.0, 0.0, 0.0], 2).await.unwrap();
    assert_eq!(m[0].id, "near");
    assert!((m[0].score - -1.0).abs() < 1e-4, "score {}", m[0].score);
    assert!((m[1].score - -9.0).abs() < 1e-4, "score {}", m[1].score);
}
