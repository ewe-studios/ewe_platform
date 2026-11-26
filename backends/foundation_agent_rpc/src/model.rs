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
//! keeps no loader, so nothing downstream can trigger a reload. Spec 83 F1/T1
//! proved concurrent generations on one candle model do not interfere.

#[cfg(any(feature = "candle", feature = "gguf"))]
use std::path::PathBuf;

#[cfg(feature = "candle")]
use foundation_ai::backends::candle::CandleBackend;
#[cfg(feature = "gguf")]
use foundation_ai::backends::llamacpp::{LlamaBackendConfig, LlamaBackends};
#[cfg(feature = "candle")]
use foundation_ai::types::ModelProvider;
#[cfg(any(feature = "candle", feature = "gguf"))]
use foundation_ai::types::ModelSpec;
use foundation_ai::types::{Model, ModelId};
use foundation_context::Ctx;

use crate::core::model::{Device, ModelKind};
use crate::error::{AgentRpcError, AgentRpcResult};

pub use crate::core::model::handle::ModelHandle;

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
    /// A GGUF model file run on llama.cpp.
    #[cfg(feature = "gguf")]
    Gguf {
        /// Name clients address the model by.
        name: String,
        /// The `.gguf` file.
        path: PathBuf,
        /// Where llama.cpp runs it.
        device: GgufDevice,
    },
}

/// The llama.cpp backend a [`ModelSource::Gguf`] model runs on.
#[cfg(feature = "gguf")]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum GgufDevice {
    /// CPU only (works everywhere llama.cpp builds).
    #[default]
    Cpu,
    /// CUDA or Vulkan, when llama.cpp was built with them.
    Gpu,
    /// Apple Metal.
    Metal,
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

    /// A GGUF model file served under `name`, on the CPU.
    #[cfg(feature = "gguf")]
    #[must_use]
    pub fn gguf(name: impl Into<String>, path: impl Into<PathBuf>) -> Self {
        Self::gguf_on(name, path, GgufDevice::Cpu)
    }

    /// A GGUF model file served under `name`, on `device`.
    #[cfg(feature = "gguf")]
    #[must_use]
    pub fn gguf_on(name: impl Into<String>, path: impl Into<PathBuf>, device: GgufDevice) -> Self {
        ModelSource::Gguf {
            name: name.into(),
            path: path.into(),
            device,
        }
    }
}

/// The catalog device of one llama.cpp backend.
#[cfg(feature = "gguf")]
fn gguf_device(device: GgufDevice) -> Device {
    match device {
        GgufDevice::Cpu => Device::Cpu,
        GgufDevice::Gpu => Device::Cuda { ordinal: 0 },
        GgufDevice::Metal => Device::Metal { ordinal: 0 },
    }
}

impl ModelHandle {
    /// Load the model described by `source`. `ctx` is the side-effect bundle
    /// the backend reads its clock from.
    ///
    /// # Errors
    /// [`AgentRpcError::ModelLoad`] when the backend cannot load the model.
    pub fn load(source: &ModelSource, ctx: Ctx) -> AgentRpcResult<Self> {
        match source {
            #[cfg(feature = "candle")]
            ModelSource::Candle { name, dir } => {
                let name = name.clone();
                let dir = dir.clone();
                Self::load_with(name.clone(), ModelKind::Candle, Device::Cpu, move || {
                    load_candle(&name, &dir, ctx)
                })
            }
            #[cfg(feature = "gguf")]
            ModelSource::Gguf { name, path, device } => {
                let name = name.clone();
                let path = path.clone();
                let device = *device;
                let (kind_device, loader_device) = (gguf_device(device), device);
                Self::load_with(name.clone(), ModelKind::Gguf, kind_device, move || {
                    load_gguf(&name, &path, loader_device, ctx)
                })
            }
            #[cfg(not(any(feature = "candle", feature = "gguf")))]
            _ => Err(AgentRpcError::Config(
                "build foundation_agent_rpc with the `candle` or `gguf` feature".into(),
            )
            .into()),
        }
    }

    /// Run `loader` exactly once and serve what it returns.
    ///
    /// For backends without a [`ModelSource`] variant, or a custom load: the
    /// loader runs here, never again.
    ///
    /// # Errors
    /// Whatever `loader` returns.
    pub fn load_with<M, F>(
        name: impl Into<String>,
        kind: ModelKind,
        device: Device,
        loader: F,
    ) -> AgentRpcResult<Self>
    where
        M: Model + Clone + Send + Sync + 'static,
        F: FnOnce() -> AgentRpcResult<M>,
    {
        let name = name.into();
        tracing::info!(model = %name, "loading model");
        let model = loader()?;
        Ok(Self::from_model(name, kind, device, model))
    }
}

#[cfg(feature = "candle")]
fn load_candle(
    name: &str,
    dir: &std::path::Path,
    ctx: Ctx,
) -> AgentRpcResult<foundation_ai::backends::candle::CandleModels> {
    let spec = ModelSpec {
        name: name.to_string(),
        id: ModelId::Name(name.to_string(), None),
        devices: None,
        model_location: Some(dir.to_path_buf()),
        lora_location: None,
    };
    CandleBackend::cpu(ctx)
        .get_model_by_spec(spec)
        .map_err(|e| AgentRpcError::ModelLoad(format!("{name} from {}: {e}", dir.display())).into())
}

/// Load a GGUF file with llama.cpp. Each generation on the returned model
/// builds its own llama.cpp context (and KV cache) from the shared weights, so
/// sessions do not interfere (pinned by `tests/gguf_share_tests.rs`).
#[cfg(feature = "gguf")]
fn load_gguf(
    name: &str,
    path: &std::path::Path,
    device: GgufDevice,
    ctx: Ctx,
) -> AgentRpcResult<foundation_ai::backends::llamacpp::LlamaModels> {
    let backend = match device {
        GgufDevice::Cpu => LlamaBackends::cpu(ctx),
        GgufDevice::Gpu => LlamaBackends::gpu(ctx),
        GgufDevice::Metal => LlamaBackends::metal(ctx),
    };
    let spec = ModelSpec {
        name: name.to_string(),
        id: ModelId::Name(name.to_string(), None),
        devices: None,
        model_location: Some(path.to_path_buf()),
        lora_location: None,
    };
    backend
        .load_model(spec, &LlamaBackendConfig::default())
        .map_err(|e| {
            AgentRpcError::ModelLoad(format!("{name} from {}: {e}", path.display())).into()
        })
}
