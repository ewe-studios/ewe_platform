//! Shared userinfo tests.

use foundation_auth::shared::userinfo::UserInfo;

#[test]
fn test_parse_userinfo_full() {
    let json = r#"{
        "sub": "user-123",
        "name": "Alice Smith",
        "given_name": "Alice",
        "family_name": "Smith",
        "email": "alice@example.com",
        "email_verified": true,
        "picture": "https://example.com/alice.jpg",
        "locale": "en-US"
    }"#;

    let info: UserInfo = serde_json::from_str(json).unwrap();
    assert_eq!(info.sub, "user-123");
    assert_eq!(info.name, Some("Alice Smith".into()));
    assert!(info.email_verified.unwrap());
    assert_eq!(info.locale, Some("en-US".into()));
}

#[test]
fn test_parse_userinfo_minimal() {
    let json = r#"{"sub": "user-456"}"#;
    let info: UserInfo = serde_json::from_str(json).unwrap();
    assert_eq!(info.sub, "user-456");
    assert!(info.name.is_none());
    assert!(info.email.is_none());
}

#[test]
fn test_validate_missing_subject() {
    let json = r#"{"sub": ""}"#;
    let info: UserInfo = serde_json::from_str(json).unwrap();
    assert!(info.validate().is_err());
}
