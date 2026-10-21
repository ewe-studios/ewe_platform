//! T9 — the bootstrap payload: base64 + QR, and a client that connects from
//! the payload alone.

use std::time::Duration;

use foundation_agent_rpc::auth::MtlsConfig;
use foundation_agent_rpc::secret::{SignedSecret, SigningKeyConfig};
use foundation_agent_rpc::bootstrap::{self, BootstrapPayload};
use foundation_agent_rpc::client::connect_with_payload;
use foundation_agent_rpc::proto::v1::{ChatRequest, CreateInstanceRequest, HealthRequest};
use foundation_agent_rpc::{AgentRpcError, AgentRpcServer, ModelHandle};
use foundation_ai::agentic::testing::{mock_text, MockModelProvider};
use foundation_connectrpc::{Ctx, Request};
use foundation_core::valtron::valtron_test;

fn ctx() -> Ctx {
    Ctx::background().with_deadline(Duration::from_secs(20))
}

fn sample() -> BootstrapPayload {
    BootstrapPayload {
        version: bootstrap::PAYLOAD_VERSION,
        addr: "10.0.0.7:7443".into(),
        bootstrap_addr: "10.0.0.7:7444".into(),
        tls_fingerprint_sha256: "ab".repeat(32),
        token: "t".repeat(43),
        capabilities: foundation_agent_rpc::proto::v1::Capabilities {
            protocols: vec!["connect".into(), "grpc".into(), "grpc-web".into()],
            models: vec!["tiny".into()],
            auth_mode: "mtls".into(),
            ..Default::default()
        }
        .into(),
        ..Default::default()
    }
}

#[test]
fn base64_round_trip_keeps_every_field() {
    let payload = sample();
    let encoded = bootstrap::encode(&payload);
    assert!(
        encoded
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_'),
        "base64url, no padding: safe in a URL, a shell and a QR code"
    );
    let decoded = bootstrap::decode(&encoded).expect("decodes");
    assert_eq!(decoded, payload);
    assert_eq!(decoded.addr, "10.0.0.7:7443");
    assert_eq!(decoded.tls_fingerprint_sha256, "ab".repeat(32));
    assert_eq!(decoded.token, "t".repeat(43));
    let caps = decoded.capabilities.as_option().expect("capabilities");
    assert_eq!(caps.protocols, vec!["connect", "grpc", "grpc-web"]);
    assert_eq!(caps.models, vec!["tiny"]);
    assert_eq!(caps.auth_mode, "mtls");
}

#[test]
fn the_qr_renderings_encode_the_same_payload() {
    let encoded = bootstrap::encode(&sample());
    let text = bootstrap::qr_text(&encoded).expect("text QR");
    assert!(text.lines().count() > 10, "a scannable terminal QR code");
    assert!(text.contains('█') || text.contains('▀') || text.contains('▄'));
    let svg = bootstrap::qr_svg(&encoded).expect("svg QR");
    assert!(svg.contains("<svg"), "an SVG document");
}

#[test]
fn malformed_payloads_are_codec_errors() {
    for bad in ["", "not base64 !!", "AAAA"] {
        let err = bootstrap::decode(bad).expect_err(bad);
        assert!(
            matches!(err.current_context(), AgentRpcError::Codec(_)),
            "{bad:?}: {err:?}"
        );
    }
    let mut future = sample();
    future.version = bootstrap::PAYLOAD_VERSION + 1;
    let err = bootstrap::decode(&bootstrap::encode(&future)).expect_err("unknown version");
    assert!(matches!(err.current_context(), AgentRpcError::Codec(_)));
}

#[test]
fn the_payload_is_redacted_when_printed() {
    let shown = format!("{:?}", bootstrap::EncodedPayload::new(&sample()));
    assert!(!shown.contains(&"t".repeat(43)), "{shown}");
}

#[valtron_test(threads = 8)]
async fn a_client_connects_from_the_payload_alone() {
    let mut mock = MockModelProvider::new();
    mock.on_any(vec![mock_text("hello via payload")]);
    let dir = tempfile::tempdir().expect("tempdir");
    let config = MtlsConfig::from_state_dir_with(
        dir.path(),
        &SigningKeyConfig::in_state_dir(dir.path()).without_env_var(),
    )
    .await
    .expect("state");
    let server = AgentRpcServer::builder()
        .with_model(ModelHandle::from_router("mock", "mock", mock.into_router()))
        .with_mtls(config)
        .build()
        .expect("server")
        .serve("127.0.0.1:0")
        .expect("serves");

    let payload = server
        .bootstrap_payload_for(Duration::from_secs(600))
        .expect("payload");
    let caps = payload.capabilities.as_option().expect("capabilities");
    assert_eq!(caps.models, vec!["mock"]);
    assert_eq!(caps.auth_mode, "mtls");
    let secret = SignedSecret::decode(&payload.token).expect("a signed secret");
    assert!(secret.verify_self_signed());
    assert_eq!(secret.valid_until() - secret.valid_from(), 600);
    let encoded = bootstrap::encode(&payload);
    // The whole payload — addresses, pin, signed secret — still fits a QR code.
    bootstrap::qr_text(&encoded).expect("QR");

    let (client, _identity) = connect_with_payload(&encoded, "phone")
        .await
        .expect("connect from the payload");
    assert_eq!(
        client
            .health(ctx(), Request::new(HealthRequest::default()))
            .await
            .expect("health")
            .msg
            .status,
        "serving"
    );
    client
        .create_instance(
            ctx(),
            Request::new(CreateInstanceRequest {
                name: "alpha".into(),
                ..Default::default()
            }),
        )
        .await
        .expect("create");
    let reply = client
        .chat(
            ctx(),
            Request::new(ChatRequest {
                instance: "alpha".into(),
                prompt: "hi".into(),
                ..Default::default()
            }),
        )
        .await
        .expect("chat")
        .msg;
    assert_eq!(reply.text, "hello via payload");

    server.graceful_shutdown().expect("shutdown");
}

#[valtron_test(threads = 4)]
async fn the_client_checks_the_secret_before_dialling() {
    use foundation_agent_rpc::secret::{BootstrapSecrets, SigningKey};
    use std::time::SystemTime;

    let secrets = BootstrapSecrets::new(SigningKey::generate().expect("key"));
    let past = SystemTime::now() - Duration::from_secs(600);
    let mut payload = sample();
    // Unroutable addresses: a network attempt would fail differently.
    payload.addr = "127.0.0.1:1".into();
    payload.bootstrap_addr = "127.0.0.1:1".into();

    payload.token = secrets
        .issue_window(past, past + Duration::from_secs(60))
        .expect("issue")
        .encode()
        .to_string();
    let err = connect_with_payload(&bootstrap::encode(&payload), "phone")
        .await
        .expect_err("expired");
    assert_eq!(
        err.current_context().code(),
        foundation_connectrpc::Code::Unauthenticated
    );
    assert!(err.current_context().to_string().contains("validity window"), "{err:?}");

    let good = secrets.issue().expect("issue").encode().to_string();
    payload.token = format!("{}AA", &good[..good.len() - 2]);
    let err = connect_with_payload(&bootstrap::encode(&payload), "phone")
        .await
        .expect_err("corrupted");
    assert_eq!(
        err.current_context().code(),
        foundation_connectrpc::Code::InvalidArgument,
        "{err:?}"
    );
}
