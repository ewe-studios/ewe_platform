//! A version 1 secret made by the agent RPC branch code still verifies
//! unchanged, and its signed bytes are the stored golden bytes.

use foundation_auth::secret::{
    DecodeLimits, SecretPurpose, SecretVerifier, SigningKey, VerifyPolicy,
};

use super::common;

/// The branch key, made with `openssl genpkey -algorithm ed25519`.
const KEY_PEM: &str = include_str!("../fixtures/test-only-ed25519-key.pem");
/// A version 1 secret that the branch format makes.
const SECRET_V1: &str = include_str!("../fixtures/secret_v1.txt");
/// The exact bytes that the signature covers.
const MESSAGE_HEX: &str = include_str!("../fixtures/secret_v1_message.hex");

#[test]
fn branch_v1_secret_verifies_unchanged() {
    let (ctx, _clock) = common::ctx_at(1_792_000_000 + 100);
    let key = SigningKey::from_pkcs8_pem(KEY_PEM, None).expect("the fixture key loads");

    let secret =
        foundation_auth::secret::SignedSecret::decode(SECRET_V1.trim(), &DecodeLimits::default())
            .expect("the branch secret decodes");

    assert_eq!(secret.version(), 1);
    assert_eq!(secret.purpose(), SecretPurpose::Enroll);
    assert_eq!(secret.audience(), None);
    assert!(secret.verify_self_signed(), "the branch signature verifies");
    assert_eq!(
        common::hex(secret.signed_bytes(common::domain()).as_slice()),
        MESSAGE_HEX.trim(),
        "the signed bytes are the stored golden bytes"
    );

    let verifier = SecretVerifier::new(
        common::domain(),
        vec![key.verifying_key().clone()],
        ctx,
        VerifyPolicy::enroll(),
    );
    let checked = verifier.verify(SECRET_V1.trim()).expect("verifies");
    assert_eq!(checked.purpose, SecretPurpose::Enroll);
    assert_eq!(checked.valid_from, 1_792_000_000);
    assert_eq!(checked.valid_until, 1_792_086_400);
    assert_eq!(checked.id, secret.id());
}
