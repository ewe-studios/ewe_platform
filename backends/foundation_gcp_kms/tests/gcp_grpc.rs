//! The Google Cloud KMS gRPC backend against localgcp, a local Cloud emulator.
//!
//! The tests run only when `GCP_KMS_ENDPOINT` names the emulator (for example
//! `http://127.0.0.1:8091`) and `GCP_KMS_PROTOCOL=grpc`. Without them each
//! test prints the missing value and returns: a skip is not a pass.
//!
//! `foundation-ci services localgcp --keep-running` starts the emulator, and
//! `foundation-ci services localgcp-stop` stops it. The emulator accepts any
//! token, so the tests configure the token directly and use no secret.

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

use foundation_auth::keystore::{
    EnvelopeSealer, KeySealer, KeyStoreError, KmsBackend as _, SecretRef,
};
use foundation_compact::crypto::SecureBytes;
use foundation_context::Ctx;
use foundation_core::env::Env;
use foundation_core::valtron::valtron_test;
use foundation_gcp_kms::proto::google::cloud::kms::v1::{
    crypto_key::CryptoKeyPurpose, crypto_key_version::CryptoKeyVersionState, CryptoKey,
    GetCryptoKeyRequest,
};
use foundation_gcp_kms::{GcpKmsGrpcBackend, GcpKmsGrpcConfig};

/// The key encryption key (KEK) ID. It is the same on both sealers on
/// purpose: the wrong-key test must reach the KMS, not stop at the sealer's
/// own key-ID check.
const KEK_ID: &str = "gcp-kms-grpc";

/// The project and the location that the emulator ignores. They must be
/// present, because the backend refuses a configuration without them.
const PROJECT: &str = "localgcp-project";
const LOCATION: &str = "us-central1";

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

/// The emulator endpoint, when both variables select the gRPC protocol.
fn endpoint() -> Option<String> {
    let endpoint = required("GCP_KMS_ENDPOINT")?;
    match std::env::var("GCP_KMS_PROTOCOL") {
        Ok(protocol) if protocol.eq_ignore_ascii_case("grpc") => Some(endpoint),
        _ => {
            eprintln!("skip: set GCP_KMS_PROTOCOL=grpc");
            None
        }
    }
}

/// A host context with the real effect sources installed.
fn test_ctx() -> (Ctx, Env) {
    let ctx = Ctx::host();
    let env = foundation_core::env::install(&ctx, foundation_core::env::system)
        .expect("install the effect sources");
    (ctx, env)
}

/// A name that no earlier run used. The emulator keeps its data while it
/// runs, so a repeated run must not collide with an old key ring.
fn unique(prefix: &str) -> String {
    static NEXT: AtomicU64 = AtomicU64::new(0);
    format!(
        "{prefix}-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    )
}

/// A backend for the key `key_id` in `key_ring`.
fn backend_for(endpoint: &str, key_ring: &str, key_id: &str) -> GcpKmsGrpcBackend {
    let config = GcpKmsGrpcConfig::new(endpoint)
        .with_project(PROJECT)
        .with_location(LOCATION)
        .with_key_ring(key_ring)
        // The emulator accepts any token, and the transport is cleartext.
        .with_token(SecretRef::Inline(SecureBytes::new(b"localgcp".to_vec())));
    let key_name = config
        .crypto_key_name(key_id)
        .expect("the full key resource name");
    GcpKmsGrpcBackend::new(key_name, config).expect("the gRPC backend")
}

/// A backend for a new key ring `key_ring` and its new key `key_id`, after
/// creating both.
async fn create_key(endpoint: &str, key_ring: &str, key_id: &str) -> GcpKmsGrpcBackend {
    let backend = backend_for(endpoint, key_ring, key_id);
    backend
        .create_key_ring(key_ring)
        .await
        .expect("create the key ring");
    backend.create_key(key_id).await.expect("create the key");
    backend
}

/// The version name of the primary key version.
fn primary_version(key: &CryptoKey) -> String {
    key.primary
        .as_option()
        .map(|version| version.name.clone())
        .expect("the key has a primary version")
}

#[valtron_test(threads = 4, timeout = 600_000)]
async fn gcp_grpc_seal_and_open() {
    let Some(endpoint) = endpoint() else {
        return;
    };
    let (_ctx, env) = test_ctx();
    let backend = create_key(&endpoint, &unique("ring"), &unique("key")).await;
    let cipher = EnvelopeSealer::new(KEK_ID, Arc::new(backend), env);

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
async fn gcp_grpc_refuses_a_wrong_key() {
    let Some(endpoint) = endpoint() else {
        return;
    };
    let (_ctx, env) = test_ctx();
    let ring = unique("ring");
    let right_id = unique("key");
    let wrong_id = unique("key");
    let right = backend_for(&endpoint, &ring, &right_id);
    right
        .create_key_ring(&ring)
        .await
        .expect("create the key ring");
    right
        .create_key(&right_id)
        .await
        .expect("create the right key");
    let wrong = backend_for(&endpoint, &ring, &wrong_id);
    wrong
        .create_key(&wrong_id)
        .await
        .expect("create the wrong key");
    let right = EnvelopeSealer::new(KEK_ID, Arc::new(right), env.clone());
    let wrong = EnvelopeSealer::new(KEK_ID, Arc::new(wrong), env);

    let aad = b"record:ca:1";
    let sealed = right
        .seal(aad, b"a private key in PKCS#8 DER")
        .await
        .expect("seal");

    match wrong.open(aad, &sealed).await {
        Ok(_) => panic!("the wrong Cloud KMS key opened the blob"),
        // The emulator does not authenticate Decrypt: it returns the
        // ciphertext XORed with the wrong key. The sealer's own
        // ChaCha20-Poly1305 tag then refuses the plaintext, which still proves
        // that the wrong key cannot open the record. The real service refuses
        // the call with a storage error instead.
        Err(error) => eprintln!("the wrong Cloud KMS key was refused: {error}"),
    }
}

#[valtron_test(threads = 4, timeout = 600_000)]
async fn gcp_grpc_create_and_destroy() {
    let Some(endpoint) = endpoint() else {
        return;
    };
    let (_ctx, env) = test_ctx();
    let backend = Arc::new(create_key(&endpoint, &unique("ring"), &unique("key")).await);
    let key_name = backend.key_id().to_string();

    // The key exists and its primary version is enabled.
    let described = backend
        .describe_key(&key_name)
        .await
        .expect("describe the key");
    assert_eq!(described.name, key_name);
    assert_eq!(
        described.purpose,
        buffa::EnumValue::Known(CryptoKeyPurpose::ENCRYPT_DECRYPT)
    );
    let version_name = primary_version(&described);
    let version = backend
        .describe_key_version(&version_name)
        .await
        .expect("describe the key version");
    assert_eq!(
        version.state,
        buffa::EnumValue::Known(CryptoKeyVersionState::ENABLED)
    );

    // The key seals before its version is destroyed.
    let kms: Arc<dyn foundation_auth::keystore::KmsBackend> = backend.clone();
    let cipher = EnvelopeSealer::new(KEK_ID, kms, env);
    let sealed = cipher.seal(b"aad", b"secret").await.expect("seal");

    // Destroy the only version: the key material is gone and cannot come back.
    let destroyed = backend
        .destroy_key_version(&version_name)
        .await
        .expect("destroy the key version");
    assert_eq!(
        destroyed.state,
        buffa::EnumValue::Known(CryptoKeyVersionState::DESTROYED)
    );
    let after = backend
        .describe_key_version(&version_name)
        .await
        .expect("describe the destroyed key version");
    assert_eq!(
        after.state,
        buffa::EnumValue::Known(CryptoKeyVersionState::DESTROYED)
    );
    match cipher.open(b"aad", &sealed).await {
        Ok(_) => panic!("the destroyed Cloud KMS key version still opened the blob"),
        Err(error) => eprintln!("the destroyed key version was refused: {error}"),
    }
}

#[valtron_test(threads = 4, timeout = 600_000)]
async fn gcp_grpc_records_methods_without_a_v1_rpc() {
    let Some(endpoint) = endpoint() else {
        return;
    };
    let backend = create_key(&endpoint, &unique("ring"), &unique("key")).await;
    let key_name = backend.key_id().to_string();
    let request = GetCryptoKeyRequest {
        name: key_name.clone(),
        ..Default::default()
    };

    // `GenerateDataKey` belongs to the Amazon Web Services (AWS) KMS API.
    // `google.cloud.kms.v1.KeyManagementService` has no such method, so the
    // emulator answers `unimplemented`. The line below records the answer.
    match backend
        .invoke::<_, CryptoKey>("GenerateDataKey", request.clone())
        .await
    {
        Ok(_) => panic!("localgcp served GenerateDataKey, which Cloud KMS v1 does not define"),
        Err(KeyStoreError::Storage(message)) => {
            assert!(
                message.contains("unimplemented"),
                "unexpected GenerateDataKey answer: {message}"
            );
            eprintln!("localgcp GenerateDataKey: {message}");
        }
        Err(other) => panic!("GenerateDataKey failed before the service: {other}"),
    }

    // `ScheduleKeyDeletion` is also an AWS name. The Cloud KMS v1 method that
    // schedules a deletion is `DeleteCryptoKey`.
    match backend
        .invoke::<_, CryptoKey>("ScheduleKeyDeletion", request)
        .await
    {
        Ok(_) => panic!("localgcp served ScheduleKeyDeletion, which Cloud KMS v1 does not define"),
        Err(KeyStoreError::Storage(message)) => {
            assert!(
                message.contains("unimplemented"),
                "unexpected ScheduleKeyDeletion answer: {message}"
            );
            eprintln!("localgcp ScheduleKeyDeletion: {message}");
        }
        Err(other) => panic!("ScheduleKeyDeletion failed before the service: {other}"),
    }

    // `DeleteCryptoKey` exists in the current v1 proto. This emulator version
    // does not serve it: record whether it does, and test the rest of the
    // calls. A transport or configuration error is still a failure.
    match backend.schedule_key_deletion(&key_name).await {
        Ok(()) => eprintln!("localgcp DeleteCryptoKey: served"),
        Err(KeyStoreError::Storage(message)) => {
            assert!(
                message.contains("unimplemented"),
                "unexpected DeleteCryptoKey answer: {message}"
            );
            eprintln!("localgcp DeleteCryptoKey: {message}");
        }
        Err(other) => panic!("DeleteCryptoKey failed before the service: {other}"),
    }
}
