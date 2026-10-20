//! T8 — the bootstrap token never reaches the logs.
//!
//! Its own test binary: it installs a global subscriber that captures every
//! event at TRACE level (from all threads, including the valtron workers and
//! the serve loops) *before* the valtron pool starts, then runs a full
//! secure-server lifecycle: refused enrollment, enrollment, mTLS call, replay.

use std::io::Write;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use foundation_agent_rpc::auth::{BootstrapToken, CertificateAuthority, MtlsConfig};
use foundation_agent_rpc::client::{connect_mtls, enroll};
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
fn the_bootstrap_token_is_never_logged() {
    let logs = Captured::default();
    let writer = logs.clone();
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::TRACE)
        .with_writer(move || writer.clone())
        .try_init()
        .expect("this binary's only subscriber");

    let _pool = foundation_core::valtron::initialize_pool(7, Some(8));
    let token = BootstrapToken::generate().expect("token");
    let secret = token.expose_secret().to_string();

    foundation_core::valtron::block_on_future(async move {
        let mut mock = MockModelProvider::new();
        mock.on_any(vec![mock_text("hi")]);
        let ca = Arc::new(CertificateAuthority::generate("log test CA").expect("ca"));
        let server = AgentRpcServer::builder()
            .with_model(ModelHandle::from_router("mock", "mock", mock.into_router()))
            .with_bootstrap_token(token.clone())
            .with_mtls(MtlsConfig::new(ca))
            .build()
            .expect("server")
            .serve("127.0.0.1:0")
            .expect("serves");
        tracing::info!(?token, "the token's Debug form, logged on purpose");

        let bootstrap = server.bootstrap_url().expect("bootstrap url");
        let fingerprint = server.tls_fingerprint().expect("fingerprint").to_string();
        let wrong = BootstrapToken::generate().expect("token");
        enroll(&bootstrap, &fingerprint, wrong.expose_secret(), "laptop")
            .await
            .expect_err("wrong token");
        let identity = enroll(&bootstrap, &fingerprint, token.expose_secret(), "laptop")
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
        enroll(&bootstrap, &fingerprint, token.expose_secret(), "laptop")
            .await
            .expect_err("replay");
        server.graceful_shutdown().expect("shutdown");
    });

    let captured = String::from_utf8_lossy(&logs.0.lock().expect("log buffer")).to_string();
    assert!(
        captured.contains("enrollment refused") && captured.contains("client enrolled"),
        "the capture saw the enrollment events"
    );
    assert!(
        !captured.contains(&secret),
        "the bootstrap token leaked into the logs"
    );
}
