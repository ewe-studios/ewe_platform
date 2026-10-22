//! `LibsqlVectorStore` tests (F29) — native `DiskANN` (`vector_top_k`) over a
//! real local libSQL database, with namespace over-fetch + exact re-rank.

use std::sync::Arc;

use foundation_core::valtron::valtron_test;
use foundation_db::LibsqlStore;
use foundation_vectors::metric::DistanceMetric;
use foundation_vectors::store::{
    AsyncVectorStore, VectorEntry, VectorMatch, VectorMetadata, VectorStoreConfig,
    VectorStoreError,
};
use foundation_vectors::vector::Vector;
use foundation_vectors::LibsqlVectorStore;
use tempfile::TempDir;

async fn make_store() -> (TempDir, LibsqlVectorStore<LibsqlStore>) {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("vec.db").to_string_lossy().into_owned();
    let backend = Arc::new(LibsqlStore::new_kv(&path, None).await.unwrap());
    let store = LibsqlVectorStore::new(backend, VectorStoreConfig::new(3, DistanceMetric::Cosine))
        .await
        .unwrap();
    (dir, store)
}

fn entry(id: &str, v: [f32; 3]) -> VectorEntry {
    VectorEntry {
        id: id.into(),
        vector: Vector::new(v.to_vec()),
        metadata: VectorMetadata::default(),
    }
}

fn ids(m: &[VectorMatch]) -> Vec<&str> {
    m.iter().map(|x| x.id.as_str()).collect()
}

#[valtron_test]
async fn diskann_search_returns_nearest_first() {
    let (_d, store) = make_store().await;
    store.insert_async("ns", entry("x", [1.0, 0.0, 0.0])).await.unwrap();
    store.insert_async("ns", entry("y", [0.0, 1.0, 0.0])).await.unwrap();
    store.insert_async("ns", entry("z", [0.9, 0.1, 0.0])).await.unwrap();

    let m = store.search_async("ns", &[1.0, 0.0, 0.0], 2).await.unwrap();
    assert_eq!(m.len(), 2);
    assert_eq!(ids(&m), vec!["x", "z"]);
    assert!(m[0].score >= m[1].score);
}

#[valtron_test]
async fn namespaces_isolated_via_overfetch_filter() {
    let (_d, store) = make_store().await;
    store.insert_async("a", entry("a1", [1.0, 0.0, 0.0])).await.unwrap();
    store.insert_async("a", entry("a2", [0.8, 0.2, 0.0])).await.unwrap();
    store.insert_async("b", entry("b1", [1.0, 0.0, 0.0])).await.unwrap();

    let ma = store.search_async("a", &[1.0, 0.0, 0.0], 5).await.unwrap();
    let mut got = ids(&ma);
    got.sort_unstable();
    assert_eq!(got, vec!["a1", "a2"], "namespace a only");
    assert!(!ids(&ma).contains(&"b1"));

    let mb = store.search_async("b", &[1.0, 0.0, 0.0], 5).await.unwrap();
    assert_eq!(ids(&mb), vec!["b1"]);
}

#[valtron_test]
async fn dimension_and_zero_validation() {
    let (_d, store) = make_store().await;
    let bad = VectorEntry {
        id: "d".into(),
        vector: Vector::new(vec![1.0, 2.0]),
        metadata: VectorMetadata::default(),
    };
    assert!(matches!(
        store.insert_async("ns", bad).await.unwrap_err(),
        VectorStoreError::DimensionMismatch { expected: 3, got: 2 }
    ));
    assert!(matches!(
        store.insert_async("ns", entry("z", [0.0, 0.0, 0.0])).await.unwrap_err(),
        VectorStoreError::ZeroVector
    ));
    assert!(matches!(
        store.search_async("ns", &[1.0], 1).await.unwrap_err(),
        VectorStoreError::DimensionMismatch { expected: 3, got: 1 }
    ));
}

#[valtron_test]
async fn get_update_and_delete() {
    let (_d, store) = make_store().await;
    store.insert_async("ns", entry("k", [0.2, 0.4, 0.6])).await.unwrap();
    let got = store.get_async("ns", "k").await.unwrap().expect("present");
    assert_eq!(got.id, "k");
    assert_eq!(got.vector.data.len(), 3);
    assert!((got.vector.data[0] - 0.2).abs() < 1e-4);

    // Upsert replaces the vector for the same (namespace, id).
    store.insert_async("ns", entry("k", [0.0, 0.0, 1.0])).await.unwrap();
    assert_eq!(store.len_async("ns").await, 1);
    let updated = store.get_async("ns", "k").await.unwrap().unwrap();
    assert!((updated.vector.data[2] - 1.0).abs() < 1e-4);

    assert!(store.get_async("ns", "missing").await.unwrap().is_none());
    store.delete_async("ns", "k").await.unwrap();
    assert!(store.get_async("ns", "k").await.unwrap().is_none());
    assert_eq!(store.len_async("ns").await, 0);
    assert!(matches!(
        store.delete_async("ns", "k").await.unwrap_err(),
        VectorStoreError::NotFound { .. }
    ));
}

#[valtron_test]
async fn metadata_round_trips() {
    let (_d, store) = make_store().await;
    let mut e = entry("m", [1.0, 0.0, 0.0]);
    e.metadata.tags.insert("k".into(), "v".into());
    store.insert_async("ns", e).await.unwrap();
    let got = store.get_async("ns", "m").await.unwrap().unwrap();
    assert_eq!(got.metadata.tags.get("k").map(String::as_str), Some("v"));
}
