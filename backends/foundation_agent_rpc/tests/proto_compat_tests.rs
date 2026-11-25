//! T13 — the bootstrap payload contract of proto.md section 6.
//!
//! A native payload and a Worker payload round-trip and derive the trust and
//! access forms. A feature-01 payload is refused, and each validation rule has
//! a failing case.

use foundation_agent_rpc::core::bootstrap::{decode, encode, BootstrapAccess, TlsTrust};
use foundation_agent_rpc::proto::v1 as pb;
use foundation_agent_rpc::AgentRpcError;
use foundation_auth::secret::{
    SecretDomain, SecretIssuer, SecretRequest, SignedSecret, SigningKey,
};
use foundation_compact::Duration;
use foundation_context::Ctx;

const URL: &str = "https://203.0.113.7:7443";
const ENROLL_URL: &str = "https://203.0.113.7:7444";

/// A test context with the real effect bundle installed.
fn test_ctx() -> Ctx {
    let ctx = Ctx::host();
    foundation_core::env::install(&ctx, foundation_core::env::system).expect("Env");
    ctx
}

fn issuer(ctx: &Ctx) -> SecretIssuer {
    let key = SigningKey::generate_ed25519(ctx, None).expect("key");
    SecretIssuer::new(
        SecretDomain::AGENT_RPC,
        key,
        ctx.clone(),
        Duration::from_hours(24),
    )
}

fn enroll_token(issuer: &SecretIssuer, audience: &str) -> String {
    let mut request = SecretRequest::enroll(Duration::from_secs(600));
    request.audience = Some(audience.to_string());
    issuer.issue(&request).expect("issue").encode().to_string()
}

fn access_token(issuer: &SecretIssuer, audience: &str) -> String {
    issuer
        .issue(&SecretRequest::access(Duration::from_secs(600), audience))
        .expect("issue")
        .encode()
        .to_string()
}

fn native_payload(ctx: &Ctx) -> pb::BootstrapPayload {
    let issuer = issuer(ctx);
    pb::BootstrapPayload {
        url: URL.into(),
        enroll_url: ENROLL_URL.into(),
        tls_fingerprint_sha256: "ab".repeat(32),
        token: enroll_token(&issuer, URL),
        capabilities: Some(pb::Capabilities {
            protocols: vec!["connect".into(), "grpc".into()],
            models: vec!["tiny".into()],
            model_kinds: vec![buffa::EnumValue::Known(pb::ModelKind::MODEL_KIND_CANDLE)],
            auth_modes: vec!["mtls".into()],
            ..Default::default()
        })
        .into(),
        ..Default::default()
    }
}

fn worker_payload(ctx: &Ctx) -> pb::BootstrapPayload {
    let issuer = issuer(ctx);
    pb::BootstrapPayload {
        url: "https://agents.example.com/rpc".into(),
        token: access_token(&issuer, "https://agents.example.com/rpc"),
        capabilities: Some(pb::Capabilities {
            protocols: vec!["connect".into(), "grpc-web".into()],
            ..Default::default()
        })
        .into(),
        ..Default::default()
    }
}

#[test]
fn a_native_payload_derives_pinned_trust_and_enrollment() {
    let ctx = test_ctx();
    let bootstrap = decode(&encode(&native_payload(&ctx))).expect("decodes");
    assert_eq!(bootstrap.url.as_str().trim_end_matches('/'), URL);
    assert_eq!(bootstrap.trust, TlsTrust::PinnedSha256([0xab; 32]));
    match bootstrap.access {
        BootstrapAccess::Enroll { enroll_url, token } => {
            assert_eq!(enroll_url.as_str().trim_end_matches('/'), ENROLL_URL);
            assert_eq!(token.audience(), Some(URL));
        }
        BootstrapAccess::Bearer { .. } => panic!("not enrollment"),
    }
    assert_eq!(bootstrap.capabilities.models, vec!["tiny"]);
    assert_eq!(
        bootstrap.capabilities.model_kinds,
        vec![buffa::EnumValue::Known(pb::ModelKind::MODEL_KIND_CANDLE)]
    );
    assert_eq!(bootstrap.capabilities.auth_modes, vec!["mtls"]);
}

#[test]
fn a_worker_payload_derives_system_roots_and_a_bearer_token() {
    let ctx = test_ctx();
    let bootstrap = decode(&encode(&worker_payload(&ctx))).expect("decodes");
    assert_eq!(bootstrap.trust, TlsTrust::SystemRoots);
    match bootstrap.access {
        BootstrapAccess::Bearer { token } => {
            assert_eq!(
                token.purpose(),
                foundation_auth::secret::SecretPurpose::Access
            );
        }
        BootstrapAccess::Enroll { .. } => panic!("not bearer"),
    }
}

#[test]
fn a_plaintext_loopback_url_is_development_trust() {
    let ctx = test_ctx();
    let issuer = issuer(&ctx);
    let payload = pb::BootstrapPayload {
        url: "http://127.0.0.1:8080".into(),
        enroll_url: "http://127.0.0.1:8081".into(),
        token: enroll_token(&issuer, "http://127.0.0.1:8080"),
        ..Default::default()
    };
    let bootstrap = decode(&encode(&payload)).expect("decodes");
    assert_eq!(bootstrap.trust, TlsTrust::Plaintext);
}

#[test]
fn a_feature_01_payload_is_refused_with_the_new_one_message() {
    let old = pb::BootstrapPayload {
        tls_fingerprint_sha256: "ab".repeat(32),
        token: "a-token".into(),
        ..Default::default()
    };
    let error = decode(&encode(&old)).expect_err("old payload");
    assert!(matches!(error.current_context(), AgentRpcError::Codec(_)));
    assert!(
        error
            .current_context()
            .to_string()
            .contains("payloads made before feature 07 are not accepted, issue a new one"),
        "{error:?}"
    );
}

#[test]
fn each_validation_rule_has_a_failing_case() {
    let ctx = test_ctx();
    let issuer = issuer(&ctx);
    let good = native_payload(&ctx);

    let cases: Vec<(&str, pb::BootstrapPayload)> = vec![
        ("not base64", good.clone()),
        (
            "http not loopback",
            pb::BootstrapPayload {
                url: "http://example.com".into(),
                enroll_url: "http://example.com/enroll".into(),
                token: enroll_token(&issuer, "http://example.com"),
                ..Default::default()
            },
        ),
        (
            "short fingerprint",
            pb::BootstrapPayload {
                tls_fingerprint_sha256: "ab".into(),
                ..good.clone()
            },
        ),
        (
            "uppercase fingerprint",
            pb::BootstrapPayload {
                tls_fingerprint_sha256: "AB".repeat(32),
                ..good.clone()
            },
        ),
        (
            "fingerprint with http",
            pb::BootstrapPayload {
                url: "http://127.0.0.1:8080".into(),
                enroll_url: "http://127.0.0.1:8081".into(),
                tls_fingerprint_sha256: "ab".repeat(32),
                token: enroll_token(&issuer, "http://127.0.0.1:8080"),
                ..Default::default()
            },
        ),
        (
            "missing token",
            pb::BootstrapPayload {
                token: String::new(),
                ..good.clone()
            },
        ),
        (
            "wrong audience",
            pb::BootstrapPayload {
                token: enroll_token(&issuer, "https://elsewhere.example"),
                ..good.clone()
            },
        ),
        (
            "enroll without enroll_url",
            pb::BootstrapPayload {
                enroll_url: String::new(),
                ..good.clone()
            },
        ),
        (
            "access with enroll_url",
            pb::BootstrapPayload {
                url: "https://agents.example.com/rpc".into(),
                enroll_url: "https://agents.example.com/rpc".into(),
                token: access_token(&issuer, "https://agents.example.com/rpc"),
                ..Default::default()
            },
        ),
    ];
    for (name, payload) in cases {
        let encoded = if name == "not base64" {
            "this is not base64 !!".to_string()
        } else {
            encode(&payload)
        };
        let error = decode(&encoded).expect_err(name);
        assert!(
            matches!(error.current_context(), AgentRpcError::Codec(_)),
            "{name}: {error:?}"
        );
    }
}

#[test]
fn a_tampered_token_signature_is_refused() {
    let ctx = test_ctx();
    let payload = native_payload(&ctx);
    let mut token = payload.token.clone();
    let last = token.pop().expect("token");
    token.push(if last == 'A' { 'B' } else { 'A' });
    let error = decode(&encode(&pb::BootstrapPayload { token, ..payload })).expect_err("tampered");
    assert!(matches!(error.current_context(), AgentRpcError::Codec(_)));
}

#[test]
fn the_decoded_token_keeps_its_window() {
    let ctx = test_ctx();
    let issuer = issuer(&ctx);
    let token = enroll_token(&issuer, URL);
    let decoded = SignedSecret::decode(&token, &foundation_auth::secret::DecodeLimits::default())
        .expect("token");
    assert_eq!(
        decoded.purpose(),
        foundation_auth::secret::SecretPurpose::Enroll
    );
    assert_eq!(decoded.audience(), Some(URL));
    assert!(decoded.valid_until() > decoded.valid_from());
}
