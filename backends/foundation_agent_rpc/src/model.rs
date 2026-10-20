//! `ModelHandle` — the shared, load-once handle to one model.
//!
//! WHY: agent-rpc loads a model once per machine and hands it to every agent
//! instance. Loading per instance would multiply memory by the instance count
//! (the reason the server exists).
//!
//! WHAT: [`ModelHandle`] (`Clone + Send + Sync + 'static`, `Arc`-backed) and
//! [`ModelSource`] (what to load). A handle carries the model's name, the
//! [`ModelId`] sessions ask for, and a [`ProviderRouter`] that serves the
//! already-loaded model; every clone shares all three.
//!
//! HOW: a local model is loaded once and wrapped in a
//! [`PreloadedProvider`] router, which hands each session a clone of the same
//! model handle (`CandleModels`/`LlamaModels` clones share weights). The handle
//! keeps no loader, so nothing downstream can trigger a reload. Spec 65 F1/T1
//! proved concurrent generations on one candle model do not interfere.

#[cfg(feature = "candle")]
use std::path::PathBuf;
use std::sync::Arc;

use foundation_ai::types::{Model, ModelId, PreloadedProvider, ProviderRouter};

use crate::error::{AgentRpcError, AgentRpcResult};

/// What to load into a [`ModelHandle`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ModelSource {
    /// A safetensors model directory (`config.json`, `tokenizer.json`,
    /// `*.safetensors`) run on the Candle CPU backend.
    #[cfg(feature = "candle")]
    Candle {
        /// Name clients address the model by.
        name: String,
        /// The model directory.
        dir: PathBuf,
    },
}

impl ModelSource {
    /// A Candle model directory, served under `name`.
    #[cfg(feature = "candle")]
    #[must_use]
    pub fn candle(name: impl Into<String>, dir: impl Into<PathBuf>) -> Self {
        ModelSource::Candle {
            name: name.into(),
            dir: dir.into(),
        }
    }
}

/// The shared handle to one loaded model.
///
/// Clones are cheap (`Arc`) and all refer to the same loaded model.
#[derive(Clone)]
pub struct ModelHandle {
    inner: Arc<ModelHandleInner>,
}

struct ModelHandleInner {
    name: String,
    model_id: ModelId,
    router: ProviderRouter,
}

impl std::fmt::Debug for ModelHandle {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ModelHandle")
            .field("name", &self.inner.name)
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
    /// Load the model described by `source`.
    ///
    /// # Errors
    /// [`AgentRpcError::ModelLoad`] when the backend cannot load the model.
    pub fn load(source: &ModelSource) -> AgentRpcResult<Self> {
        match source {
            #[cfg(feature = "candle")]
            ModelSource::Candle { name, dir } => {
                let name = name.clone();
                let dir = dir.clone();
                Self::load_with(name.clone(), move || load_candle(&name, &dir))
            }
        }
    }

    /// Run `loader` exactly once and serve what it returns.
    ///
    /// For backends without a [`ModelSource`] variant (e.g. a GGUF
    /// `LlamaModels`): the loader runs here, never again.
    ///
    /// # Errors
    /// Whatever `loader` returns.
    pub fn load_with<M, F>(name: impl Into<String>, loader: F) -> AgentRpcResult<Self>
    where
        M: Model + Clone + Send + Sync + 'static,
        F: FnOnce() -> AgentRpcResult<M>,
    {
        let name = name.into();
        tracing::info!(model = %name, "loading model");
        let model = loader()?;
        Ok(Self::from_model(name, model))
    }

    /// Serve an already-loaded model under `name`.
    pub fn from_model<M>(name: impl Into<String>, model: M) -> Self
    where
        M: Model + Clone + Send + Sync + 'static,
    {
        let name = name.into();
        let model_id = ModelId::Name(name.clone(), None);
        let router = PreloadedProvider::new(model, model_id.clone()).into_router();
        Self::from_parts(name, model_id, router)
    }

    /// Serve `model` (a model id the router knows) from an existing router —
    /// a cloud provider, a multi-provider router, or a test mock.
    pub fn from_router(
        name: impl Into<String>,
        model: impl Into<ModelId>,
        router: ProviderRouter,
    ) -> Self {
        Self::from_parts(name.into(), model.into(), router)
    }

    fn from_parts(name: String, model_id: ModelId, router: ProviderRouter) -> Self {
        Self {
            inner: Arc::new(ModelHandleInner {
                name,
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

#[cfg(feature = "candle")]
fn load_candle(
    name: &str,
    dir: &std::path::Path,
) -> AgentRpcResult<foundation_ai::backends::candle::CandleModels> {
    use foundation_ai::backends::candle::CandleBackend;
    use foundation_ai::types::{ModelProvider, ModelSpec};

    let spec = ModelSpec {
        name: name.to_string(),
        id: ModelId::Name(name.to_string(), None),
        devices: None,
        model_location: Some(dir.to_path_buf()),
        lora_location: None,
    };
    CandleBackend::cpu()
        .get_model_by_spec(spec)
        .map_err(|e| AgentRpcError::ModelLoad(format!("{name} from {}: {e}", dir.display())).into())
}
