//! The local model catalog and the background model loader.
//!
//! WHY: a local model loads once per machine and every instance shares it. The
//! first `CreateInstance` must not wait for a multi-second load on a valtron
//! worker, so the load runs on [`run_background_job`].
//!
//! WHAT: [`ModelLoader`] (the seam), [`BackgroundLoader`] (the real loader) and
//! [`LocalCatalog`] (one load slot per configured model).
//!
//! HOW: a slot has a futures lock around its load and a state that says whether
//! the model is not loaded, loading, loaded or failed. The selection rules are
//! the shared [`select`] function, so the codes are the same on every host.

use std::collections::HashMap;
use std::future::Future;
use std::pin::Pin;
use std::sync::{Arc, Mutex};

use foundation_context::Ctx;
use foundation_core::valtron;
use foundation_errstacks::ErrorTrace;

use crate::core::model::{
    select, AgentRpcFuture, CatalogEntry, Device, KindSupport, LoadState, ModelCapabilities,
    ModelCatalog, ModelDescriptor, ModelHandle, ModelKind, ModelRequest, SelectionError,
};
use crate::error::{AgentRpcError, AgentRpcResult};
use crate::model::ModelSource;

/// The async future that a [`ModelLoader`] returns.
pub type LoadFuture<'a> = Pin<Box<dyn Future<Output = AgentRpcResult<ModelHandle>> + Send + 'a>>;

/// Loads one configured model.
pub trait ModelLoader: Send + Sync + 'static {
    /// Load `source`. The call may wait for seconds; the catalog calls it on a
    /// background job.
    fn load<'a>(&'a self, source: &'a ModelSource, ctx: &'a Ctx) -> LoadFuture<'a>;
}

/// The real loader: [`ModelHandle::load`] on [`valtron::run_background_job`].
#[derive(Debug, Clone, Copy, Default)]
pub struct BackgroundLoader;

impl ModelLoader for BackgroundLoader {
    fn load<'a>(&'a self, source: &'a ModelSource, ctx: &'a Ctx) -> LoadFuture<'a> {
        Box::pin(async move {
            let source = source.clone();
            let ctx = ctx.clone();
            let (sender, receiver) = futures::channel::oneshot::channel();
            valtron::run_background_job(move || {
                let result = load_model(&source, ctx);
                let _ = sender.send(result);
            })
            .map_err(|error| {
                ErrorTrace::new(AgentRpcError::Config(format!(
                    "starting the model load job: {error}"
                )))
            })?;
            receiver.await.map_err(|_| {
                ErrorTrace::new(AgentRpcError::Config(
                    "the model load job ended without a result".to_string(),
                ))
            })?
        })
    }
}

/// Load one model with the backend that the source names.
#[cfg(any(feature = "candle", feature = "gguf"))]
fn load_model(source: &ModelSource, ctx: Ctx) -> AgentRpcResult<ModelHandle> {
    ModelHandle::load(source, ctx)
}

/// Load one model when this build has no local backend.
#[cfg(not(any(feature = "candle", feature = "gguf")))]
fn load_model(_source: &ModelSource, _ctx: Ctx) -> AgentRpcResult<ModelHandle> {
    Err(AgentRpcError::Unsupported(
        "no local model backend in this build; enable `candle` or `gguf`".to_string(),
    )
    .into())
}

/// One configured model and its load slot.
struct Entry {
    /// The selection entry.
    catalog: CatalogEntry,
    /// The source to load.
    source: ModelSource,
    /// The load slot.
    slot: Arc<LoadSlot>,
}

/// The load state and result of one model.
struct LoadSlot {
    handle: Mutex<Option<ModelHandle>>,
    state: Mutex<LoadState>,
    load: futures::lock::Mutex<()>,
}

impl LoadSlot {
    fn new() -> Self {
        Self {
            handle: Mutex::new(None),
            state: Mutex::new(LoadState::NotLoaded),
            load: futures::lock::Mutex::new(()),
        }
    }

    fn state(&self) -> LoadState {
        self.state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clone()
    }

    fn set_state(&self, state: LoadState) {
        *self
            .state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = state;
    }

    fn handle(&self) -> Option<ModelHandle> {
        self.handle
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clone()
    }
}

/// The configured local models, each loaded at most once.
pub struct LocalCatalog {
    entries: Vec<Entry>,
    by_name: HashMap<String, usize>,
    kinds: Vec<KindSupport>,
    loader: Arc<dyn ModelLoader>,
    ctx: Ctx,
}

impl std::fmt::Debug for LocalCatalog {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LocalCatalog")
            .field("models", &self.by_name.keys().collect::<Vec<_>>())
            .field("kinds", &self.kinds)
            .finish_non_exhaustive()
    }
}

impl LocalCatalog {
    /// A catalog over `models`. Every model loads through `loader`.
    ///
    /// # Errors
    /// [`AgentRpcError::Config`] when two models share a name.
    pub fn new(
        models: &[ModelSource],
        loader: Arc<dyn ModelLoader>,
        ctx: Ctx,
    ) -> AgentRpcResult<Self> {
        let mut entries = Vec::with_capacity(models.len());
        let mut by_name = HashMap::with_capacity(models.len());
        let mut kinds: Vec<KindSupport> = Vec::new();
        for source in models {
            let (name, kind, device) = describe(source);
            if by_name.contains_key(&name) {
                return Err(
                    AgentRpcError::Config(format!("two models share the name {name:?}")).into(),
                );
            }
            by_name.insert(name.clone(), entries.len());
            add_kind(&mut kinds, kind, device);
            entries.push(Entry {
                catalog: CatalogEntry { name, kind, device },
                source: source.clone(),
                slot: Arc::new(LoadSlot::new()),
            });
        }
        Ok(Self {
            entries,
            by_name,
            kinds,
            loader,
            ctx,
        })
    }

    /// Load every model now, so the first `CreateInstance` does not wait.
    ///
    /// # Errors
    /// The first load error. A failed model stays failed in
    /// [`capabilities`](ModelCatalog::capabilities).
    pub async fn preload_all(&self) -> AgentRpcResult<()> {
        for entry in &self.entries {
            self.load_entry(entry).await?;
        }
        Ok(())
    }

    async fn load_entry(&self, entry: &Entry) -> AgentRpcResult<ModelHandle> {
        if let Some(handle) = entry.slot.handle() {
            return Ok(handle);
        }
        let _guard = entry.slot.load.lock().await;
        // Another caller may have loaded the model while this one waited.
        if let Some(handle) = entry.slot.handle() {
            return Ok(handle);
        }
        entry.slot.set_state(LoadState::Loading);
        match self.loader.load(&entry.source, &self.ctx).await {
            Ok(handle) => {
                *entry
                    .slot
                    .handle
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner) = Some(handle.clone());
                entry.slot.set_state(LoadState::Loaded);
                Ok(handle)
            }
            Err(error) => {
                entry
                    .slot
                    .set_state(LoadState::Failed(error.current_context().to_string()));
                Err(error)
            }
        }
    }
}

impl ModelCatalog for LocalCatalog {
    fn capabilities(&self) -> ModelCapabilities {
        ModelCapabilities {
            kinds: self.kinds.clone(),
            models: self
                .entries
                .iter()
                .map(|entry| ModelDescriptor {
                    name: entry.catalog.name.clone(),
                    kind: entry.catalog.kind,
                    device: entry.catalog.device,
                    state: entry.slot.state(),
                })
                .collect(),
        }
    }

    fn resolve<'a>(&'a self, request: &'a ModelRequest) -> AgentRpcFuture<'a, ModelHandle> {
        Box::pin(async move {
            let entries: Vec<CatalogEntry> = self
                .entries
                .iter()
                .map(|entry| entry.catalog.clone())
                .collect();
            let chosen = select(&entries, &self.kinds, request).map_err(|code| {
                ErrorTrace::new(AgentRpcError::ModelSelection {
                    code,
                    detail: code.message().to_string(),
                })
            })?;
            let index = self.by_name.get(&chosen.name).copied().ok_or_else(|| {
                ErrorTrace::new(AgentRpcError::ModelSelection {
                    code: SelectionError::NotFound,
                    detail: format!("model {:?} is not in this catalog", chosen.name),
                })
            })?;
            self.load_entry(&self.entries[index]).await
        })
    }
}

/// The name, kind and device of one model source.
#[cfg(feature = "candle")]
fn describe(source: &ModelSource) -> (String, ModelKind, Device) {
    match source {
        ModelSource::Candle { name, .. } => (name.clone(), ModelKind::Candle, Device::Cpu),
        #[cfg(feature = "gguf")]
        ModelSource::Gguf { name, device, .. } => {
            let device = match device {
                crate::model::GgufDevice::Cpu => Device::Cpu,
                crate::model::GgufDevice::Gpu => Device::Cuda { ordinal: 0 },
                crate::model::GgufDevice::Metal => Device::Metal { ordinal: 0 },
            };
            (name.clone(), ModelKind::Gguf, device)
        }
    }
}

/// The name, kind and device of one model source in a build with no local
/// backend.
#[cfg(not(feature = "candle"))]
fn describe(source: &ModelSource) -> (String, ModelKind, Device) {
    #[cfg(feature = "gguf")]
    {
        let ModelSource::Gguf { name, device, .. } = source;
        let device = match device {
            crate::model::GgufDevice::Cpu => Device::Cpu,
            crate::model::GgufDevice::Gpu => Device::Cuda { ordinal: 0 },
            crate::model::GgufDevice::Metal => Device::Metal { ordinal: 0 },
        };
        return (name.clone(), ModelKind::Gguf, device);
    }
    #[cfg(not(feature = "gguf"))]
    {
        let _ = source;
        (String::new(), ModelKind::Candle, Device::Cpu)
    }
}

/// Add one kind and device to the supported list, without a duplicate.
fn add_kind(kinds: &mut Vec<KindSupport>, kind: ModelKind, device: Device) {
    if let Some(support) = kinds.iter_mut().find(|support| support.kind == kind) {
        if !support.devices.contains(&device) {
            support.devices.push(device);
        }
        return;
    }
    kinds.push(KindSupport {
        kind,
        devices: vec![device],
    });
}
