//! `LibsqlVectorStore` tests (F29) — native `DiskANN` (`vector_top_k`) over a
//! real local libSQL database, with namespace over-fetch + exact re-rank.

use std::sync::{Arc, Mutex};

use foundation_db::LibsqlStore;
use foundation_vectors::metric::DistanceMetric;
use foundation_vectors::store::{
    VectorEntry, VectorMatch, VectorMetadata, VectorStore, VectorStoreConfig, VectorStoreError,
};
use foundation_vectors::vector::Vector;
use foundation_vectors::LibsqlVectorStore;
use tempfile::TempDir;

static POOL: Mutex<Option<foundation_core::valtron::PoolGuard>> = Mutex::new(None);

fn init() {
    let mut g = POOL.lock().unwrap();
    if g.is_none() {
        *g = Some(foundation_core::valtron::initialize_pool(42, None));
    }
}

fn make_store() -> (TempDir, LibsqlVectorStore<LibsqlStore>) {
    init();
    let dir = TempDir::new().unwrap();
    let db = dir.path().join("vec.db");
    let backend = Arc::new(LibsqlStore::new_kv(db.to_str().unwrap(), None).unwrap());
    let store = LibsqlVectorStore::new(backend, VectorStoreConfig::new(3, DistanceMetric::Cosine))
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

#[test]
fn diskann_search_returns_nearest_first() {
    let (_d, store) = make_store();
    store.insert("ns", entry("x", [1.0, 0.0, 0.0])).unwrap();
    store.insert("ns", entry("y", [0.0, 1.0, 0.0])).unwrap();
    store.insert("ns", entry("z", [0.9, 0.1, 0.0])).unwrap();

    let m = store.search("ns", &[1.0, 0.0, 0.0], 2).unwrap();
    assert_eq!(m.len(), 2);
    assert_eq!(ids(&m), vec!["x", "z"]);
    assert!(m[0].score >= m[1].score);
}

#[test]
fn namespaces_isolated_via_overfetch_filter() {
    let (_d, store) = make_store();
    store.insert("a", entry("a1", [1.0, 0.0, 0.0])).unwrap();
    store.insert("a", entry("a2", [0.8, 0.2, 0.0])).unwrap();
    store.insert("b", entry("b1", [1.0, 0.0, 0.0])).unwrap();

    let ma = store.search("a", &[1.0, 0.0, 0.0], 5).unwrap();
    let mut got = ids(&ma);
    got.sort_unstable();
    assert_eq!(got, vec!["a1", "a2"], "namespace a only");
    assert!(!ids(&ma).contains(&"b1"));

    let mb = store.search("b", &[1.0, 0.0, 0.0], 5).unwrap();
    assert_eq!(ids(&mb), vec!["b1"]);
}

#[test]
fn dimension_and_zero_validation() {
    let (_d, store) = make_store();
    let bad = VectorEntry {
        id: "d".into(),
        vector: Vector::new(vec![1.0, 2.0]),
        metadata: VectorMetadata::default(),
    };
    assert!(matches!(
        store.insert("ns", bad).unwrap_err(),
        VectorStoreError::DimensionMismatch { expected: 3, got: 2 }
    ));
    assert!(matches!(
        store.insert("ns", entry("z", [0.0, 0.0, 0.0])).unwrap_err(),
        VectorStoreError::ZeroVector
    ));
    assert!(matches!(
        store.search("ns", &[1.0], 1).unwrap_err(),
        VectorStoreError::DimensionMismatch { expected: 3, got: 1 }
    ));
}

#[test]
fn get_update_and_delete() {
    let (_d, store) = make_store();
    store.insert("ns", entry("k", [0.2, 0.4, 0.6])).unwrap();
    let got = store.get("ns", "k").unwrap().expect("present");
    assert_eq!(got.id, "k");
    assert_eq!(got.vector.data.len(), 3);
    assert!((got.vector.data[0] - 0.2).abs() < 1e-4);

    // Upsert replaces the vector for the same (namespace, id).
    store.insert("ns", entry("k", [0.0, 0.0, 1.0])).unwrap();
    assert_eq!(store.len("ns"), 1);
    let updated = store.get("ns", "k").unwrap().unwrap();
    assert!((updated.vector.data[2] - 1.0).abs() < 1e-4);

    assert!(store.get("ns", "missing").unwrap().is_none());
    store.delete("ns", "k").unwrap();
    assert!(store.get("ns", "k").unwrap().is_none());
    assert_eq!(store.len("ns"), 0);
    assert!(matches!(
        store.delete("ns", "k").unwrap_err(),
        VectorStoreError::NotFound { .. }
    ));
}

#[test]
fn metadata_round_trips() {
    let (_d, store) = make_store();
    let mut e = entry("m", [1.0, 0.0, 0.0]);
    e.metadata.tags.insert("k".into(), "v".into());
    store.insert("ns", e).unwrap();
    let got = store.get("ns", "m").unwrap().unwrap();
    assert_eq!(got.metadata.tags.get("k").map(String::as_str), Some("v"));
}
