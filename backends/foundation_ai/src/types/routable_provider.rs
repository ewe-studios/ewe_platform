//! `RoutableProvider` and `ProviderRouter` — object-safe routing surface over
//! multiple model providers (F12).
//!
//! WHY: `ModelProvider` is NOT object-safe (associated types `Config`, `Model`),
//! so `Arc<dyn ModelProvider>` cannot exist. A router needs an erased surface
//! that can hold heterogeneous providers (`OpenAI`, Anthropic, `llama.cpp`, etc.).
//!
//! WHAT: `RoutableProvider` is object-safe, wraps any concrete `ModelProvider`,
//! and returns `BoxModel` (`Box<dyn Model>`) which the `Model` trait now exposes
//! via concrete return types (no associated types, no RPIT — see F12 decisions).

use std::collections::HashMap;
use std::sync::RwLock;

use super::base_types::{
    BoxModel, ModelId, ModelProvider, ModelProviderDescriptor, ModelProviders, ModelSpec,
};

// ---------------------------------------------------------------------------
// RoutableProvider — object-safe provider surface

/// Object-safe provider that can return an erased model for a given model id.
pub trait RoutableProvider: Send + Sync {
    /// Provider name for routing/debugging.
    fn name(&self) -> &str;
    /// The `ModelProviders` enum variant for explicit-rule matching.
    fn provider_id(&self) -> ModelProviders;
    /// Descriptor for this provider.
    fn describe(&self) -> Option<ModelProviderDescriptor>;
    /// Does this provider serve `model_id`? (`get_one` resolves vs `NotFound`.)
    fn serves(&self, model_id: &ModelId) -> bool;
    /// Look up a single model spec by id.
    fn get_one(&self, model_id: &ModelId) -> Option<ModelSpec>;
    /// Look up all model specs matching the given id prefix/name.
    fn get_all(&self, model_id: &ModelId) -> Vec<ModelSpec>;
    /// Create an erased model for the given model id.
    fn get_model(&self, model_id: &ModelId) -> Option<BoxModel>;
}

// ---------------------------------------------------------------------------
// RoutableProviderBox — blanket adapter

/// Wrap any concrete `P: ModelProvider` into a `RoutableProvider`.
pub struct RoutableProviderBox<P: Send + Sync> {
    provider: P,
    name: String,
    provider_id: ModelProviders,
}

impl<P: ModelProvider + Send + Sync + 'static> RoutableProviderBox<P>
where
    P::Model: Send + Sync,
{
    /// Wrap a provider, deriving name/id from its descriptor.
    #[must_use]
    pub fn new(provider: P) -> Self {
        let (name, provider_id) = provider.describe().ok().map_or_else(
            || {
                (
                    "unknown".to_owned(),
                    ModelProviders::Custom("unknown".into()),
                )
            },
            |d| (d.name.to_owned(), d.provider.clone()),
        );
        Self {
            provider,
            name,
            provider_id,
        }
    }

    /// Wrap a provider with an explicit name and provider id.
    #[must_use]
    pub fn with_identity(
        provider: P,
        name: impl Into<String>,
        provider_id: ModelProviders,
    ) -> Self {
        Self {
            provider,
            name: name.into(),
            provider_id,
        }
    }
}

impl<P: ModelProvider + Send + Sync + 'static> RoutableProvider for RoutableProviderBox<P>
where
    P::Model: Send + Sync,
{
    fn name(&self) -> &str {
        &self.name
    }

    fn provider_id(&self) -> ModelProviders {
        self.provider_id.clone()
    }

    fn describe(&self) -> Option<ModelProviderDescriptor> {
        self.provider.describe().ok()
    }

    fn serves(&self, model_id: &ModelId) -> bool {
        self.provider.get_one(model_id.clone()).is_ok()
    }

    fn get_one(&self, model_id: &ModelId) -> Option<ModelSpec> {
        self.provider.get_one(model_id.clone()).ok()
    }

    fn get_all(&self, model_id: &ModelId) -> Vec<ModelSpec> {
        self.provider.get_all(model_id.clone()).unwrap_or_default()
    }

    fn get_model(&self, model_id: &ModelId) -> Option<BoxModel> {
        self.provider
            .get_model(model_id.clone())
            .ok()
            .map(|m| Box::new(m) as BoxModel)
    }
}

// ---------------------------------------------------------------------------
// RoutingRule — explicit model→provider override

/// An explicit routing override: "always route `model` to the provider whose
/// `provider_id()` matches `provider`."
#[derive(Debug, Clone)]
pub struct RoutingRule {
    pub model: ModelId,
    pub provider: ModelProviders,
}

// ---------------------------------------------------------------------------
// RouterError

/// Routing-specific errors (surfaced via `AgenticError::Routing` at the boundary).
#[derive(Debug, Clone, PartialEq)]
pub enum RouterError {
    /// No registered provider serves the requested model.
    NoProviderForModel(String),
    /// An explicit routing rule references a provider that isn't registered.
    RuleMismatch { model: String, provider: String },
}

impl std::fmt::Display for RouterError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RouterError::NoProviderForModel(id) => {
                write!(f, "no provider registered for model: {id}")
            }
            RouterError::RuleMismatch { model, provider } => {
                write!(
                    f,
                    "routing rule references unregistered provider {provider} for model {model}"
                )
            }
        }
    }
}

// ---------------------------------------------------------------------------
// ProviderRouter

/// Holds multiple `RoutableProvider`s and routes model lookups/generations
/// to the provider that owns the matching model.
///
/// Resolution order: explicit rule → declared support (`serves()`) → error.
/// Single-provider mode (via `single()`) skips rule/probe and always delegates.
pub struct ProviderRouter {
    inner: std::sync::Arc<RouterInner>,
}

struct RouterInner {
    providers: Vec<Box<dyn RoutableProvider>>,
    /// model → provider indices (Vec for future F02 same-model fallback).
    routes: RwLock<HashMap<String, Vec<usize>>>,
    rules: Vec<RoutingRule>,
}

impl ProviderRouter {
    /// Single-provider pass-through — no routing table, one provider serves all.
    #[must_use]
    pub fn single(provider: Box<dyn RoutableProvider>) -> Self {
        Self {
            inner: std::sync::Arc::new(RouterInner {
                providers: vec![provider],
                routes: RwLock::new(HashMap::new()),
                rules: Vec::new(),
            }),
        }
    }

    /// Start building a multi-provider router.
    #[must_use]
    pub fn builder() -> ProviderRouterBuilder {
        ProviderRouterBuilder {
            providers: Vec::new(),
            rules: Vec::new(),
        }
    }

    /// Is this router in single-provider mode?
    #[must_use]
    pub fn is_single(&self) -> bool {
        self.inner.rules.is_empty() && self.inner.providers.len() == 1
    }

    /// Number of registered providers.
    #[must_use]
    pub fn provider_count(&self) -> usize {
        self.inner.providers.len()
    }

    /// Resolve a model id to the provider index.
    ///
    /// 1. Explicit rule — exact override wins.
    /// 2. Cached route — previously resolved model→provider(s).
    /// 3. Declared support — first provider where `serves(model_id)` is true.
    /// 4. Single-provider mode — index 0.
    /// 5. Unresolved → error.
    fn resolve_index(&self, model_id: &ModelId) -> Result<usize, RouterError> {
        let key = model_id.name().to_owned();

        // 1. Explicit rule.
        for rule in &self.inner.rules {
            if rule.model.name() == model_id.name() {
                for (idx, p) in self.inner.providers.iter().enumerate() {
                    if p.provider_id() == rule.provider {
                        return Ok(idx);
                    }
                }
                return Err(RouterError::RuleMismatch {
                    model: key,
                    provider: format!("{:?}", rule.provider),
                });
            }
        }

        // 2. Cached route.
        {
            let routes = self.inner.routes.read().unwrap();
            if let Some(indices) = routes.get(&key) {
                if let Some(&idx) = indices.first() {
                    return Ok(idx);
                }
            }
        }

        // 3. Declared support — probe each provider.
        for (idx, p) in self.inner.providers.iter().enumerate() {
            if p.serves(model_id) {
                let mut routes = self.inner.routes.write().unwrap();
                routes.entry(key).or_default().push(idx);
                return Ok(idx);
            }
        }

        // 4. Single-provider mode.
        if self.inner.providers.len() == 1 {
            return Ok(0);
        }

        Err(RouterError::NoProviderForModel(key))
    }

    /// Resolve the provider for a model id.
    ///
    /// # Errors
    ///
    /// Returns `RouterError` if no provider can serve the model.
    pub fn resolve(&self, model_id: &ModelId) -> Result<&dyn RoutableProvider, RouterError> {
        let idx = self.resolve_index(model_id)?;
        Ok(self.inner.providers[idx].as_ref())
    }

    /// Get an erased model for the given model id.
    ///
    /// # Errors
    ///
    /// Returns `RouterError` if no provider can serve or create the model.
    pub fn get_model(&self, model_id: &ModelId) -> Result<BoxModel, RouterError> {
        let provider = self.resolve(model_id)?;
        provider
            .get_model(model_id)
            .ok_or_else(|| RouterError::NoProviderForModel(model_id.name().to_owned()))
    }

    /// Find the provider that owns the given model id (returns `None` if unresolved).
    #[must_use]
    pub fn find_provider(&self, model_id: &ModelId) -> Option<&dyn RoutableProvider> {
        self.resolve(model_id).ok()
    }

    /// List all available model specs across all providers.
    #[must_use]
    pub fn list_all(&self) -> Vec<ModelSpec> {
        let mut all = Vec::new();
        for p in &self.inner.providers {
            all.extend(p.get_all(&ModelId::Name(String::new(), None)));
        }
        all
    }

    /// Return the provider serving the memory/reflection model. Downstream
    /// features (F15, F02) use a different model id for memory generation
    /// than for chat. If `memory_model_id` is `None`, falls back to the
    /// first registered provider.
    ///
    /// # Errors
    ///
    /// Returns `RouterError` if the memory model can't be resolved.
    pub fn memory_model(
        &self,
        memory_model_id: Option<&ModelId>,
    ) -> Result<&dyn RoutableProvider, RouterError> {
        match memory_model_id {
            Some(id) => self.resolve(id),
            None => self
                .inner
                .providers
                .first()
                .map(std::convert::AsRef::as_ref)
                .ok_or_else(|| RouterError::NoProviderForModel("(memory default)".into())),
        }
    }
}

impl Clone for ProviderRouter {
    fn clone(&self) -> Self {
        Self {
            inner: std::sync::Arc::clone(&self.inner),
        }
    }
}

impl Default for ProviderRouter {
    fn default() -> Self {
        Self {
            inner: std::sync::Arc::new(RouterInner {
                providers: Vec::new(),
                routes: RwLock::new(HashMap::new()),
                rules: Vec::new(),
            }),
        }
    }
}

// ---------------------------------------------------------------------------
// ProviderRouterBuilder

/// Builder for `ProviderRouter` — add providers and explicit routing rules.
pub struct ProviderRouterBuilder {
    providers: Vec<Box<dyn RoutableProvider>>,
    rules: Vec<RoutingRule>,
}

impl ProviderRouterBuilder {
    /// Add a routable provider.
    #[must_use]
    pub fn add_provider(mut self, provider: Box<dyn RoutableProvider>) -> Self {
        self.providers.push(provider);
        self
    }

    /// Add an explicit routing rule (model → provider override).
    #[must_use]
    pub fn rule(mut self, rule: RoutingRule) -> Self {
        self.rules.push(rule);
        self
    }

    /// Build the router.
    #[must_use]
    pub fn build(self) -> ProviderRouter {
        ProviderRouter {
            inner: std::sync::Arc::new(RouterInner {
                providers: self.providers,
                routes: RwLock::new(HashMap::new()),
                rules: self.rules,
            }),
        }
    }
}
