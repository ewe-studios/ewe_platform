//! The model catalog trait and the pure selection function.
//!
//! WHY: the registry asks the catalog for a model and gets one shared handle.
//! The selection rules and their status codes must be the same on every host,
//! so they live in this one function.
//!
//! WHAT: [`ModelCatalog`], the capabilities that feed `GetCapabilities` and
//! `Health`, [`CatalogEntry`], [`SelectionError`] and [`select`].
//!
//! HOW: a host implements [`ModelCatalog`] over its configuration. [`select`]
//! is pure: it reads a list of entries and the supported kinds and never
//! loads a model.

use core::future::Future;
use core::pin::Pin;

use crate::core::model::{Device, ModelKind, ModelRequest};
use crate::error::AgentRpcResult;

/// The future that core traits return. It is `Send` so a host can spawn it.
pub type AgentRpcFuture<'a, T> = Pin<Box<dyn Future<Output = AgentRpcResult<T>> + Send + 'a>>;

/// One configured model, for the selection rules.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CatalogEntry {
    /// The name that clients use.
    pub name: String,
    /// The family of the model.
    pub kind: ModelKind,
    /// Where the model runs.
    pub device: Device,
}

/// One model kind and the devices on which this host can run it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KindSupport {
    /// The family of the model.
    pub kind: ModelKind,
    /// The devices that this host supports for the kind.
    pub devices: Vec<Device>,
}

/// The load state of one configured model.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LoadState {
    /// Nothing has loaded the model yet.
    NotLoaded,
    /// A load is in progress.
    Loading,
    /// The model is ready.
    Loaded,
    /// The load failed. The text says why.
    Failed(String),
    /// Nothing to load: the remote provider holds the model.
    Remote,
}

impl LoadState {
    /// The wire value.
    #[must_use]
    pub const fn to_wire(&self) -> crate::proto::v1::ModelLoadState {
        use crate::proto::v1::ModelLoadState;
        match self {
            Self::NotLoaded => ModelLoadState::MODEL_LOAD_STATE_NOT_LOADED,
            Self::Loading => ModelLoadState::MODEL_LOAD_STATE_LOADING,
            Self::Loaded => ModelLoadState::MODEL_LOAD_STATE_LOADED,
            Self::Failed(_) => ModelLoadState::MODEL_LOAD_STATE_FAILED,
            Self::Remote => ModelLoadState::MODEL_LOAD_STATE_REMOTE,
        }
    }
}

/// What a host supports and has configured.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModelCapabilities {
    /// The kinds and their devices.
    pub kinds: Vec<KindSupport>,
    /// Every configured model.
    pub models: Vec<ModelDescriptor>,
}

/// One configured model, for `GetCapabilities` and `Health`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModelDescriptor {
    /// The name that clients use.
    pub name: String,
    /// The family of the model.
    pub kind: ModelKind,
    /// Where the model runs.
    pub device: Device,
    /// The load state.
    pub state: LoadState,
}

/// The models that a host can run.
pub trait ModelCatalog: Send + Sync + 'static {
    /// What this host supports and has configured. Cheap and sync: it feeds
    /// `GetCapabilities` and `Health`.
    fn capabilities(&self) -> ModelCapabilities;

    /// Pick a model with the rules of proto.md section 5 and return its
    /// handle. A local model loads on its first use (once), unless the host
    /// loaded it at start.
    ///
    /// # Errors
    /// `AgentRpcError::ModelSelection` (the codes of proto.md section 5);
    /// `AgentRpcError::ModelLoad` when a load fails.
    fn resolve<'a>(&'a self, request: &'a ModelRequest) -> AgentRpcFuture<'a, super::ModelHandle>;
}

/// Why no model was picked. Each case has one Connect code (proto.md section
/// 5).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SelectionError {
    /// No configured model matches (`NOT_FOUND`).
    NotFound,
    /// The named model has another kind (`INVALID_ARGUMENT`).
    KindMismatch,
    /// The named model runs on another device (`FAILED_PRECONDITION`).
    DeviceMismatch,
    /// This host does not run the kind (`UNIMPLEMENTED`).
    KindUnsupported,
    /// This host does not run the kind on that device (`FAILED_PRECONDITION`).
    DeviceUnsupported,
    /// More than one model matches (`INVALID_ARGUMENT`).
    Ambiguous,
    /// An API kind with a local device, or the other way round
    /// (`INVALID_ARGUMENT`).
    RemoteMismatch,
    /// The request has an enum number that this build does not know
    /// (`INVALID_ARGUMENT`).
    UnknownEnum,
}

/// The selection rules.
///
/// Every catalog calls this one function, so the rules and their codes are the
/// same on every host.
///
/// # Errors
/// [`SelectionError`].
pub fn select<'e>(
    entries: &'e [CatalogEntry],
    supported: &[KindSupport],
    request: &ModelRequest,
) -> Result<&'e CatalogEntry, SelectionError> {
    if let (Some(kind), Some(device)) = (request.kind, request.device) {
        let remote_kind = kind == ModelKind::Api;
        let remote_device = device == Device::Remote;
        if remote_kind != remote_device {
            return Err(SelectionError::RemoteMismatch);
        }
    }
    if let Some(name) = &request.name {
        let entry = entries
            .iter()
            .find(|entry| &entry.name == name)
            .ok_or(SelectionError::NotFound)?;
        if request.kind.is_some_and(|kind| kind != entry.kind) {
            return Err(SelectionError::KindMismatch);
        }
        if request.device.is_some_and(|device| device != entry.device) {
            return Err(SelectionError::DeviceMismatch);
        }
        return Ok(entry);
    }
    let matches: Vec<&CatalogEntry> = entries
        .iter()
        .filter(|entry| request.kind.is_none_or(|kind| kind == entry.kind))
        .filter(|entry| request.device.is_none_or(|device| device == entry.device))
        .collect();
    match matches.as_slice() {
        [one] => Ok(one),
        [] => {
            let Some(kind) = request.kind else {
                return Err(SelectionError::NotFound);
            };
            let Some(support) = supported.iter().find(|support| support.kind == kind) else {
                return Err(SelectionError::KindUnsupported);
            };
            match request.device {
                Some(device) if !support.devices.contains(&device) => {
                    Err(SelectionError::DeviceUnsupported)
                }
                _ => Err(SelectionError::NotFound),
            }
        }
        _ => Err(SelectionError::Ambiguous),
    }
}
