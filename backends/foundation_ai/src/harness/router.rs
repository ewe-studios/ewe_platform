//! Router mixing — compose heterogeneous providers into one [`ProviderRouter`]
//! and bridge the result into an [`AgentSessionBuilder`].
//!
//! WHY: A real agent session usually wants more than one model: a strong
//! "main" model for the conversation and a cheaper "memory" model for
//! reflection/summarisation, sometimes with fallbacks. The providers behind
//! those models are *different concrete types* (`HuggingFaceGGUFProvider`,
//! `AnthropicMessagesProvider`, `OpenAIProvider`, …) that cannot live in the
//! same `Vec` without erasure. [`crate::types::ProviderRouter`] already solves
//! erasure, but wiring it correctly is subtle:
//!
//! * `HuggingFaceGGUFProvider::serves()` always returns `false` (it has no
//!   catalog), so the router's automatic `serves()` probe never matches a local
//!   GGUF model.
//! * `AnthropicMessagesProvider::serves()` always returns `true` (it claims
//!   every id), so in a mixed router it would greedily capture models that
//!   belong to another provider.
//!
//! Relying on the probe is therefore unsafe for mixing. The robust approach is
//! **explicit routing rules with distinct provider identities**, which is
//! exactly what this module sets up for the caller.
//!
//! WHAT: [`RouterMix`] — a small builder that registers each provider under a
//! unique name (its model id) and emits a matching [`RoutingRule`] so model →
//! provider resolution is deterministic regardless of probe behaviour.
//! [`RouterPreset`] — the built router plus the model ids that should be wired
//! into the agent (`primary_model`, `memory_model`, `fallback_models`), with
//! [`RouterPreset::into_agent_builder`] to hand back a ready-to-customise
//! [`AgentSessionBuilder`].
//!
//! HOW: each role method boxes its provider via
//! [`RoutableProviderBox::with_identity`] (name = the model's id string,
//! provider id taken from `describe()`), records a `RoutingRule` mapping that
//! model id to that name, and remembers which role the model fills. `build()`
//! folds everything into a [`ProviderRouter`] via its builder.

use foundation_db::traits::DocumentStore;

use crate::agentic::{AgentSession, AgentSessionBuilder, MemoryStore};
use crate::types::{
    ModelId, ModelProvider, ModelProviders, ProviderRouter, RoutableProvider, RoutableProviderBox,
    RoutingRule, SessionId,
};

/// Which slot a registered model fills in the resulting agent configuration.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Role {
    Primary,
    Memory,
    Fallback,
}

/// Builder that mixes heterogeneous providers into a single routed preset.
///
/// Each model id must be unique within the mix — the id string doubles as the
/// provider's routing name, so two roles sharing one id would collide. (The
/// built-in presets always use distinct ids, e.g. a 26B main model and an E2B
/// memory model.)
#[derive(Default)]
pub struct RouterMix {
    providers: Vec<Box<dyn RoutableProvider>>,
    rules: Vec<RoutingRule>,
    primary: Option<ModelId>,
    memory: Option<ModelId>,
    fallbacks: Vec<ModelId>,
}

impl RouterMix {
    /// Start an empty mix.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Register the **main** model + the provider that serves it.
    #[must_use]
    pub fn primary<P>(self, provider: P, model: ModelId) -> Self
    where
        P: ModelProvider + Send + Sync + 'static,
        P::Model: Send + Sync,
    {
        self.add_role(Role::Primary, provider, model)
    }

    /// Register the **memory/reflection** model + the provider that serves it.
    #[must_use]
    pub fn memory<P>(self, provider: P, model: ModelId) -> Self
    where
        P: ModelProvider + Send + Sync + 'static,
        P::Model: Send + Sync,
    {
        self.add_role(Role::Memory, provider, model)
    }

    /// Register an additional **fallback** model + its provider. May be called
    /// multiple times; order is preserved.
    #[must_use]
    pub fn fallback<P>(self, provider: P, model: ModelId) -> Self
    where
        P: ModelProvider + Send + Sync + 'static,
        P::Model: Send + Sync,
    {
        self.add_role(Role::Fallback, provider, model)
    }

    fn add_role<P>(mut self, role: Role, provider: P, model: ModelId) -> Self
    where
        P: ModelProvider + Send + Sync + 'static,
        P::Model: Send + Sync,
    {
        // Provider identity for explicit routing: name is the model id string
        // (guaranteed distinct per role), provider kind comes from describe().
        let provider_id = provider
            .describe()
            .map(|d| d.provider)
            .unwrap_or_else(|_| ModelProviders::Custom("harness".to_string()));
        let name = model.name().to_string();

        let boxed: Box<dyn RoutableProvider> =
            Box::new(RoutableProviderBox::with_identity(provider, name.clone(), provider_id));
        self.providers.push(boxed);
        self.rules.push(RoutingRule {
            model: model.clone(),
            provider_name: name,
        });

        match role {
            Role::Primary => self.primary = Some(model),
            Role::Memory => self.memory = Some(model),
            Role::Fallback => self.fallbacks.push(model),
        }
        self
    }

    /// Finish the mix into a [`RouterPreset`].
    #[must_use]
    pub fn build(self) -> RouterPreset {
        let mut builder = ProviderRouter::builder();
        for provider in self.providers {
            builder = builder.add_provider(provider);
        }
        for rule in self.rules {
            builder = builder.rule(rule);
        }

        RouterPreset {
            router: builder.build(),
            primary_model: self
                .primary
                .unwrap_or_else(|| ModelId::Name(String::new(), None)),
            memory_model: self.memory,
            fallback_models: self.fallbacks,
        }
    }
}

/// A built router together with the model ids that should drive an agent.
///
/// Returned by the preset functions in [`crate::harness::agents`] and by
/// [`RouterMix::build`]. Use [`RouterPreset::into_agent_builder`] to obtain an
/// [`AgentSessionBuilder`] that already has the primary/memory/fallback models
/// wired — callers then chain their own `.with_toolshed()`,
/// `.with_system_prompt()`, etc. before `.build()`.
pub struct RouterPreset {
    /// The configured multi-provider router.
    pub router: ProviderRouter,
    /// The model the agent should converse with.
    pub primary_model: ModelId,
    /// The model used for memory/reflection, if any.
    pub memory_model: Option<ModelId>,
    /// Ordered fallback models for the circuit breaker, if any.
    pub fallback_models: Vec<ModelId>,
}

impl RouterPreset {
    /// Bridge into an [`AgentSessionBuilder`] for the given session, applying
    /// the preset's primary/memory/fallback models. The caller picks the
    /// document store `D` and memory store `M` and finishes the build.
    #[must_use]
    pub fn into_agent_builder<D, M>(self, session_id: SessionId) -> AgentSessionBuilder<D, M>
    where
        D: DocumentStore + 'static,
        M: MemoryStore + 'static,
    {
        let mut builder =
            AgentSession::<D, M>::builder(session_id, self.router).with_model(self.primary_model);
        if let Some(memory) = self.memory_model {
            builder = builder.with_memory_model(memory);
        }
        if !self.fallback_models.is_empty() {
            builder = builder.with_fallback_models(self.fallback_models);
        }
        builder
    }
}
