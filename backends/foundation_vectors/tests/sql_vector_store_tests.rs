use std::sync::Arc;

use foundation_core::valtron::valtron_test;
use foundation_db::TursoStorage;
use foundation_vectors::metric::DistanceMetric;
use foundation_vectors::store::{
    AsyncVectorStore, VectorEntry, VectorMetadata, VectorStoreConfig, VectorStoreError,
};
use foundation_vectors::vector::Vector;
use foundation_vectors::SqlVectorStore;
use tempfile::TempDir;

async fn make_store() -> (TempDir, SqlVectorStore<TursoStorage>) {
    let dir = TempDir::new().unwrap();
    let url = dir.path().join("vectors.db").to_string_lossy().into_owned();
    let storage = Arc::new(TursoStorage::new(&url).await.unwrap());
    let config = VectorStoreConfig::new(3, DistanceMetric::Cosine);
    let store = SqlVectorStore::new(storage, config).await.unwrap();
    (dir, store)
}

fn entry(id: &str, data: Vec<f32>) -> VectorEntry {
    VectorEntry {
        id: id.to_string(),
        vector: Vector::new(data),
        metadata: VectorMetadata::default(),
    }
}

const NS: &str = "test";

#[valtron_test]
async fn insert_and_get() {
    let (_dir, store) = make_store().await;
    store.insert_async(NS, entry("a", vec![1.0, 0.0, 0.0])).await.unwrap();
    let got = store.get_async(NS, "a").await.unwrap().unwrap();
    assert_eq!(got.id, "a");
    assert_eq!(got.vector.data, vec![1.0, 0.0, 0.0]);
}

#[valtron_test]
async fn dimension_mismatch_rejected() {
    let (_dir, store) = make_store().await;
    let err = store.insert_async(NS, entry("bad", vec![1.0, 0.0])).await.unwrap_err();
    assert!(matches!(
        err,
        VectorStoreError::DimensionMismatch {
            expected: 3,
            got: 2
        }
    ));
}

#[valtron_test]
async fn zero_vector_rejected() {
    let (_dir, store) = make_store().await;
    let err = store
        .insert_async(NS, entry("zero", vec![0.0, 0.0, 0.0])).await
        .unwrap_err();
    assert!(matches!(err, VectorStoreError::ZeroVector));
}

#[valtron_test]
async fn search_top_k() {
    let (_dir, store) = make_store().await;
    store.insert_async(NS, entry("x", vec![1.0, 0.0, 0.0])).await.unwrap();
    store.insert_async(NS, entry("y", vec![0.0, 1.0, 0.0])).await.unwrap();
    store.insert_async(NS, entry("z", vec![0.9, 0.1, 0.0])).await.unwrap();

    let results = store.search_async(NS, &[1.0, 0.0, 0.0], 2).await.unwrap();
    assert_eq!(results.len(), 2);
    assert_eq!(results[0].id, "x");
    assert_eq!(results[1].id, "z");
}

#[valtron_test]
async fn namespace_isolation() {
    let (_dir, store) = make_store().await;
    store
        .insert_async("ns_a", entry("v1", vec![1.0, 0.0, 0.0])).await
        .unwrap();
    store
        .insert_async("ns_b", entry("v2", vec![0.0, 1.0, 0.0])).await
        .unwrap();

    assert_eq!(store.len_async("ns_a").await, 1);
    assert_eq!(store.len_async("ns_b").await, 1);

    let results = store.search_async("ns_a", &[1.0, 0.0, 0.0], 10).await.unwrap();
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].id, "v1");

    assert!(store.get_async("ns_a", "v2").await.unwrap().is_none());
    assert!(store.get_async("ns_b", "v1").await.unwrap().is_none());
}

#[valtron_test]
async fn delete_entry() {
    let (_dir, store) = make_store().await;
    store.insert_async(NS, entry("a", vec![1.0, 0.0, 0.0])).await.unwrap();
    assert_eq!(store.len_async(NS).await, 1);
    store.delete_async(NS, "a").await.unwrap();
    assert_eq!(store.len_async(NS).await, 0);
    assert!(store.get_async(NS, "a").await.unwrap().is_none());
}

#[valtron_test]
async fn delete_not_found() {
    let (_dir, store) = make_store().await;
    let err = store.delete_async(NS, "nope").await.unwrap_err();
    assert!(matches!(err, VectorStoreError::NotFound { .. }));
}

#[valtron_test]
async fn overwrite_on_insert() {
    let (_dir, store) = make_store().await;
    store.insert_async(NS, entry("a", vec![1.0, 0.0, 0.0])).await.unwrap();
    store.insert_async(NS, entry("a", vec![0.0, 1.0, 0.0])).await.unwrap();
    let got = store.get_async(NS, "a").await.unwrap().unwrap();
    assert_eq!(got.vector.data, vec![0.0, 1.0, 0.0]);
    assert_eq!(store.len_async(NS).await, 1);
}

#[valtron_test]
async fn empty_store() {
    let (_dir, store) = make_store().await;
    assert_eq!(store.len_async(NS).await, 0);
    let results = store.search_async(NS, &[1.0, 0.0, 0.0], 5).await.unwrap();
    assert_eq!(results.len(), 0);
}

#[valtron_test]
async fn metadata_round_trip() {
    let (_dir, store) = make_store().await;
    let mut meta = VectorMetadata::default();
    meta.tags.insert("source".into(), "test".into());
    meta.tags.insert("type".into(), "embedding".into());

    let e = VectorEntry {
        id: "m1".into(),
        vector: Vector::new(vec![1.0, 0.0, 0.0]),
        metadata: meta.clone(),
    };
    store.insert_async(NS, e).await.unwrap();

    let got = store.get_async(NS, "m1").await.unwrap().unwrap();
    assert_eq!(got.metadata.tags.get("source").unwrap(), "test");
    assert_eq!(got.metadata.tags.get("type").unwrap(), "embedding");
}

#[valtron_test]
async fn persistence_across_reopen() {
    let dir = TempDir::new().unwrap();
    let db_path = dir.path().join("persist.db");
    let url = db_path.to_str().unwrap();
    let config = VectorStoreConfig::new(3, DistanceMetric::Cosine);

    {
        let storage = Arc::new(TursoStorage::new(url).await.unwrap());
        let store = SqlVectorStore::new(storage, config.clone()).await.unwrap();
        store.insert_async(NS, entry("p1", vec![1.0, 0.0, 0.0])).await.unwrap();
        store.insert_async(NS, entry("p2", vec![0.0, 1.0, 0.0])).await.unwrap();
    }

    {
        let storage = Arc::new(TursoStorage::new(url).await.unwrap());
        let store = SqlVectorStore::new(storage, config).await.unwrap();
        assert_eq!(store.len_async(NS).await, 2);
        let got = store.get_async(NS, "p1").await.unwrap().unwrap();
        assert_eq!(got.vector.data, vec![1.0, 0.0, 0.0]);

        let results = store.search_async(NS, &[1.0, 0.0, 0.0], 1).await.unwrap();
        assert_eq!(results[0].id, "p1");
    }
}
