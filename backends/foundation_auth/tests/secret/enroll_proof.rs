//! The enrollment server proof (spec 81 `auth.md` section 9.1.1).

use foundation_auth::secret::{
    enroll_proof_message, SecretDomain, SecretIssuer, SigningKey, ENROLL_PROOF_DOMAIN,
};
use foundation_compact::Duration;

use super::common;

fn issuer() -> SecretIssuer {
    let (ctx, _clock) = common::ctx_at(1_792_000_000);
    SecretIssuer::new(
        SecretDomain::PROXY_REGISTRATION,
        SigningKey::from_seed(common::TEST_SEED, Some("2026-10".to_string())),
        ctx,
        Duration::from_hours(48),
    )
}

const SECRET_ID: &str = "5f0d1c9a8b7e6d5c";
const CLIENT_ID: &str = "host-17";
const NONCE: [u8; 4] = [0xde, 0xad, 0xbe, 0xef];
const CLIENT_KEY: [u8; 3] = [0x01, 0x02, 0x03];

#[test]
fn the_token_key_checks_the_server_proof() {
    let issuer = issuer();
    let token = issuer
        .issue(&foundation_auth::secret::SecretRequest::enroll(
            Duration::from_hours(48),
        ))
        .expect("issue the enrollment secret");
    let proof = issuer
        .sign_enroll_proof(SECRET_ID, &NONCE, &CLIENT_KEY, CLIENT_ID)
        .expect("sign the server proof");
    assert!(
        token
            .verifying_key()
            .verify_enroll_proof(SECRET_ID, &NONCE, &CLIENT_KEY, CLIENT_ID, &proof)
            .expect("verify"),
        "the token's public key must accept its server proof"
    );
}

#[test]
fn each_changed_field_breaks_the_proof() {
    let issuer = issuer();
    let proof = issuer
        .sign_enroll_proof(SECRET_ID, &NONCE, &CLIENT_KEY, CLIENT_ID)
        .expect("sign the server proof");
    let key = issuer.signing_key().verifying_key().clone();
    let changed = [
        (
            "secret_id",
            key.verify_enroll_proof("another-id", &NONCE, &CLIENT_KEY, CLIENT_ID, &proof),
        ),
        (
            "nonce",
            key.verify_enroll_proof(SECRET_ID, &[0; 4], &CLIENT_KEY, CLIENT_ID, &proof),
        ),
        (
            "client_key",
            key.verify_enroll_proof(SECRET_ID, &NONCE, &[9], CLIENT_ID, &proof),
        ),
        (
            "client_id",
            key.verify_enroll_proof(SECRET_ID, &NONCE, &CLIENT_KEY, "host-18", &proof),
        ),
    ];
    for (field, result) in changed {
        assert!(
            !result.expect("verify"),
            "a changed {field} must break the proof"
        );
    }
}

#[test]
fn a_proof_from_another_key_fails() {
    let issuer = issuer();
    let proof = issuer
        .sign_enroll_proof(SECRET_ID, &NONCE, &CLIENT_KEY, CLIENT_ID)
        .expect("sign the server proof");
    let other = SigningKey::from_seed([0x11; 32], None)
        .verifying_key()
        .clone();
    assert!(
        !other
            .verify_enroll_proof(SECRET_ID, &NONCE, &CLIENT_KEY, CLIENT_ID, &proof)
            .expect("verify"),
        "another key must not accept the proof"
    );
}

#[test]
fn the_message_has_the_designed_layout() {
    let message = enroll_proof_message("abc", &[0xde, 0xad], &[0x01], "cid");
    let mut expected = ENROLL_PROOF_DOMAIN.as_bytes().to_vec();
    expected.push(0);
    expected.extend_from_slice(&3u32.to_be_bytes());
    expected.extend_from_slice(b"abc");
    expected.extend_from_slice(&2u32.to_be_bytes());
    expected.extend_from_slice(&[0xde, 0xad]);
    expected.extend_from_slice(&1u32.to_be_bytes());
    expected.extend_from_slice(&[0x01]);
    expected.extend_from_slice(&3u32.to_be_bytes());
    expected.extend_from_slice(b"cid");
    assert_eq!(message, expected);
    assert_eq!(ENROLL_PROOF_DOMAIN, "ewe.agent_rpc.enroll_proof.v1");
}
