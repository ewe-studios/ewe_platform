//! `RoutableProviderBox` — the adapter that erases a concrete `ModelProvider`
//! into the object-safe `RoutableProvider` a `ProviderRouter` stores.
//!
//! WHY: every provider reaches a router through this box, yet it was almost
//! entirely uncovered (74% lines): `provider_router_tests.rs` implements
//! `RoutableProvider` directly and so never exercises the adapter. The box's job
//! is turning `Result`-returning `ModelProvider` calls into the `Option`/`bool`/
//! `Vec` shapes the router expects — every one of those conversions swallows an
//! error, so a wrong arm silently makes a model "unavailable" with no diagnostic.
//!
//! WHAT: covers both constructors (`new` via `describe()`, `with_identity`) and
//! each forwarded method on the success AND failure side of the underlying
//! provider.
//!
//! HOW: a minimal in-test `ModelProvider` whose lookups succeed only for a known
//! model id, so every `Result` → `Option` conversion can be observed both ways.
//! No network, no weights.

use foundation_ai::errors::{GenerationResult, ModelProviderErrors, ModelProviderResult};
use foundation_ai::types::{
    AuthProvider, MessageType, Model, ModelAPI, ModelId, ModelInteraction, ModelParams,
    ModelProvider, ModelProviderDescriptor, ModelProviders, ModelSpec, ModelStreamBox,
    ModelUsageCosting, RoutableProvider, RoutableProviderBox, TextBasedFormatter, ToolFormatter,
    UsageReport,
};

const KNOWN: &str = "known-model";

// ---------------------------------------------------------------------------
// Minimal ModelProvider / Model under test
// ---------------------------------------------------------------------------

#[derive(Clone, Default)]
struct StubConfig;

impl AuthProvider for StubConfig {
    fn auth(&self) -> Option<&foundation_auth::AuthCredential> {
        None
    }
}

struct StubModel {
    spec: ModelSpec,
}

impl Model for StubModel {
    fn spec(&self) -> ModelSpec {
        self.spec.clone()
    }
    fn tool_formatter(&self) -> Box<dyn ToolFormatter> {
        Box::new(TextBasedFormatter)
    }
    fn descriptor(&self) -> Option<ModelProviderDescriptor> {
        None
    }
    fn costing(&self) -> GenerationResult<UsageReport> {
        unreachable!("routing tests never generate")
    }
    fn generate(
        &self,
        _interaction: ModelInteraction,
        _specs: Option<ModelParams>,
    ) -> GenerationResult<Vec<foundation_ai::types::Messages>> {
        unreachable!("routing tests never generate")
    }
    fn stream(
        &self,
        _interaction: ModelInteraction,
        _specs: Option<ModelParams>,
    ) -> GenerationResult<ModelStreamBox> {
        unreachable!("routing tests never stream")
    }
}

/// Serves exactly one model id; everything else errors. `describes` controls
/// whether `describe()` succeeds, which is what `RoutableProviderBox::new`
/// depends on.
struct StubProvider {
    describes: bool,
}

fn spec_for(name: &str) -> ModelSpec {
    ModelSpec {
        name: name.into(),
        id: ModelId::Name(name.into(), None),
        devices: None,
        model_location: None,
        lora_location: None,
    }
}

fn not_found(id: &ModelId) -> ModelProviderErrors {
    ModelProviderErrors::NotFound(id.name().to_string())
}

impl ModelProvider for StubProvider {
    type Config = StubConfig;
    type Model = StubModel;

    fn create(self, _config: Option<Self::Config>) -> ModelProviderResult<Self> {
        Ok(self)
    }

    fn describe(&self) -> ModelProviderResult<ModelProviderDescriptor> {
        if !self.describes {
            return Err(ModelProviderErrors::NotFound("no descriptor".into()));
        }
        Ok(ModelProviderDescriptor {
            id: "stub",
            name: "stub-provider",
            reasoning: false,
            api: ModelAPI::Custom("stub".to_string()),
            provider: ModelProviders::Custom("stub".into()),
            base_url: None,
            inputs: MessageType::Text,
            cost: ModelUsageCosting {
                input: 0.0,
                output: 0.0,
                cache_read: 0.0,
                cache_write: 0.0,
            },
            context_window: 4096,
            max_tokens: 1024,
        })
    }

    fn get_model(&self, model_id: ModelId) -> ModelProviderResult<Self::Model> {
        if model_id.name() == KNOWN {
            Ok(StubModel {
                spec: spec_for(KNOWN),
            })
        } else {
            Err(not_found(&model_id))
        }
    }

    fn get_model_by_spec(&self, model_spec: ModelSpec) -> ModelProviderResult<Self::Model> {
        self.get_model(model_spec.id)
    }

    fn get_one(&self, model_id: ModelId) -> ModelProviderResult<ModelSpec> {
        if model_id.name() == KNOWN {
            Ok(spec_for(KNOWN))
        } else {
            Err(not_found(&model_id))
        }
    }

    fn get_all(&self, model_id: ModelId) -> ModelProviderResult<Vec<ModelSpec>> {
        if model_id.name() == KNOWN {
            Ok(vec![spec_for(KNOWN), spec_for(KNOWN)])
        } else {
            Err(not_found(&model_id))
        }
    }
}

fn boxed(describes: bool) -> RoutableProviderBox<StubProvider> {
    RoutableProviderBox::new(StubProvider { describes })
}

fn known() -> ModelId {
    ModelId::Name(KNOWN.into(), None)
}

fn unknown() -> ModelId {
    ModelId::Name("nope".into(), None)
}

// ---------------------------------------------------------------------------
// Constructors
// ---------------------------------------------------------------------------

#[test]
fn new_takes_identity_from_describe() {
    let b = boxed(true);
    assert_eq!(b.name(), "stub-provider");
    assert_eq!(b.provider_id(), ModelProviders::Custom("stub".into()));
}

#[test]
#[should_panic(expected = "provider must return a descriptor")]
fn new_panics_when_the_provider_cannot_describe_itself() {
    // A provider with no identity cannot be routed. Panicking here is the
    // documented contract — inventing a default name would silently mis-route.
    let _ = boxed(false);
}

#[test]
fn with_identity_overrides_describe() {
    // The escape hatch for providers whose describe() is unavailable or wrong.
    let b = RoutableProviderBox::with_identity(
        StubProvider { describes: false },
        "explicit-name",
        ModelProviders::OPENROUTER,
    );
    assert_eq!(b.name(), "explicit-name");
    assert_eq!(b.provider_id(), ModelProviders::OPENROUTER);
}

// ---------------------------------------------------------------------------
// Forwarded lookups — success and failure sides
// ---------------------------------------------------------------------------

#[test]
fn describe_forwards_ok_as_some() {
    assert!(boxed(true).describe().is_some());
}

#[test]
fn describe_forwards_err_as_none() {
    // `new` would panic on a non-describing provider, so build via with_identity.
    let b = RoutableProviderBox::with_identity(
        StubProvider { describes: false },
        "n",
        ModelProviders::Custom("x".into()),
    );
    assert!(
        b.describe().is_none(),
        "a failing describe() must become None, not panic"
    );
}

#[test]
fn serves_is_true_only_for_a_resolvable_model() {
    let b = boxed(true);
    assert!(b.serves(&known()), "the known model must be served");
    assert!(
        !b.serves(&unknown()),
        "an unresolvable model must not be reported as served"
    );
}

#[test]
fn get_one_forwards_ok_as_some() {
    let got = boxed(true).get_one(&known()).expect("known model resolves");
    assert_eq!(got.name, KNOWN);
}

#[test]
fn get_one_forwards_err_as_none() {
    assert!(
        boxed(true).get_one(&unknown()).is_none(),
        "a lookup error must become None"
    );
}

#[test]
fn get_all_forwards_ok_as_the_vec() {
    let all = boxed(true).get_all(&known());
    assert_eq!(all.len(), 2);
    assert!(all.iter().all(|s| s.name == KNOWN));
}

#[test]
fn get_all_forwards_err_as_empty_vec() {
    // `unwrap_or_default()` — an error must degrade to "no models", not panic.
    assert!(
        boxed(true).get_all(&unknown()).is_empty(),
        "a failing get_all must yield an empty Vec"
    );
}

#[test]
fn get_model_forwards_ok_as_some_boxed_model() {
    let m = boxed(true)
        .get_model(&known())
        .expect("known model must load");
    assert_eq!(m.spec().name, KNOWN, "the boxed model must be the right one");
}

#[test]
fn get_model_forwards_err_as_none() {
    assert!(
        boxed(true).get_model(&unknown()).is_none(),
        "a failing get_model must become None"
    );
}

// ---------------------------------------------------------------------------
// Consistency
// ---------------------------------------------------------------------------

#[test]
fn serves_agrees_with_get_one() {
    // The router uses `serves` to pick a provider and `get_one` to resolve the
    // spec; if they disagree, resolution succeeds then immediately fails.
    let b = boxed(true);
    for id in [known(), unknown()] {
        assert_eq!(
            b.serves(&id),
            b.get_one(&id).is_some(),
            "serves() and get_one() must agree for {id:?}"
        );
    }
}

#[test]
fn identity_is_stable_across_calls() {
    let b = boxed(true);
    assert_eq!(b.name(), b.name());
    assert_eq!(b.provider_id(), b.provider_id());
}
