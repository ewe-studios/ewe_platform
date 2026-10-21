//! T8 — the bootstrap secret (and the signing key) never reach the logs.
//!
//! Its own test binary: it installs a global subscriber that captures every
//! event at TRACE level (from all threads, including the valtron workers and
//! the serve loops) *before* the valtron pool starts, then runs a full
//! secure-server lifecycle: key generation, refused enrollment, enrollment,
//! mTLS call, replay.

use std::io::Write;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use foundation_agent_rpc::auth::MtlsConfig;
use foundation_agent_rpc::client::{connect_mtls, enroll};
use foundation_agent_rpc::secret::{BootstrapSecrets, SigningKey, SigningKeyConfig, SIGNING_KEY_FILE};
use foundation_agent_rpc::proto::v1::HealthRequest;
use foundation_agent_rpc::{AgentRpcServer, ModelHandle};
use foundation_ai::agentic::testing::{mock_text, MockModelProvider};
use foundation_connectrpc::{Ctx, Request};

#[derive(Clone, Default)]
struct Captured(Arc<Mutex<Vec<u8>>>);

impl Write for Captured {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        self.0.lock().expect("log buffer").extend_from_slice(buf);
        Ok(buf.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

#[test]
fn the_bootstrap_secret_is_never_logged() {
    let logs = Captured::default();
    let writer = logs.clone();
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::TRACE)
        .with_writer(move || writer.clone())
        .try_init()
        .expect("this binary's only subscriber");

    let _pool = foundation_core::valtron::initialize_pool(7, Some(8));
    let dir = tempfile::tempdir().expect("tempdir");
    let state = dir.path().to_path_buf();
    let secret_seen = Arc::new(Mutex::new(String::new()));
    let secret_out = Arc::clone(&secret_seen);

    foundation_core::valtron::block_on_future(async move {
        let mut mock = MockModelProvider::new();
        mock.on_any(vec![mock_text("hi")]);
        let config = MtlsConfig::from_state_dir_with(
            &state,
            &SigningKeyConfig::in_state_dir(&state).without_env_var(),
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
        let issued = server
            .enrollment()
            .expect("secure")
            .issue_secret()
            .expect("secret");
        let secret = issued.encode().to_string();
        *secret_out.lock().expect("secret slot") = secret.clone();
        tracing::info!(?issued, "the secret's Debug form, logged on purpose");
        tracing::info!(%issued, "the secret's Display form, logged on purpose");

        let bootstrap = server.bootstrap_url().expect("bootstrap url");
        let fingerprint = server.tls_fingerprint().expect("fingerprint").to_string();
        let wrong = BootstrapSecrets::new(SigningKey::generate().expect("key"))
            .issue()
            .expect("issue");
        enroll(&bootstrap, &fingerprint, &wrong.encode(), "laptop")
            .await
            .expect_err("wrong secret");
        let identity = enroll(&bootstrap, &fingerprint, &secret, "laptop")
            .await
            .expect("enroll");
        connect_mtls(&server.base_url(), &identity)
            .expect("client")
            .health(
                Ctx::background().with_deadline(Duration::from_secs(20)),
                Request::new(HealthRequest::default()),
            )
            .await
            .expect("health");
        enroll(&bootstrap, &fingerprint, &secret, "laptop")
            .await
            .expect_err("replay");
        server.graceful_shutdown().expect("shutdown");
    });

    let captured = String::from_utf8_lossy(&logs.0.lock().expect("log buffer")).to_string();
    assert!(
        captured.contains("enrollment refused") && captured.contains("client enrolled"),
        "the capture saw the enrollment events"
    );
    let secret = secret_seen.lock().expect("secret slot").clone();
    assert!(!secret.is_empty());
    assert!(
        !captured.contains(&secret),
        "the bootstrap secret leaked into the logs"
    );
    // Nor the random bytes inside it, nor the signing key.
    let json: serde_json::Value = serde_json::from_slice(
        &base64::Engine::decode(&base64::engine::general_purpose::URL_SAFE_NO_PAD, &secret)
            .expect("b64"),
    )
    .expect("json");
    assert!(!captured.contains(json["secret"].as_str().expect("str")));
    let key_pem = std::fs::read_to_string(dir.path().join(SIGNING_KEY_FILE)).expect("key file");
    let key_body: String = key_pem.lines().nth(1).expect("pem body").to_string();
    assert!(!captured.contains(&key_body), "the signing key leaked into the logs");
}
