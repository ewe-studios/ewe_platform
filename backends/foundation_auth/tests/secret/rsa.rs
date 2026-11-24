//! RSA-PSS with SHA-256 signed secrets (native + feature `secret-rsa`).
//!
//! RSA-PSS signing is randomized. The issuer takes its salt from the entropy
//! source of `Ctx`, so two runs with the same seeded source make the same
//! bytes. These tests prove that, and that the signature verifies.

use base64::Engine;
use foundation_auth::secret::{
    SecretDomain, SecretError, SecretIssuer, SecretRequest, SecretVerifier, SigningAlgorithm,
    SigningKey, VerifyPolicy, VerifyingKey,
};
use foundation_compact::Duration;
use foundation_context::Ctx;
use foundation_core::valtron::valtron_test;

use super::common;

/// The fixture key made with `openssl genpkey -algorithm RSA`.
const RSA_PEM: &str = include_str!("../fixtures/rsa-pkcs8-key.pem");
const AUDIENCE: &str = "https://agents.example.com/rpc";
const T0: u64 = 1_800_000_000;

fn rsa_key() -> SigningKey {
    SigningKey::from_pkcs8_pem(RSA_PEM, Some("rsa-2026".to_string())).expect("the fixture RSA key")
}

fn issuer(ctx: Ctx, key: SigningKey) -> SecretIssuer {
    SecretIssuer::new(SecretDomain::AGENT_RPC, key, ctx, Duration::from_hours(2))
}

fn access_secret(issuer: &SecretIssuer) -> String {
    issuer
        .issue(&SecretRequest::access(Duration::from_hours(1), AUDIENCE))
        .expect("issue")
        .encode()
        .to_string()
}

fn verifier(ctx: Ctx, key: &SigningKey) -> SecretVerifier {
    SecretVerifier::new(
        SecretDomain::AGENT_RPC,
        vec![key.verifying_key().clone()],
        ctx,
        VerifyPolicy::access(AUDIENCE),
    )
}

#[test]
fn rsa_key_loads_and_round_trips() {
    let key = rsa_key();
    assert_eq!(key.algorithm(), SigningAlgorithm::RsaPssSha256);
    assert_eq!(
        key.verifying_key().algorithm(),
        SigningAlgorithm::RsaPssSha256
    );
    // The public half is DER `RSAPublicKey`, which starts with a SEQUENCE.
    assert_eq!(key.verifying_key().as_bytes().first(), Some(&0x30));
    assert_eq!(key.verifying_key().kid(), Some("rsa-2026"));

    let pem = key.to_pkcs8_pem();
    let again = SigningKey::from_pkcs8_pem(&pem, Some("rsa-2026".to_string()))
        .expect("the PEM round-trips");
    assert_eq!(
        again.verifying_key().fingerprint(),
        key.verifying_key().fingerprint()
    );
}

#[valtron_test]
async fn rsa_secret_signs_and_verifies() {
    let (ctx, _clock) = common::ctx_at(T0);
    let key = rsa_key();
    let issuer = issuer(ctx.clone(), key);
    let encoded = access_secret(&issuer);
    let secret = foundation_auth::secret::SignedSecret::decode(
        &encoded,
        &foundation_auth::secret::DecodeLimits::default(),
    )
    .expect("decode");
    assert_eq!(secret.algorithm(), SigningAlgorithm::RsaPssSha256);
    assert_eq!(secret.kid(), Some("rsa-2026"));
    assert!(secret.verify_self_signed_for(SecretDomain::AGENT_RPC));

    let verifier = verifier(ctx, issuer.signing_key());
    let verified = verifier.verify(&encoded).expect("the RSA secret verifies");
    assert_eq!(verified.id, secret.id());
}

#[valtron_test]
async fn rsa_signing_draws_from_the_injected_entropy() {
    // Same seed, same key, same request: the same bytes, including the
    // randomized RSA-PSS signature. A system random source would differ.
    let (ctx_a, _) = common::ctx_at(T0);
    let (ctx_b, _) = common::ctx_at(T0);
    let first = access_secret(&issuer(ctx_a, rsa_key()));
    let second = access_secret(&issuer(ctx_b, rsa_key()));
    assert_eq!(first, second, "the seeded entropy gives the same run");

    // A different seed gives a different signature.
    let (ctx_c, _) = common::ctx_at(T0 + 1);
    let other = access_secret(&issuer(ctx_c, rsa_key()));
    assert_ne!(first, other);
}

#[valtron_test]
async fn rsa_secret_with_an_ed25519_verifier_has_an_unknown_key() {
    let (ctx, _clock) = common::ctx_at(T0);
    let issuer = issuer(ctx.clone(), rsa_key());
    let encoded = access_secret(&issuer);

    let ed25519 = SigningKey::from_seed(common::TEST_SEED, None);
    let verifier = SecretVerifier::new(
        SecretDomain::AGENT_RPC,
        vec![ed25519.verifying_key().clone()],
        ctx,
        VerifyPolicy::access(AUDIENCE),
    );
    assert!(matches!(
        verifier.verify(&encoded),
        Err(SecretError::UnknownKey)
    ));
}

#[valtron_test]
async fn rsa_secret_with_a_changed_signature_is_refused() {
    let (ctx, _clock) = common::ctx_at(T0);
    let key = rsa_key();
    let issuer = issuer(ctx.clone(), key);
    let encoded = access_secret(&issuer);

    let b64 = base64::engine::general_purpose::URL_SAFE_NO_PAD;
    let json = b64.decode(&encoded).expect("base64url");
    let mut value: serde_json::Value = serde_json::from_slice(&json).expect("JSON");
    // A signature of the right length, but not the one that the key made.
    value["signature"] = serde_json::Value::String(b64.encode([0u8; 256]));
    let tampered = b64.encode(serde_json::to_vec(&value).expect("JSON"));

    let verifier = verifier(ctx, issuer.signing_key());
    assert!(matches!(
        verifier.verify(&tampered),
        Err(SecretError::BadSignature)
    ));
}

#[test]
fn rsa_public_key_loads_from_the_json_key_list() {
    let key = rsa_key();
    let b64 =
        base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(key.verifying_key().as_bytes());
    let json = format!(r#"[{{"kid":"rsa-2026","alg":"rsa-pss-sha256","public_key":"{b64}"}}]"#);
    let keys = VerifyingKey::list_from_json(&json).expect("the key list loads");
    assert_eq!(keys.len(), 1);
    assert_eq!(keys[0].algorithm(), SigningAlgorithm::RsaPssSha256);
    assert_eq!(keys[0].kid(), Some("rsa-2026"));
}

#[test]
fn an_rsa_key_below_2048_bits_is_refused() {
    // A 1024-bit key is outside the designed 2048 to 8192 bits.
    const SMALL: &str = include_str!("../fixtures/rsa-1024-pkcs8-key.pem");
    assert!(matches!(
        SigningKey::from_pkcs8_pem(SMALL, None),
        Err(SecretError::BadKey(_))
    ));
}
