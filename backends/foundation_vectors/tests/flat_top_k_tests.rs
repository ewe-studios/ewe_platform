use foundation_vectors::flat::{flat_top_k, flat_top_k_owned};
use foundation_vectors::metric::DistanceMetric;

fn entries() -> Vec<(String, Vec<f32>)> {
    vec![
        ("a".into(), vec![1.0, 0.0]),
        ("b".into(), vec![0.7, 0.7]),
        ("c".into(), vec![0.0, 1.0]),
        ("d".into(), vec![-1.0, 0.0]),
    ]
}

#[test]
fn top_k_cosine() {
    let data = entries();
    let refs: Vec<(&str, &[f32])> = data
        .iter()
        .map(|(id, v)| (id.as_str(), v.as_slice()))
        .collect();
    let query = [1.0, 0.0];
    let results = flat_top_k(&query, refs.into_iter(), 2, DistanceMetric::Cosine);
    assert_eq!(results.len(), 2);
    assert_eq!(results[0].id, "a");
    assert_eq!(results[1].id, "b");
}

#[test]
fn top_k_l2() {
    let data = entries();
    let refs: Vec<(&str, &[f32])> = data
        .iter()
        .map(|(id, v)| (id.as_str(), v.as_slice()))
        .collect();
    let query = [0.0, 0.0];
    let results = flat_top_k(&query, refs.into_iter(), 1, DistanceMetric::L2);
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].id, "b");
}

#[test]
fn k_zero_returns_empty() {
    let data = entries();
    let refs: Vec<(&str, &[f32])> = data
        .iter()
        .map(|(id, v)| (id.as_str(), v.as_slice()))
        .collect();
    let results = flat_top_k(&[1.0, 0.0], refs.into_iter(), 0, DistanceMetric::Cosine);
    assert!(results.is_empty());
}

#[test]
fn k_greater_than_n() {
    let data = entries();
    let refs: Vec<(&str, &[f32])> = data
        .iter()
        .map(|(id, v)| (id.as_str(), v.as_slice()))
        .collect();
    let results = flat_top_k(&[1.0, 0.0], refs.into_iter(), 100, DistanceMetric::Cosine);
    assert_eq!(results.len(), 4);
}

#[test]
fn empty_entries() {
    let results = flat_top_k(&[1.0], std::iter::empty(), 5, DistanceMetric::Cosine);
    assert!(results.is_empty());
}

#[test]
fn nan_scores_excluded() {
    let data: Vec<(&str, &[f32])> = vec![("zero", &[0.0, 0.0]), ("good", &[1.0, 0.0])];
    let query = [1.0, 0.0];
    let results = flat_top_k(&query, data.into_iter(), 2, DistanceMetric::Cosine);
    assert_eq!(results.len(), 2);
    assert_eq!(results[0].id, "good");
}

#[test]
fn tie_breaking_by_id() {
    let data: Vec<(&str, &[f32])> = vec![("b", &[1.0, 0.0]), ("a", &[1.0, 0.0])];
    let query = [1.0, 0.0];
    let results = flat_top_k(&query, data.into_iter(), 2, DistanceMetric::Cosine);
    assert_eq!(results[0].id, "a");
    assert_eq!(results[1].id, "b");
}

#[test]
fn flat_top_k_owned_matches_borrowed() {
    let data = entries();
    let refs: Vec<(&str, &[f32])> = data
        .iter()
        .map(|(id, v)| (id.as_str(), v.as_slice()))
        .collect();
    let query = [1.0, 0.0];
    let borrowed = flat_top_k(&query, refs.into_iter(), 3, DistanceMetric::Cosine);
    let owned = flat_top_k_owned(&query, data.into_iter(), 3, DistanceMetric::Cosine);
    assert_eq!(borrowed.len(), owned.len());
    for (b, o) in borrowed.iter().zip(owned.iter()) {
        assert_eq!(b.id, o.id);
        assert!((b.score - o.score).abs() < 1e-6);
    }
}

#[test]
fn results_sorted_descending() {
    let data = entries();
    let refs: Vec<(&str, &[f32])> = data
        .iter()
        .map(|(id, v)| (id.as_str(), v.as_slice()))
        .collect();
    let results = flat_top_k(&[1.0, 0.0], refs.into_iter(), 4, DistanceMetric::Cosine);
    for w in results.windows(2) {
        assert!(
            w[0].score >= w[1].score,
            "{} should be >= {}",
            w[0].score,
            w[1].score
        );
    }
}
