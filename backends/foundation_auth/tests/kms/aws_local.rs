//! The AWS KMS backend against a local AWS KMS emulator.
//!
//! The test runs only when `AWS_ENDPOINT_URL_KMS` names a running emulator,
//! for example the moto server that `mise run kms:moto` starts. It uses the
//! dummy credentials `AWS_ACCESS_KEY_ID=test` and `AWS_SECRET_ACCESS_KEY=test`.
//!
//! The test fails when the emulator is reachable but an operation is wrong. A
//! skip happens only when the endpoint variable is absent. The setup sends
//! `CreateKey` and `ScheduleKeyDeletion` itself, because the backend has no
//! key-management call.

use std::sync::Arc;

use foundation_auth::keystore::kms::AwsKmsBackend;
use foundation_auth::keystore::{EnvelopeSealer, KeySealer};
use foundation_core::valtron::{block_on_future, valtron_test};
use foundation_netio::{DynNetClient, HttpClientBuilder, PreparedRequestBuilder};
use sha2::Digest as _;

use crate::common::test_ctx;

/// The region of the emulator. moto uses the credential scope of the request.
const REGION: &str = "us-east-1";

/// A key that the cleanup must schedule for deletion.
///
/// The drop runs on a separate thread, so the deletion happens after the test
/// body, including when an assertion panics.
struct KeyCleanup {
    endpoint: String,
    http: DynNetClient,
    key_ids: Vec<String>,
}

impl Drop for KeyCleanup {
    fn drop(&mut self) {
        let endpoint = std::mem::take(&mut self.endpoint);
        let http = self.http.clone();
        let key_ids = std::mem::take(&mut self.key_ids);
        if key_ids.is_empty() {
            return;
        }
        // The blocking wait owns a dedicated thread. It must not nest a
        // runtime on the test thread, and it must not stop on a panic.
        let handle = std::thread::spawn(move || {
            for key_id in key_ids {
                let body = serde_json::json!({
                    "KeyId": key_id,
                    "PendingWindowInDays": 7,
                })
                .to_string();
                let endpoint = endpoint.clone();
                let http = http.clone();
                let outcome = block_on_future(async move {
                    kms_request(&http, &endpoint, "TrentService.ScheduleKeyDeletion", &body).await
                });
                if let Err(error) = outcome {
                    eprintln!("cleanup: schedule deletion of {key_id}: {error}");
                }
            }
        });
        let _ = handle.join();
    }
}

/// Lowercase hexadecimal text.
fn hex_lower(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

/// The `SigV4` signature of a string-to-sign.
fn signature(secret_key: &str, date: &str, string_to_sign: &str) -> String {
    use hmac::{Hmac, KeyInit as _, Mac as _};
    use sha2::Sha256;

    type HmacSha256 = Hmac<Sha256>;
    let hmac = |key: &[u8], data: &[u8]| -> Vec<u8> {
        let mut mac = HmacSha256::new_from_slice(key).expect("HMAC takes any key length");
        mac.update(data);
        mac.finalize().into_bytes().to_vec()
    };
    let k_date = hmac(format!("AWS4{secret_key}").as_bytes(), date.as_bytes());
    let k_region = hmac(&k_date, REGION.as_bytes());
    let k_service = hmac(&k_region, b"kms");
    let k_signing = hmac(&k_service, b"aws4_request");
    hex_lower(&hmac(&k_signing, string_to_sign.as_bytes()))
}

/// Send one signed AWS KMS JSON request to the emulator.
async fn kms_request(
    http: &DynNetClient,
    endpoint: &str,
    target: &str,
    body: &str,
) -> Result<String, String> {
    let access_key = std::env::var("AWS_ACCESS_KEY_ID")
        .map_err(|_| "AWS_ACCESS_KEY_ID is not set for the local KMS test".to_string())?;
    let secret_key = std::env::var("AWS_SECRET_ACCESS_KEY")
        .map_err(|_| "AWS_SECRET_ACCESS_KEY is not set for the local KMS test".to_string())?;
    let base = endpoint.trim_end_matches('/');
    let uri = foundation_core::url::Uri::parse(base).map_err(|e| e.to_string())?;
    let host_name = uri
        .host_str()
        .ok_or_else(|| "the AWS KMS endpoint has no host".to_string())?;
    let host = match uri.port() {
        Some(port) => format!("{host_name}:{port}"),
        None => host_name,
    };
    let now = chrono::Utc::now();
    let amz_date = now.format("%Y%m%dT%H%M%SZ").to_string();
    let date = now.format("%Y%m%d").to_string();
    let content_type = "application/x-amz-json-1.1";
    let signed_headers = "content-type;host;x-amz-date;x-amz-target";
    let canonical_headers = format!(
        "content-type:{content_type}\nhost:{host}\nx-amz-date:{amz_date}\nx-amz-target:{target}\n"
    );
    let payload_hash = hex_lower(&sha2::Sha256::digest(body.as_bytes()));
    let canonical_request =
        format!("POST\n/\n\n{canonical_headers}\n{signed_headers}\n{payload_hash}");
    let scope = format!("{date}/{REGION}/kms/aws4_request");
    let string_to_sign = format!(
        "AWS4-HMAC-SHA256\n{amz_date}\n{scope}\n{}",
        hex_lower(&sha2::Sha256::digest(canonical_request.as_bytes()))
    );
    let signature = signature(&secret_key, &date, &string_to_sign);
    let authorization = format!(
        "AWS4-HMAC-SHA256 Credential={access_key}/{scope}, SignedHeaders={signed_headers}, Signature={signature}"
    );
    let request = PreparedRequestBuilder::post(&format!("{base}/"))
        .map_err(|e| e.to_string())?
        .header("Content-Type".to_string(), content_type)
        .header("X-Amz-Date".to_string(), amz_date)
        .header("X-Amz-Target".to_string(), target.to_string())
        .header("Authorization".to_string(), authorization)
        .body_text(body);
    let response = http
        .send_async(request.build())
        .await
        .map_err(|e| e.to_string())?;
    let status: usize = response.get_status().into();
    let text = foundation_auth::shared::body::read_body_text(response.take_async_body()).await?;
    if !(200..=299).contains(&status) {
        return Err(format!("KMS {target} answered HTTP {status}: {text}"));
    }
    Ok(text)
}

/// Create a symmetric key and return its identifier.
async fn create_key(http: &DynNetClient, endpoint: &str) -> Result<String, String> {
    let body = serde_json::json!({
        "Description": "ewe foundation_auth local KMS test",
    })
    .to_string();
    let response = kms_request(http, endpoint, "TrentService.CreateKey", &body).await?;
    let value: serde_json::Value = serde_json::from_str(&response).map_err(|e| e.to_string())?;
    value
        .get("KeyMetadata")
        .and_then(|metadata| metadata.get("KeyId"))
        .and_then(serde_json::Value::as_str)
        .map(str::to_string)
        .ok_or_else(|| format!("the CreateKey answer has no key id: {response}"))
}

#[valtron_test(threads = 4, timeout = 120_000)]
async fn aws_kms_local_seals_opens_and_refuses_a_wrong_key() {
    let Ok(endpoint) = std::env::var("AWS_ENDPOINT_URL_KMS") else {
        println!("skip: set AWS_ENDPOINT_URL_KMS to a moto or LocalStack endpoint");
        return;
    };
    let endpoint = endpoint.trim().to_string();
    if endpoint.is_empty() {
        println!("skip: set AWS_ENDPOINT_URL_KMS to a moto or LocalStack endpoint");
        return;
    }
    let (ctx, env) = test_ctx();
    let http = HttpClientBuilder::new().build();
    let mut cleanup = KeyCleanup {
        endpoint: endpoint.clone(),
        http: http.clone(),
        key_ids: Vec::new(),
    };

    let key_a = create_key(&http, &endpoint)
        .await
        .expect("create the first key");
    cleanup.key_ids.push(key_a.clone());
    let key_b = create_key(&http, &endpoint)
        .await
        .expect("create the second key");
    cleanup.key_ids.push(key_b.clone());

    let clock = ctx.clock();
    let backend = AwsKmsBackend::new(REGION.to_string(), key_a, http.clone(), clock.clone())
        .with_endpoint(endpoint.clone());
    let sealer = EnvelopeSealer::new("aws-kms-local", Arc::new(backend), env.clone());

    let other = AwsKmsBackend::new(REGION.to_string(), key_b, http.clone(), clock)
        .with_endpoint(endpoint.clone());
    let other = EnvelopeSealer::new("aws-kms-local", Arc::new(other), env);

    let aad = b"record:local-kms:1";
    let secret = b"a private key in PKCS#8 DER";
    let sealed = sealer.seal(aad, secret).await.expect("seal");
    assert_eq!(sealed.alg, "kms-envelope+chacha20poly1305");
    let opened = sealer.open(aad, &sealed).await.expect("open");
    assert_eq!(opened.as_slice(), secret);

    assert!(
        other.open(aad, &sealed).await.is_err(),
        "a backend for another key must not open the blob"
    );
}
