//! The GCP Cloud KMS backend against the real service (spec 92 feature 04).
//!
//! The tests run only when `GCP_KMS_KEY_NAME` and `GCP_ACCESS_TOKEN` are set;
//! the wrong-key test also needs `GCP_WRONG_KEY_NAME`. Without them each test
//! prints the missing variable and returns: a skip is not a pass.
//! `GCP_ACCESS_TOKEN` is the value of `gcloud auth print-access-token`; it is a
//! secret and never printed.
//!
//! The key name is the full resource name, for example
//! `projects/ewestudiosnigeria/locations/europe-west1/keyRings/ewe-live-test/cryptoKeys/ewe-live-test`.
//! `foundation-ci live-kms` creates the key ring and the keys, exports the
//! variables and destroys the key versions afterwards.

use std::sync::Arc;

use foundation_auth::keystore::{
    build_backend, EnvelopeSealer, KeySealer, KeyStoreError, KmsConfig,
};
use foundation_context::Ctx;
use foundation_core::env::Env;
use foundation_core::valtron::valtron_test;
use foundation_netio::HttpClientBuilder;

use super::common::test_ctx;

/// The KEK ID of both sealers. It is the same on purpose: the "wrong key" test
/// must reach the KMS, not stop at the sealer's own key-ID check.
const KEK_ID: &str = "gcp-kms-live";

/// A value from the environment, or a printed skip line.
fn required(name: &str) -> Option<String> {
    match std::env::var(name) {
        Ok(value) if !value.trim().is_empty() => Some(value),
        _ => {
            eprintln!("skip: set {name}");
            None
        }
    }
}

/// The right key name, or `None` after printing the missing names.
fn live_key() -> Option<String> {
    let key = required("GCP_KMS_KEY_NAME");
    // The backend reads the token itself. The test checks that it is present,
    // so a missing token is a printed skip and not a metadata-server timeout.
    required("GCP_ACCESS_TOKEN")?;
    key
}

/// The right and the wrong key names, or `None` after printing the missing
/// names.
fn live_keys() -> Option<(String, String)> {
    let key = live_key()?;
    let wrong = required("GCP_WRONG_KEY_NAME")?;
    Some((key, wrong))
}

/// A sealer over the real GCP Cloud KMS, for `key_name`.
fn sealer(ctx: &Ctx, env: &Env, key_name: &str) -> EnvelopeSealer {
    let backend = build_backend(
        &KmsConfig::GcpKms {
            key_name: key_name.to_string(),
        },
        HttpClientBuilder::new().build(),
        ctx.clock(),
    )
    .expect("build the GCP KMS backend");
    EnvelopeSealer::new(KEK_ID, backend, env.clone())
}

#[valtron_test(threads = 4, timeout = 600_000)]
async fn gcp_live_seal_and_open() {
    let Some(key_name) = live_key() else {
        return;
    };
    let (ctx, env) = test_ctx();
    let cipher = sealer(&ctx, &env, &key_name);

    let aad = b"record:ca:1";
    let secret = b"a private key in PKCS#8 DER";
    let sealed = cipher.seal(aad, secret).await.expect("seal");
    assert_eq!(sealed.alg, "kms-envelope+chacha20poly1305");
    let opened = cipher.open(aad, &sealed).await.expect("open");
    assert_eq!(opened.as_slice(), secret);

    // Another record cannot open the blob: the associated data binds it.
    assert!(cipher.open(b"record:ca:2", &sealed).await.is_err());
}

#[valtron_test(threads = 4, timeout = 600_000)]
async fn gcp_live_wrong_key() {
    let Some((key_name, wrong_name)) = live_keys() else {
        return;
    };
    let (ctx, env) = test_ctx();
    let right = sealer(&ctx, &env, &key_name);
    let wrong = Arc::new(sealer(&ctx, &env, &wrong_name));

    let aad = b"record:ca:1";
    let sealed = right
        .seal(aad, b"a private key in PKCS#8 DER")
        .await
        .expect("seal");

    match wrong.open(aad, &sealed).await {
        Ok(_) => panic!("the wrong GCP KMS key opened the blob"),
        // A storage error is the KMS refusal. Any other error would mean the
        // sealer refused the blob before the KMS call, which proves nothing.
        Err(KeyStoreError::Storage(message)) => {
            eprintln!("the wrong GCP KMS key was refused: {message}");
        }
        Err(other) => panic!("the wrong key was refused before the KMS call: {other}"),
    }
}
