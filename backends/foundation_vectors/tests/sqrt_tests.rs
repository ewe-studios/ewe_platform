use foundation_vectors::sqrt::SqrtStrategy;

#[test]
fn libm_sqrt_accuracy() {
    let s = SqrtStrategy::Libm;
    assert!((s.sqrt(4.0) - 2.0).abs() < 1e-6);
    assert!((s.sqrt(9.0) - 3.0).abs() < 1e-6);
}

#[test]
fn fast_inv_sqrt_reasonable() {
    let s = SqrtStrategy::FastInvSqrt;
    let result = s.sqrt(4.0);
    assert!((result - 2.0).abs() < 0.1, "fast sqrt(4) = {result}");
}

#[test]
fn sqrt_zero() {
    for strat in [SqrtStrategy::Libm, SqrtStrategy::FastInvSqrt] {
        assert_eq!(strat.sqrt(0.0), 0.0);
    }
}

#[test]
fn inv_sqrt_accuracy() {
    let s = SqrtStrategy::Libm;
    assert!((s.inv_sqrt(4.0) - 0.5).abs() < 1e-6);
}
