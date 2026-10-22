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

#[test]
fn top_k_accumulator_matches_flat_top_k() {
    use foundation_vectors::TopK;

    let query = [1.0_f32, 0.0, 0.0];
    let data: Vec<(String, Vec<f32>)> = (0..50)
        .map(|i| {
            #[allow(clippy::cast_precision_loss)]
            let x = i as f32 / 50.0;
            (format!("v{i:02}"), vec![x, 1.0 - x, 0.5])
        })
        .collect();

    // Fed one candidate at a time (as a streaming store does)...
    let mut top = TopK::new(&query, 5, DistanceMetric::Cosine);
    for (id, v) in &data {
        top.push(id, v);
    }
    let streamed = top.finish();

    // ...the result equals the batch helper's.
    let batch = flat_top_k(
        &query,
        data.iter().map(|(id, v)| (id.as_str(), v.as_slice())),
        5,
        DistanceMetric::Cosine,
    );
    assert_eq!(streamed.len(), 5);
    let ids = |m: &[foundation_vectors::store::VectorMatch]| m.iter().map(|x| x.id.clone()).collect::<Vec<_>>();
    assert_eq!(ids(&streamed), ids(&batch));
    assert_eq!(streamed[0].id, "v49");
}

#[test]
fn top_k_accumulator_with_k_zero_keeps_nothing() {
    let query = [1.0_f32, 0.0];
    let mut top = foundation_vectors::TopK::new(&query, 0, DistanceMetric::Cosine);
    top.push("a", &[1.0, 0.0]);
    assert_eq!(top.finish(), Vec::new());
}
