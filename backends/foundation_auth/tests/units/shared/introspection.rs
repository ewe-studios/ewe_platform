//! Shared introspection tests.

use foundation_auth::shared::introspection::IntrospectionResult;

#[test]
fn test_parse_active_token() {
    let json = r#"{
        "active": true,
        "scope": "openid profile email",
        "client_id": "resource-server",
        "sub": "user-123",
        "username": "alice",
        "token_type": "Bearer",
        "exp": 9999999999,
        "iat": 1700000000,
        "jti": "token-abc"
    }"#;

    let result: IntrospectionResult = serde_json::from_str(json).unwrap();
    assert!(result.active);
    assert_eq!(result.scope.as_deref(), Some("openid profile email"));
    assert_eq!(result.sub.as_deref(), Some("user-123"));
    assert!(result.is_valid());
}

#[test]
fn test_parse_inactive_token() {
    let json = r#"{"active": false}"#;
    let result: IntrospectionResult = serde_json::from_str(json).unwrap();
    assert!(!result.active);
    assert!(!result.is_valid());
}
