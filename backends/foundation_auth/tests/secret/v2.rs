//! Version 2 round trips, and every changed signed field breaks the signature.

use base64::Engine;
use foundation_auth::secret::{
    DecodeLimits, SecretDomain, SecretIssuer, SecretPurpose, SecretRequest, SecretVerifier,
    SignedSecret, SigningKey, VerifyPolicy,
};
use foundation_compact::Duration;

use super::common;

const AUDIENCE: &str = "https://agents.example.com/rpc";

fn issuer(ctx: foundation_context::Ctx) -> SecretIssuer {
    let key = SigningKey::from_seed(common::TEST_SEED, Some("2026-10".to_string()));
    SecretIssuer::new(common::domain(), key, ctx, Duration::from_hours(48))
}

fn encoded_value(secret: &SignedSecret) -> serde_json::Value {
    let text = secret.encode();
    let json = base64::engine::general_purpose::URL_SAFE_NO_PAD
        .decode(text.as_bytes())
        .expect("base64url");
    serde_json::from_slice(&json).expect("JSON")
}

fn reencode(value: &serde_json::Value) -> String {
    let json = serde_json::to_vec(value).expect("JSON");
    base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(json)
}

#[test]
fn enroll_request_without_extras_is_version_1() {
    let (ctx, _clock) = common::ctx_at(1_792_000_000);
    // A key with no key ID, so the enrollment request stays version 1.
    let issuer = SecretIssuer::new(
        common::domain(),
        SigningKey::from_seed(common::TEST_SEED, None),
        ctx,
        Duration::from_hours(48),
    );
    let secret = issuer
        .issue(&SecretRequest::enroll(Duration::from_hours(24)))
        .expect("issue");

    assert_eq!(secret.version(), 1);
    assert_eq!(secret.purpose(), SecretPurpose::Enroll);
    let decoded = SignedSecret::decode(&secret.encode(), &DecodeLimits::default()).expect("decode");
    assert_eq!(decoded.version(), 1);
    assert!(decoded.verify_self_signed());
}

#[test]
fn access_secret_is_version_2_and_round_trips() {
    let (ctx, _clock) = common::ctx_at(1_792_000_000);
    let issuer = issuer(ctx.clone());
    let request = SecretRequest::access(Duration::from_hours(8), AUDIENCE)
        .with_max_uses(3)
        .with_subject("client-1");
    let secret = issuer.issue(&request).expect("issue");

    assert_eq!(secret.version(), 2);
    assert_eq!(secret.purpose(), SecretPurpose::Access);
    assert_eq!(secret.audience(), Some(AUDIENCE));
    assert_eq!(secret.max_uses(), Some(3));
    assert_eq!(secret.subject(), Some("client-1"));
    assert_eq!(secret.kid(), Some("2026-10"));

    let decoded = SignedSecret::decode(&secret.encode(), &DecodeLimits::default()).expect("decode");
    assert_eq!(decoded.version(), 2);
    assert_eq!(decoded.audience(), Some(AUDIENCE));
    assert_eq!(decoded.max_uses(), Some(3));
    assert_eq!(decoded.subject(), Some("client-1"));
    assert!(decoded.verify_self_signed());

    let verifier = SecretVerifier::new(
        common::domain(),
        vec![issuer.signing_key().verifying_key().clone()],
        ctx,
        VerifyPolicy::access(AUDIENCE),
    );
    let checked = verifier.verify(&secret.encode()).expect("verifies");
    assert_eq!(checked.subject.as_deref(), Some("client-1"));
    assert_eq!(checked.max_uses, Some(3));
}

#[test]
fn each_changed_signed_field_breaks_the_signature() {
    let (ctx, _clock) = common::ctx_at(1_792_000_000);
    let issuer = issuer(ctx);
    let request = SecretRequest::access(Duration::from_hours(8), AUDIENCE)
        .with_max_uses(3)
        .with_subject("client-1");
    let secret = issuer.issue(&request).expect("issue");
    let original = encoded_value(&secret);
    assert!(secret.verify_self_signed());

    let changes: Vec<(&str, serde_json::Value)> = vec![
        ("kid", serde_json::json!("2026-11")),
        ("purpose", serde_json::json!("enroll")),
        ("aud", serde_json::json!("https://other.example.com/rpc")),
        ("max_uses", serde_json::json!(5)),
        ("sub", serde_json::json!("client-2")),
        ("valid_from", serde_json::json!(1_792_000_001_u64)),
        ("valid_until", serde_json::json!(1_792_000_002_u64)),
        (
            "public_key",
            serde_json::json!(flip_first_char(
                original["public_key"].as_str().expect("public_key")
            )),
        ),
        (
            "secret",
            serde_json::json!(flip_first_char(
                original["secret"].as_str().expect("secret")
            )),
        ),
    ];

    for (field, value) in changes {
        let mut mutated = original.clone();
        mutated[field] = value;
        let decoded = SignedSecret::decode(&reencode(&mutated), &DecodeLimits::default())
            .unwrap_or_else(|error| panic!("field {field}: the change must still decode: {error}"));
        assert!(
            !decoded.verify_self_signed_for(SecretDomain::AGENT_RPC),
            "a changed {field} must break the signature"
        );
    }
}

/// Change the first character of a base64url string to another valid one.
fn flip_first_char(text: &str) -> String {
    let mut chars: Vec<char> = text.chars().collect();
    chars[0] = if chars[0] == 'A' { 'B' } else { 'A' };
    chars.into_iter().collect()
}
