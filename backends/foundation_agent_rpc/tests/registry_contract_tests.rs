//! T15 — the registry contract on `LocalInstanceRegistry`.
//!
//! The same test bodies run on `LocalInstanceRegistry` and, in the Worker
//! feature, on `DurableObjectRegistry`. This file runs the local one.

// The `#[valtron_test]` `Send` check descends the deeply nested async chain of
// the agent session build. Raise the recursion limit for this test crate.
#![recursion_limit = "256"]

use std::sync::Arc;

use foundation_agent_rpc::core::config::{RegistryConfig, TurnLimits};
use foundation_agent_rpc::core::model::{
    AgentRpcFuture, Device, KindSupport, LoadState, ModelCapabilities, ModelCatalog,
    ModelDescriptor, ModelHandle, ModelKind, ModelRequest, SelectionError,
};
use foundation_agent_rpc::core::registry::{
    Caller, InMemoryStores, InstanceInfo, InstanceRegistry, InstanceSpec, LocalInstanceRegistry,
    Scope, ScopePolicy,
};
use foundation_agent_rpc::AgentRpcError;
use foundation_ai::agentic::testing::{mock_text, MockModelProvider};
use foundation_auth::client_auth::{AuthMethod, Principal};
use foundation_context::Ctx;
use foundation_core::valtron::valtron_test;
use futures::StreamExt;

struct OneModel {
    handle: ModelHandle,
}

impl OneModel {
    fn new() -> Self {
        let mut mock = MockModelProvider::new();
        mock.on_any(vec![mock_text("hello")]);
        Self {
            handle: ModelHandle::from_router(
                "mock",
                ModelKind::Api,
                Device::Remote,
                "mock",
                mock.into_router(),
            ),
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
                state: LoadState::Remote,
            }],
        }
    }

    fn resolve<'a>(&'a self, request: &'a ModelRequest) -> AgentRpcFuture<'a, ModelHandle> {
        Box::pin(async move {
            let entries = [foundation_agent_rpc::core::model::CatalogEntry {
                name: "mock".into(),
                kind: ModelKind::Api,
                device: Device::Remote,
            }];
            let capabilities = self.capabilities();
            foundation_agent_rpc::core::model::select(&entries, &capabilities.kinds, request)
                .map_err(|code| AgentRpcError::ModelSelection {
                    code,
                    detail: code.message().to_string(),
                })?;
            Ok(self.handle.clone())
        })
    }
}

/// A test context with the real effect bundle installed.
fn test_ctx() -> Ctx {
    let ctx = Ctx::host();
    foundation_core::env::install(&ctx, foundation_core::env::system).expect("Env");
    ctx
}

fn caller(scope: Scope) -> Caller {
    Caller {
        scope,
        principal: Principal {
            subject: "alice".into(),
            method: AuthMethod::Insecure,
            roles: Vec::new(),
        },
    }
}

fn registry() -> LocalInstanceRegistry {
    LocalInstanceRegistry::new(
        Arc::new(OneModel::new()),
        InMemoryStores,
        RegistryConfig::default(),
        TurnLimits::default(),
        test_ctx(),
    )
    .expect("registry")
}

fn spec(name: &str) -> InstanceSpec {
    InstanceSpec {
        name: name.into(),
        model: ModelRequest::default(),
        ..Default::default()
    }
}

fn names(page: &[InstanceInfo]) -> Vec<&str> {
    page.iter().map(|info| info.name.as_str()).collect()
}

#[valtron_test]
async fn create_get_list_destroy_round_trip() {
    let registry = registry();
    let alice = caller(Scope::Shared);
    let created = registry
        .create(&alice, spec("alpha"))
        .await
        .expect("create");
    assert_eq!(created.name, "alpha");
    assert_eq!(created.kind, ModelKind::Api);
    assert_eq!(created.device, Device::Remote);
    registry.create(&alice, spec("beta")).await.expect("create");

    assert_eq!(
        registry.get(&alice, "alpha").await.expect("get").name,
        "alpha"
    );
    let page = registry.list(&alice, 0, None).await.expect("list");
    assert_eq!(names(&page.items), vec!["alpha", "beta"]);
    assert!(page.next_token.is_none());
    assert_eq!(registry.count(&alice).await.expect("count"), 2);

    registry.destroy(&alice, "alpha").await.expect("destroy");
    let err = registry.get(&alice, "alpha").await.expect_err("gone");
    assert!(matches!(
        err.current_context(),
        AgentRpcError::InstanceNotFound(_)
    ));
}

#[valtron_test]
async fn duplicate_limit_and_names_are_typed_errors() {
    let registry = LocalInstanceRegistry::new(
        Arc::new(OneModel::new()),
        InMemoryStores,
        RegistryConfig::default()
            .with_max_instances(2)
            .with_max_name_len(4),
        TurnLimits::default(),
        test_ctx(),
    )
    .expect("registry");
    let alice = caller(Scope::Shared);
    registry.create(&alice, spec("a")).await.expect("a");
    let duplicate = registry.create(&alice, spec("a")).await.expect_err("dup");
    assert!(matches!(
        duplicate.current_context(),
        AgentRpcError::InstanceExists(_)
    ));
    registry.create(&alice, spec("b")).await.expect("b");
    let limit = registry.create(&alice, spec("c")).await.expect_err("limit");
    assert!(matches!(
        limit.current_context(),
        AgentRpcError::InstanceLimit(2)
    ));
    for bad in ["", "has space", "UPPER", "../x", "toolong"] {
        let err = registry.create(&alice, spec(bad)).await.expect_err(bad);
        assert!(
            matches!(err.current_context(), AgentRpcError::InvalidName(_)),
            "{bad:?}: {err:?}"
        );
    }
}

#[valtron_test]
async fn an_unknown_model_is_a_selection_error() {
    let registry = registry();
    let alice = caller(Scope::Shared);
    let mut request = spec("alpha");
    request.model.name = Some("missing".into());
    let err = registry.create(&alice, request).await.expect_err("missing");
    assert!(matches!(
        err.current_context(),
        AgentRpcError::ModelSelection {
            code: SelectionError::NotFound,
            ..
        }
    ));
}

#[valtron_test]
async fn paging_returns_every_instance_once_in_name_order() {
    let registry = LocalInstanceRegistry::new(
        Arc::new(OneModel::new()),
        InMemoryStores,
        RegistryConfig::default().with_default_page_size(2),
        TurnLimits::default(),
        test_ctx(),
    )
    .expect("registry");
    let alice = caller(Scope::Shared);
    for name in ["delta", "alpha", "charlie", "bravo", "echo"] {
        registry.create(&alice, spec(name)).await.expect("create");
    }
    let mut seen = Vec::new();
    let mut token: Option<String> = None;
    loop {
        let page = registry
            .list(&alice, 0, token.as_deref())
            .await
            .expect("page");
        assert!(page.items.len() <= 2);
        seen.extend(page.items.into_iter().map(|info| info.name));
        match page.next_token {
            Some(next) => token = Some(next),
            None => break,
        }
    }
    assert_eq!(seen, vec!["alpha", "bravo", "charlie", "delta", "echo"]);
}

#[valtron_test]
async fn a_bad_page_token_is_a_codec_error() {
    let registry = registry();
    let alice = caller(Scope::Shared);
    let err = registry
        .list(&alice, 0, Some("not base64 !!"))
        .await
        .expect_err("bad token");
    assert!(matches!(err.current_context(), AgentRpcError::Codec(_)));
}

#[valtron_test]
async fn one_turn_at_a_time() {
    let registry = registry();
    let alice = caller(Scope::Shared);
    registry
        .create(&alice, spec("alpha"))
        .await
        .expect("create");
    let first = registry
        .start_turn(&alice, "alpha", "hi".into())
        .await
        .expect("first turn");
    let err = registry
        .start_turn(&alice, "alpha", "again".into())
        .await
        .err()
        .expect("busy");
    assert!(matches!(
        err.current_context(),
        AgentRpcError::InstanceBusy(_)
    ));
    drop(first);
    let free = registry
        .start_turn(&alice, "alpha", "later".into())
        .await
        .expect("free again");
    drop(free);
}

#[valtron_test]
async fn a_turn_streams_the_model_events() {
    let registry = registry();
    let alice = caller(Scope::Shared);
    registry
        .create(&alice, spec("alpha"))
        .await
        .expect("create");
    let mut stream = registry
        .start_turn(&alice, "alpha", "hi".into())
        .await
        .expect("turn");
    let mut text = String::new();
    let mut done = false;
    while let Some(event) = stream.next().await {
        let event = event.expect("event");
        match event.event {
            Some(foundation_agent_rpc::proto::v1::__buffa::oneof::turn_event::Event::Text(
                chunk,
            )) => text.push_str(&chunk),
            Some(foundation_agent_rpc::proto::v1::__buffa::oneof::turn_event::Event::Done(_)) => {
                done = true;
                break;
            }
            _ => {}
        }
    }
    assert_eq!(text, "hello");
    assert!(done, "a terminal done event");
}

#[valtron_test]
async fn a_long_prompt_is_invalid_argument() {
    let registry = LocalInstanceRegistry::new(
        Arc::new(OneModel::new()),
        InMemoryStores,
        RegistryConfig::default(),
        TurnLimits::default().with_max_prompt_bytes(4),
        test_ctx(),
    )
    .expect("registry");
    let alice = caller(Scope::Shared);
    registry
        .create(&alice, spec("alpha"))
        .await
        .expect("create");
    let err = registry
        .start_turn(&alice, "alpha", "too long".into())
        .await
        .err()
        .expect("long prompt");
    assert!(matches!(
        err.current_context(),
        AgentRpcError::InvalidArgument(_)
    ));
}

#[valtron_test]
async fn per_owner_scope_separates_clients() {
    let registry = registry();
    let alice = caller(ScopePolicy::PerOwner.scope_for(&Principal {
        subject: "alice".into(),
        method: AuthMethod::Insecure,
        roles: Vec::new(),
    }));
    let bob = caller(ScopePolicy::PerOwner.scope_for(&Principal {
        subject: "bob".into(),
        method: AuthMethod::Insecure,
        roles: Vec::new(),
    }));
    registry.create(&alice, spec("alpha")).await.expect("alice");
    assert!(registry.get(&bob, "alpha").await.is_err());
    assert_eq!(registry.count(&bob).await.expect("count"), 0);
    let bob_page = registry.list(&bob, 0, None).await.expect("list");
    assert_eq!(bob_page.items, Vec::<InstanceInfo>::new());
}

#[valtron_test]
async fn destroy_ends_the_session_and_removes_it() {
    let registry = registry();
    let alice = caller(Scope::Shared);
    registry
        .create(&alice, spec("alpha"))
        .await
        .expect("create");
    registry.destroy(&alice, "alpha").await.expect("destroy");
    let err = registry.destroy(&alice, "alpha").await.expect_err("gone");
    assert!(matches!(
        err.current_context(),
        AgentRpcError::InstanceNotFound(_)
    ));
}
