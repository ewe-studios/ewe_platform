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
