use foundation_vectors::index::{FlatIndex, VectorError, VectorIndex, load_index};
use foundation_vectors::metric::DistanceMetric;

#[test]
fn flat_insert_and_search() {
    let index = FlatIndex::new(DistanceMetric::Cosine, 3);
    index.insert("a", &[1.0, 0.0, 0.0]).unwrap();
    index.insert("b", &[0.0, 1.0, 0.0]).unwrap();
    index.insert("c", &[0.9, 0.1, 0.0]).unwrap();

    let results = index.search(&[1.0, 0.0, 0.0], 2).unwrap();
    assert_eq!(results.len(), 2);
    assert_eq!(results[0].id, "a");
    assert_eq!(results[1].id, "c");
}

#[test]
fn flat_dimension_mismatch_on_insert() {
    let index = FlatIndex::new(DistanceMetric::Cosine, 3);
    let err = index.insert("a", &[1.0, 0.0]).unwrap_err();
    assert!(matches!(err, VectorError::DimensionMismatch { expected: 3, got: 2 }));
}

#[test]
fn flat_dimension_mismatch_on_search() {
    let index = FlatIndex::new(DistanceMetric::Cosine, 3);
    index.insert("a", &[1.0, 0.0, 0.0]).unwrap();
    let err = index.search(&[1.0, 0.0], 1).unwrap_err();
    assert!(matches!(err, VectorError::DimensionMismatch { .. }));
}

#[test]
fn flat_nan_rejected() {
    let index = FlatIndex::new(DistanceMetric::Cosine, 2);
    let err = index.insert("a", &[f32::NAN, 0.0]).unwrap_err();
    assert!(matches!(err, VectorError::ContainsNaN));
}

#[test]
fn flat_remove() {
    let index = FlatIndex::new(DistanceMetric::Cosine, 2);
    index.insert("a", &[1.0, 0.0]).unwrap();
    assert_eq!(index.len(), 1);
    index.remove("a").unwrap();
    assert_eq!(index.len(), 0);
    assert!(index.is_empty());
}

#[test]
fn flat_remove_not_found() {
    let index = FlatIndex::new(DistanceMetric::Cosine, 2);
    let err = index.remove("nope").unwrap_err();
    assert!(matches!(err, VectorError::NotFound { .. }));
}

#[test]
fn flat_overwrite() {
    let index = FlatIndex::new(DistanceMetric::Cosine, 2);
    index.insert("a", &[1.0, 0.0]).unwrap();
    index.insert("a", &[0.0, 1.0]).unwrap();
    assert_eq!(index.len(), 1);
    let results = index.search(&[0.0, 1.0], 1).unwrap();
    assert_eq!(results[0].id, "a");
    assert!((results[0].score - 1.0).abs() < 1e-6);
}

#[test]
fn flat_serialize_roundtrip() {
    let index = FlatIndex::new(DistanceMetric::Cosine, 3);
    index.insert("x", &[1.0, 0.0, 0.0]).unwrap();
    index.insert("y", &[0.0, 1.0, 0.0]).unwrap();
    index.insert("z", &[0.0, 0.0, 1.0]).unwrap();

    let bytes = index.to_bytes();
    let restored = load_index(&bytes).unwrap();

    assert_eq!(restored.len(), 3);
    let results = restored.search(&[1.0, 0.0, 0.0], 1).unwrap();
    assert_eq!(results[0].id, "x");
}

#[test]
fn flat_empty_search() {
    let index = FlatIndex::new(DistanceMetric::Cosine, 2);
    let results = index.search(&[1.0, 0.0], 5).unwrap();
    assert!(results.is_empty());
}

#[test]
fn flat_l2_metric() {
    let index = FlatIndex::new(DistanceMetric::L2, 2);
    index.insert("near", &[0.1, 0.0]).unwrap();
    index.insert("far", &[10.0, 10.0]).unwrap();
    let results = index.search(&[0.0, 0.0], 1).unwrap();
    assert_eq!(results[0].id, "near");
}
