//! Shared JWKS tests.

use foundation_auth::shared::jwks::Jwks;

#[test]
fn test_parse_jwks_ed25519() {
    let json = r#"{
        "keys": [{
            "kty": "OKP",
            "crv": "Ed25519",
            "x": "Fb6xzYJqMxN0KqZ6F3h4YvGkHxT0zG0qHqZ3Yb1x4Yc",
            "kid": "key-2025-01",
            "use": "sig",
            "alg": "EdDSA"
        }]
    }"#;

    let jwks = Jwks::from_json(json).unwrap();
    assert_eq!(jwks.keys.len(), 1);
    let key = jwks.find_by_kid("key-2025-01").unwrap();
    assert_eq!(key.key_type, "OKP");
    assert_eq!(key.algorithm, "EdDSA");
}

#[test]
fn test_parse_jwks_multiple_keys() {
    let json = r#"{
        "keys": [
            {
                "kty": "OKP", "crv": "Ed25519",
                "x": "Fb6xzYJqMxN0KqZ6F3h4YvGkHxT0zG0qHqZ3Yb1x4Yc",
                "kid": "key-1", "use": "sig", "alg": "EdDSA"
            },
            {
                "kty": "OKP", "crv": "Ed25519",
                "x": "Fb6xzYJqMxN0KqZ6F3h4YvGkHxT0zG0qHqZ3Yb1x4Yc",
                "kid": "key-2", "use": "sig", "alg": "EdDSA"
            }
        ]
    }"#;

    let jwks = Jwks::from_json(json).unwrap();
    assert_eq!(jwks.keys.len(), 2);
    assert!(jwks.find_by_kid("key-1").is_some());
    assert!(jwks.find_by_kid("key-2").is_some());
    assert!(jwks.find_by_kid("key-3").is_none());
}

#[test]
fn test_jwks_to_json_roundtrip() {
    let json = r#"{
        "keys": [{
            "kty": "OKP", "crv": "Ed25519",
            "x": "Fb6xzYJqMxN0KqZ6F3h4YvGkHxT0zG0qHqZ3Yb1x4Yc",
            "kid": "key-1", "use": "sig", "alg": "EdDSA"
        }]
    }"#;

    let jwks = Jwks::from_json(json).unwrap();
    let serialized = jwks.to_json().unwrap();
    let parsed = Jwks::from_json(&serialized).unwrap();
    assert_eq!(parsed.keys.len(), 1);
    assert_eq!(parsed.keys[0].key_id, "key-1");
}
