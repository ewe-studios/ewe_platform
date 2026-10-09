use foundation_vectors::index::{VectorError, VectorIndex, load_index};
use foundation_vectors::ivf::IvfIndex;
use foundation_vectors::metric::DistanceMetric;

fn make_ivf() -> IvfIndex {
    IvfIndex::new(DistanceMetric::Cosine, 3, 4, 2)
}

#[test]
fn ivf_insert_and_search() {
    let index = make_ivf();
    index.insert("a", &[1.0, 0.0, 0.0]).unwrap();
    index.insert("b", &[0.0, 1.0, 0.0]).unwrap();
    index.insert("c", &[0.9, 0.1, 0.0]).unwrap();

    let results = index.search(&[1.0, 0.0, 0.0], 2).unwrap();
    assert!(!results.is_empty());
    assert_eq!(results[0].id, "a");
}

#[test]
fn ivf_dimension_mismatch() {
    let index = make_ivf();
    let err = index.insert("a", &[1.0]).unwrap_err();
    assert!(matches!(err, VectorError::DimensionMismatch { expected: 3, got: 1 }));
}

#[test]
fn ivf_remove() {
    let index = make_ivf();
    index.insert("a", &[1.0, 0.0, 0.0]).unwrap();
    index.insert("b", &[0.0, 1.0, 0.0]).unwrap();
    assert_eq!(index.len(), 2);
    index.remove("a").unwrap();
    assert_eq!(index.len(), 1);
}

#[test]
fn ivf_remove_not_found() {
    let index = make_ivf();
    let err = index.remove("nope").unwrap_err();
    assert!(matches!(err, VectorError::NotFound { .. }));
}

#[test]
fn ivf_builds_centroids_on_threshold() {
    let index = IvfIndex::new(DistanceMetric::Cosine, 2, 2, 2);
    for i in 0..10 {
        let angle = (i as f32) * 0.628;
        index
            .insert(&format!("v{i}"), &[libm::cosf(angle), libm::sinf(angle)])
            .unwrap();
    }
    assert_eq!(index.len(), 10);
    let results = index.search(&[1.0, 0.0], 3).unwrap();
    assert!(!results.is_empty());
}

#[test]
fn ivf_serialize_roundtrip() {
    let index = IvfIndex::with_seed(DistanceMetric::Cosine, 2, 2, 2, 42);
    for i in 0..8 {
        let angle = (i as f32) * 0.785;
        index
            .insert(&format!("v{i}"), &[libm::cosf(angle), libm::sinf(angle)])
            .unwrap();
    }

    let bytes = index.to_bytes();
    let restored = load_index(&bytes).unwrap();

    assert_eq!(restored.len(), 8);
    let orig_results = index.search(&[1.0, 0.0], 3).unwrap();
    let rest_results = restored.search(&[1.0, 0.0], 3).unwrap();
    assert_eq!(orig_results.len(), rest_results.len());
    assert_eq!(orig_results[0].id, rest_results[0].id);
}

#[test]
fn ivf_deterministic_with_seed() {
    let build = |seed: u64| {
        let index = IvfIndex::with_seed(DistanceMetric::Cosine, 3, 3, 2, seed);
        for i in 0..12 {
            let v = [
                (i as f32 * 0.3).cos(),
                (i as f32 * 0.5).sin(),
                (i as f32 * 0.7).cos(),
            ];
            index.insert(&format!("d{i}"), &v).unwrap();
        }
        index.search(&[1.0, 0.0, 0.0], 5).unwrap()
    };

    let r1 = build(42);
    let r2 = build(42);
    assert_eq!(r1.len(), r2.len());
    for (a, b) in r1.iter().zip(r2.iter()) {
        assert_eq!(a.id, b.id);
        assert!((a.score - b.score).abs() < 1e-6);
    }
}

#[test]
fn ivf_overwrite_existing() {
    let index = make_ivf();
    index.insert("a", &[1.0, 0.0, 0.0]).unwrap();
    index.insert("a", &[0.0, 1.0, 0.0]).unwrap();
    assert_eq!(index.len(), 1);
    let results = index.search(&[0.0, 1.0, 0.0], 1).unwrap();
    assert_eq!(results[0].id, "a");
}

#[test]
fn ivf_empty_search() {
    let index = make_ivf();
    let results = index.search(&[1.0, 0.0, 0.0], 5).unwrap();
    assert!(results.is_empty());
}
