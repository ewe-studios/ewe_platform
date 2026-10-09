use foundation_db::{ZeroizingSecret, ZeroizingString};

#[test]
fn test_zeroizing_secret() {
    let secret = ZeroizingSecret::new(vec![1u8, 2, 3, 4]);
    assert_eq!(secret.get(), &vec![1u8, 2, 3, 4]);

    let exposed = secret.expose();
    assert_eq!(exposed, vec![1u8, 2, 3, 4]);
}

#[test]
fn test_zeroizing_string() {
    let secret = ZeroizingString::from_string("sensitive data".to_string());
    assert_eq!(secret.as_str(), "sensitive data");
}
