//! No `Debug` output and no error text holds key bytes.

use std::sync::Arc;

use foundation_auth::keystore::{
    EnvelopeSealer, FakeKms, KekSealer, KeyMaterial, KeyProtection, KeySealer, MemoryKeyStore,
};
use foundation_core::valtron::valtron_test;

use super::common::{passphrase_sealer, test_ctx};

const SECRET: &[u8] = b"a very secret private key payload";

#[valtron_test]
async fn a_kek_sealer_debug_holds_no_key_bytes() {
    let (_ctx, env) = test_ctx();
    let cipher = KekSealer::new("kek-1", [0xAB; 32], env);
    let debug = format!("{cipher:?}");
    assert!(debug.contains("KekSealer"));
    assert!(!debug.contains("q6ur"), "the base64url key is not printed");
    assert!(!debug.to_lowercase().contains("abab"));
}

#[valtron_test]
async fn a_passphrase_sealer_debug_holds_no_passphrase() {
    let (_ctx, env) = test_ctx();
    let cipher = passphrase_sealer(&env);
    let debug = format!("{cipher:?}");
    assert!(debug.contains("PassphraseSealer"));
    assert!(!debug.contains("correct horse"));
}

#[valtron_test]
async fn an_envelope_sealer_debug_holds_no_key_bytes() {
    let (_ctx, env) = test_ctx();
    let cipher = EnvelopeSealer::new(
        "kms-key",
        Arc::new(FakeKms::new("kms-key", [0xCD; 32])),
        env,
    );
    let debug = format!("{cipher:?}");
    assert!(debug.contains("EnvelopeSealer"));
    assert!(!debug.to_lowercase().contains("cdcd"));
}

#[valtron_test]
async fn a_key_protection_debug_names_only_the_kek_id() {
    let (_ctx, env) = test_ctx();
    let protection = KeyProtection::new(Arc::new(KekSealer::new("kek-1", [0xEF; 32], env)));
    let debug = format!("{protection:?}");
    assert!(debug.contains("kek-1"));
    assert!(!debug.to_lowercase().contains("efef"));
}

#[valtron_test]
async fn a_key_material_debug_holds_no_key_bytes() {
    let (ctx, env) = test_ctx();
    let material = KeyMaterial::new(
        Arc::new(MemoryKeyStore::new()),
        KeyProtection::new(Arc::new(KekSealer::new("kek-1", [0x12; 32], env))),
        ctx,
    );
    let debug = format!("{material:?}");
    assert!(debug.contains("KeyMaterial"));
    assert!(!debug.contains("1212"));
}

#[valtron_test]
async fn a_seal_error_holds_no_plaintext() {
    let (_ctx, env) = test_ctx();
    let cipher = KekSealer::new("kek-1", [7; 32], env.clone());
    let mut sealed = cipher.seal(b"aad", SECRET).await.expect("seal");
    sealed.ciphertext_b64url = "AAAA".to_string();
    let error = cipher
        .open(b"aad", &sealed)
        .await
        .expect_err("the blob is bad");
    let text = format!("{error}");
    assert!(!text.contains("secret private key"));
    assert!(text.contains("sealing failed"));
}

#[valtron_test]
async fn a_wrong_passphrase_error_holds_no_plaintext() {
    let (_ctx, env) = test_ctx();
    let one = KekSealer::new("kek-1", [1; 32], env.clone());
    let other = KekSealer::new("kek-1", [2; 32], env);
    let sealed = one.seal(b"aad", SECRET).await.expect("seal");
    let error = other
        .open(b"aad", &sealed)
        .await
        .expect_err("the key is wrong");
    assert!(!format!("{error}").contains("secret private key"));
}
