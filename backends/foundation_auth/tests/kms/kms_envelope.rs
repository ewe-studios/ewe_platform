//! The sealer contract for envelope encryption, with a fake key management
//! service (KMS).

use std::sync::Arc;

use foundation_auth::keystore::{EnvelopeSealer, FakeKms, KeySealer};
use foundation_core::valtron::valtron_test;

use super::common::test_ctx;

fn sealer(env: &foundation_core::env::Env, master: u8) -> EnvelopeSealer {
    EnvelopeSealer::new(
        "kms-test",
        Arc::new(FakeKms::new("kms-test", [master; 32])),
        env.clone(),
    )
}

#[valtron_test]
async fn envelope_sealer_round_trips() {
    let (_ctx, env) = test_ctx();
    let cipher = sealer(&env, 9);
    let aad = b"record:ca:1";
    let secret = b"a private key in PKCS#8 DER";
    let sealed = cipher.seal(aad, secret).await.expect("seal");
    assert_eq!(sealed.alg, "kms-envelope+chacha20poly1305");
    assert!(sealed.wrapped_dek_b64url.is_some());
    let opened = cipher.open(aad, &sealed).await.expect("open");
    assert_eq!(opened.as_slice(), secret);
}

#[valtron_test]
async fn envelope_sealer_refuses_another_context() {
    let (_ctx, env) = test_ctx();
    let cipher = sealer(&env, 9);
    let sealed = cipher.seal(b"record:ca:1", b"secret").await.expect("seal");
    assert!(cipher.open(b"record:ca:2", &sealed).await.is_err());
}

#[valtron_test]
async fn envelope_sealer_refuses_another_master_key() {
    let (_ctx, env) = test_ctx();
    let one = sealer(&env, 1);
    let other = sealer(&env, 2);
    let sealed = one.seal(b"aad", b"secret").await.expect("seal");
    assert!(other.open(b"aad", &sealed).await.is_err());
}
