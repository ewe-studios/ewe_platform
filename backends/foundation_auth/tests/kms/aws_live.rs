//! The AWS KMS backend against the real service (spec 92 feature 04).
//!
//! The tests run only when `AWS_KMS_KEY_ARN` and `AWS_WRONG_KEY_ARN` are set.
//! Without them each test prints the missing variable and returns: a skip is
//! not a pass.
//!
//! The credentials come from the AWS command-line interface (CLI) default
//! chain, so `AWS_ACCESS_KEY_ID` and `AWS_SECRET_ACCESS_KEY` (or a role) must
//! be in the environment. The region is `AWS_REGION`, then
//! `AWS_DEFAULT_REGION`, then `us-east-1`.
//!
//! `foundation-ci live-kms` creates the two keys, exports the variables and
//! removes the keys again.

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
const KEK_ID: &str = "aws-kms-live";

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

/// The two key ARNs, or `None` after printing the missing names.
fn live_keys() -> Option<(String, String)> {
    let key = required("AWS_KMS_KEY_ARN");
    let wrong = required("AWS_WRONG_KEY_ARN");
    Some((key?, wrong?))
}

/// A sealer over the real AWS KMS, for `key_arn`.
fn sealer(ctx: &Ctx, env: &Env, key_arn: &str) -> EnvelopeSealer {
    let region = std::env::var("AWS_REGION")
        .or_else(|_| std::env::var("AWS_DEFAULT_REGION"))
        .unwrap_or_else(|_| "us-east-1".to_string());
    let backend = build_backend(
        &KmsConfig::AwsKms {
            region,
            key_id: key_arn.to_string(),
        },
        HttpClientBuilder::new().build(),
        ctx.clock(),
    )
    .expect("build the AWS KMS backend");
    EnvelopeSealer::new(KEK_ID, backend, env.clone())
}

#[valtron_test(threads = 4, timeout = 600_000)]
async fn aws_live_seal_and_open() {
    let Some((key_arn, _wrong_arn)) = live_keys() else {
        return;
    };
    let (ctx, env) = test_ctx();
    let cipher = sealer(&ctx, &env, &key_arn);

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
async fn aws_live_wrong_key() {
    let Some((key_arn, wrong_arn)) = live_keys() else {
        return;
    };
    let (ctx, env) = test_ctx();
    let right = sealer(&ctx, &env, &key_arn);
    let wrong = Arc::new(sealer(&ctx, &env, &wrong_arn));

    let aad = b"record:ca:1";
    let sealed = right
        .seal(aad, b"a private key in PKCS#8 DER")
        .await
        .expect("seal");

    match wrong.open(aad, &sealed).await {
        Ok(_) => panic!("the wrong AWS KMS key opened the blob"),
        // A storage error is the KMS refusal. Any other error would mean the
        // sealer refused the blob before the KMS call, which proves nothing.
        Err(KeyStoreError::Storage(message)) => {
            eprintln!("the wrong AWS KMS key was refused: {message}");
        }
        Err(other) => panic!("the wrong key was refused before the KMS call: {other}"),
    }
}
