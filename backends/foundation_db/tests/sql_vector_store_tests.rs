use std::sync::{Arc, Mutex};

use foundation_db::{SqlVectorStore, TursoStorage};
use foundation_vectors::store::{
    VectorEntry, VectorMetadata, VectorStore, VectorStoreConfig, VectorStoreError,
};
use foundation_vectors::metric::DistanceMetric;
use foundation_vectors::vector::Vector;
use tempfile::TempDir;

static POOL_GUARD: Mutex<Option<foundation_core::valtron::PoolGuard>> = Mutex::new(None);

fn init_valtron() {
    let mut guard = POOL_GUARD.lock().unwrap();
    if guard.is_none() {
        *guard = Some(foundation_core::valtron::initialize_pool(42, None));
    }
}

fn make_store() -> (TempDir, SqlVectorStore<TursoStorage>) {
    init_valtron();
    let dir = TempDir::new().unwrap();
    let db_path = dir.path().join("vectors.db");
    let storage = Arc::new(TursoStorage::new(db_path.to_str().unwrap()).unwrap());
    let config = VectorStoreConfig::new(3, DistanceMetric::Cosine);
    let store = SqlVectorStore::new(storage, config).unwrap();
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

#[test]
fn insert_and_get() {
    let (_dir, store) = make_store();
    store.insert(NS, entry("a", vec![1.0, 0.0, 0.0])).unwrap();
    let got = store.get(NS, "a").unwrap().unwrap();
    assert_eq!(got.id, "a");
    assert_eq!(got.vector.data, vec![1.0, 0.0, 0.0]);
}

#[test]
fn dimension_mismatch_rejected() {
    let (_dir, store) = make_store();
    let err = store.insert(NS, entry("bad", vec![1.0, 0.0])).unwrap_err();
    assert!(matches!(
        err,
        VectorStoreError::DimensionMismatch {
            expected: 3,
            got: 2
        }
    ));
}

#[test]
fn zero_vector_rejected() {
    let (_dir, store) = make_store();
    let err = store
        .insert(NS, entry("zero", vec![0.0, 0.0, 0.0]))
        .unwrap_err();
    assert!(matches!(err, VectorStoreError::ZeroVector));
}

#[test]
fn search_top_k() {
    let (_dir, store) = make_store();
    store.insert(NS, entry("x", vec![1.0, 0.0, 0.0])).unwrap();
    store.insert(NS, entry("y", vec![0.0, 1.0, 0.0])).unwrap();
    store.insert(NS, entry("z", vec![0.9, 0.1, 0.0])).unwrap();

    let results = store.search(NS, &[1.0, 0.0, 0.0], 2).unwrap();
    assert_eq!(results.len(), 2);
    assert_eq!(results[0].id, "x");
    assert_eq!(results[1].id, "z");
}

#[test]
fn namespace_isolation() {
    let (_dir, store) = make_store();
    store
        .insert("ns_a", entry("v1", vec![1.0, 0.0, 0.0]))
        .unwrap();
    store
        .insert("ns_b", entry("v2", vec![0.0, 1.0, 0.0]))
        .unwrap();

    assert_eq!(store.len("ns_a"), 1);
    assert_eq!(store.len("ns_b"), 1);

    let results = store.search("ns_a", &[1.0, 0.0, 0.0], 10).unwrap();
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].id, "v1");

    assert!(store.get("ns_a", "v2").unwrap().is_none());
    assert!(store.get("ns_b", "v1").unwrap().is_none());
}

#[test]
fn delete_entry() {
    let (_dir, store) = make_store();
    store.insert(NS, entry("a", vec![1.0, 0.0, 0.0])).unwrap();
    assert_eq!(store.len(NS), 1);
    store.delete(NS, "a").unwrap();
    assert_eq!(store.len(NS), 0);
    assert!(store.get(NS, "a").unwrap().is_none());
}

#[test]
fn delete_not_found() {
    let (_dir, store) = make_store();
    let err = store.delete(NS, "nope").unwrap_err();
    assert!(matches!(err, VectorStoreError::NotFound { .. }));
}

#[test]
fn overwrite_on_insert() {
    let (_dir, store) = make_store();
    store.insert(NS, entry("a", vec![1.0, 0.0, 0.0])).unwrap();
    store.insert(NS, entry("a", vec![0.0, 1.0, 0.0])).unwrap();
    let got = store.get(NS, "a").unwrap().unwrap();
    assert_eq!(got.vector.data, vec![0.0, 1.0, 0.0]);
    assert_eq!(store.len(NS), 1);
}

#[test]
fn empty_store() {
    let (_dir, store) = make_store();
    assert!(store.is_empty(NS));
    let results = store.search(NS, &[1.0, 0.0, 0.0], 5).unwrap();
    assert!(results.is_empty());
}

#[test]
fn metadata_round_trip() {
    let (_dir, store) = make_store();
    let mut meta = VectorMetadata::default();
    meta.tags.insert("source".into(), "test".into());
    meta.tags.insert("type".into(), "embedding".into());

    let e = VectorEntry {
        id: "m1".into(),
        vector: Vector::new(vec![1.0, 0.0, 0.0]),
        metadata: meta.clone(),
    };
    store.insert(NS, e).unwrap();

    let got = store.get(NS, "m1").unwrap().unwrap();
    assert_eq!(got.metadata.tags.get("source").unwrap(), "test");
    assert_eq!(got.metadata.tags.get("type").unwrap(), "embedding");
}

#[test]
fn persistence_across_reopen() {
    init_valtron();
    let dir = TempDir::new().unwrap();
    let db_path = dir.path().join("persist.db");
    let url = db_path.to_str().unwrap();
    let config = VectorStoreConfig::new(3, DistanceMetric::Cosine);

    {
        let storage = Arc::new(TursoStorage::new(url).unwrap());
        let store = SqlVectorStore::new(storage, config.clone()).unwrap();
        store.insert(NS, entry("p1", vec![1.0, 0.0, 0.0])).unwrap();
        store.insert(NS, entry("p2", vec![0.0, 1.0, 0.0])).unwrap();
    }

    {
        let storage = Arc::new(TursoStorage::new(url).unwrap());
        let store = SqlVectorStore::new(storage, config).unwrap();
        assert_eq!(store.len(NS), 2);
        let got = store.get(NS, "p1").unwrap().unwrap();
        assert_eq!(got.vector.data, vec![1.0, 0.0, 0.0]);

        let results = store.search(NS, &[1.0, 0.0, 0.0], 1).unwrap();
        assert_eq!(results[0].id, "p1");
    }
}
