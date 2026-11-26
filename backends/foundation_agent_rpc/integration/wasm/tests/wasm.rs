//! The browser client's payload rules (spec 83 feature 05, sections 6.3 and
//! 6.4). Each case names its oracle in a comment.

use foundation_agent_rpc::client::web::WebClient;
use foundation_agent_rpc::core::bootstrap::encode;
use foundation_agent_rpc::proto::v1 as pb;
use foundation_auth::secret::{SecretDomain, SecretIssuer, SecretRequest, SigningKey};
use foundation_compact::Duration;
use foundation_context::Ctx;
use wasm_bindgen_test::wasm_bindgen_test;

const URL: &str = "https://agents.example.com";

/// A host context with the browser `Env` installed: the issuer reads entropy
/// and time from it.
fn ctx() -> Ctx {
    let ctx = Ctx::host();
    foundation_core::env::install(&ctx, foundation_core::env::system).expect("install the Env");
    ctx
}

fn signed_capabilities(auth_modes: &[&str]) -> pb::Capabilities {
    pb::Capabilities {
        auth_modes: auth_modes.iter().map(|mode| (*mode).to_string()).collect(),
        ..Default::default()
    }
}

/// A payload with an access token for [`URL`].
fn access_payload(capabilities: pb::Capabilities, fingerprint: &str) -> String {
    let key = SigningKey::from_seed([7u8; 32], Some("k1".to_string()));
    let issuer = SecretIssuer::new(
        SecretDomain::AGENT_RPC,
        key,
        ctx(),
        Duration::from_hours(24),
    );
    let secret = issuer
        .issue(&SecretRequest::access(Duration::from_hours(1), URL))
        .expect("issue an access secret");
    let payload = pb::BootstrapPayload {
        url: URL.to_string(),
        tls_fingerprint_sha256: fingerprint.to_string(),
        token: secret.encode().to_string(),
        capabilities: Some(capabilities).into(),
        ..Default::default()
    };
    encode(&payload)
}

#[wasm_bindgen_test]
fn an_operator_access_payload_builds_a_browser_client() {
    // Oracle: the payload rules accept system roots and the signed-secret
    // mode, and the constructor builds a client with the bearer secret.
    let payload = access_payload(signed_capabilities(&["signed-secret"]), "");
    let client = WebClient::from_access_payload(&payload).expect("a valid access payload");
    assert_eq!(client.url().trim_end_matches('/'), URL);
    assert!(client.access_secret_len() > 0, "the bearer secret travels");
}

#[wasm_bindgen_test]
fn a_host_without_access_secrets_is_refused() {
    // Oracle: section 6.4 refuses a host whose auth_modes lack signed-secret.
    let payload = access_payload(signed_capabilities(&["mtls"]), "");
    let error = WebClient::from_access_payload(&payload).expect_err("mutual TLS is not a page");
    assert!(
        error.to_string().contains("access secrets"),
        "the refusal names the missing mode: {error}"
    );
}

#[wasm_bindgen_test]
fn a_pinned_certificate_is_refused() {
    // Oracle: section 6.4 refuses a pinned certificate, because a browser
    // cannot pin one through fetch.
    let payload = access_payload(signed_capabilities(&["signed-secret"]), &"ab".repeat(32));
    let error = WebClient::from_access_payload(&payload).expect_err("a pin is not a page");
    assert!(
        error.to_string().contains("pin"),
        "the refusal names the pin: {error}"
    );
}

#[wasm_bindgen_test]
fn an_enrollment_payload_is_not_an_access_payload() {
    // Oracle: a token with purpose Enroll needs the enrollment flow; the
    // bearer constructor refuses it with a configuration error.
    let key = SigningKey::from_seed([9u8; 32], Some("k2".to_string()));
    let issuer = SecretIssuer::new(
        SecretDomain::AGENT_RPC,
        key,
        ctx(),
        Duration::from_hours(24),
    );
    let mut request = SecretRequest::enroll(Duration::from_hours(1));
    request.audience = Some(URL.to_string());
    let secret = issuer.issue(&request).expect("issue an enroll secret");
    let payload = pb::BootstrapPayload {
        url: URL.to_string(),
        enroll_url: URL.to_string(),
        token: secret.encode().to_string(),
        capabilities: Some(signed_capabilities(&["signed-secret"])).into(),
        ..Default::default()
    };
    let payload = encode(&payload);
    let error =
        WebClient::from_access_payload(&payload).expect_err("an enroll payload is not a bearer");
    assert!(
        error.to_string().contains("enroll"),
        "the refusal names enrollment: {error}"
    );
}

// ── #939: the WebCrypto key, the stores and the refresh signature ─────────

/// A generated key signs, and the public key verifies with the same Rust
/// implementation that a host uses.
#[wasm_bindgen_test]
async fn a_generated_key_signs_and_the_public_key_verifies() {
    use foundation_agent_rpc::client::web::WebKeyPair;

    let key = WebKeyPair::generate().await.expect("a WebCrypto key");
    let message = b"ewe.agent_rpc.refresh.v1\0client-1nonce";
    let signature = key.sign(message).await.expect("sign");
    let public = foundation_auth::secret::ClientPublicKey::parse(key.alg(), key.public_key())
        .expect("the public key parses");
    public
        .verify(message, &signature)
        .expect("the signature verifies with the host's verifier");
}

/// The in-memory store keeps an identity (the default IndexedDB store is
/// tested in a headless browser; Node has no IndexedDB).
#[wasm_bindgen_test]
async fn the_memory_store_round_trips_an_identity() {
    use foundation_agent_rpc::client::web::{
        MemoryCredentialStore, WebCredentialStore, WebIdentity, WebKeyPair,
    };

    let key = WebKeyPair::generate().await.expect("a WebCrypto key");
    let identity = WebIdentity {
        url: URL.to_string(),
        enroll_url: URL.to_string(),
        client_id: "client-1".to_string(),
        alg: key.alg().to_string(),
        public_key: key.public_key().to_vec(),
        private_key: key.private_key().clone(),
    };
    let store = MemoryCredentialStore::default();
    store.save(URL, &identity).await.expect("save");
    let loaded = store
        .load(URL)
        .await
        .expect("load")
        .expect("an identity");
    assert_eq!(loaded.client_id, "client-1");
    assert_eq!(loaded.public_key, key.public_key());
    assert_eq!(loaded.alg, key.alg());
}

/// IndexedDB round-trips when the host has it. Node has no IndexedDB, so this
/// case returns there; the headless browser CI job proves it.
#[wasm_bindgen_test]
async fn indexed_db_round_trips_when_the_host_has_it() {
    use foundation_agent_rpc::client::web::{
        IndexedDbCredentials, WebCredentialStore, WebIdentity, WebKeyPair,
    };
    use wasm_bindgen::JsValue;

    let global = js_sys::global();
    let has_indexed_db = js_sys::Reflect::get(&global, &JsValue::from_str("indexedDB"))
        .map(|value| !value.is_undefined() && !value.is_null())
        .unwrap_or(false);
    let has_window = js_sys::Reflect::get(&global, &JsValue::from_str("window"))
        .map(|value| !value.is_undefined() && !value.is_null())
        .unwrap_or(false);
    if !has_indexed_db || !has_window {
        return;
    }
    let key = WebKeyPair::generate().await.expect("a WebCrypto key");
    let url = format!("https://agents.example.com/{}", js_sys::Date::now() as u64);
    let identity = WebIdentity {
        url: url.clone(),
        enroll_url: url.clone(),
        client_id: "client-idb".to_string(),
        alg: key.alg().to_string(),
        public_key: key.public_key().to_vec(),
        private_key: key.private_key().clone(),
    };
    let store = IndexedDbCredentials;
    store.save(&url, &identity).await.expect("save to IndexedDB");
    let loaded = store
        .load(&url)
        .await
        .expect("load from IndexedDB")
        .expect("an identity");
    assert_eq!(loaded.client_id, "client-idb");
    assert_eq!(loaded.public_key, key.public_key());
}
