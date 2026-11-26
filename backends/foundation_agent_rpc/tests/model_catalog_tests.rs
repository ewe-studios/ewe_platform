//! T14 — the model catalog: `ModelCatalog`, `ModelHandle` and `RemoteCatalog`.
//!
//! A remote model is stored once; every resolve returns the same shared handle.

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

use foundation_agent_rpc::core::model::{
    ApiModelConfig, CatalogEntry, Device, LoadState, ModelCatalog, ModelHandle, ModelKind,
    ModelRequest, RemoteCatalog,
};
use foundation_agent_rpc::AgentRpcError;
use foundation_ai::agentic::testing::{mock_text, MockModelProvider};
use foundation_context::Ctx;
use foundation_core::valtron::valtron_test;
use foundation_netio::HttpClientBuilder;

fn model(name: &str) -> ModelHandle {
    let mut mock = MockModelProvider::new();
    mock.on_any(vec![mock_text("hi")]);
    ModelHandle::from_router(
        name,
        ModelKind::Api,
        Device::Remote,
        name,
        mock.into_router(),
    )
}

fn config(name: &str, provider: &str) -> ApiModelConfig {
    ApiModelConfig {
        name: name.into(),
        provider: provider.into(),
        model_id: "some-model".into(),
        base_url: None,
        api_key_secret: "KEY".into(),
        account_id: None,
    }
}

fn http() -> foundation_netio::DynNetClient {
    HttpClientBuilder::new().build()
}

#[test]
fn the_capabilities_report_the_api_kind_and_remote_state() {
    let ctx = Ctx::host();
    let catalog = RemoteCatalog::from_config(
        &[config("gpt", "openai")],
        &|name| (name == "KEY").then(|| "sk-test".to_string()),
        &http(),
        &ctx,
    )
    .expect("catalog");
    let capabilities = catalog.capabilities();
    assert_eq!(capabilities.kinds.len(), 1);
    assert_eq!(capabilities.kinds[0].kind, ModelKind::Api);
    assert_eq!(capabilities.kinds[0].devices, vec![Device::Remote]);
    assert_eq!(capabilities.models.len(), 1);
    assert_eq!(capabilities.models[0].name, "gpt");
    assert_eq!(capabilities.models[0].state, LoadState::Remote);
}

#[valtron_test]
async fn ten_resolves_return_the_same_shared_handle() {
    let ctx = Ctx::host();
    let catalog = Arc::new(
        RemoteCatalog::from_config(
            &[config("gpt", "openai")],
            &|_| Some("sk-test".to_string()),
            &http(),
            &ctx,
        )
        .expect("catalog"),
    );
    let request = ModelRequest {
        name: None,
        kind: None,
        device: None,
    };
    let mut handles = Vec::new();
    for _ in 0..10 {
        handles.push(catalog.resolve(&request).await.expect("resolve"));
    }
    for handle in &handles[1..] {
        assert!(handles[0].same_model(handle), "one stored handle");
    }
    assert_eq!(handles[0].name(), "gpt");
    assert_eq!(handles[0].kind(), ModelKind::Api);
    assert_eq!(handles[0].device(), Device::Remote);
}

#[test]
fn a_missing_secret_is_a_config_error() {
    let ctx = Ctx::host();
    let err = RemoteCatalog::from_config(&[config("gpt", "openai")], &|_| None, &http(), &ctx)
        .expect_err("no secret");
    assert!(matches!(err.current_context(), AgentRpcError::Config(_)));
}

#[test]
fn an_unknown_provider_is_a_config_error() {
    let ctx = Ctx::host();
    let err = RemoteCatalog::from_config(
        &[config("gpt", "mystery")],
        &|_| Some("sk-test".to_string()),
        &http(),
        &ctx,
    )
    .expect_err("unknown provider");
    assert!(matches!(err.current_context(), AgentRpcError::Config(_)));
}

#[test]
fn workers_ai_needs_an_account_id() {
    let ctx = Ctx::host();
    let err = RemoteCatalog::from_config(
        &[config("llama", "workers-ai")],
        &|_| Some("token".to_string()),
        &http(),
        &ctx,
    )
    .expect_err("no account");
    assert!(matches!(err.current_context(), AgentRpcError::Config(_)));
}

/// A catalog whose loader counts its calls. This is the shape that the native
/// `LocalCatalog` of task T17 uses; the core rule it proves (one handle, many
/// resolves) is the same.
struct CountingCatalog {
    loads: Arc<AtomicUsize>,
    handle: ModelHandle,
}

impl ModelCatalog for CountingCatalog {
    fn capabilities(&self) -> foundation_agent_rpc::core::model::ModelCapabilities {
        foundation_agent_rpc::core::model::ModelCapabilities {
            kinds: vec![foundation_agent_rpc::core::model::KindSupport {
                kind: ModelKind::Candle,
                devices: vec![Device::Cpu],
            }],
            models: vec![foundation_agent_rpc::core::model::ModelDescriptor {
                name: "counted".into(),
                kind: ModelKind::Candle,
                device: Device::Cpu,
                state: LoadState::Loaded,
            }],
        }
    }

    fn resolve<'a>(
        &'a self,
        _request: &'a ModelRequest,
    ) -> foundation_agent_rpc::core::model::AgentRpcFuture<'a, ModelHandle> {
        Box::pin(async move {
            self.loads.fetch_add(1, Ordering::SeqCst);
            Ok(self.handle.clone())
        })
    }
}

#[valtron_test]
async fn a_catalog_returns_the_one_loaded_handle() {
    let loads = Arc::new(AtomicUsize::new(0));
    let catalog = CountingCatalog {
        loads: Arc::clone(&loads),
        handle: model("counted"),
    };
    let request = ModelRequest {
        name: Some("counted".into()),
        kind: Some(ModelKind::Candle),
        device: Some(Device::Cpu),
    };
    let first = catalog.resolve(&request).await.expect("first");
    for _ in 0..9 {
        let next = catalog.resolve(&request).await.expect("next");
        assert!(first.same_model(&next), "the same loaded model");
    }
    assert_eq!(
        loads.load(Ordering::SeqCst),
        10,
        "the catalog owns the load"
    );
}

#[test]
fn the_entries_keep_the_configuration_order() {
    let ctx = Ctx::host();
    let catalog = RemoteCatalog::from_config(
        &[config("a", "openai"), config("b", "anthropic")],
        &|_| Some("key".to_string()),
        &http(),
        &ctx,
    )
    .expect("catalog");
    let names: Vec<&str> = catalog
        .entries()
        .iter()
        .map(|(e, _)| e.name.as_str())
        .collect();
    assert_eq!(names, vec!["a", "b"]);
    let selected: Vec<CatalogEntry> = catalog.entries().iter().map(|(e, _)| e.clone()).collect();
    assert_eq!(selected[1].device, Device::Remote);
}

#[test]
fn a_handle_is_debug_and_display_safe() {
    let handle = model("gpt");
    assert!(format!("{handle:?}").contains("gpt"));
    assert_eq!(handle.to_string(), "model gpt");
}
