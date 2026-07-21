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
        Self::Cpu {
            config: CandleBackendConfig::default(),
            cache: Arc::new(Mutex::new(HashMap::new())),
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
        .map(|s| s.to_lowercase())
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
        CandleArchitecture::Llama => detected
            .clone()
            .unwrap_or_else(|| "llama".to_string()),
    };

    // Llama-family names candle's llama loader handles.
    if arch.contains("llama") {
        return build_llama_model(config_path, tokenizer_path, weights_files, dtype, device, spec);
    }
    if arch.contains("gemma2") {
        return build_gemma2_model(config_path, tokenizer_path, weights_files, dtype, device, spec);
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

    let file_refs: Vec<&std::path::Path> = weights_files.iter().map(std::path::PathBuf::as_path).collect();
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

    let cache = candle_llama::Cache::new(false, dtype, &llama_config, device).map_err(|e| {
        ModelProviderErrors::ModelErrors(ModelErrors::CandleModelLoad(format!(
            "Failed to create Llama KV cache: {e}"
        )))
    })?;
    let inner = CandleModelInner::Llama {
        model,
        cache,
        config: llama_config,
        dtype,
        device: device.clone(),
    };

    let chat_template = ChatTemplate::load(tokenizer_path);
    if chat_template.is_none() {
        tracing::debug!("no chat_template for {}; using plain prompt fallback", spec.name);
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
        serde_json::Value::Number(n) => n.as_u64().map(|v| v as u32),
        serde_json::Value::Array(a) => a.first().and_then(|v| v.as_u64()).map(|v| v as u32),
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

    let load_err = |e: String| {
        ModelProviderErrors::ModelErrors(ModelErrors::CandleModelLoad(e))
    };

    let config_content = std::fs::read_to_string(config_path)
        .map_err(|e| load_err(format!("Failed to read config: {e}")))?;
    let config: gemma2::Config = serde_json::from_str(&config_content)
        .map_err(|e| load_err(format!("Failed to parse Gemma2 Config: {e}")))?;

    let tokenizer = Tokenizer::from_file(tokenizer_path)
        .map_err(|e| load_err(format!("Failed to load tokenizer: {e}")))?;

    let file_refs: Vec<&std::path::Path> =
        weights_files.iter().map(std::path::PathBuf::as_path).collect();
    let vb = unsafe { VarBuilder::from_mmaped_safetensors(&file_refs, dtype, device) }
        .map_err(|e| load_err(format!("Failed to load weights: {e}")))?;

    // use_flash_attn = false: CPU/portable path.
    let model = gemma2::Model::new(false, &config, vb)
        .map_err(|e| load_err(format!("Failed to build Gemma2 model: {e}")))?;

    let eos_token_id = eos_from_config_json(config_path);
    let chat_template = ChatTemplate::load(tokenizer_path);

    Ok(CandleModels::new(
        CandleModelInner::Gemma2(model),
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

/// Architecture-specific model dispatch.
/// A loaded model plus the per-architecture inference state it owns.
///
/// WHY (decision 06): architectures manage KV state differently — Llama takes
/// an EXTERNAL `Cache`, Gemma2 keeps its cache INTERNAL and resets via
/// `clear_kv_cache`. Forcing every architecture through a Llama-shaped external
/// cache would break the moment a second one was added, so each variant owns
/// whatever state it needs and the callers below drive a uniform interface
/// (`forward`, `reset_cache`) without knowing which variant they hold.
enum CandleModelInner {
    Llama {
        model: candle_llama::Llama,
        cache: candle_llama::Cache,
        config: candle_llama::Config,
        dtype: DType,
        device: Device,
    },
    Gemma2(candle_transformers::models::gemma2::Model),
}

impl CandleModelInner {
    /// One forward pass. `seq_start` is the KV offset (0 for the prompt).
    fn forward(&mut self, input: &Tensor, seq_start: usize) -> Result<Tensor, candle_core::Error> {
        match self {
            CandleModelInner::Llama { model, cache, .. } => model.forward(input, seq_start, cache),
            CandleModelInner::Gemma2(m) => m.forward(input, seq_start),
        }
    }

    /// Reset the KV cache for a fresh generation.
    fn reset_cache(&mut self) -> Result<(), candle_core::Error> {
        match self {
            CandleModelInner::Llama {
                cache,
                config,
                dtype,
                device,
                ..
            } => {
                *cache = candle_llama::Cache::new(false, *dtype, config, device)?;
                Ok(())
            }
            CandleModelInner::Gemma2(m) => {
                m.clear_kv_cache();
                Ok(())
            }
        }
    }
}

struct CandleModelsState {
    model: CandleModelInner,
    tokenizer: Tokenizer,
    device: Device,
    eos_token_id: Option<u32>,
    spec: ModelSpec,
    last_usage: Option<UsageReport>,
    tokens_generated: usize,
    pricing: ModelUsageCosting,
    cumulative_cost: CostAccumulator,
    /// The model's own chat template (from `tokenizer_config.json`), if any.
    chat_template: Option<ChatTemplate>,
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
        env.add_function("raise_exception", |msg: String| -> Result<(), minijinja::Error> {
            Err(minijinja::Error::new(minijinja::ErrorKind::InvalidOperation, msg))
        });
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
/// Uses interior mutability (`Mutex`) so that `&self` methods can mutate
/// state during generation.
pub struct CandleModels {
    inner: Arc<Mutex<CandleModelsState>>,
}

impl core::fmt::Debug for CandleModels {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        let inner = self.inner.lock().unwrap();
        f.debug_struct("CandleModels")
            .field("spec", &inner.spec)
            .field("tokens_generated", &inner.tokens_generated)
            .finish_non_exhaustive()
    }
}

impl Clone for CandleModels {
    fn clone(&self) -> Self {
        Self {
            inner: Arc::clone(&self.inner),
        }
    }
}

impl CandleModels {
    fn new(
        model: CandleModelInner,
        tokenizer: Tokenizer,
        device: Device,
        eos_token_id: Option<u32>,
        spec: ModelSpec,
        chat_template: Option<ChatTemplate>,
    ) -> Self {
        Self {
            inner: Arc::new(Mutex::new(CandleModelsState {
                model,
                tokenizer,
                device,
                eos_token_id,
                spec,
                last_usage: None,
                tokens_generated: 0,
                pricing: ModelUsageCosting::default(),
                cumulative_cost: CostAccumulator::new(),
                chat_template,
            })),
        }
    }
}

impl Model for CandleModels {
    fn tool_formatter(&self) -> Box<dyn crate::types::ToolFormatter> {
        Box::new(TextBasedFormatter)
    }

    fn spec(&self) -> ModelSpec {
        self.inner.lock().unwrap().spec.clone()
    }

    fn costing(&self) -> GenerationResult<UsageReport> {
        let inner = self.inner.lock().unwrap();
        let cost = inner.cumulative_cost.result();
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
        let inner = self.inner.lock().unwrap();
        Some(ModelProviderDescriptor {
            id: "candle",
            name: "Candle",
            reasoning: false,
            api: crate::types::ModelAPI::Candle,
            provider: ModelProviders::CANDLE,
            base_url: None,
            inputs: crate::types::MessageType::TextAndImages,
            cost: inner.pricing,
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
        let mut inner = self.inner.lock().unwrap();

        let prompt = build_prompt(inner.chat_template.as_ref(), &interaction);

        let tokens = inner
            .tokenizer
            .encode(prompt.as_str(), true)
            .map_err(|e| GenerationError::Tokenizer(format!("{e}")))?;
        let input_ids = tokens.get_ids().to_vec();
        let input_len = input_ids.len();

        // Reset cache for fresh generation (per-arch — decision 06).
        inner.model.reset_cache().map_err(GenerationError::Candle)?;

        let device = inner.device.clone();

        let mut all_tokens = input_ids.clone();
        let mut next_tokens = input_ids;
        // One processor for the whole generation so its rng advances per token.
        let mut processor = build_logits_processor(&params);

        for index in 0..params.max_tokens {
            let input_tensor =
                Tensor::new(&next_tokens[..], &device).map_err(GenerationError::Candle)?;
            let input_tensor = input_tensor.unsqueeze(0).map_err(GenerationError::Candle)?;

            let seq_start = if index == 0 { 0 } else { all_tokens.len() - 1 };
            let logits = forward(&mut inner, &input_tensor, seq_start)?;

            let next_token = sample_next(&mut processor, &logits, &params, &all_tokens)
                .map_err(GenerationError::Candle)?;

            if inner.eos_token_id == Some(next_token) {
                break;
            }

            all_tokens.push(next_token);
            next_tokens = vec![next_token];

            let decoded = inner
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
        let output_text = inner
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

        inner.last_usage = Some(usage.clone());

        Ok(vec![Messages::Assistant {
            id: foundation_compact::ids::new_scru128(),
            model: inner.spec.id.clone(),
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
        let stream = CandleStream::new(self.clone(), interaction, specs)?;
        Ok(Box::new(stream))
    }
}

// ==================================
// CandleStream
// ==================================

/// Stream iterator for token-by-token Candle generation.
pub struct CandleStream {
    inner: Arc<Mutex<CandleStreamState>>,
}

struct CandleStreamState {
    model: CandleModels,
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
    #[allow(clippy::needless_pass_by_value)]
    fn new(
        model: CandleModels,
        interaction: ModelInteraction,
        specs: Option<ModelParams>,
    ) -> GenerationResult<Self> {
        let params = specs.unwrap_or_default();

        let (input_ids, _prompt) = {
            let inner = model.inner.lock().unwrap();
            let prompt = build_prompt(inner.chat_template.as_ref(), &interaction);
            let tokens = inner
                .tokenizer
                .encode(prompt.as_str(), true)
                .map_err(|e| GenerationError::Tokenizer(format!("{e}")))?;
            (tokens.get_ids().to_vec(), prompt)
        };

        let input_len = input_ids.len();

        // Reset cache (per-arch — decision 06).
        {
            let mut inner = model.inner.lock().unwrap();
            inner.model.reset_cache().map_err(GenerationError::Candle)?;
        }

        let processor = build_logits_processor(&params);

        Ok(Self {
            inner: Arc::new(Mutex::new(CandleStreamState {
                model,
                params,
                processor,
                all_tokens: input_ids,
                input_len,
                tokens_generated: 0,
                finished: false,
                initialized: false,
            })),
        })
    }
}

impl Iterator for CandleStream {
    type Item = Stream<Messages, ModelState>;

    #[allow(clippy::cast_precision_loss)]
    fn next(&mut self) -> Option<Self::Item> {
        let mut state = self.inner.lock().unwrap();

        if state.finished {
            return None;
        }

        if !state.initialized {
            state.initialized = true;
            return Some(Stream::Init);
        }

        if state.tokens_generated >= state.params.max_tokens {
            state.finished = true;
            return Some(Stream::Pending(ModelState::Finished));
        }

        let next_input: Vec<u32> = if state.tokens_generated == 0 {
            state.all_tokens.clone()
        } else {
            vec![*state.all_tokens.last().unwrap()]
        };

        let seq_start = if state.tokens_generated == 0 {
            0
        } else {
            state.all_tokens.len() - 1
        };

        let model_rc = Arc::clone(&state.model.inner);
        let params = state.params.clone();

        // Scope the model borrow so it's dropped before we mutate state
        let (next_token, token_str, eos_hit, spec) = {
            let mut model_inner = model_rc.lock().unwrap();

            let device = model_inner.device.clone();

        #[allow(clippy::manual_let_else)]
            let input_tensor = if let Ok(t) = Tensor::new(&next_input[..], &device) { t } else {
                state.finished = true;
                return Some(Stream::Pending(ModelState::Finished));
            };
        #[allow(clippy::manual_let_else)]
            let input_tensor = if let Ok(t) = input_tensor.unsqueeze(0) { t } else {
                state.finished = true;
                return Some(Stream::Pending(ModelState::Finished));
            };

        #[allow(clippy::manual_let_else)]
            let logits = if let Ok(l) = forward(&mut model_inner, &input_tensor, seq_start) { l } else {
                state.finished = true;
                return Some(Stream::Pending(ModelState::Finished));
            };

            let next_token = {
                // Reborrow so `processor` (mut) and `all_tokens` (shared) are
                // disjoint field borrows rather than two borrows of the guard.
                let st = &mut *state;
                match sample_next(&mut st.processor, &logits, &params, &st.all_tokens) {
                    Ok(t) => t,
                    Err(_) => {
                        st.finished = true;
                        return Some(Stream::Pending(ModelState::Finished));
                    }
                }
            };

            let eos_hit = model_inner
                .eos_token_id == Some(next_token);

            let token_str = model_inner
                .tokenizer
                .decode(&[next_token], true)
                .unwrap_or_default();

            let spec = model_inner.spec.clone();

            (next_token, token_str, eos_hit, spec)
        };

        if eos_hit {
            state.finished = true;
            return Some(Stream::Pending(ModelState::Finished));
        }

        state.all_tokens.push(next_token);
        state.tokens_generated += 1;

        Some(Stream::Next(Messages::Assistant {
            id: foundation_compact::ids::new_scru128(),
            model: spec.id.clone(),
            timestamp: SystemTime::now(),
            usage: UsageReport {
                input: state.input_len as f64,
                output: state.tokens_generated as f64,
                cache_read: 0.0,
                cache_write: 0.0,
                total_tokens: (state.input_len + state.tokens_generated) as f64,
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

fn forward(
    state: &mut CandleModelsState,
    input: &Tensor,
    seq_start: usize,
) -> GenerationResult<Tensor> {
    state
        .model
        .forward(input, seq_start)
        .map_err(GenerationError::Candle)
}

/// Convert an interaction to the `[{role, content}]` list HF chat templates
/// expect, folding system_prompt + soul + tool definitions into a leading
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
            system.push_str(&format!("- {}({})\n", t.name(), t.arg_summary()));
        }
    }
    if !system.is_empty() {
        out.push(serde_json::json!({"role": "system", "content": system}));
    }

    for msg in &interaction.messages {
        let (role, content) = match msg {
            Messages::User { content: UserModelContent::Text(t), .. } => ("user", t.content.clone()),
            Messages::Assistant { content: ModelOutput::Text(t), .. } => {
                ("assistant", t.content.clone())
            }
            Messages::ToolResult { content: UserModelContent::Text(t), .. } => {
                ("tool", t.content.clone())
            }
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
            (0, true) => Sampling::TopP { p: top_p, temperature },
            (0, false) => Sampling::All { temperature },
            (k, true) => Sampling::TopKThenTopP { k, p: top_p, temperature },
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
