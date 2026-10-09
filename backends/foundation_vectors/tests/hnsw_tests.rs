use foundation_vectors::hnsw::HnswIndex;
use foundation_vectors::index::{VectorError, VectorIndex, load_index};
use foundation_vectors::metric::DistanceMetric;

fn make_hnsw() -> HnswIndex {
    HnswIndex::with_params(DistanceMetric::Cosine, 3, 4, 20, 10, 42)
}

#[test]
fn hnsw_insert_and_search() {
    let index = make_hnsw();
    index.insert("a", &[1.0, 0.0, 0.0]).unwrap();
    index.insert("b", &[0.0, 1.0, 0.0]).unwrap();
    index.insert("c", &[0.9, 0.1, 0.0]).unwrap();

    let results = index.search(&[1.0, 0.0, 0.0], 2).unwrap();
    assert_eq!(results.len(), 2);
    assert_eq!(results[0].id, "a");
}

#[test]
fn hnsw_dimension_mismatch() {
    let index = make_hnsw();
    let err = index.insert("a", &[1.0]).unwrap_err();
    assert!(matches!(err, VectorError::DimensionMismatch { expected: 3, got: 1 }));
}

#[test]
fn hnsw_search_dimension_mismatch() {
    let index = make_hnsw();
    index.insert("a", &[1.0, 0.0, 0.0]).unwrap();
    let err = index.search(&[1.0, 0.0], 1).unwrap_err();
    assert!(matches!(err, VectorError::DimensionMismatch { .. }));
}

#[test]
fn hnsw_soft_delete() {
    let index = make_hnsw();
    index.insert("a", &[1.0, 0.0, 0.0]).unwrap();
    index.insert("b", &[0.0, 1.0, 0.0]).unwrap();
    assert_eq!(index.len(), 2);
    index.remove("a").unwrap();
    assert_eq!(index.len(), 1);

    let results = index.search(&[1.0, 0.0, 0.0], 5).unwrap();
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].id, "b");
}

#[test]
fn hnsw_remove_not_found() {
    let index = make_hnsw();
    let err = index.remove("nope").unwrap_err();
    assert!(matches!(err, VectorError::NotFound { .. }));
}

#[test]
fn hnsw_overwrite() {
    let index = make_hnsw();
    index.insert("a", &[1.0, 0.0, 0.0]).unwrap();
    index.insert("a", &[0.0, 1.0, 0.0]).unwrap();
    let results = index.search(&[0.0, 1.0, 0.0], 1).unwrap();
    assert_eq!(results[0].id, "a");
    assert!((results[0].score - 1.0).abs() < 1e-5);
}

#[test]
fn hnsw_larger_dataset() {
    let index = HnswIndex::with_params(DistanceMetric::Cosine, 2, 8, 50, 20, 42);
    for i in 0..100 {
        let angle = (i as f32) * 0.0628;
        index
            .insert(&format!("v{i}"), &[libm::cosf(angle), libm::sinf(angle)])
            .unwrap();
    }
    assert_eq!(index.len(), 100);
    let results = index.search(&[1.0, 0.0], 5).unwrap();
    assert_eq!(results.len(), 5);
    // v0 = [cos(0), sin(0)] = [1, 0] should be the best match
    assert_eq!(results[0].id, "v0");
}

#[test]
fn hnsw_serialize_roundtrip() {
    let index = HnswIndex::with_params(DistanceMetric::Cosine, 2, 4, 20, 10, 42);
    for i in 0..20 {
        let angle = (i as f32) * 0.314;
        index
            .insert(&format!("v{i}"), &[libm::cosf(angle), libm::sinf(angle)])
            .unwrap();
    }

    let bytes = index.to_bytes();
    let restored = load_index(&bytes).unwrap();

    assert_eq!(restored.len(), 20);
    let orig = index.search(&[1.0, 0.0], 3).unwrap();
    let rest = restored.search(&[1.0, 0.0], 3).unwrap();
    assert_eq!(orig.len(), rest.len());
    assert_eq!(orig[0].id, rest[0].id);
}

#[test]
fn hnsw_serialize_roundtrip_with_deletes() {
    let index = HnswIndex::with_params(DistanceMetric::Cosine, 2, 4, 20, 10, 42);
    for i in 0..10 {
        index
            .insert(&format!("v{i}"), &[libm::cosf(i as f32), libm::sinf(i as f32)])
            .unwrap();
    }
    index.remove("v3").unwrap();
    index.remove("v7").unwrap();

    let bytes = index.to_bytes();
    let restored = load_index(&bytes).unwrap();
    assert_eq!(restored.len(), 8);
}

#[test]
fn hnsw_empty_search() {
    let index = make_hnsw();
    let results = index.search(&[1.0, 0.0, 0.0], 5).unwrap();
    assert!(results.is_empty());
}

#[test]
fn hnsw_nan_rejected() {
    let index = make_hnsw();
    let err = index.insert("a", &[f32::NAN, 0.0, 0.0]).unwrap_err();
    assert!(matches!(err, VectorError::ContainsNaN));
}

#[test]
fn hnsw_recall_vs_flat() {
    use foundation_vectors::index::FlatIndex;

    let flat = FlatIndex::new(DistanceMetric::Cosine, 3);
    let hnsw = HnswIndex::with_params(DistanceMetric::Cosine, 3, 16, 200, 64, 42);

    for i in 0..200 {
        let v = [
            libm::cosf(i as f32 * 0.1),
            libm::sinf(i as f32 * 0.2),
            libm::cosf(i as f32 * 0.3),
        ];
        flat.insert(&format!("d{i}"), &v).unwrap();
        hnsw.insert(&format!("d{i}"), &v).unwrap();
    }

    let query = [1.0, 0.0, 0.0];
    let k = 10;
    let flat_results = flat.search(&query, k).unwrap();
    let hnsw_results = hnsw.search(&query, k).unwrap();

    let flat_ids: std::collections::HashSet<&str> =
        flat_results.iter().map(|m| m.id.as_str()).collect();
    let hnsw_ids: std::collections::HashSet<&str> =
        hnsw_results.iter().map(|m| m.id.as_str()).collect();

    let overlap = flat_ids.intersection(&hnsw_ids).count();
    let recall = overlap as f32 / k as f32;
    assert!(
        recall >= 0.8,
        "HNSW recall vs flat should be >= 0.8, got {recall} (overlap={overlap}/{k})"
    );
}
