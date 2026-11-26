//! The sealer contract: `open(seal(x))` is `x`; a wrong input fails.

use base64::Engine as _;
use foundation_auth::keystore::{record_aad, KeyId, KeyKind, KeySealer, PassphraseSealer};
use foundation_core::valtron::valtron_test;

use super::common::{kek_sealer, passphrase_sealer, test_ctx};

async fn check_sealer(cipher: &dyn KeySealer) {
    let aad = b"record:ca:1";
    let plaintext = b"a private key in PKCS#8 DER";

    let sealed = cipher.seal(aad, plaintext).await.expect("seal");
    let opened = cipher.open(aad, &sealed).await.expect("open");
    assert_eq!(opened.as_slice(), plaintext);

    // A different associated data fails: the blob is bound to its record.
    assert!(cipher.open(b"record:ca:2", &sealed).await.is_err());

    // A changed ciphertext byte fails.
    let engine = base64::engine::general_purpose::URL_SAFE_NO_PAD;
    let mut tampered = sealed.clone();
    let mut bytes = engine.decode(&tampered.ciphertext_b64url).expect("base64");
    bytes[0] ^= 1;
    tampered.ciphertext_b64url = engine.encode(&bytes);
    assert!(cipher.open(aad, &tampered).await.is_err());
}

#[valtron_test]
async fn kek_sealer_follows_the_contract() {
    let (_ctx, env) = test_ctx();
    check_sealer(&kek_sealer(&env)).await;
}

#[valtron_test]
async fn a_kek_sealer_with_another_key_fails() {
    let (_ctx, env) = test_ctx();
    let one = kek_sealer(&env);
    let other = foundation_auth::keystore::KekSealer::new("test-kek", [8; 32], env.clone());
    let sealed = one.seal(b"aad", b"secret").await.expect("seal");
    assert!(other.open(b"aad", &sealed).await.is_err());
}

#[valtron_test]
async fn passphrase_sealer_follows_the_contract() {
    let (_ctx, env) = test_ctx();
    check_sealer(&passphrase_sealer(&env)).await;
}

#[valtron_test]
async fn a_passphrase_sealer_with_another_passphrase_fails() {
    let (_ctx, env) = test_ctx();
    let one = passphrase_sealer(&env);
    let other = PassphraseSealer::new(
        "test-passphrase",
        zeroize::Zeroizing::new("the wrong passphrase".to_string()),
        foundation_auth::keystore::Argon2Params {
            memory_kib: 8 * 1024,
            iterations: 1,
            lanes: 1,
        },
        env.clone(),
    );
    let sealed = one.seal(b"aad", b"secret").await.expect("seal");
    assert!(other.open(b"aad", &sealed).await.is_err());
}

#[test]
fn record_aad_binds_id_kind_and_revision() {
    let ca = KeyId::new("ca").expect("valid id");
    let signing = KeyId::new("secret-signing").expect("valid id");
    let base = record_aad(&ca, KeyKind::CertificateAuthority, 1);
    assert_ne!(base, record_aad(&signing, KeyKind::CertificateAuthority, 1));
    assert_ne!(base, record_aad(&ca, KeyKind::SecretSigning, 1));
    assert_ne!(base, record_aad(&ca, KeyKind::CertificateAuthority, 2));
}
