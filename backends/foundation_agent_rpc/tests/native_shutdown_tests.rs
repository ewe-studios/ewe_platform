//! Native shutdown: the listeners stop and the calls on a kept-alive
//! connection fail.
//!
//! Feature 04, `hosts.md` section 4.7. A running turn gets the terminal
//! `TurnFailed("server shutting down")` from the turn stream; the turn stream
//! aborts its session. This test drives the listener and session part, which
//! runs on the host.

#![cfg(unix)]
#![recursion_limit = "256"]

use std::sync::Arc;
use std::task::{Context, Poll};
use std::time::{Duration, Instant};

use foundation_agent_rpc::core::config::TurnLimits;
use foundation_agent_rpc::core::model::{
    select, AgentRpcFuture, CatalogEntry, Device, KindSupport, LoadState, ModelCapabilities,
    ModelCatalog, ModelDescriptor, ModelHandle, ModelKind, ModelRequest,
};
use foundation_agent_rpc::core::turn::{guarded_turn, TurnShutdown};
use foundation_agent_rpc::host::native::{NativeHost, NativeHostConfig, SIGNING_KEY_FILE};
use foundation_agent_rpc::proto::v1::__buffa::oneof::turn_event::Event;
use foundation_agent_rpc::proto::v1::agent_service::AgentServiceClient;
use foundation_agent_rpc::proto::v1::{CreateInstanceRequest, HealthRequest};
use foundation_agent_rpc::AgentRpcError;
use foundation_ai::agentic::testing::{mock_text, MockModelProvider};
use foundation_ai::agentic::TurnEvent as ModelTurnEvent;
use foundation_auth::pki::{CaKey, KeyAlgorithm};
use foundation_auth::secret::SigningKey;
use foundation_connectrpc::{Request, RpcCtx};
use foundation_context::{Ctx, MonotonicClock};
use foundation_core::valtron::valtron_test;
use futures::Stream;

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

fn rpc() -> RpcCtx {
    RpcCtx::background().with_deadline(Duration::from_secs(10))
}

#[valtron_test(threads = 8)]
async fn native_shutdown_stops_the_listeners_and_ends_the_sessions() {
    let dir = tempfile::tempdir().expect("tempdir");
    let ctx = ctx();
    write_operator_keys(dir.path(), &ctx);
    let grace = Duration::from_millis(250);
    let mut config = NativeHostConfig::default()
        .with_state_dir(dir.path())
        .with_bind("127.0.0.1:0")
        .with_catalog(Arc::new(OneModel::new()))
        .with_preload(false);
    config.http.shutdown_grace = grace;
    let server = NativeHost::start(config, ctx.clone())
        .await
        .expect("host starts");

    let payload = server.bootstrap_payload().expect("payload");
    let encoded = foundation_agent_rpc::core::bootstrap::encode(&payload);
    let client: AgentServiceClient =
        foundation_agent_rpc::client::connect_payload(&encoded, "laptop", &ctx)
            .await
            .expect("connect");
    client
        .create_instance(
            rpc(),
            Request::new(CreateInstanceRequest {
                name: "alpha".into(),
                ..Default::default()
            }),
        )
        .await
        .expect("create");
    let registry = Arc::clone(server.registry());
    assert_eq!(registry.instance_count(), 1, "the instance is live");

    let started = Instant::now();
    server.graceful_shutdown().expect("clean shutdown");
    assert!(
        started.elapsed() <= grace + Duration::from_secs(5),
        "shutdown took {:?}",
        started.elapsed()
    );
    assert_eq!(
        registry.instance_count(),
        0,
        "every session ends during shutdown"
    );

    // The listener is closed: the kept-alive client cannot call it.
    assert!(
        client
            .health(rpc(), Request::new(HealthRequest::default()))
            .await
            .is_err(),
        "a stopped listener must refuse the call"
    );
}

#[test]
fn native_shutdown_emits_the_terminal_event_for_a_running_turn() {
    let ctx = ctx();
    let shutdown = TurnShutdown::default();
    let env = ctx
        .require_env()
        .expect("the test context installs an Env")
        .as_ref()
        .clone();
    let abort: Arc<dyn Fn() + Send + Sync> = Arc::new(|| {});
    let events = futures::stream::pending::<ModelTurnEvent>();
    let mut stream = Box::pin(guarded_turn(
        events,
        (),
        abort,
        shutdown.clone(),
        ctx,
        env,
        &TurnLimits::default(),
    ));

    let waker = futures::task::noop_waker();
    let mut call = Context::from_waker(&waker);
    assert!(
        stream.as_mut().poll_next(&mut call).is_pending(),
        "the turn waits for its model"
    );
    shutdown.fire();
    match stream.as_mut().poll_next(&mut call) {
        Poll::Ready(Some(Ok(event))) => match event.event {
            Some(Event::Failed(failed)) => {
                assert_eq!(failed.message, "server shutting down");
            }
            other => panic!("expected TurnFailed, got {other:?}"),
        },
        other => panic!("expected the terminal event, got {other:?}"),
    }
    assert!(
        stream.as_mut().poll_next(&mut call).is_ready(),
        "the turn ends after its terminal event"
    );
}
