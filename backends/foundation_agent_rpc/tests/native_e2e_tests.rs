//! Native end to end: enroll, mutual TLS, create, stream a turn, destroy.
//!
//! Feature 04, section 9 of `hosts.md`: the flow over real loopback sockets.

#![cfg(unix)]
// The `#[valtron_test]` `Send` check descends the nested async chain of the
// transport and the trust store.
#![recursion_limit = "256"]

use std::sync::Arc;
use std::time::Duration;

use foundation_agent_rpc::core::model::{
    select, AgentRpcFuture, CatalogEntry, Device, KindSupport, LoadState, ModelCapabilities,
    ModelCatalog, ModelDescriptor, ModelHandle, ModelKind, ModelRequest,
};
use foundation_agent_rpc::host::native::{NativeHost, NativeHostConfig, SIGNING_KEY_FILE};
use foundation_agent_rpc::proto::v1::__buffa::oneof::turn_event::Event;
use foundation_agent_rpc::proto::v1::agent_service::AgentServiceClient;
use foundation_agent_rpc::proto::v1::{ChatRequest, CreateInstanceRequest, DestroyInstanceRequest};
use foundation_agent_rpc::AgentRpcError;
use foundation_ai::agentic::testing::{mock_text, MockModelProvider};
use foundation_auth::pki::{CaKey, KeyAlgorithm};
use foundation_auth::secret::SigningKey;
use foundation_connectrpc::{Request, RpcCtx};
use foundation_context::{Ctx, MonotonicClock};
use foundation_core::valtron::valtron_test;

struct OneModel {
    handle: ModelHandle,
    entry: CatalogEntry,
}

impl OneModel {
    fn new() -> Self {
        let mut mock = MockModelProvider::new();
        mock.on_any(vec![mock_text("one "), mock_text("two")]);
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

fn rpc() -> RpcCtx {
    RpcCtx::background().with_deadline(Duration::from_secs(60))
}

async fn start(
    dir: &std::path::Path,
    ctx: &Ctx,
) -> foundation_agent_rpc::host::native::RunningServer {
    write_operator_keys(dir, ctx);
    let mut config = NativeHostConfig::default()
        .with_state_dir(dir)
        .with_bind("127.0.0.1:0")
        .with_catalog(Arc::new(OneModel::new()))
        .with_preload(false);
    config.http.shutdown_grace = Duration::from_millis(250);
    NativeHost::start(config, ctx.clone())
        .await
        .expect("host starts")
}

#[valtron_test(threads = 8)]
async fn native_enroll_mtls_create_stream_and_destroy() {
    let dir = tempfile::tempdir().expect("tempdir");
    let ctx = ctx();
    let server = start(dir.path(), &ctx).await;

    let payload = server.bootstrap_payload().expect("payload");
    let encoded = foundation_agent_rpc::core::bootstrap::encode(&payload);
    let client: AgentServiceClient =
        foundation_agent_rpc::client::connect_payload(&encoded, "laptop", &ctx)
            .await
            .expect("the client enrolls and connects over mTLS");

    let info = client
        .create_instance(
            rpc(),
            Request::new(CreateInstanceRequest {
                name: "alpha".into(),
                ..Default::default()
            }),
        )
        .await
        .expect("create");
    assert_eq!(info.msg.name, "alpha");

    let mut stream = client
        .chat_stream(
            rpc(),
            Request::new(ChatRequest {
                instance: "alpha".into(),
                prompt: "hello".into(),
                ..Default::default()
            }),
        )
        .await
        .expect("stream opens");
    let mut text = String::new();
    let mut done = false;
    while let Some(event) = stream.receive().await.expect("event") {
        match event.event {
            Some(Event::Text(chunk)) => text.push_str(&chunk),
            Some(Event::Done(_)) => done = true,
            Some(Event::Failed(failed)) => panic!("turn failed: {}", failed.message),
            _ => {}
        }
    }
    assert_eq!(text, "one two");
    assert!(done, "the stream ends with Done");

    client
        .destroy_instance(
            rpc(),
            Request::new(DestroyInstanceRequest {
                name: "alpha".into(),
                ..Default::default()
            }),
        )
        .await
        .expect("destroy");

    let clients = server
        .enrollment()
        .expect("secure")
        .trusted_clients()
        .clients()
        .await
        .expect("clients");
    assert_eq!(clients.len(), 1);
    assert_eq!(clients[0].client_name, "laptop");

    drop(client);
    server.graceful_shutdown().expect("clean shutdown");
}
