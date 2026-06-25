use foundation_vectors::metric::DistanceMetric;
use foundation_vectors::sqrt::SqrtStrategy;
use foundation_vectors::store::{
    InMemoryVectorStore, VectorEntry, VectorMetadata, VectorStore, VectorStoreConfig,
    VectorStoreError,
};
use foundation_vectors::vector::Vector;

const NS: &str = "test";

fn make_store() -> InMemoryVectorStore {
    InMemoryVectorStore::new(VectorStoreConfig::new(3, DistanceMetric::Cosine))
}

fn entry(id: &str, data: Vec<f32>) -> VectorEntry {
    VectorEntry {
        id: id.to_string(),
        vector: Vector::new(data),
        metadata: VectorMetadata::default(),
    }
}

#[test]
fn insert_and_get() {
    let store = make_store();
    store.insert(NS, entry("a", vec![1.0, 0.0, 0.0])).unwrap();
    let got = store.get(NS, "a").unwrap().unwrap();
    assert_eq!(got.id, "a");
    assert_eq!(got.vector.data, vec![1.0, 0.0, 0.0]);
}

#[test]
fn dimension_mismatch_rejected() {
    let store = make_store();
    let err = store
        .insert(NS, entry("bad", vec![1.0, 0.0]))
        .unwrap_err();
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
    let store = make_store();
    let err = store
        .insert(NS, entry("zero", vec![0.0, 0.0, 0.0]))
        .unwrap_err();
    assert!(matches!(err, VectorStoreError::ZeroVector));
}

#[test]
fn search_top_k() {
    let store = make_store();
    store.insert(NS, entry("x", vec![1.0, 0.0, 0.0])).unwrap();
    store.insert(NS, entry("y", vec![0.0, 1.0, 0.0])).unwrap();
    store.insert(NS, entry("z", vec![0.9, 0.1, 0.0])).unwrap();

    let results = store.search(NS, &[1.0, 0.0, 0.0], 2).unwrap();
    assert_eq!(results.len(), 2);
    assert_eq!(results[0].id, "x");
    assert_eq!(results[1].id, "z");
}

#[test]
fn search_dimension_mismatch() {
    let store = make_store();
    store.insert(NS, entry("a", vec![1.0, 0.0, 0.0])).unwrap();
    let err = store.search(NS, &[1.0, 0.0], 1).unwrap_err();
    assert!(matches!(err, VectorStoreError::DimensionMismatch { .. }));
}

#[test]
fn delete_entry() {
    let store = make_store();
    store.insert(NS, entry("a", vec![1.0, 0.0, 0.0])).unwrap();
    assert_eq!(store.len(NS), 1);
    store.delete(NS, "a").unwrap();
    assert_eq!(store.len(NS), 0);
    assert!(store.get(NS, "a").unwrap().is_none());
}

#[test]
fn delete_not_found() {
    let store = make_store();
    let err = store.delete(NS, "nope").unwrap_err();
    assert!(matches!(err, VectorStoreError::NotFound { .. }));
}

#[test]
fn overwrite_on_insert() {
    let store = make_store();
    store.insert(NS, entry("a", vec![1.0, 0.0, 0.0])).unwrap();
    store.insert(NS, entry("a", vec![0.0, 1.0, 0.0])).unwrap();
    let got = store.get(NS, "a").unwrap().unwrap();
    assert_eq!(got.vector.data, vec![0.0, 1.0, 0.0]);
    assert_eq!(store.len(NS), 1);
}

#[test]
fn empty_store() {
    let store = make_store();
    assert!(store.is_empty(NS));
    let results = store.search(NS, &[1.0, 0.0, 0.0], 5).unwrap();
    assert!(results.is_empty());
}

#[test]
fn namespace_isolation() {
    let store = make_store();
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
fn normalized_vectors_strategy() {
    let config = VectorStoreConfig {
        dimension: 2,
        metric: DistanceMetric::Cosine,
        sqrt_strategy: SqrtStrategy::NormalizedVectors,
    };
    let store = InMemoryVectorStore::new(config);
    store.insert(NS, entry("a", vec![3.0, 4.0])).unwrap();
    let got = store.get(NS, "a").unwrap().unwrap();
    let mag = got.vector.magnitude();
    assert!(
        (mag - 1.0).abs() < 1e-5,
        "stored vector should be normalized, mag={mag}"
    );
}

#[test]
fn l2_search() {
    let config = VectorStoreConfig::new(2, DistanceMetric::L2);
    let store = InMemoryVectorStore::new(config);
    store.insert(NS, entry("near", vec![0.1, 0.0])).unwrap();
    store.insert(NS, entry("far", vec![10.0, 10.0])).unwrap();
    let results = store.search(NS, &[0.0, 0.0], 1).unwrap();
    assert_eq!(results[0].id, "near");
}

#[test]
fn dot_search() {
    let config = VectorStoreConfig::new(2, DistanceMetric::Dot);
    let store = InMemoryVectorStore::new(config);
    store.insert(NS, entry("high", vec![10.0, 10.0])).unwrap();
    store.insert(NS, entry("low", vec![0.1, 0.1])).unwrap();
    let results = store.search(NS, &[1.0, 1.0], 1).unwrap();
    assert_eq!(results[0].id, "high");
}
