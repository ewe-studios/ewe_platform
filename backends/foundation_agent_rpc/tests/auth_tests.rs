//! T8 — client auth: bootstrap token → client certificate → mTLS.
//!
//! WHAT: a secure server runs two TLS listeners: the bootstrap listener
//! (server-auth TLS, `BootstrapService/Enroll` only) and the agent listener
//! (mutual TLS, `AgentService`). A client enrolls once with the token and a
//! CSR, then calls the agent listener with the issued certificate.

use std::sync::Arc;
use std::time::Duration;

use foundation_agent_rpc::auth::{BootstrapToken, CertificateAuthority, MtlsConfig};
use foundation_agent_rpc::client::{connect_mtls, enroll, ClientIdentity};
use foundation_agent_rpc::proto::v1::HealthRequest;
use foundation_agent_rpc::{AgentRpcServer, ModelHandle, RunningServer};
use foundation_ai::agentic::testing::{mock_text, MockModelProvider};
use foundation_connectrpc::{Code, Ctx, Request};
use foundation_core::valtron::valtron_test;

fn mock_model() -> ModelHandle {
    let mut mock = MockModelProvider::new();
    mock.on_any(vec![mock_text("secure hello")]);
    ModelHandle::from_router("mock", "mock", mock.into_router())
}

fn ctx() -> Ctx {
    Ctx::background().with_deadline(Duration::from_secs(20))
}

fn secure_server(token: &BootstrapToken) -> RunningServer {
    let ca = Arc::new(CertificateAuthority::generate("agent-rpc test CA").expect("ca"));
    AgentRpcServer::builder()
        .with_model(mock_model())
        .with_bootstrap_token(token.clone())
        .with_mtls(MtlsConfig::new(ca))
        .build()
        .expect("server builds")
        .serve("127.0.0.1:0")
        .expect("serves")
}

#[test]
fn bootstrap_tokens_are_high_entropy_and_never_printed() {
    let a = BootstrapToken::generate().expect("token");
    let b = BootstrapToken::generate().expect("token");
    assert_ne!(a.expose_secret(), b.expose_secret());
    assert!(a.expose_secret().len() >= 43, "256 bits, base64url");
    assert!(a.matches(a.expose_secret()));
    assert!(!a.matches(b.expose_secret()));
    assert!(!a.matches(""));
    for shown in [format!("{a:?}"), format!("{a}")] {
        assert!(!shown.contains(a.expose_secret()), "{shown}");
    }
    assert!(
        BootstrapToken::from_secret("short").is_err(),
        "weak secrets are refused"
    );
}

#[valtron_test(threads = 8)]
async fn token_enrolls_once_then_mtls_is_required() {
    let token = BootstrapToken::generate().expect("token");
    let server = secure_server(&token);
    let bootstrap = server.bootstrap_url().expect("bootstrap listener");
    let fingerprint = server.tls_fingerprint().expect("fingerprint").to_string();

    // A wrong token is refused.
    let wrong = BootstrapToken::generate().expect("token");
    let err = enroll(&bootstrap, &fingerprint, wrong.expose_secret(), "laptop")
        .await
        .expect_err("wrong token");
    assert_eq!(
        err.current_context().code(),
        Code::Unauthenticated,
        "{err:?}"
    );

    // The right token yields a client certificate…
    let identity: ClientIdentity =
        enroll(&bootstrap, &fingerprint, token.expose_secret(), "laptop")
            .await
            .expect("enroll");
    assert!(identity.cert_pem.contains("BEGIN CERTIFICATE"));

    // …which the mTLS listener accepts.
    let client = connect_mtls(&server.base_url(), &identity).expect("mtls client");
    let health = client
        .health(ctx(), Request::new(HealthRequest::default()))
        .await
        .expect("health over mTLS")
        .msg;
    assert_eq!(health.status, "serving");

    // The token is single-use: replaying it is refused.
    let replay = enroll(&bootstrap, &fingerprint, token.expose_secret(), "thief")
        .await
        .expect_err("replayed token");
    assert_eq!(replay.current_context().code(), Code::Unauthenticated);

    // That the token never reaches the logs is `auth_log_tests` (it needs
    // its own process-wide subscriber).

    server.graceful_shutdown().expect("clean shutdown");
}

#[valtron_test(threads = 8)]
async fn calls_without_a_valid_client_certificate_are_rejected() {
    let token = BootstrapToken::generate().expect("token");
    let server = secure_server(&token);

    // A certificate from some other CA is refused by the mTLS listener.
    let other_ca = CertificateAuthority::generate("someone else").expect("ca");
    let (key_pem, csr_pem) = foundation_agent_rpc::client::generate_csr("intruder").expect("csr");
    let foreign = ClientIdentity {
        cert_pem: other_ca
            .sign_client_csr(&csr_pem, "intruder", Duration::from_secs(3600))
            .expect("sign"),
        key_pem,
        ca_pem: server.ca_cert_pem().expect("ca pem").to_string(),
    };
    let client = connect_mtls(&server.base_url(), &foreign).expect("client builds");
    let err = client
        .health(ctx(), Request::new(HealthRequest::default()))
        .await
        .expect_err("foreign certificate");
    assert_eq!(err.current_context().code(), Code::Unavailable, "{err:?}");

    // The agent listener does not speak plaintext at all.
    let plain = foundation_agent_rpc::proto::v1::agent_service::AgentServiceClient::new(
        Arc::new(foundation_connectrpc::H1Transport::new(Arc::new(
            foundation_netio::http::NativeHttpClient::from_system(),
        ))),
        &server.base_url().replacen("https://", "http://", 1),
        foundation_connectrpc::ClientOptions::new(),
    )
    .expect("client");
    assert!(plain
        .health(ctx(), Request::new(HealthRequest::default()))
        .await
        .is_err());

    server.graceful_shutdown().expect("clean shutdown");
}

#[test]
fn a_secure_server_needs_both_token_and_mtls() {
    let token = BootstrapToken::generate().expect("token");
    let err = AgentRpcServer::builder()
        .with_model(mock_model())
        .with_bootstrap_token(token)
        .build()
        .expect_err("token without mTLS");
    assert_eq!(err.current_context().code(), Code::FailedPrecondition);
}
