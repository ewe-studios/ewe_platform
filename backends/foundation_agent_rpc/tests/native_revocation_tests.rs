//! Revocation on a live connection: the next call fails with
//! `UNAUTHENTICATED`.
//!
//! Feature 04, invariant I7. The mutual-TLS provider checks the trust store on
//! every call, so a revocation takes effect without a new handshake.

#![cfg(unix)]
#![recursion_limit = "256"]

use std::sync::Arc;
use std::time::Duration;

use foundation_agent_rpc::core::model::{
    select, AgentRpcFuture, CatalogEntry, Device, KindSupport, LoadState, ModelCapabilities,
    ModelCatalog, ModelDescriptor, ModelHandle, ModelKind, ModelRequest,
};
use foundation_agent_rpc::host::native::{NativeHost, NativeHostConfig, SIGNING_KEY_FILE};
use foundation_agent_rpc::proto::v1::agent_service::AgentServiceClient;
use foundation_agent_rpc::proto::v1::HealthRequest;
use foundation_agent_rpc::AgentRpcError;
use foundation_ai::agentic::testing::{mock_text, MockModelProvider};
use foundation_auth::pki::{CaKey, KeyAlgorithm};
use foundation_auth::secret::SigningKey;
use foundation_connectrpc::{Code, Request, RpcCtx};
use foundation_context::{Ctx, MonotonicClock};
use foundation_core::valtron::valtron_test;

struct OneModel {
    handle: ModelHandle,
    entry: CatalogEntry,
}

impl OneModel {
    fn new() -> Self {
        let mut mock = MockModelProvider::new();
        mock.on_any(vec![mock_text("hi")]);
        Self {
            handle: ModelHandle::from_router(
                "mock",
                ModelKind::Api,
                Device::Remote,
                "mock",
                mock.into_router(),
            ),
            entry: CatalogEntry {
                name: "mock".into(),
                kind: ModelKind::Api,
                device: Device::Remote,
            },
        }
    }
}

impl ModelCatalog for OneModel {
    fn capabilities(&self) -> ModelCapabilities {
        ModelCapabilities {
            kinds: vec![KindSupport {
                kind: ModelKind::Api,
                devices: vec![Device::Remote],
            }],
            models: vec![ModelDescriptor {
                name: "mock".into(),
                kind: ModelKind::Api,
                device: Device::Remote,
                state: LoadState::Loaded,
            }],
        }
    }

    fn resolve<'a>(&'a self, request: &'a ModelRequest) -> AgentRpcFuture<'a, ModelHandle> {
        Box::pin(async move {
            select(
                std::slice::from_ref(&self.entry),
                &self.capabilities().kinds,
                request,
            )
            .map_err(|code| {
                foundation_errstacks::ErrorTrace::new(AgentRpcError::ModelSelection {
                    code,
                    detail: "no match".into(),
                })
            })?;
            Ok(self.handle.clone())
        })
    }
}

fn ctx() -> Ctx {
    let ctx = Ctx::fixed(Arc::new(MonotonicClock::new()));
    foundation_core::env::install(&ctx, foundation_core::env::system).expect("Env");
    ctx
}

fn write_operator_keys(dir: &std::path::Path, ctx: &Ctx) {
    let signing = SigningKey::generate_ed25519(ctx, None).expect("signing key");
    std::fs::write(
        dir.join(SIGNING_KEY_FILE),
        signing.to_pkcs8_pem().as_bytes(),
    )
    .expect("signing key file");
    let ca = CaKey::generate(KeyAlgorithm::Ed25519, ctx).expect("ca key");
    std::fs::write(dir.join("ca-key.pem"), ca.to_pem().as_bytes()).expect("ca key file");
    let server = CaKey::generate(KeyAlgorithm::Ed25519, ctx).expect("server key");
    std::fs::write(dir.join("server-key.pem"), server.to_pem().as_bytes())
        .expect("server key file");
}

#[valtron_test(threads = 8)]
async fn native_revoked_client_fails_its_next_call_on_the_same_connection() {
    let dir = tempfile::tempdir().expect("tempdir");
    let ctx = ctx();
    write_operator_keys(dir.path(), &ctx);
    let mut config = NativeHostConfig::default()
        .with_state_dir(dir.path())
        .with_bind("127.0.0.1:0")
        .with_catalog(Arc::new(OneModel::new()))
        .with_preload(false);
    config.http.shutdown_grace = Duration::from_millis(250);
    let server = NativeHost::start(config, ctx.clone())
        .await
        .expect("host starts");

    let payload = server.bootstrap_payload().expect("payload");
    let encoded = foundation_agent_rpc::core::bootstrap::encode(&payload);
    let client: AgentServiceClient =
        foundation_agent_rpc::client::connect_payload(&encoded, "laptop", &ctx)
            .await
            .expect("connect");

    let rpc = || RpcCtx::background().with_deadline(Duration::from_secs(30));
    let status = client
        .health(rpc(), Request::new(HealthRequest::default()))
        .await
        .expect("first call");
    assert_eq!(status.msg.status, "serving");

    let store = server
        .enrollment()
        .expect("secure")
        .trusted_clients()
        .clone();
    let fingerprint = store
        .clients()
        .await
        .expect("clients")
        .into_iter()
        .next()
        .expect("one client")
        .fingerprint_sha256;
    assert!(store.revoke_client(&fingerprint).await.expect("revoke"));

    let error = client
        .health(rpc(), Request::new(HealthRequest::default()))
        .await
        .expect_err("the next call is refused");
    assert_eq!(
        error.current_context().code(),
        Code::Unauthenticated,
        "the kept-alive connection must reach the auth provider: {error:?}"
    );

    drop(client);
    server.graceful_shutdown().expect("clean shutdown");
}
