use foundation_vectors::metric::{DistanceMetric, OrderedScore};

#[test]
fn cosine_identical_vectors() {
    let a = [1.0, 0.0, 0.0];
    let s = DistanceMetric::Cosine.score(&a, &a);
    assert!((s - 1.0).abs() < 1e-6);
}

#[test]
fn cosine_orthogonal() {
    let a = [1.0, 0.0];
    let b = [0.0, 1.0];
    let s = DistanceMetric::Cosine.score(&a, &b);
    assert!(s.abs() < 1e-6);
}

#[test]
fn cosine_opposite() {
    let a = [1.0, 0.0];
    let b = [-1.0, 0.0];
    let s = DistanceMetric::Cosine.score(&a, &b);
    assert!((s - (-1.0)).abs() < 1e-6);
}

#[test]
fn cosine_zero_vector_returns_neg_inf() {
    let a = [0.0, 0.0];
    let b = [1.0, 0.0];
    assert_eq!(DistanceMetric::Cosine.score(&a, &b), f32::NEG_INFINITY);
}

#[test]
fn dot_product_known() {
    let a = [1.0, 2.0, 3.0];
    let b = [4.0, 5.0, 6.0];
    let s = DistanceMetric::Dot.score(&a, &b);
    assert!((s - 32.0).abs() < 1e-6);
}

#[test]
fn l2_identical_is_zero() {
    let a = [1.0, 2.0];
    let s = DistanceMetric::L2.score(&a, &a);
    assert!((s - 0.0).abs() < 1e-6);
}

#[test]
fn l2_higher_means_closer() {
    let q = [0.0, 0.0];
    let near = [1.0, 0.0];
    let far = [3.0, 4.0];
    let s_near = DistanceMetric::L2.score(&q, &near);
    let s_far = DistanceMetric::L2.score(&q, &far);
    assert!(s_near > s_far, "near={s_near} should be > far={s_far}");
}

#[test]
fn try_score_dimension_mismatch() {
    let a = [1.0, 2.0];
    let b = [1.0, 2.0, 3.0];
    let err = DistanceMetric::Cosine.try_score(&a, &b).unwrap_err();
    assert_eq!(err.expected, 2);
    assert_eq!(err.got, 3);
}

#[test]
fn ordered_score_nan_is_least() {
    let nan = OrderedScore(f32::NAN);
    let neg = OrderedScore(f32::NEG_INFINITY);
    let zero = OrderedScore(0.0);
    assert!(nan < neg);
    assert!(nan < zero);
}

#[test]
fn ordered_score_sorts_correctly() {
    let mut scores = vec![
        OrderedScore(0.5),
        OrderedScore(f32::NAN),
        OrderedScore(0.9),
        OrderedScore(-0.1),
    ];
    scores.sort();
    assert!(scores[0].0.is_nan());
    assert!((scores[1].0 - (-0.1)).abs() < 1e-6);
    assert!((scores[2].0 - 0.5).abs() < 1e-6);
    assert!((scores[3].0 - 0.9).abs() < 1e-6);
}
