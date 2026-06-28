//! `ProviderRouter` integration tests (F12): object-safe routing across
//! multiple providers — single-provider pass-through, multi-provider declared
//! support, explicit routing rules, unresolved model errors, memory model
//! resolution, and route caching.

use foundation_ai::types::{
    ModelId, ModelProviderDescriptor, ModelProviders, ModelSpec, ProviderRouter, RoutableProvider,
    RouterError, RoutingRule,
};

// ---------------------------------------------------------------------------
// Test provider — implements RoutableProvider directly (no real backend needed)

struct TestProvider {
    provider_name: &'static str,
    provider_id: ModelProviders,
    supported_models: Vec<ModelSpec>,
}

impl TestProvider {
    fn new(name: &'static str, provider_id: ModelProviders, model_names: &[&str]) -> Self {
        let supported_models = model_names
            .iter()
            .map(|n| ModelSpec {
                name: (*n).into(),
                id: ModelId::Name((*n).into(), None),
                devices: None,
                model_location: None,
                lora_location: None,
            })
            .collect();
        Self {
            provider_name: name,
            provider_id,
            supported_models,
        }
    }
}

impl RoutableProvider for TestProvider {
    fn name(&self) -> &str {
        self.provider_name
    }

    fn provider_id(&self) -> ModelProviders {
        self.provider_id.clone()
    }

    fn describe(&self) -> Option<ModelProviderDescriptor> {
        None
    }

    fn serves(&self, model_id: &ModelId) -> bool {
        self.supported_models
            .iter()
            .any(|s| s.id.name() == model_id.name())
    }

    fn get_one(&self, model_id: &ModelId) -> Option<ModelSpec> {
        self.supported_models
            .iter()
            .find(|s| s.id.name() == model_id.name())
            .cloned()
    }

    fn get_all(&self, _model_id: &ModelId) -> Vec<ModelSpec> {
        self.supported_models.clone()
    }

    fn get_model(&self, _model_id: &ModelId) -> Option<foundation_ai::types::BoxModel> {
        // No real model to return — tests that need a BoxModel would use a
        // full mock Model impl. F12 tests focus on routing, not generation.
        None
    }
}

fn anthropic_provider() -> Box<dyn RoutableProvider> {
    Box::new(TestProvider::new(
        "anthropic",
        ModelProviders::ANTHROPIC,
        &["claude-sonnet-4-6", "claude-opus-4-6", "claude-haiku-4-5"],
    ))
}

fn openai_provider() -> Box<dyn RoutableProvider> {
    Box::new(TestProvider::new(
        "openai",
        ModelProviders::OPENAI,
        &["gpt-4o", "gpt-4o-mini", "o3"],
    ))
}

fn llama_provider() -> Box<dyn RoutableProvider> {
    Box::new(TestProvider::new(
        "llama.cpp",
        ModelProviders::LLAMACPP,
        &["llama-3.1-8b", "mistral-7b"],
    ))
}

// ---------------------------------------------------------------------------
// Single-provider pass-through

#[test]
fn single_provider_delegates_all_lookups() {
    let router = ProviderRouter::single(anthropic_provider());
    assert!(router.is_single());
    assert_eq!(router.provider_count(), 1);

    let id = ModelId::Name("claude-sonnet-4-6".into(), None);
    let provider = router.find_provider(&id).unwrap();
    assert_eq!(provider.name(), "anthropic");

    let spec = provider.get_one(&id).unwrap();
    assert_eq!(spec.name, "claude-sonnet-4-6");
}

#[test]
fn single_provider_serves_even_unknown_models() {
    let router = ProviderRouter::single(anthropic_provider());
    let id = ModelId::Name("nonexistent-model".into(), None);
    // Single-provider mode always resolves to the sole provider (index 0).
    let provider = router.find_provider(&id).unwrap();
    assert_eq!(provider.name(), "anthropic");
}

// ---------------------------------------------------------------------------
// Multi-provider routing by declared support

#[test]
fn multi_provider_routes_by_declared_support() {
    let router = ProviderRouter::builder()
        .add_provider(anthropic_provider())
        .add_provider(openai_provider())
        .build();

    assert!(!router.is_single());
    assert_eq!(router.provider_count(), 2);

    let claude = ModelId::Name("claude-sonnet-4-6".into(), None);
    let gpt = ModelId::Name("gpt-4o".into(), None);

    let p1 = router.resolve(&claude).unwrap();
    assert_eq!(p1.name(), "anthropic");

    let p2 = router.resolve(&gpt).unwrap();
    assert_eq!(p2.name(), "openai");
}

#[test]
fn multi_provider_caches_resolved_routes() {
    let router = ProviderRouter::builder()
        .add_provider(anthropic_provider())
        .add_provider(openai_provider())
        .build();

    let claude = ModelId::Name("claude-opus-4-6".into(), None);

    // First resolve probes providers and caches.
    let p1 = router.resolve(&claude).unwrap();
    assert_eq!(p1.name(), "anthropic");

    // Second resolve hits the cache — same result.
    let p2 = router.resolve(&claude).unwrap();
    assert_eq!(p2.name(), "anthropic");
}

#[test]
fn three_providers_route_independently() {
    let router = ProviderRouter::builder()
        .add_provider(anthropic_provider())
        .add_provider(openai_provider())
        .add_provider(llama_provider())
        .build();

    assert_eq!(router.provider_count(), 3);

    let tests = [
        ("claude-haiku-4-5", "anthropic"),
        ("gpt-4o-mini", "openai"),
        ("llama-3.1-8b", "llama.cpp"),
        ("mistral-7b", "llama.cpp"),
    ];
    for (model, expected_provider) in tests {
        let id = ModelId::Name(model.into(), None);
        let p = router.resolve(&id).unwrap();
        assert_eq!(p.name(), expected_provider, "wrong provider for {model}");
    }
}

// ---------------------------------------------------------------------------
// Explicit routing rules

#[test]
fn explicit_rule_overrides_declared_support() {
    // Both providers serve "shared-model" — the rule forces OpenAI.
    let anthropic_with_shared = Box::new(TestProvider::new(
        "anthropic",
        ModelProviders::ANTHROPIC,
        &["claude-sonnet-4-6", "shared-model"],
    ));
    let openai_with_shared = Box::new(TestProvider::new(
        "openai",
        ModelProviders::OPENAI,
        &["gpt-4o", "shared-model"],
    ));

    let router = ProviderRouter::builder()
        .add_provider(anthropic_with_shared)
        .add_provider(openai_with_shared)
        .rule(RoutingRule {
            model: ModelId::Name("shared-model".into(), None),
            provider_name: "openai".into(),
        })
        .build();

    let id = ModelId::Name("shared-model".into(), None);
    let p = router.resolve(&id).unwrap();
    assert_eq!(p.name(), "openai");
}

#[test]
fn rule_referencing_missing_provider_returns_rule_mismatch() {
    let router = ProviderRouter::builder()
        .add_provider(anthropic_provider())
        .rule(RoutingRule {
            model: ModelId::Name("some-model".into(), None),
            // XAI isn't registered — the rule can't resolve.
            provider_name: "xai".into(),
        })
        .build();

    let id = ModelId::Name("some-model".into(), None);
    match router.resolve(&id) {
        Err(RouterError::RuleMismatch { model, .. }) => assert_eq!(model, "some-model"),
        Err(other) => panic!("expected RuleMismatch, got: {other}"),
        Ok(_) => panic!("expected error, got Ok"),
    }
}

// ---------------------------------------------------------------------------
// Unresolved model

#[test]
fn unresolved_model_returns_error() {
    let router = ProviderRouter::builder()
        .add_provider(anthropic_provider())
        .add_provider(openai_provider())
        .build();

    let id = ModelId::Name("nonexistent-model".into(), None);
    match router.resolve(&id) {
        Err(e) => assert_eq!(
            e,
            RouterError::NoProviderForModel("nonexistent-model".into())
        ),
        Ok(_) => panic!("expected error, got Ok"),
    }
}

#[test]
fn empty_router_returns_no_provider() {
    let router = ProviderRouter::default();
    let id = ModelId::Name("any-model".into(), None);
    match router.resolve(&id) {
        Err(e) => assert_eq!(e, RouterError::NoProviderForModel("any-model".into())),
        Ok(_) => panic!("expected error, got Ok"),
    }
}

// ---------------------------------------------------------------------------
// memory_model helper

#[test]
fn memory_model_resolves_specific_model() {
    let router = ProviderRouter::builder()
        .add_provider(anthropic_provider())
        .add_provider(openai_provider())
        .build();

    let memory_id = ModelId::Name("claude-haiku-4-5".into(), None);
    let p = router.memory_model(Some(&memory_id)).unwrap();
    assert_eq!(p.name(), "anthropic");
}

#[test]
fn memory_model_falls_back_to_first_provider() {
    let router = ProviderRouter::builder()
        .add_provider(openai_provider())
        .add_provider(anthropic_provider())
        .build();

    let p = router.memory_model(None).unwrap();
    assert_eq!(p.name(), "openai");
}

#[test]
fn memory_model_on_empty_router_returns_error() {
    let router = ProviderRouter::default();
    match router.memory_model(None) {
        Err(e) => assert_eq!(
            e,
            RouterError::NoProviderForModel("(memory default)".into())
        ),
        Ok(_) => panic!("expected error, got Ok"),
    }
}

// ---------------------------------------------------------------------------
// get_model (routing + model creation)

#[test]
fn get_model_routes_then_returns_none_from_test_provider() {
    let router = ProviderRouter::builder()
        .add_provider(anthropic_provider())
        .build();

    // TestProvider.get_model() returns None (no real model backend),
    // so get_model returns NoProviderForModel after resolving.
    let id = ModelId::Name("claude-sonnet-4-6".into(), None);
    match router.get_model(&id) {
        Err(e) => assert_eq!(
            e,
            RouterError::NoProviderForModel("claude-sonnet-4-6".into())
        ),
        Ok(_) => panic!("expected error, got Ok"),
    }
}

// ---------------------------------------------------------------------------
// list_all aggregates across providers

#[test]
fn list_all_aggregates_across_providers() {
    let router = ProviderRouter::builder()
        .add_provider(anthropic_provider())
        .add_provider(openai_provider())
        .build();

    let all = router.list_all();
    let names: Vec<&str> = all.iter().map(|s| s.name.as_str()).collect();
    assert!(names.contains(&"claude-sonnet-4-6"));
    assert!(names.contains(&"gpt-4o"));
    assert_eq!(all.len(), 6); // 3 anthropic + 3 openai
}

// ---------------------------------------------------------------------------
// Clone / Arc sharing

#[test]
fn router_clone_shares_state() {
    let router = ProviderRouter::builder()
        .add_provider(anthropic_provider())
        .build();

    let clone = router.clone();

    let id = ModelId::Name("claude-sonnet-4-6".into(), None);
    // Resolve on the original.
    let p1 = router.resolve(&id).unwrap();
    assert_eq!(p1.name(), "anthropic");
    // The clone sees the same cached route.
    let p2 = clone.resolve(&id).unwrap();
    assert_eq!(p2.name(), "anthropic");
}

// ---------------------------------------------------------------------------
// RouterError Display

#[test]
fn router_error_display_messages() {
    let e1 = RouterError::NoProviderForModel("test-model".into());
    assert!(e1.to_string().contains("test-model"));

    let e2 = RouterError::RuleMismatch {
        model: "m".into(),
        provider: "p".into(),
    };
    assert!(e2.to_string().contains("unregistered provider"));
}

// ---------------------------------------------------------------------------
// AgenticError::from(RouterError)

#[test]
fn router_error_converts_to_agentic_error() {
    use foundation_ai::agentic::AgenticError;

    let router_err = RouterError::NoProviderForModel("test".into());
    let agentic_err: AgenticError = router_err.into();
    match agentic_err {
        AgenticError::Routing(msg) => assert!(msg.contains("test")),
        other => panic!("expected Routing, got: {other}"),
    }
}
