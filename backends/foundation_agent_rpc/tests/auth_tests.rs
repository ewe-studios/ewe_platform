//! Client auth end to end: signed bootstrap secret → client certificate →
//! mTLS checked against the trusted-client store.
//!
//! WHAT: a secure server runs two TLS listeners: the bootstrap listener
//! (server-auth TLS, `BootstrapService/Enroll` only) and the agent listener
//! (mutual TLS, `AgentService`). A client enrolls once with a signed,
//! time-boxed secret and a CSR, then calls the agent listener with the issued
//! certificate — which must be recorded, unrevoked, in the store.

use std::path::Path;
use std::sync::Arc;
use std::time::{Duration, SystemTime};

use foundation_agent_rpc::auth::{CertificateAuthority, MtlsConfig};
use foundation_agent_rpc::client::{connect_mtls, enroll, generate_csr, ClientIdentity};
use foundation_agent_rpc::proto::v1::HealthRequest;
use foundation_agent_rpc::secret::{
    unix_secs, BootstrapSecrets, SigningKey, SigningKeyConfig, SIGNING_KEY_FILE,
};
use foundation_agent_rpc::trust::TrustedClientStore;
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

/// Everything from `dir`, ignoring the process environment.
async fn state(dir: &Path) -> MtlsConfig {
    MtlsConfig::from_state_dir_with(dir, &SigningKeyConfig::in_state_dir(dir).without_env_var())
        .await
        .expect("state dir")
}

fn serve(config: MtlsConfig) -> RunningServer {
    AgentRpcServer::builder()
        .with_model(mock_model())
        .with_mtls(config)
        .build()
        .expect("server builds")
        .serve("127.0.0.1:0")
        .expect("serves")
}

/// The bootstrap URL, pinned fingerprint and a fresh secret from `server`.
fn bootstrap(server: &RunningServer) -> (String, String, String) {
    let payload = server.bootstrap_payload().expect("payload");
    (
        server.bootstrap_url().expect("bootstrap url"),
        payload.tls_fingerprint_sha256,
        payload.token,
    )
}

async fn health(server: &RunningServer, identity: &ClientIdentity) -> Result<String, Code> {
    connect_mtls(&server.base_url(), identity)
        .expect("client builds")
        .health(ctx(), Request::new(HealthRequest::default()))
        .await
        .map(|r| r.msg.status)
        .map_err(|e| e.current_context().code())
}

#[valtron_test(threads = 8)]
async fn a_secret_enrolls_once_then_mtls_is_required() {
    let dir = tempfile::tempdir().expect("tempdir");
    let server = serve(state(dir.path()).await);
    let (url, fingerprint, secret) = bootstrap(&server);

    // A secret signed by some other key is refused.
    let foreign = BootstrapSecrets::new(SigningKey::generate().expect("key"))
        .issue()
        .expect("issue");
    let err = enroll(&url, &fingerprint, &foreign.encode(), "laptop")
        .await
        .expect_err("foreign secret");
    assert_eq!(err.current_context().code(), Code::Unauthenticated, "{err:?}");

    // The real secret yields a client certificate…
    let identity = enroll(&url, &fingerprint, &secret, "laptop")
        .await
        .expect("enroll");
    assert!(identity.cert_pem.contains("BEGIN CERTIFICATE"));

    // …recorded in the trusted-client store…
    let store = Arc::clone(server.enrollment().expect("secure").trusted_clients());
    let clients = store.list();
    assert_eq!(clients.len(), 1);
    assert_eq!(clients[0].client_name, "laptop");
    assert!(clients[0].last_seen_at.is_none());

    // …which the mTLS listener accepts, and records the handshake.
    assert_eq!(health(&server, &identity).await.as_deref(), Ok("serving"));
    let fp = clients[0].fingerprint_sha256.clone();
    let deadline = SystemTime::now() + Duration::from_secs(10);
    while store.get(&fp).expect("client").last_seen_at.is_none() {
        assert!(SystemTime::now() < deadline, "last_seen never recorded");
        std::thread::sleep(Duration::from_millis(20));
    }

    // The secret is single use: replaying it is refused.
    let replay = enroll(&url, &fingerprint, &secret, "thief")
        .await
        .expect_err("replayed secret");
    assert_eq!(replay.current_context().code(), Code::Unauthenticated);
    assert_eq!(store.list().len(), 1);

    server.graceful_shutdown().expect("clean shutdown");
}

#[valtron_test(threads = 8)]
async fn expired_and_tampered_secrets_are_refused_by_the_server() {
    let dir = tempfile::tempdir().expect("tempdir");
    let config = state(dir.path()).await;
    let secrets = Arc::clone(config.secrets());
    let server = serve(config);
    let (url, fingerprint, _) = bootstrap(&server);

    let past = SystemTime::now() - Duration::from_secs(600);
    let expired = secrets
        .issue_window(past, past + Duration::from_secs(60))
        .expect("issue");
    let err = enroll(&url, &fingerprint, &expired.encode(), "late")
        .await
        .expect_err("expired");
    assert_eq!(err.current_context().code(), Code::Unauthenticated);
    assert!(err.current_context().to_string().contains("expired"), "{err:?}");

    let truncated = {
        let s = secrets.issue().expect("issue").encode().to_string();
        s[..s.len() - 4].to_string()
    };
    let err = enroll(&url, &fingerprint, &truncated, "typo")
        .await
        .expect_err("truncated");
    assert_eq!(err.current_context().code(), Code::Unauthenticated);
    assert!(server
        .enrollment()
        .expect("secure")
        .trusted_clients()
        .list()
        .is_empty());

    server.graceful_shutdown().expect("clean shutdown");
}

#[valtron_test(threads = 8)]
async fn certificates_not_enrolled_here_are_refused() {
    let dir = tempfile::tempdir().expect("tempdir");
    let config = state(dir.path()).await;
    let ca = Arc::clone(config.ca());
    let server = serve(config);
    let ca_pem = server.ca_cert_pem().expect("ca pem").to_string();

    // Signed by the server's own CA, but never enrolled: not in the store.
    let (key_pem, csr_pem) = generate_csr("sideloaded").expect("csr");
    let sideloaded = ClientIdentity {
        cert_pem: ca
            .sign_client_csr(&csr_pem, "sideloaded", Duration::from_secs(3600))
            .expect("sign")
            .cert_pem,
        key_pem,
        ca_pem: ca_pem.clone(),
    };
    assert_eq!(health(&server, &sideloaded).await, Err(Code::Unavailable));

    // From some other CA.
    let other_ca = CertificateAuthority::generate("someone else").expect("ca");
    let (key_pem, csr_pem) = generate_csr("intruder").expect("csr");
    let foreign = ClientIdentity {
        cert_pem: other_ca
            .sign_client_csr(&csr_pem, "intruder", Duration::from_secs(3600))
            .expect("sign")
            .cert_pem,
        key_pem,
        ca_pem,
    };
    assert_eq!(health(&server, &foreign).await, Err(Code::Unavailable));

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

#[valtron_test(threads = 8)]
async fn a_revoked_client_is_refused_from_the_next_handshake() {
    let dir = tempfile::tempdir().expect("tempdir");
    let server = serve(state(dir.path()).await);

    let (url, fingerprint, secret) = bootstrap(&server);
    let laptop = enroll(&url, &fingerprint, &secret, "laptop")
        .await
        .expect("enroll laptop");
    let (_, _, secret) = bootstrap(&server);
    let phone = enroll(&url, &fingerprint, &secret, "phone")
        .await
        .expect("enroll phone");
    assert!(health(&server, &laptop).await.is_ok());
    assert!(health(&server, &phone).await.is_ok());

    let store = server.enrollment().expect("secure").trusted_clients();
    let laptop_fp = store
        .list()
        .into_iter()
        .find(|c| c.client_name == "laptop")
        .expect("laptop")
        .fingerprint_sha256;
    assert!(store
        .revoke(&laptop_fp, unix_secs(SystemTime::now()))
        .await
        .expect("revoke"));

    assert_eq!(health(&server, &laptop).await, Err(Code::Unavailable));
    assert_eq!(health(&server, &phone).await.as_deref(), Ok("serving"));

    server.graceful_shutdown().expect("clean shutdown");
}

#[valtron_test(threads = 8)]
async fn enrollment_survives_a_restart_and_spent_secrets_stay_spent() {
    let dir = tempfile::tempdir().expect("tempdir");
    let first = serve(state(dir.path()).await);
    let (url, fingerprint, used) = bootstrap(&first);
    let identity = enroll(&url, &fingerprint, &used, "laptop")
        .await
        .expect("enroll");
    let (_, _, unused) = bootstrap(&first);
    first.graceful_shutdown().expect("shutdown");

    // Same state directory: same CA, signing key and store.
    let second = serve(state(dir.path()).await);
    assert_eq!(
        health(&second, &identity).await.as_deref(),
        Ok("serving"),
        "an enrolled client reconnects after a restart"
    );
    let (url, fingerprint, _) = bootstrap(&second);
    let err = enroll(&url, &fingerprint, &used, "thief")
        .await
        .expect_err("spent before the restart");
    assert_eq!(err.current_context().code(), Code::Unauthenticated);
    enroll(&url, &fingerprint, &unused, "phone")
        .await
        .expect("an unused secret from before the restart still enrolls");
    assert_eq!(
        second.enrollment().expect("secure").trusted_clients().list().len(),
        2
    );

    second.graceful_shutdown().expect("shutdown");
}

#[valtron_test(threads = 8)]
async fn losing_the_signing_key_invalidates_outstanding_secrets_only() {
    let dir = tempfile::tempdir().expect("tempdir");
    let first = serve(state(dir.path()).await);
    let (url, fingerprint, secret) = bootstrap(&first);
    let identity = enroll(&url, &fingerprint, &secret, "laptop")
        .await
        .expect("enroll");
    let (_, _, outstanding) = bootstrap(&first);
    first.graceful_shutdown().expect("shutdown");

    std::fs::remove_file(dir.path().join(SIGNING_KEY_FILE)).expect("lose the key");
    let second = serve(state(dir.path()).await);
    let (url, fingerprint, _) = bootstrap(&second);
    let err = enroll(&url, &fingerprint, &outstanding, "phone")
        .await
        .expect_err("signed by the lost key");
    assert_eq!(err.current_context().code(), Code::Unauthenticated);
    assert_eq!(
        health(&second, &identity).await.as_deref(),
        Ok("serving"),
        "enrolled clients are unaffected: the key only signs secrets"
    );

    second.graceful_shutdown().expect("shutdown");
}

#[valtron_test(threads = 4)]
async fn the_server_must_choose_its_auth() {
    let err = AgentRpcServer::builder()
        .with_model(mock_model())
        .build()
        .expect_err("no auth chosen");
    assert_eq!(err.current_context().code(), Code::FailedPrecondition);

    let dir = tempfile::tempdir().expect("tempdir");
    let err = AgentRpcServer::builder()
        .with_model(mock_model())
        .with_mtls(state(dir.path()).await)
        .insecure_plaintext()
        .build()
        .expect_err("both");
    assert_eq!(err.current_context().code(), Code::FailedPrecondition);

    // A trusted-client store over any foundation_db SQL backend works.
    let store = TrustedClientStore::open(Arc::new(
        foundation_db::TursoStorage::open(":memory:").await.expect("memory db"),
    ))
    .await
    .expect("store");
    let config = MtlsConfig::new(
        Arc::new(CertificateAuthority::generate("in-memory CA").expect("ca")),
        Arc::new(BootstrapSecrets::new(SigningKey::generate().expect("key"))),
        store,
    );
    AgentRpcServer::builder()
        .with_model(mock_model())
        .with_mtls(config)
        .build()
        .expect("explicit parts build");
}
