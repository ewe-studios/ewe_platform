//! `SqliteVecVectorStore` tests (F29) — native sqlite-vec KNN with bundled .so.

use tempfile::TempDir;

use foundation_vectors::metric::DistanceMetric;
use foundation_vectors::store::{
    VectorEntry, VectorMatch, VectorMetadata, VectorStore, VectorStoreConfig, VectorStoreError,
};
use foundation_vectors::vector::Vector;
use foundation_vectors::SqliteVecVectorStore;

fn cfg() -> VectorStoreConfig {
    VectorStoreConfig::new(3, DistanceMetric::Cosine)
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
fn vec0_search_returns_nearest_first() {
    let dir = TempDir::new().unwrap();
    let store = SqliteVecVectorStore::open(cfg(), dir.path().join("v.db")).unwrap();

    store.insert("ns", entry("x", [1.0, 0.0, 0.0])).unwrap();
    store.insert("ns", entry("y", [0.0, 1.0, 0.0])).unwrap();
    store.insert("ns", entry("z", [0.9, 0.1, 0.0])).unwrap();

    let m = store.search("ns", &[1.0, 0.0, 0.0], 2).unwrap();
    assert_eq!(m.len(), 2);
    assert_eq!(ids(&m), vec!["x", "z"]);
    assert!(m[0].score >= m[1].score);
}

#[test]
fn namespaces_isolated() {
    let dir = TempDir::new().unwrap();
    let store = SqliteVecVectorStore::open(cfg(), dir.path().join("v.db")).unwrap();

    store.insert("a", entry("a1", [1.0, 0.0, 0.0])).unwrap();
    store.insert("b", entry("b1", [1.0, 0.0, 0.0])).unwrap();

    let ma = store.search("a", &[1.0, 0.0, 0.0], 10).unwrap();
    assert_eq!(ids(&ma), vec!["a1"]);
    let mb = store.search("b", &[1.0, 0.0, 0.0], 10).unwrap();
    assert_eq!(ids(&mb), vec!["b1"]);
}

#[test]
fn dimension_and_zero_validation() {
    let dir = TempDir::new().unwrap();
    let store = SqliteVecVectorStore::open(cfg(), dir.path().join("v.db")).unwrap();

    let bad_dim = VectorEntry {
        id: "d".into(),
        vector: Vector::new(vec![1.0, 2.0]),
        metadata: VectorMetadata::default(),
    };
    assert!(matches!(
        store.insert("ns", bad_dim).unwrap_err(),
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
fn get_returns_none_and_delete_works() {
    let dir = TempDir::new().unwrap();
    let store = SqliteVecVectorStore::open(cfg(), dir.path().join("v.db")).unwrap();

    store.insert("ns", entry("k", [0.2, 0.4, 0.6])).unwrap();
    // vec0 doesn't expose raw vectors via SELECT.
    assert!(store.get("ns", "k").unwrap().is_none());

    store.delete("ns", "k").unwrap();
    assert_eq!(store.len("ns"), 0);
    assert!(store.search("ns", &[0.2, 0.4, 0.6], 5).unwrap().is_empty());
}
