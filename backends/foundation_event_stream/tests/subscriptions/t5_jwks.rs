//! T5: the JSON Web Key Set (JWKS) endpoint data.

use crate::subscriptions::support::harness;

#[test]
fn jwks_lists_public_keys() {
    let harness = harness();
    let first = harness.engine.jwks_json().expect("jwks");
    let second = harness.engine.jwks_json().expect("jwks");
    assert_eq!(first, second, "the key identifier is stable");

    let parsed: serde_json::Value = serde_json::from_str(&first).expect("json");
    let key = &parsed["keys"][0];
    assert_eq!(
        key["kid"].as_str(),
        Some(harness.engine.signing().kid()),
        "the published key matches the active signing key"
    );
    assert_eq!(key["kty"].as_str(), Some("OKP"));
    assert_eq!(key["crv"].as_str(), Some("Ed25519"));
    assert_eq!(key["alg"].as_str(), Some("EdDSA"));
    assert!(key["x"].as_str().is_some_and(|value| !value.is_empty()));
}
