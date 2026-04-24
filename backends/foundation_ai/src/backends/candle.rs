//! Candle inference backend — pure-Rust alternative to `llama.cpp`.
//!
//! Provides [`CandleBackend`] (CPU/CUDA/Metal) implementing [`ModelProvider`],
//! and [`CandleModels`] implementing [`Model`] for safetensors models via
//! HuggingFace's Candle framework.

use std::cell::RefCell;
use std::collections::HashMap;
use std::path::PathBuf;
use std::rc::Rc;
use std::time::SystemTime;

use candle_core::{DType, Device, Tensor};
use candle_nn::VarBuilder;
use candle_transformers::models::llama as candle_llama;
use tokenizers::Tokenizer;

use foundation_core::valtron::{Stream, StreamIterator};

use crate::errors::{
    GenerationError, GenerationResult, ModelErrors, ModelProviderErrors, ModelProviderResult,
};
use crate::types::{
    Messages, Model, ModelId, ModelInteraction, ModelOutput, ModelParams, ModelProvider,
    ModelProviders, ModelSpec, ModelState, StopReason, TextContent, UsageCosting, UsageReport,
    UserModelContent,
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
    /// Authentication credential (e.g. HuggingFace token).
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
        cache: Rc<RefCell<HashMap<String, CandleModels>>>,
    },
    /// CUDA GPU execution.
    #[cfg(feature = "candle-cuda")]
    Cuda {
        config: CandleBackendConfig,
        device_id: usize,
        cache: Rc<RefCell<HashMap<String, CandleModels>>>,
    },
    /// Apple Metal execution.
    #[cfg(feature = "candle-metal")]
    Metal {
        config: CandleBackendConfig,
        device_id: usize,
        cache: Rc<RefCell<HashMap<String, CandleModels>>>,
    },
}

impl CandleBackend {
    /// Create a CPU backend with default config.
    #[must_use]
    pub fn cpu() -> Self {
        Self::Cpu {
            config: CandleBackendConfig::default(),
            cache: Rc::new(RefCell::new(HashMap::new())),
        }
    }

    fn config(&self) -> &CandleBackendConfig {
        match self {
            CandleBackend::Cpu { config, .. } => config,
            #[cfg(feature = "candle-cuda")]
            CandleBackend::Cuda { config, .. } => config,
            #[cfg(feature = "candle-metal")]
            CandleBackend::Metal { config, .. } => config,
        }
    }

    fn device(&self) -> Result<Device, candle_core::Error> {
        match self {
            CandleBackend::Cpu { .. } => Ok(Device::Cpu),
            #[cfg(feature = "candle-cuda")]
            CandleBackend::Cuda { device_id, .. } => Device::new_cuda(*device_id),
            #[cfg(feature = "candle-metal")]
            CandleBackend::Metal { device_id, .. } => Device::new_metal(*device_id),
        }
    }

    fn cache(&self) -> &Rc<RefCell<HashMap<String, CandleModels>>> {
        match self {
            CandleBackend::Cpu { cache, .. } => cache,
            #[cfg(feature = "candle-cuda")]
            CandleBackend::Cuda { cache, .. } => cache,
            #[cfg(feature = "candle-metal")]
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

    fn create(
        self,
        config: Option<Self::Config>,
    ) -> ModelProviderResult<Self>
    where
        Self: Sized,
    {
        if let Some(config) = config {
            let cache = Rc::new(RefCell::new(HashMap::new()));
            match self {
                CandleBackend::Cpu { .. } => Ok(CandleBackend::Cpu { config, cache }),
                #[cfg(feature = "candle-cuda")]
                CandleBackend::Cuda { device_id, .. } => {
                    Ok(CandleBackend::Cuda { config, device_id, cache })
                }
                #[cfg(feature = "candle-metal")]
                CandleBackend::Metal { device_id, .. } => {
                    Ok(CandleBackend::Metal { config, device_id, cache })
                }
            }
        } else {
            Ok(self)
        }
    }

    fn describe(&self) -> ModelProviderResult<crate::types::ModelProviderDescriptor> {
        let variant = match self {
            CandleBackend::Cpu { .. } => "CPU",
            #[cfg(feature = "candle-cuda")]
            CandleBackend::Cuda { .. } => "CUDA",
            #[cfg(feature = "candle-metal")]
            CandleBackend::Metal { .. } => "Metal",
        };
        Ok(crate::types::ModelProviderDescriptor {
            id: "candle".to_string(),
            name: format!("Candle ({variant})"),
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
            context_window: self.config().context_length as u32,
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

        if let Some(model) = self.cache().borrow().get(&key) {
            return Ok(model.clone());
        }

        let path = model_spec.model_location.as_deref().ok_or_else(|| {
            ModelProviderErrors::NotFound(
                "CandleBackend requires model_location (local path to safetensors directory)".to_string(),
            )
        })?;

        let model = load_from_local(self, path, &model_spec)?;

        self.cache().borrow_mut().insert(key, model.clone());
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
            .filter_map(|e| e.ok())
            .map(|e| e.path())
            .filter(|p| {
                p.extension()
                    .map_or(false, |ext| ext == "safetensors")
            })
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

fn build_candle_model(
    config_path: &std::path::Path,
    tokenizer_path: &std::path::Path,
    weights_files: &[PathBuf],
    dtype: DType,
    device: &Device,
    architecture: &CandleArchitecture,
    spec: ModelSpec,
) -> ModelProviderResult<CandleModels> {
    match architecture {
        CandleArchitecture::Llama => build_llama_model(
            config_path,
            tokenizer_path,
            weights_files,
            dtype,
            device,
            spec,
        ),
        CandleArchitecture::Custom(name) => Err(ModelProviderErrors::ModelErrors(
            ModelErrors::UnsupportedArchitecture(name.clone()),
        )),
    }
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

    let file_refs: Vec<&std::path::Path> = weights_files.iter().map(|p| p.as_path()).collect();
    let vb = unsafe { VarBuilder::from_mmaped_safetensors(&file_refs, dtype, device) }.map_err(
        |e| {
            ModelProviderErrors::ModelErrors(ModelErrors::CandleModelLoad(format!(
                "Failed to load weights: {e}"
            )))
        },
    )?;

    let model = candle_llama::Llama::load(vb, &llama_config).map_err(|e| {
        ModelProviderErrors::ModelErrors(ModelErrors::CandleModelLoad(format!(
            "Failed to build Llama model: {e}"
        )))
    })?;

    let eos_token_id = llama_config.eos_token_id.as_ref().map(|eos| match eos {
        candle_llama::LlamaEosToks::Single(id) => *id,
        candle_llama::LlamaEosToks::Multiple(ids) => ids.first().copied().unwrap_or(0),
    });

    Ok(CandleModels::new(
        CandleModelInner::Llama(model),
        tokenizer,
        llama_config,
        dtype,
        device.clone(),
        eos_token_id,
        spec,
    ))
}

// ==================================
// CandleModels
// ==================================

/// Architecture-specific model dispatch.
enum CandleModelInner {
    Llama(candle_llama::Llama),
}

struct CandleModelsState {
    model: CandleModelInner,
    tokenizer: Tokenizer,
    config: candle_llama::Config,
    cache: candle_llama::Cache,
    device: Device,
    dtype: DType,
    eos_token_id: Option<u32>,
    spec: ModelSpec,
    last_usage: Option<UsageReport>,
    tokens_generated: usize,
}

/// Candle model wrapper implementing the [`Model`] trait.
///
/// Uses interior mutability (`RefCell`) so that `&self` methods can mutate
/// state during generation.
pub struct CandleModels {
    inner: Rc<RefCell<CandleModelsState>>,
}

impl core::fmt::Debug for CandleModels {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        let inner = self.inner.borrow();
        f.debug_struct("CandleModels")
            .field("spec", &inner.spec)
            .field("tokens_generated", &inner.tokens_generated)
            .finish_non_exhaustive()
    }
}

impl Clone for CandleModels {
    fn clone(&self) -> Self {
        Self {
            inner: Rc::clone(&self.inner),
        }
    }
}

impl CandleModels {
    fn new(
        model: CandleModelInner,
        tokenizer: Tokenizer,
        config: candle_llama::Config,
        dtype: DType,
        device: Device,
        eos_token_id: Option<u32>,
        spec: ModelSpec,
    ) -> Self {
        let cache = candle_llama::Cache::new(false, dtype, &config, &device)
            .expect("Failed to create KV cache");
        Self {
            inner: Rc::new(RefCell::new(CandleModelsState {
                model,
                tokenizer,
                config,
                cache,
                device,
                dtype,
                eos_token_id,
                spec,
                last_usage: None,
                tokens_generated: 0,
            })),
        }
    }
}

impl Model for CandleModels {
    fn spec(&self) -> ModelSpec {
        self.inner.borrow().spec.clone()
    }

    fn costing(&self) -> GenerationResult<UsageReport> {
        let inner = self.inner.borrow();
        Ok(inner.last_usage.clone().unwrap_or_else(|| UsageReport {
            input: 0.0,
            output: 0.0,
            cache_read: 0.0,
            cache_write: 0.0,
            total_tokens: 0.0,
            cost: UsageCosting {
                currency: "USD".to_string(),
                input: 0.0,
                output: 0.0,
                cache_read: 0.0,
                cache_write: 0.0,
                total_tokens: 0.0,
            },
        }))
    }

    fn generate(
        &self,
        interaction: ModelInteraction,
        specs: Option<ModelParams>,
    ) -> GenerationResult<Vec<Messages>> {
        let params = specs.unwrap_or_default();
        let mut inner = self.inner.borrow_mut();

        let prompt = build_prompt(&inner.tokenizer, &interaction);

        let tokens = inner
            .tokenizer
            .encode(prompt.as_str(), true)
            .map_err(|e| GenerationError::Tokenizer(format!("{e}")))?;
        let input_ids = tokens.get_ids().to_vec();
        let input_len = input_ids.len();

        // Reset cache for fresh generation
        let new_cache = candle_llama::Cache::new(false, inner.dtype, &inner.config, &inner.device)
            .map_err(GenerationError::Candle)?;
        inner.cache = new_cache;

        let device = inner.device.clone();

        let mut all_tokens = input_ids.clone();
        let mut next_tokens = input_ids;

        for index in 0..params.max_tokens {
            let input_tensor =
                Tensor::new(&next_tokens[..], &device).map_err(GenerationError::Candle)?;
            let input_tensor = input_tensor
                .unsqueeze(0)
                .map_err(GenerationError::Candle)?;

            let seq_start = if index == 0 { 0 } else { all_tokens.len() - 1 };
            let logits = forward(&mut inner, &input_tensor, seq_start)?;

            let next_token =
                sample_token(&logits, &params).map_err(GenerationError::Candle)?;

            if inner.eos_token_id.map_or(false, |eos| next_token == eos) {
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
            },
        };

        inner.last_usage = Some(usage.clone());

        Ok(vec![Messages::Assistant {
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
        }])
    }

    fn stream(
        &self,
        interaction: ModelInteraction,
        specs: Option<ModelParams>,
    ) -> GenerationResult<impl StreamIterator<D = Messages, P = ModelState>> {
        CandleStream::new(self.clone(), interaction, specs)
    }
}

// ==================================
// CandleStream
// ==================================

/// Stream iterator for token-by-token Candle generation.
pub struct CandleStream {
    inner: Rc<RefCell<CandleStreamState>>,
}

struct CandleStreamState {
    model: CandleModels,
    params: ModelParams,
    all_tokens: Vec<u32>,
    input_len: usize,
    tokens_generated: usize,
    finished: bool,
    initialized: bool,
}

impl CandleStream {
    fn new(
        model: CandleModels,
        interaction: ModelInteraction,
        specs: Option<ModelParams>,
    ) -> GenerationResult<Self> {
        let params = specs.unwrap_or_default();

        let (input_ids, _prompt) = {
            let inner = model.inner.borrow();
            let prompt = build_prompt(&inner.tokenizer, &interaction);
            let tokens = inner
                .tokenizer
                .encode(prompt.as_str(), true)
                .map_err(|e| GenerationError::Tokenizer(format!("{e}")))?;
            (tokens.get_ids().to_vec(), prompt)
        };

        let input_len = input_ids.len();

        // Reset cache
        {
            let mut inner = model.inner.borrow_mut();
            inner.cache = candle_llama::Cache::new(false, inner.dtype, &inner.config, &inner.device)
                .map_err(GenerationError::Candle)?;
        }

        Ok(Self {
            inner: Rc::new(RefCell::new(CandleStreamState {
                model,
                params,
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
        let mut state = self.inner.borrow_mut();

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

        let model_rc = Rc::clone(&state.model.inner);
        let params = state.params.clone();

        // Scope the model borrow so it's dropped before we mutate state
        let (next_token, token_str, eos_hit, spec) = {
            let mut model_inner = model_rc.borrow_mut();

            let device = model_inner.device.clone();

            let input_tensor = match Tensor::new(&next_input[..], &device) {
                Ok(t) => t,
                Err(_) => {
                    state.finished = true;
                    return Some(Stream::Pending(ModelState::Finished));
                }
            };
            let input_tensor = match input_tensor.unsqueeze(0) {
                Ok(t) => t,
                Err(_) => {
                    state.finished = true;
                    return Some(Stream::Pending(ModelState::Finished));
                }
            };

            let logits = match forward(&mut model_inner, &input_tensor, seq_start) {
                Ok(l) => l,
                Err(_) => {
                    state.finished = true;
                    return Some(Stream::Pending(ModelState::Finished));
                }
            };

            let next_token = match sample_token(&logits, &params) {
                Ok(t) => t,
                Err(_) => {
                    state.finished = true;
                    return Some(Stream::Pending(ModelState::Finished));
                }
            };

            let eos_hit = model_inner
                .eos_token_id
                .map_or(false, |eos| next_token == eos);

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
        }))
    }
}

// ==================================
// Helpers
// ==================================

fn forward(state: &mut CandleModelsState, input: &Tensor, seq_start: usize) -> GenerationResult<Tensor> {
    match &state.model {
        CandleModelInner::Llama(m) => m
            .forward(input, seq_start, &mut state.cache)
            .map_err(GenerationError::Candle),
    }
}

fn build_prompt(_tokenizer: &Tokenizer, interaction: &ModelInteraction) -> String {
    if interaction.messages.is_empty() {
        return interaction
            .system_prompt
            .clone()
            .unwrap_or_default();
    }

    let mut parts = Vec::new();
    if let Some(sys) = &interaction.system_prompt {
        parts.push(format!("System: {sys}"));
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

fn sample_token(logits: &Tensor, params: &ModelParams) -> Result<u32, candle_core::Error> {
    let logits = logits.squeeze(0)?;
    let seq_len = logits.dim(0)?;
    let last_logits = logits.get(seq_len - 1)?;

    if params.temperature <= 0.0 {
        return argmax(&last_logits);
    }

    let scaled = (&last_logits / params.temperature as f64)?;

    let top_k = params.top_k.round() as usize;
    if top_k > 0 {
        return sample_top_k(&scaled, top_k);
    }

    sample_from_logits(&scaled)
}

fn argmax(logits: &Tensor) -> Result<u32, candle_core::Error> {
    let vec: Vec<f32> = logits.to_vec1()?;
    let (max_idx, _) = vec
        .iter()
        .enumerate()
        .max_by(|(_, a), (_, b)| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal))
        .unwrap_or((0, &0.0));
    Ok(max_idx as u32)
}

fn sample_top_k(logits: &Tensor, k: usize) -> Result<u32, candle_core::Error> {
    let vec: Vec<f32> = logits.to_vec1()?;
    let mut indexed: Vec<(usize, f32)> = vec.into_iter().enumerate().collect();
    indexed.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
    indexed.truncate(k);

    let max_val = indexed[0].1;
    let exps: Vec<f32> = indexed.iter().map(|(_, v)| (v - max_val).exp()).collect();
    let sum: f32 = exps.iter().sum();
    let probs: Vec<f32> = exps.iter().map(|e| e / sum).collect();

    let r: f32 = fastrand::f32();
    let mut cumsum = 0.0;
    for (i, p) in probs.iter().enumerate() {
        cumsum += p;
        if r < cumsum {
            return Ok(indexed[i].0 as u32);
        }
    }
    Ok(indexed.last().unwrap().0 as u32)
}

fn sample_from_logits(logits: &Tensor) -> Result<u32, candle_core::Error> {
    let probs = candle_nn::ops::softmax(logits, 0)?;
    let vec: Vec<f32> = probs.to_vec1()?;

    let r: f32 = fastrand::f32();
    let mut cumsum = 0.0;
    for (i, p) in vec.iter().enumerate() {
        cumsum += p;
        if r < cumsum {
            return Ok(i as u32);
        }
    }
    Ok((vec.len() - 1) as u32)
}
