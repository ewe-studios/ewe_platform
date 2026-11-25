//! The shared handle to one model.
//!
//! WHY: agent-rpc loads a model once per machine and hands it to every agent
//! instance. Loading per instance would multiply memory by the instance count.
//!
//! WHAT: [`ModelHandle`] (`Clone + Send + Sync + 'static`, `Arc`-backed) with
//! the model's name, kind, device, [`ModelId`] and [`ProviderRouter`].
//!
//! HOW: a local model enters through [`PreloadedProvider`], which gives each
//! session a clone of the same model. A remote model enters through a router
//! that holds a provider. The handle keeps no loader, so nothing downstream can
//! trigger a reload.

use std::sync::Arc;

use foundation_ai::types::{Model, ModelId, PreloadedProvider, ProviderRouter};

use crate::core::model::{Device, ModelKind};

/// The shared handle to one loaded model.
///
/// Clones are cheap (`Arc`) and all refer to the same loaded model.
#[derive(Clone)]
pub struct ModelHandle {
    inner: Arc<ModelHandleInner>,
}

struct ModelHandleInner {
    name: String,
    kind: ModelKind,
    device: Device,
    model_id: ModelId,
    router: ProviderRouter,
}

impl std::fmt::Debug for ModelHandle {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ModelHandle")
            .field("name", &self.inner.name)
            .field("kind", &self.inner.kind)
            .field("device", &self.inner.device)
            .field("model_id", &self.inner.model_id)
            .finish_non_exhaustive()
    }
}

impl std::fmt::Display for ModelHandle {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "model {}", self.inner.name)
    }
}

impl ModelHandle {
    /// Serve an already-loaded local model under `name`.
    ///
    /// # Panics
    /// Never in this function. `PreloadedProvider::new` keeps the model.
    #[must_use]
    pub fn from_model<M>(name: impl Into<String>, kind: ModelKind, device: Device, model: M) -> Self
    where
        M: Model + Clone + Send + Sync + 'static,
    {
        let name = name.into();
        let model_id = ModelId::Name(name.clone(), None);
        let router = PreloadedProvider::new(model, model_id.clone()).into_router();
        Self::from_parts(name, kind, device, model_id, router)
    }

    /// Serve `model_id` from an existing router: a remote provider or a mock.
    #[must_use]
    pub fn from_router(
        name: impl Into<String>,
        kind: ModelKind,
        device: Device,
        model_id: impl Into<ModelId>,
        router: ProviderRouter,
    ) -> Self {
        Self::from_parts(name.into(), kind, device, model_id.into(), router)
    }

    fn from_parts(
        name: String,
        kind: ModelKind,
        device: Device,
        model_id: ModelId,
        router: ProviderRouter,
    ) -> Self {
        Self {
            inner: Arc::new(ModelHandleInner {
                name,
                kind,
                device,
                model_id,
                router,
            }),
        }
    }

    /// The name clients address this model by.
    #[must_use]
    pub fn name(&self) -> &str {
        &self.inner.name
    }

    /// The family of the model.
    #[must_use]
    pub fn kind(&self) -> ModelKind {
        self.inner.kind
    }

    /// Where the model runs.
    #[must_use]
    pub fn device(&self) -> Device {
        self.inner.device
    }

    /// The model id sessions request from [`router`](Self::router).
    #[must_use]
    pub fn model_id(&self) -> &ModelId {
        &self.inner.model_id
    }

    /// The router that serves this model to a session. Cheap: routers are
    /// `Arc`-backed, and every clone serves the same loaded model.
    #[must_use]
    pub fn router(&self) -> ProviderRouter {
        self.inner.router.clone()
    }

    /// True when both handles refer to the same loaded model.
    #[must_use]
    pub fn same_model(&self, other: &ModelHandle) -> bool {
        Arc::ptr_eq(&self.inner, &other.inner)
    }
}
