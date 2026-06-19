//! `RoutableProvider` and `ProviderRouter` — object-safe routing surface over
//! multiple model providers (F12).
//!
//! WHY: `ModelProvider` is NOT object-safe (associated types `Config`, `Model`),
//! so `Arc<dyn ModelProvider>` cannot exist. A router needs an erased surface
//! that can hold heterogeneous providers (OpenAI, Anthropic, llama.cpp, etc.).
//!
//! WHAT: `RoutableProvider` is object-safe, wraps any concrete `P: ModelProvider`,
//! and returns `Box<dyn ErasedModel>` which exposes `generate`/`stream` via
//! concrete return types (no associated types, no RPIT).

use foundation_core::valtron::{Stream, StreamIterator};

use super::base_types::{
    Messages, Model, ModelId, ModelInteraction, ModelParams, ModelProvider,
    ModelProviderDescriptor, ModelSpec, ModelState,
};
use crate::{
    errors::{GenerationError, GenerationResult},
    types::base_types::BoxModel,
};

// ---------------------------------------------------------------------------
// ErasedModel — object-safe wrapper around any `M: Model`

// ---------------------------------------------------------------------------
// RoutableProvider — object-safe provider surface

/// Object-safe provider that can return an erased model for a given model id.
pub trait RoutableProvider: Send + Sync {
    /// Provider name for routing/debugging.
    fn name(&self) -> &str;
    /// Descriptor for this provider.
    fn describe(&self) -> Option<ModelProviderDescriptor>;
    /// Look up a single model spec by id.
    fn get_one(&self, model_id: &ModelId) -> Option<ModelSpec>;
    /// Look up all model specs matching the given id prefix/name.
    fn get_all(&self, model_id: &ModelId) -> Vec<ModelSpec>;
    /// Create an erased model for the given model id.
    fn get_model(&self, model_id: &ModelId) -> Option<BoxModel>;
}

/// Wrap any concrete `P: ModelProvider` into a `RoutableProvider`.
pub struct RoutableProviderBox<P: Send + Sync>(P);

impl<P: Send + Sync> RoutableProviderBox<P> {
    /// Wrap a provider.
    pub fn new(provider: P) -> Self {
        Self(provider)
    }
}

impl<P: ModelProvider + Send + Sync + 'static> RoutableProvider for RoutableProviderBox<P>
where
    P::Model: Send + Sync,
{
    fn name(&self) -> &str {
        self.0.describe().map_or("unknown", |d| d.name)
    }

    fn describe(&self) -> Option<ModelProviderDescriptor> {
        self.0.describe().ok()
    }

    fn get_one(&self, model_id: &ModelId) -> Option<ModelSpec> {
        self.0.get_one(model_id.clone()).ok()
    }

    fn get_all(&self, model_id: &ModelId) -> Vec<ModelSpec> {
        self.0.get_all(model_id.clone()).unwrap_or_default()
    }

    fn get_model(&self, model_id: &ModelId) -> Option<BoxModel> {
        self.0.get_model(model_id.clone()).ok()
    }
}

// ---------------------------------------------------------------------------
// ProviderRouter — routes model ids to the right provider

/// Holds multiple `RoutableProvider`s and routes model lookups/generations
/// to the provider that owns the matching model.
pub struct ProviderRouter {
    providers: Vec<Box<dyn RoutableProvider>>,
}

impl ProviderRouter {
    pub fn new() -> Self {
        Self {
            providers: Vec::new(),
        }
    }

    /// Register a routable provider.
    pub fn register(&mut self, provider: Box<dyn RoutableProvider>) {
        self.providers.push(provider);
    }

    /// Find the provider that owns the given model id.
    pub fn find_provider(&self, model_id: &ModelId) -> Option<&dyn RoutableProvider> {
        for p in &self.providers {
            if p.get_one(model_id).is_some() {
                return Some(p.as_ref());
            }
        }
        None
    }

    /// Get an erased model for the given model id.
    pub fn get_model(&self, model_id: &ModelId) -> Option<BoxModel> {
        for p in &self.providers {
            if let Some(model) = p.get_model(model_id) {
                return Some(model);
            }
        }
        None
    }

    /// List all available model specs across all providers.
    pub fn list_all(&self) -> Vec<ModelSpec> {
        let mut all = Vec::new();
        for p in &self.providers {
            all.extend(p.get_all(&ModelId::Name(String::new(), None)));
        }
        all
    }
}

impl Default for ProviderRouter {
    fn default() -> Self {
        Self::new()
    }
}
