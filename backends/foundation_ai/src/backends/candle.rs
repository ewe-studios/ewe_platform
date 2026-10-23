//! Candle inference backend — pure-Rust alternative to `llama.cpp`.
//!
//! Provides [`CandleBackend`] (CPU/CUDA/Metal) implementing [`ModelProvider`],
//! and [`CandleModels`] implementing [`Model`] for safetensors models via
//! `HuggingFace`'s Candle framework.

use foundation_compact::SystemTime;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use candle_core::{DType, Device, Tensor};
use candle_nn::VarBuilder;
use candle_transformers::generation::{LogitsProcessor, Sampling};
use candle_transformers::models::llama as candle_llama;
use tokenizers::Tokenizer;

use foundation_core::valtron::Stream;

use crate::backends::backend_utils::flatten_tools;
use crate::costing::CostAccumulator;
use crate::errors::{
    GenerationError, GenerationResult, ModelErrors, ModelProviderErrors, ModelProviderResult,
};
use crate::types::{
    CostStatus, Messages, Model, ModelId, ModelInteraction, ModelOutput, ModelParams,
    ModelProvider, ModelProviderDescriptor, ModelProviders, ModelSpec, ModelState, ModelStreamBox,
    ModelUsageCosting, StopReason, TextBasedFormatter, TextContent, ToolFormatter, UsageCosting,
    UsageReport, UserModelContent,
};

// ==================================
// CandleBackendConfig
// ==================================

/// Configuration for Candle backend initialization.
#[derive(Debug)]
pub struct CandleBackendConfig {
    /// Context length (max tokens).
    pub context_length: usize,
    /// Data type for model weights.
    pub dtype: CandleDType,
    /// Model architecture to load.
    pub architecture: CandleArchitecture,
    /// Authentication credential (e.g. `HuggingFace` token).
    pub auth: Option<foundation_auth::AuthCredential>,
    /// Local cache directory for downloaded models.
    pub cache_dir: Option<PathBuf>,
}

impl Default for CandleBackendConfig {
    fn default() -> Self {
        Self {
            context_length: 4096,
            dtype: CandleDType::F32,
            architecture: CandleArchitecture::Llama,
            auth: None,
            cache_dir: None,
        }
    }
}

impl Clone for CandleBackendConfig {
    fn clone(&self) -> Self {
        Self {
            context_length: self.context_length,
            dtype: self.dtype,
            architecture: self.architecture.clone(),
            auth: None, // AuthCredential is not Clone; don't propagate
            cache_dir: self.cache_dir.clone(),
        }
    }
}

impl crate::types::AuthProvider for CandleBackendConfig {
    fn auth(&self) -> Option<&foundation_auth::AuthCredential> {
        self.auth.as_ref()
    }
}

impl CandleBackendConfig {
    #[must_use]
    pub fn builder() -> CandleBackendConfigBuilder {
        CandleBackendConfigBuilder::new()
    }
}

/// Supported data types for Candle model weights.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum CandleDType {
    F32,
    F16,
    BF16,
}

impl CandleDType {
    fn to_candle(self) -> DType {
        match self {
            CandleDType::F32 => DType::F32,
            CandleDType::F16 => DType::F16,
            CandleDType::BF16 => DType::BF16,
        }
    }
}

/// Supported model architectures.
#[derive(Debug, Clone, PartialEq)]
pub enum CandleArchitecture {
    Llama,
    Custom(String),
}

/// Builder for [`CandleBackendConfig`].
#[derive(Debug, Clone)]
pub struct CandleBackendConfigBuilder {
    config: CandleBackendConfig,
}

impl Default for CandleBackendConfigBuilder {
    fn default() -> Self {
        Self::new()
    }
}

impl CandleBackendConfigBuilder {
    #[must_use]
    pub fn new() -> Self {
        Self {
            config: CandleBackendConfig::default(),
        }
    }

    #[must_use]
    pub fn context_length(mut self, n: usize) -> Self {
        self.config.context_length = n;
        self
    }

    #[must_use]
    pub fn dtype(mut self, dtype: CandleDType) -> Self {
        self.config.dtype = dtype;
        self
    }

    #[must_use]
    pub fn architecture(mut self, arch: CandleArchitecture) -> Self {
        self.config.architecture = arch;
        self
    }

    #[must_use]
    pub fn auth(mut self, auth: impl Into<foundation_auth::AuthCredential>) -> Self {
        self.config.auth = Some(auth.into());
        self
    }

    #[must_use]
    pub fn cache_dir(mut self, path: impl Into<PathBuf>) -> Self {
        self.config.cache_dir = Some(path.into());
        self
    }

    #[must_use]
    pub fn build(self) -> CandleBackendConfig {
        self.config
    }
}

// ==================================
// CandleBackend
// ==================================

/// Hardware backend variants for Candle.
#[derive(Debug, Clone)]
pub enum CandleBackend {
    /// CPU-only execution.
    Cpu {
        config: CandleBackendConfig,
        cache: Arc<Mutex<HashMap<String, CandleModels>>>,
    },
    /// CUDA GPU execution.
    #[cfg(feature = "candle-cuda")]
    Cuda {
        config: CandleBackendConfig,
        device_id: usize,
        cache: Arc<Mutex<HashMap<String, CandleModels>>>,
    },
    /// Apple Metal execution.
    #[cfg(all(target_vendor = "apple", feature = "candle"))]
    Metal {
        config: CandleBackendConfig,
        device_id: usize,
        cache: Arc<Mutex<HashMap<String, CandleModels>>>,
    },
}

impl CandleBackend {
    /// Create a CPU backend with default config.
    #[must_use]
    pub fn cpu() -> Self {
        Self::cpu_with_config(CandleBackendConfig::default())
    }

    /// Create a CPU backend with an explicit config.
    #[must_use]
    pub fn cpu_with_config(config: CandleBackendConfig) -> Self {
        Self::Cpu {
            config,
            cache: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    /// Create a CUDA backend on `device_id`, with default config.
    ///
    /// Construction is lazy — the device is opened on first load, not here — so
    /// this cannot fail. Use [`try_cuda`](Self::try_cuda) when a caller needs a
    /// missing or unusable GPU reported up front rather than at load time.
    ///
    /// Before this existed, the `Cuda` variant had no public constructor at all:
    /// enabling the `candle-cuda` feature compiled the CUDA arms but nothing
    /// could build a value that reached them, so the backend never ran on a GPU.
    #[cfg(feature = "candle-cuda")]
    #[must_use]
    pub fn cuda(device_id: usize) -> Self {
        Self::cuda_with_config(device_id, CandleBackendConfig::default())
    }

    /// Create a CUDA backend on `device_id`, with an explicit config.
    #[cfg(feature = "candle-cuda")]
    #[must_use]
    pub fn cuda_with_config(device_id: usize, config: CandleBackendConfig) -> Self {
        Self::Cuda {
            config,
            device_id,
            cache: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    /// Create a CUDA backend on `device_id`, validating the device now.
    ///
    /// Opens the CUDA device eagerly so an absent GPU, a driver/library
    /// mismatch, or an out-of-range ordinal surfaces here as a clear error
    /// rather than deep inside the first model load. The returned backend
    /// reuses the already-opened device.
    ///
    /// # Errors
    ///
    /// Returns the underlying [`candle_core::Error`] when the device cannot be
    /// opened.
    #[cfg(feature = "candle-cuda")]
    pub fn try_cuda(device_id: usize) -> Result<Self, candle_core::Error> {
        // Open once to validate; the backend re-opens lazily via `device()`,
        // which is cheap once the driver context exists.
        let _device = Device::new_cuda(device_id)?;
        Ok(Self::cuda(device_id))
    }

    /// Create a Metal backend on `device_id`, with default config.
    #[cfg(all(target_vendor = "apple", feature = "candle"))]
    #[must_use]
    pub fn metal(device_id: usize) -> Self {
        Self::metal_with_config(device_id, CandleBackendConfig::default())
    }

    /// Create a Metal backend on `device_id`, with an explicit config.
    #[cfg(all(target_vendor = "apple", feature = "candle"))]
    #[must_use]
    pub fn metal_with_config(device_id: usize, config: CandleBackendConfig) -> Self {
        Self::Metal {
            config,
            device_id,
            cache: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    /// Select the best available accelerator, falling back to CPU.
    ///
    /// Tries CUDA device 0 (then Metal on Apple), and drops to CPU when no
    /// accelerator is usable. The choice is never silent: the selected device is
    /// logged at `info`, and a CUDA attempt that fails is logged at `warn` with
    /// the reason, so a run that quietly lands on CPU can still be diagnosed
    /// after the fact — a GPU job that secretly ran on CPU is the failure this
    /// method exists to make visible.
    #[must_use]
    pub fn best_available() -> Self {
        // A CUDA success returns eagerly. The Apple and non-Apple tails are
        // mutually exclusive by cfg, so exactly one is ever the tail expression
        // — no unreachable-code warning under any feature combination.
        #[cfg(feature = "candle-cuda")]
        {
            match Self::try_cuda(0) {
                Ok(backend) => {
                    tracing::info!(device = "cuda:0", "candle: using CUDA accelerator");
                    return backend;
                }
                Err(err) => {
                    tracing::warn!(%err, "candle: CUDA unavailable, falling back");
                }
            }
        }
        #[cfg(all(target_vendor = "apple", feature = "candle"))]
        {
            tracing::info!(device = "metal:0", "candle: using Metal accelerator");
            Self::metal(0)
        }
        #[cfg(not(all(target_vendor = "apple", feature = "candle")))]
        {
            tracing::info!(device = "cpu", "candle: using CPU");
            Self::cpu()
        }
    }

    fn config(&self) -> &CandleBackendConfig {
        match self {
            CandleBackend::Cpu { config, .. } => config,
            #[cfg(feature = "candle-cuda")]
            CandleBackend::Cuda { config, .. } => config,
            #[cfg(all(target_vendor = "apple", feature = "candle"))]
            CandleBackend::Metal { config, .. } => config,
        }
    }

    #[allow(clippy::unnecessary_wraps)]
    fn device(&self) -> Result<Device, candle_core::Error> {
        match self {
            CandleBackend::Cpu { .. } => Ok(Device::Cpu),
            #[cfg(feature = "candle-cuda")]
            CandleBackend::Cuda { device_id, .. } => Device::new_cuda(*device_id),
            #[cfg(all(target_vendor = "apple", feature = "candle"))]
            CandleBackend::Metal { device_id, .. } => Device::new_metal(*device_id),
        }
    }

    fn cache(&self) -> &Arc<Mutex<HashMap<String, CandleModels>>> {
        match self {
            CandleBackend::Cpu { cache, .. } => cache,
            #[cfg(feature = "candle-cuda")]
            CandleBackend::Cuda { cache, .. } => cache,
            #[cfg(all(target_vendor = "apple", feature = "candle"))]
            CandleBackend::Metal { cache, .. } => cache,
        }
    }

    fn model_id_key(model_id: &ModelId) -> String {
        format!("{model_id:?}")
    }
}

impl ModelProvider for CandleBackend {
    type Config = CandleBackendConfig;
    type Model = CandleModels;

    fn create(self, config: Option<Self::Config>) -> ModelProviderResult<Self>
    where
        Self: Sized,
    {
        if let Some(config) = config {
            let cache = Arc::new(Mutex::new(HashMap::new()));
            match self {
                CandleBackend::Cpu { .. } => Ok(CandleBackend::Cpu { config, cache }),
                #[cfg(feature = "candle-cuda")]
                CandleBackend::Cuda { device_id, .. } => Ok(CandleBackend::Cuda {
                    config,
                    device_id,
                    cache,
                }),
                #[cfg(all(target_vendor = "apple", feature = "candle"))]
                CandleBackend::Metal { device_id, .. } => Ok(CandleBackend::Metal {
                    config,
                    device_id,
                    cache,
                }),
            }
        } else {
            Ok(self)
        }
    }

    fn describe(&self) -> ModelProviderResult<crate::types::ModelProviderDescriptor> {
        let name = match self {
            CandleBackend::Cpu { .. } => "Candle (CPU)",
            #[cfg(feature = "candle-cuda")]
            CandleBackend::Cuda { .. } => "Candle (CUDA)",
            #[cfg(all(target_vendor = "apple", feature = "candle"))]
            CandleBackend::Metal { .. } => "Candle (Metal)",
        };
        Ok(crate::types::ModelProviderDescriptor {
            id: "candle",
            name,
            reasoning: false,
            api: crate::types::ModelAPI::Custom("candle".to_string()),
            provider: ModelProviders::Custom("candle".to_string()),
            base_url: None,
            inputs: crate::types::MessageType::Text,
            cost: crate::types::ModelUsageCosting {
                input: 0.0,
                output: 0.0,
                cache_read: 0.0,
                cache_write: 0.0,
            },
            context_window: u32::try_from(self.config().context_length).unwrap_or(u32::MAX),
            max_tokens: 2048,
        })
    }

    fn get_model(&self, model_id: ModelId) -> ModelProviderResult<Self::Model> {
        Err(ModelProviderErrors::NotFound(format!(
            "CandleBackend requires a local path via get_model_by_spec, got: {model_id:?}. \
             Use HuggingFaceCandleProvider to download from HuggingFace Hub."
        )))
    }

    fn get_model_by_spec(&self, model_spec: ModelSpec) -> ModelProviderResult<Self::Model> {
        let key = Self::model_id_key(&model_spec.id);

        if let Some(model) = self.cache().lock().unwrap().get(&key) {
            return Ok(model.clone());
        }

        let path = model_spec.model_location.as_deref().ok_or_else(|| {
            ModelProviderErrors::NotFound(
                "CandleBackend requires model_location (local path to safetensors directory)"
                    .to_string(),
            )
        })?;

        let model = load_from_local(self, path, &model_spec)?;

        self.cache().lock().unwrap().insert(key, model.clone());
        Ok(model)
    }

    fn get_one(&self, model_id: ModelId) -> ModelProviderResult<ModelSpec> {
        Err(ModelProviderErrors::NotFound(format!(
            "Model registry not supported for Candle: {model_id:?}"
        )))
    }

    fn get_all(&self, _model_id: ModelId) -> ModelProviderResult<Vec<ModelSpec>> {
        Err(ModelProviderErrors::NotFound(
            "Model registry not supported for Candle".to_string(),
        ))
    }
}

// ==================================
// Model Loading
// ==================================

fn load_from_local(
    backend: &CandleBackend,
    model_dir: &std::path::Path,
    model_spec: &ModelSpec,
) -> ModelProviderResult<CandleModels> {
    let config = backend.config();
    let device = backend.device().map_err(|e| {
        ModelProviderErrors::ModelErrors(ModelErrors::CandleModelLoad(format!(
            "Device init failed: {e}"
        )))
    })?;

    let config_path = model_dir.join("config.json");
    let tokenizer_path = model_dir.join("tokenizer.json");

    if !config_path.exists() {
        return Err(ModelProviderErrors::ModelErrors(
            ModelErrors::CandleModelLoad(format!(
                "config.json not found in {}",
                model_dir.display()
            )),
        ));
    }

    let weights_files = if model_dir.join("model.safetensors").exists() {
        vec![model_dir.join("model.safetensors")]
    } else {
        let mut files: Vec<PathBuf> = std::fs::read_dir(model_dir)
            .map_err(|e| {
                ModelProviderErrors::ModelErrors(ModelErrors::CandleModelLoad(format!(
                    "Failed to read model dir: {e}"
                )))
            })?
            .filter_map(std::result::Result::ok)
            .map(|e| e.path())
            .filter(|p| p.extension().is_some_and(|ext| ext == "safetensors"))
            .collect();
        files.sort();
        files
    };

    if weights_files.is_empty() {
        return Err(ModelProviderErrors::ModelErrors(
            ModelErrors::CandleModelLoad("No safetensors files found".to_string()),
        ));
    }

    let dtype = config.dtype.to_candle();

    build_candle_model(
        &config_path,
        &tokenizer_path,
        &weights_files,
        dtype,
        &device,
        &config.architecture,
        model_spec.clone(),
    )
}

/// Detect the model architecture from `config.json`'s `model_type` /
/// `architectures`, so a caller need not know a repo is Llama vs Qwen.
///
/// Returns the lowercased architecture name (e.g. `"llama"`, `"qwen2"`).
fn detect_architecture(config_path: &std::path::Path) -> Option<String> {
    let raw = std::fs::read_to_string(config_path).ok()?;
    let json: serde_json::Value = serde_json::from_str(&raw).ok()?;
    if let Some(mt) = json.get("model_type").and_then(|v| v.as_str()) {
        return Some(mt.to_lowercase());
    }
    // Fall back to the `architectures` array, e.g. ["LlamaForCausalLM"].
    json.get("architectures")
        .and_then(|a| a.as_array())
        .and_then(|a| a.first())
        .and_then(|v| v.as_str())
        .map(str::to_lowercase)
}

fn build_candle_model(
    config_path: &std::path::Path,
    tokenizer_path: &std::path::Path,
    weights_files: &[PathBuf],
    dtype: DType,
    device: &Device,
    architecture: &CandleArchitecture,
    spec: ModelSpec,
) -> ModelProviderResult<CandleModels> {
    // An explicit config override wins; otherwise detect from the model files.
    let detected = detect_architecture(config_path);
    let arch = match architecture {
        CandleArchitecture::Custom(name) => name.to_lowercase(),
        CandleArchitecture::Llama => detected.clone().unwrap_or_else(|| "llama".to_string()),
    };

    // Llama-family names candle's llama loader handles.
    if arch.contains("llama") {
        return build_llama_model(
            config_path,
            tokenizer_path,
            weights_files,
            dtype,
            device,
            spec,
        );
    }
    if arch.contains("gemma2") {
        return build_gemma2_model(
            config_path,
            tokenizer_path,
            weights_files,
            dtype,
            device,
            spec,
        );
    }

    // Unsupported: fail loudly with the DETECTED name, never silently load as
    // Llama (which would produce garbage that looks like a bad model rather than
    // a missing implementation). Supported set grows as loaders are added.
    Err(ModelProviderErrors::ModelErrors(
        ModelErrors::UnsupportedArchitecture(format!(
            "{arch} (detected from config.json; candle backend supports: llama, gemma2)"
        )),
    ))
}

fn build_llama_model(
    config_path: &std::path::Path,
    tokenizer_path: &std::path::Path,
    weights_files: &[PathBuf],
    dtype: DType,
    device: &Device,
    spec: ModelSpec,
) -> ModelProviderResult<CandleModels> {
    let config_content = std::fs::read_to_string(config_path).map_err(|e| {
        ModelProviderErrors::ModelErrors(ModelErrors::CandleModelLoad(format!(
            "Failed to read config: {e}"
        )))
    })?;
    let raw_config: candle_llama::LlamaConfig =
        serde_json::from_str(&config_content).map_err(|e| {
            ModelProviderErrors::ModelErrors(ModelErrors::CandleModelLoad(format!(
                "Failed to parse LlamaConfig: {e}"
            )))
        })?;
    let llama_config = raw_config.into_config(false);

    let tokenizer = Tokenizer::from_file(tokenizer_path).map_err(|e| {
        ModelProviderErrors::ModelErrors(ModelErrors::CandleModelLoad(format!(
            "Failed to load tokenizer: {e}"
        )))
    })?;

    let file_refs: Vec<&std::path::Path> = weights_files
        .iter()
        .map(std::path::PathBuf::as_path)
        .collect();
    let vb =
        unsafe { VarBuilder::from_mmaped_safetensors(&file_refs, dtype, device) }.map_err(|e| {
            ModelProviderErrors::ModelErrors(ModelErrors::CandleModelLoad(format!(
                "Failed to load weights: {e}"
            )))
        })?;

    let model = candle_llama::Llama::load(vb, &llama_config).map_err(|e| {
        ModelProviderErrors::ModelErrors(ModelErrors::CandleModelLoad(format!(
            "Failed to build Llama model: {e}"
        )))
    })?;

    let eos_token_id = llama_config.eos_token_id.as_ref().map(|eos| match eos {
        candle_llama::LlamaEosToks::Single(id) => *id,
        candle_llama::LlamaEosToks::Multiple(ids) => ids.first().copied().unwrap_or(0),
    });

    // KV state is per generation (`CandleWeights::start_generation`); probing
    // one here surfaces a cache-construction failure at load time.
    candle_llama::Cache::new(true, dtype, &llama_config, device).map_err(|e| {
        ModelProviderErrors::ModelErrors(ModelErrors::CandleModelLoad(format!(
            "Failed to create Llama KV cache: {e}"
        )))
    })?;
    let inner = CandleWeights::Llama {
        model,
        config: llama_config,
        dtype,
        device: device.clone(),
    };

    let chat_template = ChatTemplate::load(tokenizer_path);
    if chat_template.is_none() {
        tracing::debug!(
            "no chat_template for {}; using plain prompt fallback",
            spec.name
        );
    }

    Ok(CandleModels::new(
        inner,
        tokenizer,
        device.clone(),
        eos_token_id,
        spec,
        chat_template,
    ))
}

/// Read `eos_token_id` from a raw HF `config.json` (int or first-of-array).
fn eos_from_config_json(config_path: &std::path::Path) -> Option<u32> {
    let raw = std::fs::read_to_string(config_path).ok()?;
    let json: serde_json::Value = serde_json::from_str(&raw).ok()?;
    match json.get("eos_token_id")? {
        // An id that does not fit in u32 is not a valid token id: ignore it.
        serde_json::Value::Number(n) => n.as_u64().and_then(|v| u32::try_from(v).ok()),
        serde_json::Value::Array(a) => a
            .first()
            .and_then(serde_json::Value::as_u64)
            .and_then(|v| u32::try_from(v).ok()),
        _ => None,
    }
}

/// Load a Gemma2 model. Mirrors `build_llama_model` but Gemma2 owns its KV cache
/// internally (decision 06) and carries no eos in its `Config`, so eos comes
/// from the raw `config.json`.
fn build_gemma2_model(
    config_path: &std::path::Path,
    tokenizer_path: &std::path::Path,
    weights_files: &[PathBuf],
    dtype: DType,
    device: &Device,
    spec: ModelSpec,
) -> ModelProviderResult<CandleModels> {
    use candle_transformers::models::gemma2;

    let load_err = |e: String| ModelProviderErrors::ModelErrors(ModelErrors::CandleModelLoad(e));

    let config_content = std::fs::read_to_string(config_path)
        .map_err(|e| load_err(format!("Failed to read config: {e}")))?;
    let config: gemma2::Config = serde_json::from_str(&config_content)
        .map_err(|e| load_err(format!("Failed to parse Gemma2 Config: {e}")))?;

    let tokenizer = Tokenizer::from_file(tokenizer_path)
        .map_err(|e| load_err(format!("Failed to load tokenizer: {e}")))?;

    let file_refs: Vec<&std::path::Path> = weights_files
        .iter()
        .map(std::path::PathBuf::as_path)
        .collect();
    let vb = unsafe { VarBuilder::from_mmaped_safetensors(&file_refs, dtype, device) }
        .map_err(|e| load_err(format!("Failed to load weights: {e}")))?;

    // use_flash_attn = false: CPU/portable path.
    let model = gemma2::Model::new(false, &config, vb)
        .map_err(|e| load_err(format!("Failed to build Gemma2 model: {e}")))?;

    let eos_token_id = eos_from_config_json(config_path);
    let chat_template = ChatTemplate::load(tokenizer_path);

    Ok(CandleModels::new(
        CandleWeights::Gemma2(model),
        tokenizer,
        device.clone(),
        eos_token_id,
        spec,
        chat_template,
    ))
}

// ==================================
// CandleModels
// ==================================

/// A loaded model's read-only weights, shared by every generation.
///
/// WHY (decision 06, spec 65 F1/T1): architectures manage KV state differently
/// — Llama takes an EXTERNAL `Cache`, Gemma2 keeps its cache INTERNAL. That
/// state used to live next to the weights inside one `Mutex`, so two
/// generations on the same loaded model (two agent sessions sharing one model)
/// reset and overwrote each other's KV cache between tokens. The weights are
/// now shared and immutable; each generation owns its KV state
/// ([`CandleGeneration`]).
///
/// WHAT: the weights plus whatever each architecture needs to start a fresh
/// generation.
///
/// HOW: candle tensors are reference-counted, so the Gemma2 template model is
/// cloned per generation without copying weights.
enum CandleWeights {
    Llama {
        model: candle_llama::Llama,
        config: candle_llama::Config,
        dtype: DType,
        device: Device,
    },
    /// Never run directly: the template each generation clones (with its KV
    /// cache cleared).
    Gemma2(candle_transformers::models::gemma2::Model),
}

impl CandleWeights {
    /// Fresh per-generation inference state over these weights.
    fn start_generation(&self) -> Result<CandleGeneration, candle_core::Error> {
        match self {
            CandleWeights::Llama {
                config,
                dtype,
                device,
                ..
            } => Ok(CandleGeneration::Llama {
                // The stream feeds one token per step after the prompt, so the
                // KV cache must be on: without it every token after the first
                // attends only to itself.
                cache: candle_llama::Cache::new(true, *dtype, config, device)?,
            }),
            CandleWeights::Gemma2(template) => {
                let mut model = template.clone();
                model.clear_kv_cache();
                Ok(CandleGeneration::Gemma2(Box::new(model)))
            }
        }
    }
}

/// The inference state one generation owns: its KV cache.
enum CandleGeneration {
    Llama { cache: candle_llama::Cache },
    Gemma2(Box<candle_transformers::models::gemma2::Model>),
}

impl CandleGeneration {
    /// One forward pass. `seq_start` is the KV offset (0 for the prompt).
    fn forward(
        &mut self,
        weights: &CandleWeights,
        input: &Tensor,
        seq_start: usize,
    ) -> Result<Tensor, candle_core::Error> {
        match (self, weights) {
            (CandleGeneration::Llama { cache }, CandleWeights::Llama { model, .. }) => {
                model.forward(input, seq_start, cache)
            }
            (CandleGeneration::Gemma2(model), CandleWeights::Gemma2(_)) => {
                model.forward(input, seq_start)
            }
            _ => Err(candle_core::Error::msg(
                "candle generation state does not match the loaded architecture",
            )),
        }
    }
}

/// Everything about a loaded model that generations only read.
struct CandleShared {
    weights: CandleWeights,
    tokenizer: Tokenizer,
    device: Device,
    eos_token_id: Option<u32>,
    spec: ModelSpec,
    pricing: ModelUsageCosting,
    /// The model's own chat template (from `tokenizer_config.json`), if any.
    chat_template: Option<ChatTemplate>,
}

/// Usage bookkeeping — the only mutable state shared between generations.
struct CandleUsage {
    last_usage: Option<UsageReport>,
    tokens_generated: usize,
    cumulative_cost: CostAccumulator,
}

/// A model's chat template plus the special-token strings it references.
///
/// WHY: `build_prompt` used to invent a `System:/User:/Assistant:` format no
/// model was trained on and ignored the tokenizer entirely — the same
/// prompt-construction defect fixed for llama.cpp in docs/fixes/006/007. Modern
/// instruct models ship a Jinja `chat_template`; rendering it is what gives the
/// model the turn structure it expects.
#[derive(Clone)]
struct ChatTemplate {
    template: String,
    bos_token: String,
    eos_token: String,
}

impl ChatTemplate {
    /// Load from a `tokenizer_config.json` sibling of `tokenizer.json`.
    /// Returns `None` when the file or the `chat_template` field is absent.
    fn load(tokenizer_path: &std::path::Path) -> Option<Self> {
        let cfg_path = tokenizer_path.parent()?.join("tokenizer_config.json");
        let raw = std::fs::read_to_string(cfg_path).ok()?;
        let json: serde_json::Value = serde_json::from_str(&raw).ok()?;

        // chat_template may be a string or (rarely) an array of named templates;
        // take the string form only — the common case.
        let template = json.get("chat_template")?.as_str()?.to_string();

        let token_str = |key: &str| -> String {
            match json.get(key) {
                Some(serde_json::Value::String(s)) => s.clone(),
                // Some configs express it as { "content": "<bos>", ... }.
                Some(serde_json::Value::Object(o)) => o
                    .get("content")
                    .and_then(|c| c.as_str())
                    .unwrap_or_default()
                    .to_string(),
                _ => String::new(),
            }
        };

        Some(Self {
            template,
            bos_token: token_str("bos_token"),
            eos_token: token_str("eos_token"),
        })
    }

    /// Render the template for `messages`, appending the generation prompt.
    fn render(&self, messages: &[serde_json::Value]) -> Result<String, minijinja::Error> {
        let mut env = minijinja::Environment::new();
        // HF templates occasionally call raise_exception(); make it a no-op-ish
        // error so a template that uses it fails cleanly rather than at parse.
        env.add_function(
            "raise_exception",
            |msg: String| -> Result<(), minijinja::Error> {
                Err(minijinja::Error::new(
                    minijinja::ErrorKind::InvalidOperation,
                    msg,
                ))
            },
        );
        // HF chat templates are written for Python's Jinja and call Python str
        // methods minijinja lacks natively (e.g. `content.strip()` in the
        // Llama-2 template). Bridge the common ones so real templates render.
        env.set_unknown_method_callback(
            |_state, value, method, _args| -> Result<minijinja::Value, minijinja::Error> {
                use minijinja::{Error, ErrorKind, Value};
                if let Some(s) = value.as_str() {
                    match method {
                        "strip" => return Ok(Value::from(s.trim())),
                        "lstrip" => return Ok(Value::from(s.trim_start())),
                        "rstrip" => return Ok(Value::from(s.trim_end())),
                        _ => {}
                    }
                }
                Err(Error::new(
                    ErrorKind::UnknownMethod,
                    format!("string has no method named {method}"),
                ))
            },
        );
        env.add_template("chat", &self.template)?;
        let tmpl = env.get_template("chat")?;
        tmpl.render(minijinja::context! {
            messages => messages,
            bos_token => self.bos_token,
            eos_token => self.eos_token,
            add_generation_prompt => true,
        })
    }
}

/// Candle model wrapper implementing the [`Model`] trait.
///
/// WHY: one loaded model must serve many sessions at once (spec 65 F1) without
/// a reload per session and without generations disturbing each other.
///
/// WHAT: a cheap-to-clone handle. Clones share the loaded weights, tokenizer
/// and template (`Arc<CandleShared>`); each `generate`/`stream` call owns its
/// own KV state, so concurrent generations on clones of one handle run in
/// parallel and produce exactly what they would produce alone.
///
/// HOW: no lock is held across a forward pass; only usage bookkeeping sits
/// behind a `Mutex`.
pub struct CandleModels {
    shared: Arc<CandleShared>,
    usage: Arc<Mutex<CandleUsage>>,
}

impl core::fmt::Debug for CandleModels {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        let tokens_generated = self
            .usage
            .lock()
            .map(|u| u.tokens_generated)
            .unwrap_or_default();
        f.debug_struct("CandleModels")
            .field("spec", &self.shared.spec)
            .field("tokens_generated", &tokens_generated)
            .finish_non_exhaustive()
    }
}

impl Clone for CandleModels {
    fn clone(&self) -> Self {
        Self {
            shared: Arc::clone(&self.shared),
            usage: Arc::clone(&self.usage),
        }
    }
}

impl CandleModels {
    fn new(
        weights: CandleWeights,
        tokenizer: Tokenizer,
        device: Device,
        eos_token_id: Option<u32>,
        spec: ModelSpec,
        chat_template: Option<ChatTemplate>,
    ) -> Self {
        Self {
            shared: Arc::new(CandleShared {
                weights,
                tokenizer,
                device,
                eos_token_id,
                spec,
                pricing: ModelUsageCosting::default(),
                chat_template,
            }),
            usage: Arc::new(Mutex::new(CandleUsage {
                last_usage: None,
                tokens_generated: 0,
                cumulative_cost: CostAccumulator::new(),
            })),
        }
    }

    /// True when `self` and `other` are handles to the same loaded weights —
    /// i.e. one load, shared, rather than two copies in memory.
    #[must_use]
    pub fn shares_weights_with(&self, other: &CandleModels) -> bool {
        Arc::ptr_eq(&self.shared, &other.shared)
    }

    fn encode_prompt(&self, interaction: &ModelInteraction) -> GenerationResult<Vec<u32>> {
        let prompt = build_prompt(self.shared.chat_template.as_ref(), interaction);
        let tokens = self
            .shared
            .tokenizer
            .encode(prompt.as_str(), true)
            .map_err(|e| GenerationError::Tokenizer(format!("{e}")))?;
        Ok(tokens.get_ids().to_vec())
    }
}

impl Model for CandleModels {
    fn tool_formatter(&self) -> Box<dyn crate::types::ToolFormatter> {
        Box::new(TextBasedFormatter)
    }

    fn spec(&self) -> ModelSpec {
        self.shared.spec.clone()
    }

    fn costing(&self) -> GenerationResult<UsageReport> {
        let usage = self
            .usage
            .lock()
            .map_err(|_| GenerationError::Generic("candle usage lock poisoned".into()))?;
        let cost = usage.cumulative_cost.result();
        Ok(UsageReport {
            input: 0.0,
            output: 0.0,
            cache_read: 0.0,
            cache_write: 0.0,
            total_tokens: cost.total_tokens,
            cost,
        })
    }

    fn descriptor(&self) -> Option<ModelProviderDescriptor> {
        Some(ModelProviderDescriptor {
            id: "candle",
            name: "Candle",
            reasoning: false,
            api: crate::types::ModelAPI::Candle,
            provider: ModelProviders::CANDLE,
            base_url: None,
            inputs: crate::types::MessageType::TextAndImages,
            cost: self.shared.pricing,
            context_window: 0,
            max_tokens: 0,
        })
    }

    fn generate(
        &self,
        interaction: ModelInteraction,
        specs: Option<ModelParams>,
    ) -> GenerationResult<Vec<Messages>> {
        let params = specs.unwrap_or_default();
        let shared = &self.shared;

        let input_ids = self.encode_prompt(&interaction)?;
        let input_len = input_ids.len();

        // This generation's own KV state (per-arch — decision 06).
        let mut generation = shared
            .weights
            .start_generation()
            .map_err(GenerationError::Candle)?;

        let mut all_tokens = input_ids.clone();
        let mut next_tokens = input_ids;
        // One processor for the whole generation so its rng advances per token.
        let mut processor = build_logits_processor(&params);

        for index in 0..params.max_tokens {
            let input_tensor =
                Tensor::new(&next_tokens[..], &shared.device).map_err(GenerationError::Candle)?;
            let input_tensor = input_tensor.unsqueeze(0).map_err(GenerationError::Candle)?;

            let seq_start = if index == 0 { 0 } else { all_tokens.len() - 1 };
            let logits = generation
                .forward(&shared.weights, &input_tensor, seq_start)
                .map_err(GenerationError::Candle)?;

            let next_token = sample_next(&mut processor, &logits, &params, &all_tokens)
                .map_err(GenerationError::Candle)?;

            if shared.eos_token_id == Some(next_token) {
                break;
            }

            all_tokens.push(next_token);
            next_tokens = vec![next_token];

            let decoded = shared
                .tokenizer
                .decode(&[next_token], true)
                .map_err(|e| GenerationError::Tokenizer(format!("{e}")))?;
            if params
                .stop_tokens
                .iter()
                .any(|s| decoded.contains(s.as_str()))
            {
                break;
            }
        }

        let output_ids = &all_tokens[input_len..];
        let output_text = shared
            .tokenizer
            .decode(output_ids, true)
            .map_err(|e| GenerationError::Tokenizer(format!("{e}")))?;

        #[allow(clippy::cast_precision_loss)]
        let usage = UsageReport {
            input: input_len as f64,
            output: output_ids.len() as f64,
            cache_read: 0.0,
            cache_write: 0.0,
            total_tokens: all_tokens.len() as f64,
            cost: UsageCosting {
                currency: "USD".to_string(),
                input: 0.0,
                output: 0.0,
                cache_read: 0.0,
                cache_write: 0.0,
                total_tokens: 0.0,
                status: CostStatus::Actual,
            },
        };

        self.usage
            .lock()
            .map_err(|_| GenerationError::Generic("candle usage lock poisoned".into()))?
            .last_usage = Some(usage.clone());

        Ok(vec![Messages::Assistant {
            id: foundation_compact::ids::new_scru128(),
            model: shared.spec.id.clone(),
            timestamp: SystemTime::now(),
            usage,
            content: ModelOutput::Text(TextContent {
                content: output_text,
                signature: None,
            }),
            stop_reason: StopReason::Stop,
            provider: ModelProviders::Custom("candle".to_string()),
            error_detail: None,
            signature: None,
            metadata: None,
        }])
    }

    fn stream(
        &self,
        interaction: ModelInteraction,
        specs: Option<ModelParams>,
    ) -> GenerationResult<ModelStreamBox> {
        let stream = CandleStream::new(self.clone(), &interaction, specs)?;
        Ok(Box::new(stream))
    }
}

// ==================================
// CandleStream
// ==================================

/// Stream iterator for token-by-token Candle generation.
///
/// Owns its generation's KV state, so it never touches another stream's.
pub struct CandleStream {
    model: CandleModels,
    generation: CandleGeneration,
    params: ModelParams,
    all_tokens: Vec<u32>,
    input_len: usize,
    tokens_generated: usize,
    finished: bool,
    initialized: bool,
    /// Persisted across polls so its rng advances token to token (spec-60/S3).
    processor: LogitsProcessor,
}

impl CandleStream {
    fn new(
        model: CandleModels,
        interaction: &ModelInteraction,
        specs: Option<ModelParams>,
    ) -> GenerationResult<Self> {
        let params = specs.unwrap_or_default();
        let input_ids = model.encode_prompt(interaction)?;
        let input_len = input_ids.len();
        let generation = model
            .shared
            .weights
            .start_generation()
            .map_err(GenerationError::Candle)?;
        let processor = build_logits_processor(&params);

        Ok(Self {
            model,
            generation,
            params,
            processor,
            all_tokens: input_ids,
            input_len,
            tokens_generated: 0,
            finished: false,
            initialized: false,
        })
    }

    /// One decoding step: forward the pending input, sample, decode.
    /// Returns `(token, text, is_eos)`.
    fn step(&mut self) -> Result<(u32, String, bool), String> {
        let shared = Arc::clone(&self.model.shared);

        let (next_input, seq_start) = if self.tokens_generated == 0 {
            (self.all_tokens.clone(), 0)
        } else {
            let last = *self
                .all_tokens
                .last()
                .ok_or_else(|| "candle stream has no tokens".to_string())?;
            (vec![last], self.all_tokens.len() - 1)
        };

        let input = Tensor::new(&next_input[..], &shared.device)
            .and_then(|t| t.unsqueeze(0))
            .map_err(|e| format!("candle input tensor: {e}"))?;
        let logits = self
            .generation
            .forward(&shared.weights, &input, seq_start)
            .map_err(|e| format!("candle forward pass: {e}"))?;
        let token = sample_next(&mut self.processor, &logits, &self.params, &self.all_tokens)
            .map_err(|e| format!("candle sampling: {e}"))?;
        let text = shared
            .tokenizer
            .decode(&[token], true)
            .map_err(|e| format!("candle token decode: {e}"))?;
        Ok((token, text, shared.eos_token_id == Some(token)))
    }
}

/// No readiness signal: this stream never yields `Wait` while waiting on
/// external I/O, so the default `wake` (`NotSupported`) is correct.
impl foundation_core::valtron::WakeableStream for CandleStream {}

impl Iterator for CandleStream {
    type Item = Stream<Messages, ModelState>;

    #[allow(clippy::cast_precision_loss)]
    fn next(&mut self) -> Option<Self::Item> {
        if self.finished {
            return None;
        }

        if !self.initialized {
            self.initialized = true;
            return Some(Stream::Init);
        }

        if self.tokens_generated >= self.params.max_tokens {
            self.finished = true;
            return Some(Stream::Pending(ModelState::Finished));
        }

        let (next_token, token_str, eos_hit) = match self.step() {
            Ok(step) => step,
            Err(msg) => {
                // Surface the failure (the loop maps it to a FailedAction)
                // instead of ending the stream as if generation had finished.
                tracing::error!(error = %msg, "candle stream step failed");
                self.finished = true;
                return Some(Stream::Pending(ModelState::Error(msg)));
            }
        };

        if eos_hit {
            self.finished = true;
            return Some(Stream::Pending(ModelState::Finished));
        }

        self.all_tokens.push(next_token);
        self.tokens_generated += 1;

        Some(Stream::Next(Messages::Assistant {
            id: foundation_compact::ids::new_scru128(),
            model: self.model.shared.spec.id.clone(),
            timestamp: SystemTime::now(),
            usage: UsageReport {
                input: self.input_len as f64,
                output: self.tokens_generated as f64,
                cache_read: 0.0,
                cache_write: 0.0,
                total_tokens: (self.input_len + self.tokens_generated) as f64,
                cost: UsageCosting {
                    currency: "USD".to_string(),
                    input: 0.0,
                    output: 0.0,
                    cache_read: 0.0,
                    cache_write: 0.0,
                    total_tokens: 0.0,
                    status: CostStatus::Actual,
                },
            },
            content: ModelOutput::Text(TextContent {
                content: token_str,
                signature: None,
            }),
            stop_reason: StopReason::Stop,
            provider: ModelProviders::Custom("candle".to_string()),
            error_detail: None,
            signature: None,
            metadata: None,
        }))
    }
}

// ==================================
// Helpers
// ==================================

/// Convert an interaction to the `[{role, content}]` list HF chat templates
/// expect, folding `system_prompt` + soul + tool definitions into a leading
/// system message.
fn messages_as_json(interaction: &ModelInteraction) -> Vec<serde_json::Value> {
    let mut out = Vec::new();

    let mut system = match (&interaction.system_prompt, &interaction.soul) {
        (Some(sys), Some(soul)) => format!("{sys}\n\n{soul}"),
        (Some(sys), None) => sys.clone(),
        (None, Some(soul)) => soul.clone(),
        (None, None) => String::new(),
    };
    let tools = flatten_tools(&interaction.tools_shed);
    if !tools.is_empty() {
        if !system.is_empty() {
            system.push_str("\n\n");
        }
        if let Some(instr) = TextBasedFormatter.tool_calling_instructions() {
            system.push_str(&instr);
            system.push('\n');
        }
        system.push_str("Available tools:\n");
        for t in &tools {
            system.push_str("- ");
            system.push_str(t.name());
            system.push('(');
            system.push_str(&t.arg_summary());
            system.push_str(")\n");
        }
    }
    if !system.is_empty() {
        out.push(serde_json::json!({"role": "system", "content": system}));
    }

    for msg in &interaction.messages {
        let (role, content) = match msg {
            Messages::User {
                content: UserModelContent::Text(t),
                ..
            } => ("user", t.content.clone()),
            Messages::Assistant {
                content: ModelOutput::Text(t),
                ..
            } => ("assistant", t.content.clone()),
            Messages::ToolResult {
                content: UserModelContent::Text(t),
                ..
            } => ("tool", t.content.clone()),
            _ => continue,
        };
        out.push(serde_json::json!({"role": role, "content": content}));
    }
    out
}

/// Build the prompt for `interaction`, preferring the model's chat template.
///
/// When the model ships a chat template we render it (spec-60/S5), so an
/// instruct model gets its trained turn structure. Only when there is no
/// template — or rendering fails — do we fall back to the plain
/// `System:/User:/Assistant:` transcript below.
fn build_prompt(chat_template: Option<&ChatTemplate>, interaction: &ModelInteraction) -> String {
    if let Some(ct) = chat_template {
        let messages = messages_as_json(interaction);
        match ct.render(&messages) {
            Ok(rendered) => return rendered,
            Err(e) => {
                tracing::debug!("chat template render failed ({e}); using plain fallback");
            }
        }
    }

    let mut parts = Vec::new();

    // System section: combine system_prompt + soul
    let system_content = match (&interaction.system_prompt, &interaction.soul) {
        (Some(sys), Some(soul)) => Some(format!("{sys}\n\n{soul}")),
        (Some(sys), None) => Some(sys.clone()),
        (None, Some(soul)) => Some(soul.clone()),
        (None, None) => None,
    };

    if let Some(sys) = &system_content {
        parts.push(format!("System: {sys}"));
    }

    // Tool definitions from tools_shed
    let shed = &interaction.tools_shed;
    {
        let all_tools = flatten_tools(shed);
        if !all_tools.is_empty() {
            let formatter = TextBasedFormatter;
            if let Some(instructions) = formatter.tool_calling_instructions() {
                parts.push(format!("System: {instructions}"));
            }
            let tool_defs = all_tools
                .iter()
                .map(|t| format!("- {}({})", t.name(), t.arg_summary()))
                .collect::<Vec<_>>()
                .join("\n");
            parts.push(format!("Tools:\n{tool_defs}"));
        }
    }

    if interaction.messages.is_empty() {
        if parts.is_empty() {
            return interaction.system_prompt.clone().unwrap_or_default();
        }
        parts.push("Assistant:".to_string());
        return parts.join("\n");
    }

    for msg in &interaction.messages {
        match msg {
            Messages::User { content, .. } => {
                if let UserModelContent::Text(TextContent { content, .. }) = content {
                    parts.push(format!("User: {content}"));
                }
            }
            Messages::Assistant { content, .. } => {
                if let ModelOutput::Text(TextContent { content, .. }) = content {
                    parts.push(format!("Assistant: {content}"));
                }
            }
            Messages::ToolResult { content, .. } => {
                if let UserModelContent::Text(TextContent { content, .. }) = content {
                    parts.push(format!("Tool: {content}"));
                }
            }
        }
    }
    parts.push("Assistant:".to_string());
    parts.join("\n")
}

/// Build candle's `LogitsProcessor` from `ModelParams`.
///
/// WHY: the previous sampler was hand-rolled, honoured only temperature and
/// top-k (top-p and repeat-penalty in `ModelParams` were silently ignored), and
/// used unseeded `fastrand`, so candle generation could not be reproduced. This
/// adopts candle's own sampler, which implements the full set, and threads the
/// seed so a caller can force determinism (spec-60/S3).
///
/// HOW: the processor holds its own rng and must persist across the whole
/// generation so the rng advances token to token — build it ONCE per
/// generate()/stream, not per token.
fn build_logits_processor(params: &ModelParams) -> LogitsProcessor {
    // A fixed default seed keeps runs reproducible even when the caller sets
    // none; an explicit seed overrides it.
    let seed = u64::from(params.seed.unwrap_or(299_792_458));
    let temperature = f64::from(params.temperature);

    let sampling = if temperature <= 0.0 {
        // temperature <= 0 => greedy/argmax (also the deterministic test mode).
        Sampling::ArgMax
    } else {
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        let top_k = params.top_k.round() as usize;
        let top_p = f64::from(params.top_p);
        let use_top_p = top_p > 0.0 && top_p < 1.0;
        match (top_k, use_top_p) {
            (0, true) => Sampling::TopP {
                p: top_p,
                temperature,
            },
            (0, false) => Sampling::All { temperature },
            (k, true) => Sampling::TopKThenTopP {
                k,
                p: top_p,
                temperature,
            },
            (k, false) => Sampling::TopK { k, temperature },
        }
    };

    LogitsProcessor::from_sampling(seed, sampling)
}

/// Reduce raw model logits to a 1-D `[vocab]` tensor for the last position.
fn last_position_logits(logits: &Tensor) -> Result<Tensor, candle_core::Error> {
    let dims = logits.dims();
    match dims.len() {
        3 => {
            let seq_len = dims[1];
            logits.narrow(1, seq_len - 1, 1)?.squeeze(0)?.squeeze(0)
        }
        2 => {
            let seq_len = dims[0];
            logits.get(seq_len - 1)
        }
        1 => Ok(logits.clone()),
        _ => Err(candle_core::Error::msg(format!(
            "Unexpected logits rank: {} (dims: {dims:?})",
            dims.len()
        ))),
    }
}

/// Sample the next token: apply repeat penalty (if configured) over the recent
/// context, then draw from `processor`.
fn sample_next(
    processor: &mut LogitsProcessor,
    logits: &Tensor,
    params: &ModelParams,
    context: &[u32],
) -> Result<u32, candle_core::Error> {
    let last = last_position_logits(logits)?;

    let penalized = if (params.repeat_penalty - 1.0).abs() > f32::EPSILON && !context.is_empty() {
        // Match the llama.cpp path: penalize over the last 64 tokens.
        let start = context.len().saturating_sub(64);
        candle_transformers::utils::apply_repeat_penalty(
            &last,
            params.repeat_penalty,
            &context[start..],
        )?
    } else {
        last
    };

    processor.sample(&penalized)
}
