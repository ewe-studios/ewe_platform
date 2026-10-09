use foundation_vectors::vector::Vector;

#[test]
fn dimension_and_slice() {
    let v = Vector::new(vec![1.0, 2.0, 3.0]);
    assert_eq!(v.dimension(), 3);
    assert_eq!(v.as_slice(), &[1.0, 2.0, 3.0]);
}

#[test]
fn magnitude() {
    let v = Vector::new(vec![3.0, 4.0]);
    assert!((v.magnitude() - 5.0).abs() < 1e-6);
}

#[test]
fn zero_vector() {
    let v = Vector::new(vec![0.0, 0.0]);
    assert!(v.is_zero());
    assert!(v.normalize().is_none());
}

#[test]
fn normalize_unit() {
    let v = Vector::new(vec![3.0, 4.0]);
    let n = v.normalize().unwrap();
    assert!((n.magnitude() - 1.0).abs() < 1e-6);
    assert!((n.data[0] - 0.6).abs() < 1e-6);
    assert!((n.data[1] - 0.8).abs() < 1e-6);
}
