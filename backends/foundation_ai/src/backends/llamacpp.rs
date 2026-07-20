//! `llama.cpp` [`ModelBackend`] implementations.
//!
//! This module provides the `llama.cpp` integration for `foundation_ai`,
//! enabling local execution of GGUF-format models.
//!
//! # Architecture
//!
//! - [`LlamaBackends`] - Hardware variant enum (CPU/GPU/Metal) implementing `ModelProvider`
//! - [`LlamaBackendConfig`] - Configuration with builder pattern for provider initialization
//! - [`LlamaModels`] - `Model` trait implementation with interior mutability
//! - [`LlamaCppStream`] - `StreamIterator` implementation for token-by-token streaming

use infrastructure_llama_cpp::context::params::LlamaModelContextParams;
use infrastructure_llama_cpp::context::LlamaModelContext;
use infrastructure_llama_cpp::llama_backend::LlamaBackend;
use infrastructure_llama_cpp::llama_batch::LlamaBatch;
use infrastructure_llama_cpp::model::params::LlamaModelParams;
use infrastructure_llama_cpp::model::{
    AddBos, LlamaChatMessage, LlamaChatTemplate, LlamaModel, Special,
};
use infrastructure_llama_cpp::sampling::LlamaSampler;
use infrastructure_llama_cpp::speculative::{LlamaMtp, LlamaMtpModel, MtpSampling};
use infrastructure_llama_cpp::token::LlamaToken;

use foundation_compact::SystemTime;
use std::fmt::Write;
use std::num::NonZeroU32;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use foundation_core::valtron::Stream;

use crate::backends::llamacpp_helpers::build_sampler_chain;
use crate::costing::{calculate_cost, CostAccumulator};
use crate::errors::{
    GenerationError, GenerationResult, ModelErrors, ModelProviderErrors, ModelProviderResult,
};
use crate::types::base_types::{
    CostStatus, KVCacheType, Messages, Model, ModelId, ModelInteraction, ModelOutput, ModelParams,
    ModelProvider, ModelProviderDescriptor, ModelProviders, ModelSpec, ModelState, ModelStreamBox,
    ModelUsageCosting, SplitMode, StopReason, TextBasedFormatter, TextContent, ToolFormatter,
    ToolShed, UsageCosting, UsageReport, UserModelContent,
};

// ==================================
// LlamaBackendConfig
// ==================================

/// Which speculative-decoding draft strategy to use.
///
/// Currently only Multi-Token Prediction (MTP) is implemented; the enum is
/// non-exhaustive to leave room for llama.cpp's other draft types
/// (`draft-simple`, `draft-eagle3`, n-gram, …) without a breaking change.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum SpeculativeKind {
    /// Multi-Token Prediction — use the model's MTP head as the draft
    /// (llama.cpp `draft-mtp`).
    Mtp,
}

/// Opt-in speculative-decoding configuration for the llama.cpp backend.
///
/// WHY: modern models (GLM 5.2, Qwen 3.6, Gemma 4, …) ship an MTP head that
/// llama.cpp can use to draft several tokens per target step, speeding up
/// generation. Most models ship no such head, so this is opt-in and
/// capability-gated — see [`spec-51`](../../../../specifications/51-llama-mtp-speculative).
///
/// WHAT: the draft strategy plus its parameters. `mtp_model` points at a
/// separate MTP head GGUF when the head is not embedded in the main model.
///
/// HOW: set it on [`LlamaBackendConfig::speculative`] (default `None` = plain
/// decoding). When present and the model supports it, the backend engages the
/// speculative decode path; when the model does not support it, model creation
/// fails rather than silently ignoring the request.
#[derive(Debug, Clone)]
pub struct SpeculativeConfig {
    /// The draft strategy.
    pub kind: SpeculativeKind,
    /// Path to a separate MTP head GGUF, if the head is not part of the main
    /// model file. `None` uses the main model's embedded head.
    pub mtp_model: Option<PathBuf>,
    /// Maximum number of draft tokens to propose per target step
    /// (llama.cpp `n_max`).
    pub n_max: u32,
}

impl SpeculativeConfig {
    /// Multi-Token Prediction config. `mtp_model` is the (optional) separate
    /// MTP head GGUF; `n_max` is the max draft tokens per step.
    #[must_use]
    pub fn mtp(mtp_model: Option<PathBuf>, n_max: u32) -> Self {
        Self {
            kind: SpeculativeKind::Mtp,
            mtp_model,
            n_max,
        }
    }
}

/// Configuration for llama.cpp backend initialization.
///
/// Provides sensible defaults with a builder pattern for customization.
/// Use this to configure GPU offloading, context size, batch size, and more.
///
/// # Example
///
/// ```rust,no_run
/// use foundation_ai::backends::llamacpp::LlamaBackendConfig;
///
/// let config = LlamaBackendConfig::builder()
///     .n_gpu_layers(32)
///     .context_length(4096)
///     .build();
/// ```
#[derive(Debug, Clone)]
pub struct LlamaBackendConfig {
    /// Number of layers to offload to GPU.
    pub n_gpu_layers: u32,
    /// Context length (max tokens the model can attend to).
    pub context_length: usize,
    /// Batch size for inference.
    pub batch_size: usize,
    /// Number of threads for CPU operations.
    pub n_threads: usize,
    /// Enable memory mapping for model loading.
    pub use_mmap: bool,
    /// Enable memory locking to prevent swapping.
    pub use_mlock: bool,
    /// [`KVCacheType`] (`F16`, `Q8_0`, etc.).
    pub kv_cache_type: KVCacheType,
    /// Split mode for multi-GPU.
    pub split_mode: SplitMode,
    /// Main GPU index for multi-GPU systems.
    pub main_gpu: u32,
    /// Opt-in speculative decoding (MTP). `None` = standard single-token
    /// decoding (the default; zero behavior change unless set).
    pub speculative: Option<SpeculativeConfig>,
}

impl Default for LlamaBackendConfig {
    fn default() -> Self {
        Self {
            n_gpu_layers: 0,       // CPU-only by default
            context_length: 4096,  // Common default
            batch_size: 512,       // llama.cpp default
            n_threads: num_cpus(), // Use all available CPUs
            use_mmap: true,        // Enable mmap for faster loading
            use_mlock: false,      // Don't mlock by default
            kv_cache_type: KVCacheType::F16,
            split_mode: SplitMode::Layer,
            main_gpu: 0,
            speculative: None, // standard decoding by default
        }
    }
}

/// Get the number of CPUs available (cross-platform).
#[must_use]
fn num_cpus() -> usize {
    std::thread::available_parallelism().map_or(4, std::num::NonZeroUsize::get)
}

impl crate::types::base_types::AuthProvider for LlamaBackendConfig {
    fn auth(&self) -> Option<&foundation_auth::AuthCredential> {
        None
    }
}

impl LlamaBackendConfig {
    /// Create a new config builder with default values.
    #[must_use]
    pub fn builder() -> LlamaBackendConfigBuilder {
        LlamaBackendConfigBuilder::new()
    }

    /// Create a new config with all default values.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Convert this config into llama.cpp model parameters.
    #[must_use]
    pub fn to_model_params(&self) -> LlamaModelParams {
        let mut params = LlamaModelParams::default();
        params = params.with_n_gpu_layers(self.n_gpu_layers);
        // Additional model parameters can be added here as needed
        params
    }

    /// Convert this config into llama.cpp context parameters.
    ///
    /// Note: embeddings are intentionally NOT force-enabled here — that would
    /// switch the context into embedding mode and break text generation, since
    /// generation and embeddings share one context. Embedding requests are
    /// handled on their own path.
    #[must_use]
    #[allow(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        clippy::cast_possible_wrap
    )]
    pub fn to_context_params(&self) -> LlamaModelContextParams {
        let mut params = LlamaModelContextParams::default();
        params = params.with_n_ctx(NonZeroU32::new(self.context_length as u32));
        params = params.with_n_batch(self.batch_size as u32);
        params = params.with_n_threads(self.n_threads as i32);
        params
    }
}

/// Builder for [`LlamaBackendConfig`].
#[derive(Debug, Clone)]
pub struct LlamaBackendConfigBuilder {
    config: LlamaBackendConfig,
}

impl LlamaBackendConfigBuilder {
    /// Create a new builder with default values.
    #[must_use]
    pub fn new() -> Self {
        Self {
            config: LlamaBackendConfig::default(),
        }
    }

    /// Set the number of GPU layers to offload.
    #[must_use]
    pub fn n_gpu_layers(mut self, n: u32) -> Self {
        self.config.n_gpu_layers = n;
        self
    }

    /// Set the context length (max tokens).
    #[must_use]
    pub fn context_length(mut self, n: usize) -> Self {
        self.config.context_length = n;
        self
    }

    /// Set the batch size.
    #[must_use]
    pub fn batch_size(mut self, n: usize) -> Self {
        self.config.batch_size = n;
        self
    }

    /// Set the number of threads.
    #[must_use]
    pub fn n_threads(mut self, n: usize) -> Self {
        self.config.n_threads = n;
        self
    }

    /// Enable or disable memory mapping.
    #[must_use]
    pub fn use_mmap(mut self, enabled: bool) -> Self {
        self.config.use_mmap = enabled;
        self
    }

    /// Enable or disable memory locking.
    #[must_use]
    pub fn use_mlock(mut self, enabled: bool) -> Self {
        self.config.use_mlock = enabled;
        self
    }

    /// Set the KV cache type.
    #[must_use]
    pub fn kv_cache_type(mut self, t: KVCacheType) -> Self {
        self.config.kv_cache_type = t;
        self
    }

    /// Set the split mode.
    #[must_use]
    pub fn split_mode(mut self, s: SplitMode) -> Self {
        self.config.split_mode = s;
        self
    }

    /// Set the main GPU index.
    #[must_use]
    pub fn main_gpu(mut self, gpu: u32) -> Self {
        self.config.main_gpu = gpu;
        self
    }

    /// Enable speculative decoding with an explicit [`SpeculativeConfig`].
    #[must_use]
    pub fn speculative(mut self, spec: SpeculativeConfig) -> Self {
        self.config.speculative = Some(spec);
        self
    }

    /// Enable Multi-Token Prediction (MTP) speculative decoding.
    ///
    /// `mtp_model` is an optional separate MTP head GGUF (`None` uses the main
    /// model's embedded head); `n_max` is the max draft tokens per step. Only
    /// takes effect on models that support MTP — see [`SpeculativeConfig`].
    #[must_use]
    pub fn mtp(mut self, mtp_model: Option<PathBuf>, n_max: u32) -> Self {
        self.config.speculative = Some(SpeculativeConfig::mtp(mtp_model, n_max));
        self
    }

    /// Build the final config.
    #[must_use]
    pub fn build(self) -> LlamaBackendConfig {
        self.config
    }
}

impl Default for LlamaBackendConfigBuilder {
    fn default() -> Self {
        Self::new()
    }
}

// ==================================
// LlamaModels
// ==================================

/// Internal state for `LlamaModels` with interior mutability.
struct LlamaModelsInner {
    model: Arc<LlamaModel>,
    context: LlamaModelContextParams,
    #[allow(dead_code)]
    sampler: Option<LlamaSampler>,
    spec: ModelSpec,
    pricing: ModelUsageCosting,
    cumulative_cost: CostAccumulator,
    /// The backend config this model was loaded with — carries the opt-in
    /// speculative (MTP) config plus context sizing used to build the MTP
    /// generator. Validated at model creation.
    config: LlamaBackendConfig,
    /// Cached MTP draft **model** (the ~100 MB head weights), lazily loaded on
    /// the first MTP-backed `generate()`/`stream()` and shared read-only across
    /// all subsequent calls. Each generation builds its own cheap engine
    /// (contexts + speculator) from this shared model, so concurrent generations
    /// never contend. `None` until first use; only populated when
    /// `config.speculative` selects MTP with a head path.
    mtp_model: Arc<Mutex<Option<Arc<LlamaMtpModel>>>>,
}

/// `llama.cpp` model wrapper implementing the `Model` trait.
///
/// Uses interior mutability (`Mutex`) so that `&self` methods can mutate
/// the context and sampler during generation.
pub struct LlamaModels {
    inner: Arc<Mutex<LlamaModelsInner>>,
}

// SAFETY: LlamaModels uses Arc<Mutex<...>> for interior mutability.
// The underlying llama.cpp pointers are protected by the mutex.
unsafe impl Send for LlamaModels {}
unsafe impl Sync for LlamaModels {}

impl Clone for LlamaModels {
    fn clone(&self) -> Self {
        Self {
            inner: Arc::clone(&self.inner),
        }
    }
}

impl LlamaModels {
    /// Create a new `LlamaModels` instance carrying the backend `config`
    /// (already validated against the model's MTP capabilities).
    #[allow(clippy::arc_with_non_send_sync)]
    fn new_with_config(
        model: LlamaModel,
        context: LlamaModelContextParams,
        spec: ModelSpec,
        config: LlamaBackendConfig,
    ) -> Self {
        Self {
            inner: Arc::new(Mutex::new(LlamaModelsInner {
                model: Arc::new(model),
                context,
                sampler: None,
                spec,
                pricing: ModelUsageCosting::default(),
                cumulative_cost: CostAccumulator::new(),
                config,
                mtp_model: Arc::new(Mutex::new(None)),
            })),
        }
    }

    /// Get the model spec.
    #[must_use]
    /// # Errors
    /// Returns [`GenerationError`] if generation fails.
    pub fn spec(&self) -> ModelSpec {
        self.inner.lock().unwrap().spec.clone()
    }
}

impl Model for LlamaModels {
    fn spec(&self) -> ModelSpec {
        self.inner.lock().unwrap().spec.clone()
    }

    fn tool_formatter(&self) -> Box<dyn ToolFormatter> {
        Box::new(TextBasedFormatter)
    }

    fn descriptor(&self) -> Option<ModelProviderDescriptor> {
        let inner = self.inner.lock().unwrap();
        Some(ModelProviderDescriptor {
            id: "llamacpp",
            name: "llama.cpp",
            reasoning: false,
            api: crate::types::base_types::ModelAPI::Custom("llamacpp".into()),
            provider: ModelProviders::LLAMACPP,
            base_url: None,
            inputs: crate::types::base_types::MessageType::TextAndImages,
            cost: inner.pricing,
            context_window: 0,
            max_tokens: 0,
        })
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

    fn generate(
        &self,
        interaction: ModelInteraction,
        specs: Option<ModelParams>,
    ) -> GenerationResult<Vec<Messages>> {
        let backend = LlamaBackend::init_or_get().map_err(Into::<GenerationError>::into)?;

        // Get model, spec, context params, the full backend config (carries any
        // opt-in speculative/MTP setup + runtime sizing), and the shared MTP
        // draft-model cache slot.
        let (model, spec, ctx_params, backend_config, mtp_model_slot) = {
            let inner = self.inner.lock().unwrap();
            (
                Arc::clone(&inner.model),
                inner.spec.clone(),
                inner.context.clone(),
                inner.config.clone(),
                Arc::clone(&inner.mtp_model),
            )
        };

        let params = specs.unwrap_or_default();
        let is_embedding = is_embedding_request(&interaction.messages);

        // Apply chat template if messages are present
        let prompt = if interaction.messages.is_empty() {
            interaction.system_prompt.unwrap_or_default()
        } else {
            apply_chat_template(&model, &interaction)?
        };

        if is_embedding {
            let mut ctx = model
                .new_context(&backend, ctx_params)
                .map_err(Into::<GenerationError>::into)?;
            return generate_embeddings(&model, &mut ctx, &prompt, &spec);
        }

        // Speculative (MTP) engaged path — spec-51. We only run the MTP engine
        // when the caller supplied a separate MTP head GGUF (`mtp_model:
        // Some(path)`), the shape modern models ship. On success return; on
        // failure `warn!` once and fall through to standard decoding (G5). We
        // build the standard `ctx` + sampler ONLY on the fallback path, so a
        // successful MTP run never allocates a wasted context.
        if let Some(spec_cfg) = &backend_config.speculative {
            if let (SpeculativeKind::Mtp, Some(mtp_path)) = (spec_cfg.kind, &spec_cfg.mtp_model) {
                match run_mtp_generation(
                    &mtp_model_slot,
                    &model,
                    &prompt,
                    &params,
                    &backend_config,
                    spec_cfg,
                    mtp_path,
                ) {
                    Ok(messages) => return Ok(messages),
                    Err(err) => {
                        tracing::warn!(
                            error = %err,
                            n_max = spec_cfg.n_max,
                            "MTP speculative generation failed — falling back to standard decoding"
                        );
                    }
                }
            }
        }

        // Standard decoding path (also the MTP fallback).
        let mut ctx = model
            .new_context(&backend, ctx_params)
            .map_err(Into::<GenerationError>::into)?;
        let mut sampler = build_sampler_chain(&params);
        generate_text(&model, &mut ctx, &mut sampler, &prompt, &params, &spec)
    }

    fn stream(
        &self,
        interaction: ModelInteraction,
        specs: Option<ModelParams>,
    ) -> GenerationResult<ModelStreamBox> {
        let stream = LlamaCppStream::new(self.clone(), &interaction, specs)?;
        Ok(Box::new(stream))
    }
}

// ==================================
// LlamaCppStream
// ==================================

/// Stream iterator for token-by-token generation.
///
/// Implements `StreamIterator` to yield `Messages` one token at a time.
/// Holds a clone of `LlamaModels` to access the model/context during iteration.
pub struct LlamaCppStream {
    inner: Arc<Mutex<LlamaCppStreamInner>>,
}

// LlamaCppStreamInner holds FFI pointers (LlamaModelContext, LlamaSampler)
// that are !Send. These are only ever accessed inside Iterator::next() on
// the thread that polls the stream — they never cross thread boundaries.
// The Arc<Mutex<>> wrapper is for type-level Send compatibility with the
// valtron executor; the Mutex ensures exclusive access.
unsafe impl Send for LlamaCppStreamInner {}

/// MTP streaming state — present when speculative (MTP) decoding is engaged for
/// this stream. The stream owns its own [`LlamaMtp`] engine (exclusive access
/// for the stream's lifetime) and drives it one [`LlamaMtp::step`] per poll.
struct MtpStreamState {
    engine: LlamaMtp,
    /// Chat-templated prompt (parity with the non-streaming path).
    prompt: String,
    sampling: MtpSampling,
    n_predict: i32,
    /// Whether `begin()` has been called yet.
    started: bool,
}

/// Internal stream state - uses Clone for context
struct LlamaCppStreamInner {
    /// Reference to the model (cloned from `LlamaModels`)
    model: LlamaModels,
    /// MTP streaming engine + state, when speculative decoding is engaged.
    mtp: Option<MtpStreamState>,
    /// Backend (owned)
    backend: Option<LlamaBackend>,
    /// Context (cloneable)
    ctx: Option<LlamaModelContext<'static>>,
    /// Sampler
    sampler: Option<LlamaSampler>,
    /// Current position in sequence
    current_pos: i32,
    /// Tokens generated so far
    tokens_generated: i32,
    /// Maximum tokens to generate
    max_tokens: i32,
    /// Input token count
    input_tokens: usize,
    /// Whether prompt has been evaluated
    prompt_evaluated: bool,
    /// Whether stream is finished
    finished: bool,
    /// Stored prompt string (evaluated lazily)
    prompt: Option<String>,
}

impl LlamaCppStream {
    /// Create a new stream for the given model and interaction.
    ///
    /// # Errors
    ///
    /// Returns a `GenerationError` if stream initialization fails.
    pub fn new(
        model: LlamaModels,
        interaction: &ModelInteraction,
        specs: Option<ModelParams>,
    ) -> GenerationResult<Self> {
        // Initialize backend upfront - this is where we can properly report errors
        let backend = LlamaBackend::init_or_get().map_err(|e| {
            GenerationError::Generic(format!("Failed to initialize llama.cpp backend: {e}"))
        })?;

        // Build sampler from params
        let params = specs.unwrap_or_default();
        let sampler = build_sampler_chain(&params);

        // Get max_tokens from params
        #[allow(
            clippy::cast_possible_truncation,
            clippy::cast_possible_wrap,
            clippy::cast_sign_loss
        )] // FFI boundary: llama.cpp uses i32 for token counts
        let max_tokens = params.max_tokens as i32;

        // Build system prompt from system_prompt + soul + tool definitions
        let mut prompt = String::new();
        if let Some(sys) = &interaction.system_prompt {
            prompt.push_str(sys);
        }
        if let Some(soul) = &interaction.soul {
            if !prompt.is_empty() {
                prompt.push_str("\n\n");
            }
            prompt.push_str(soul);
        }
        let shed = &interaction.tools_shed;
        {
            let all_tools = flatten_tools(shed);
            if !all_tools.is_empty() {
                if !prompt.is_empty() {
                    prompt.push_str("\n\n");
                }
                let formatter = TextBasedFormatter;
                if let Some(instructions) = formatter.tool_calling_instructions() {
                    prompt.push_str(&instructions);
                    prompt.push_str("\n\nAvailable tools:\n");
                } else {
                    prompt.push_str("Available tools:\n");
                }
                for tool in &all_tools {
                    let args = tool
                        .arguments
                        .as_ref()
                        .and_then(|a| a.schema.get("properties"))
                        .and_then(|p| p.as_object())
                        .map(|props| props.keys().cloned().collect::<Vec<_>>().join(", "))
                        .unwrap_or_default();
                    let _ = writeln!(prompt, "- {}({})", tool.name, args);
                }
            }
        }

        // If MTP is engaged (config selects Mtp with a separate head path), build
        // a per-stream engine and a chat-templated prompt. On init failure we
        // `warn!` and fall back to standard streaming (mtp = None).
        let mtp = build_mtp_stream_state(&model, interaction, &params, max_tokens);

        Ok(Self {
            inner: Arc::new(Mutex::new(LlamaCppStreamInner {
                model,
                mtp,
                backend: Some(backend),
                ctx: None,
                sampler: Some(sampler),
                current_pos: 0,
                tokens_generated: 0,
                max_tokens,
                input_tokens: 0,
                prompt_evaluated: false,
                finished: false,
                prompt: Some(prompt),
            })),
        })
    }
}

impl Iterator for LlamaCppStream {
    type Item = Stream<Messages, ModelState>;

    #[allow(
        clippy::too_many_lines,
        clippy::single_match_else,
        clippy::cast_possible_truncation,
        clippy::cast_possible_wrap,
        clippy::cast_precision_loss,
        clippy::cast_lossless,
        clippy::cast_sign_loss
    )] // FFI boundary: llama.cpp integration requires these patterns for C API interop
    fn next(&mut self) -> Option<Self::Item> {
        let mut inner = self.inner.lock().unwrap();

        // Check if finished
        if inner.finished {
            return None;
        }

        // First call - return Init and mark prompt as needing evaluation
        if !inner.prompt_evaluated {
            inner.prompt_evaluated = true;
            return Some(Stream::Init);
        }

        // MTP streaming branch (spec-51): when speculative decoding is engaged,
        // drive the MTP engine one step per poll instead of the standard
        // single-token decode. The standard `ctx` is never created in this mode.
        if inner.mtp.is_some() {
            tracing::trace!("mtp_stream_next: mtp is some, driving MTP engine one step");
            return mtp_stream_next(&mut inner);
        }

        // Create backend and context on second call if not exists
        if inner.backend.is_none() {
            let Ok(backend) = LlamaBackend::init_or_get() else {
                inner.finished = true;
                return Some(Stream::Pending(ModelState::Finished));
            };

            let (model, ctx_params) = {
                let model_inner = inner.model.inner.lock().unwrap();
                (Arc::clone(&model_inner.model), model_inner.context.clone())
            };

            let ctx = match model.new_context(&backend, ctx_params) {
                Ok(ctx) => {
                    // Safety: We're transmuting the lifetime to 'static.
                    // This is safe because:
                    // 1. The context is stored in the same struct as the backend
                    // 2. Both are dropped together when the stream is dropped
                    // 3. The context only references the model which outlives the stream
                    unsafe {
                        std::mem::transmute::<LlamaModelContext<'_>, LlamaModelContext<'static>>(
                            ctx,
                        )
                    }
                }
                Err(_) => {
                    inner.finished = true;
                    return Some(Stream::Pending(ModelState::Finished));
                }
            };
            inner.backend = Some(backend);
            inner.ctx = Some(ctx);
            return Some(Stream::Pending(ModelState::GeneratingTokens(None)));
        }

        // Check max tokens first (before any locks)
        if inner.tokens_generated >= inner.max_tokens {
            inner.finished = true;
            return Some(Stream::Pending(ModelState::Finished));
        }

        // Clone context at the beginning - Clone is cheap (pointer copy + Vec clone)
        let Some(mut ctx) = inner.ctx.clone() else {
            inner.finished = true;
            return None;
        };
        let model = {
            let model_inner = inner.model.inner.lock().unwrap();
            Arc::clone(&model_inner.model)
        };

        tracing::trace!("stream_next: get model for interaction");

        // Check if sampler exists
        if inner.sampler.is_none() {
            inner.finished = true;
            return None;
        }

        // On first token generation, tokenize and evaluate the prompt
        if inner.tokens_generated == 0 {
            tracing::trace!("stream_next: evaluating token generation count");
            // Extract prompt early to avoid lock conflicts
            let prompt = inner.prompt.take().unwrap_or_default();
            let Ok(tokens) = model.str_to_token(&prompt, AddBos::Always) else {
                inner.finished = true;
                return Some(Stream::Pending(ModelState::Finished));
            };
            inner.input_tokens = tokens.len();

            // Create batch and evaluate prompt
            let mut batch = LlamaBatch::new(tokens.len(), 1);
            tracing::trace!("stream_next: evaluating prompt Batch: LlamaBatch");
            if batch.add_sequence(&tokens, 0, true).is_err() {
                inner.finished = true;
                return Some(Stream::Pending(ModelState::Finished));
            }

            // Decode with cloned context
            tracing::trace!("stream_next: decoding Batch with ctx");
            if ctx.decode(&mut batch).is_err() {
                inner.finished = true;
                return Some(Stream::Pending(ModelState::Finished));
            }

            inner.current_pos = tokens.len() as i32;
        }

        // Sample next token
        let Some(sampler) = inner.sampler.as_mut() else {
            inner.finished = true;
            return None;
        };
        let next_token = sampler.sample(&ctx, 0);
        tracing::trace!("stream_next: generated next token: {:?}", &next_token);

        // Check for end of sequence
        if model.is_eog_token(next_token) {
            tracing::trace!("stream_next: is it eog token?: {:?}", &next_token);
            inner.finished = true;
            return Some(Stream::Pending(ModelState::Finished));
        }

        // Detokenize
        #[allow(clippy::manual_unwrap_or_default)] // Explicit error handling is clearer here
        let token_str = match model.token_to_str(next_token, Special::Tokenize) {
            Ok(s) => s,
            Err(_) => String::new(),
        };

        tracing::trace!("stream_next: get token str?: {:?}", &token_str);
        inner.tokens_generated += 1;
        inner.current_pos += 1;

        // Return token as Messages::Assistant
        #[allow(clippy::cast_precision_loss)]
        let stream_usage = UsageReport {
            input: inner.input_tokens as f64,
            output: inner.tokens_generated as f64,
            cache_read: 0.0,
            cache_write: 0.0,
            total_tokens: (inner.input_tokens + inner.tokens_generated as usize) as f64,
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
        let zero_pricing = ModelUsageCosting::default();
        let stream_cost = calculate_cost(&zero_pricing, &stream_usage, CostStatus::Actual);
        Some(Stream::Next(Messages::Assistant {
            id: foundation_compact::ids::new_scru128(),
            model: ModelId::Name("llamacpp".to_string(), None),
            timestamp: SystemTime::now(),
            usage: UsageReport {
                cost: stream_cost,
                ..stream_usage
            },
            content: ModelOutput::Text(TextContent {
                content: token_str,
                signature: None,
            }),
            stop_reason: StopReason::Stop,
            provider: ModelProviders::LLAMACPP,
            error_detail: None,
            signature: None,
            metadata: None,
        }))
    }
}

/// Build a streaming `Assistant` message for a text `piece` (local model → $0).
fn build_stream_assistant(input_tokens: usize, output_tokens: usize, piece: String) -> Messages {
    #[allow(clippy::cast_precision_loss)]
    let usage = UsageReport {
        input: input_tokens as f64,
        output: output_tokens as f64,
        cache_read: 0.0,
        cache_write: 0.0,
        total_tokens: (input_tokens + output_tokens) as f64,
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
    let zero_pricing = ModelUsageCosting::default();
    let cost = calculate_cost(&zero_pricing, &usage, CostStatus::Actual);
    Messages::Assistant {
        id: foundation_compact::ids::new_scru128(),
        model: ModelId::Name("llamacpp".to_string(), None),
        timestamp: SystemTime::now(),
        usage: UsageReport { cost, ..usage },
        content: ModelOutput::Text(TextContent {
            content: piece,
            signature: None,
        }),
        stop_reason: StopReason::Stop,
        provider: ModelProviders::LLAMACPP,
        error_detail: None,
        signature: None,
        metadata: None,
    }
}

/// Advance the MTP streaming engine one step per poll.
///
/// First poll after `Init` calls `begin()`; subsequent polls call `step()` and
/// yield the committed text piece. A step that produces no text (e.g. only an
/// EOG) yields a `Pending` rather than an empty message. Any engine error
/// finishes the stream gracefully (spec-51 G5).
#[allow(clippy::cast_sign_loss)]
fn mtp_stream_next(inner: &mut LlamaCppStreamInner) -> Option<Stream<Messages, ModelState>> {
    let need_begin = match inner.mtp.as_ref() {
        Some(m) => !m.started,
        None => return None,
    };

    tracing::trace!("mtp_stream_next: need_begin: {:?}", need_begin);

    if need_begin {
        let res = {
            let mtp = inner.mtp.as_ref().unwrap();
            mtp.engine.begin(&mtp.prompt, mtp.n_predict, mtp.sampling)
        };
        return match res {
            Ok(n) => {
                inner.mtp.as_mut().unwrap().started = true;
                inner.input_tokens = n.max(0) as usize;

                tracing::trace!("mtp_stream_next: get next token");
                Some(Stream::Pending(ModelState::GeneratingTokens(None)))
            }
            Err(err) => {
                tracing::warn!(error = %err, "MTP stream begin failed");
                inner.finished = true;
                Some(Stream::Pending(ModelState::Finished))
            }
        };
    }

    tracing::trace!("mtp_stream_next: get next step");
    let res = inner.mtp.as_ref().unwrap().engine.step();
    match res {
        Ok(step) => {
            if step.piece.is_empty() {
                if step.done {
                    inner.finished = true;
                    return Some(Stream::Pending(ModelState::Finished));
                }
                return Some(Stream::Pending(ModelState::GeneratingTokens(None)));
            }
            inner.tokens_generated += 1;
            let msg = build_stream_assistant(
                inner.input_tokens,
                inner.tokens_generated as usize,
                step.piece,
            );
            tracing::trace!("mtp_stream_next: get msg: {:?}", &msg);
            if step.done {
                inner.finished = true;
            }
            Some(Stream::Next(msg))
        }
        Err(err) => {
            tracing::warn!(error = %err, "MTP stream step failed");
            inner.finished = true;
            Some(Stream::Pending(ModelState::Finished))
        }
    }
}

// Note: StreamIterator is automatically implemented via blanket impl
// for any Iterator<Item = Stream<D, P>>

// ==================================
// Helper Functions for generate()
// ==================================

/// Check if the request is for embeddings by looking for Embedding content.
fn is_embedding_request(messages: &[Messages]) -> bool {
    messages.iter().any(|msg| {
        if let Messages::Assistant { content, .. } = msg {
            matches!(content, ModelOutput::Embedding { .. })
        } else {
            false
        }
    })
}

/// Flatten a `ToolShed` into a Vec<Tool> for formatting.
fn flatten_tools(shed: &ToolShed) -> Vec<crate::types::base_types::Tool> {
    shed.all_tools()
}

/// Apply a chat template to the interaction messages.
///
/// Uses a custom template if provided, otherwise falls back to the model's default.
/// Prepends a system message combining `system_prompt` + soul + tool definitions.
fn apply_chat_template(
    model: &LlamaModel,
    interaction: &ModelInteraction,
) -> GenerationResult<String> {
    // Build system message content: system_prompt + soul + tool definitions
    let mut system_content = String::new();
    if let Some(sys) = &interaction.system_prompt {
        system_content.push_str(sys);
    }
    if let Some(soul) = &interaction.soul {
        if !system_content.is_empty() {
            system_content.push_str("\n\n");
        }
        system_content.push_str(soul);
    }

    // Append tool definitions and calling instructions from tools_shed
    let shed = &interaction.tools_shed;
    {
        let all_tools = flatten_tools(shed);
        if !all_tools.is_empty() {
            if !system_content.is_empty() {
                system_content.push_str("\n\n");
            }
            let formatter = TextBasedFormatter;
            if let Some(instructions) = formatter.tool_calling_instructions() {
                system_content.push_str(&instructions);
                system_content.push_str("\n\nAvailable tools:\n");
            } else {
                system_content.push_str("Available tools:\n");
            }
            for tool in &all_tools {
                let args = tool
                    .arguments
                    .as_ref()
                    .and_then(|a| a.schema.get("properties"))
                    .and_then(|p| p.as_object())
                    .map(|props| props.keys().cloned().collect::<Vec<_>>().join(", "))
                    .unwrap_or_default();
                let _ = writeln!(system_content, "- {}({})", tool.name, args);
            }
        }
    }

    // Convert Messages to LlamaChatMessage format, prepending system message
    let mut chat_messages: Vec<LlamaChatMessage> = Vec::new();

    // Prepend system message if we have content
    if !system_content.is_empty() {
        if let Ok(msg) = LlamaChatMessage::new("system".to_string(), system_content) {
            chat_messages.push(msg);
        }
    }

    // Add user/assistant/tool messages
    for msg in &interaction.messages {
        let (role, content_str) = match msg {
            Messages::User { content, .. } => {
                let text = match content {
                    UserModelContent::Text(TextContent { content, .. }) => content.clone(),
                    UserModelContent::Image(_) => String::new(),
                };
                ("user", text)
            }
            Messages::Assistant { content, .. } => {
                let text = match content {
                    ModelOutput::Text(TextContent { content, .. }) => content.clone(),
                    ModelOutput::ToolCall {
                        name, arguments, ..
                    } => {
                        let args_str = arguments
                            .as_ref()
                            .map(|a| serde_json::to_string(a).unwrap_or_else(|_| "{}".to_string()))
                            .unwrap_or_default();
                        format!("[Tool call: {name}({args_str})]")
                    }
                    ModelOutput::ThinkingContent { thinking, .. } => thinking.clone(),
                    ModelOutput::Embedding { .. } | ModelOutput::Image(_) => String::new(),
                };
                ("assistant", text)
            }
            Messages::ToolResult { content, .. } => {
                let text = match content {
                    UserModelContent::Text(TextContent { content, .. }) => content.clone(),
                    UserModelContent::Image(_) => String::new(),
                };
                ("tool", text)
            }
        };
        if !content_str.is_empty() {
            if let Ok(chat_msg) = LlamaChatMessage::new(role.to_string(), content_str) {
                chat_messages.push(chat_msg);
            }
        }
    }

    // Render the prompt.
    //
    // When no explicit template override is given, prefer the Jinja (minja)
    // path: it applies the model's embedded chat template and handles the
    // modern Jinja templates that the legacy `llama_chat_apply_template` cannot
    // (Gemma 4, Qwen3-Next, GLM, DeepSeek, ...). Fall back to the legacy path
    // if the Jinja render fails, so models whose templates the legacy API
    // handles keep working.
    if let Some(custom_template) = &interaction.chat_template {
        let template = LlamaChatTemplate::new(custom_template).map_err(|e| {
            GenerationError::Generic(format!("Failed to create chat template: {e}"))
        })?;
        return model
            .apply_chat_template(&template, &chat_messages, true)
            .map_err(Into::<GenerationError>::into);
    }

    match model.apply_jinja_chat_template(&chat_messages, true) {
        Ok(prompt) => Ok(prompt),
        Err(jinja_err) => {
            tracing::debug!(
                "Jinja chat template failed ({jinja_err}); falling back to legacy template path"
            );
            let template = model
                .chat_template(None)
                .map_err(Into::<GenerationError>::into)?;
            model
                .apply_chat_template(&template, &chat_messages, true)
                .map_err(Into::<GenerationError>::into)
        }
    }
}

/// Generate embeddings from the input prompt.
#[allow(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    clippy::cast_precision_loss
)]
fn generate_embeddings(
    model: &LlamaModel,
    ctx: &mut LlamaModelContext,
    prompt: &str,
    _spec: &ModelSpec,
) -> GenerationResult<Vec<Messages>> {
    // Tokenize the prompt
    let tokens = model
        .str_to_token(prompt, AddBos::Always)
        .map_err(Into::<GenerationError>::into)?;

    // Create batch and add sequence
    let mut batch = LlamaBatch::new(tokens.len(), 1);
    batch
        .add_sequence(&tokens, 0, false)
        .map_err(|e| GenerationError::Generic(format!("Failed to add tokens to batch: {e}")))?;

    // Encode to get embeddings
    ctx.encode(&mut batch)
        .map_err(Into::<GenerationError>::into)?;

    // Get embeddings for the first sequence
    let embeddings = ctx
        .embeddings_seq_ith(0)
        .map_err(Into::<GenerationError>::into)?;

    // Return embeddings as Assistant message
    let dimensions = embeddings.len();
    #[allow(clippy::cast_precision_loss)]
    let emb_usage = UsageReport {
        input: tokens.len() as f64,
        output: 0.0,
        cache_read: 0.0,
        cache_write: 0.0,
        total_tokens: tokens.len() as f64,
        cost: UsageCosting {
            currency: "USD".to_string(),
            input: 0.0,
            output: 0.0,
            cache_read: 0.0,
            cache_write: 0.0,
            total_tokens: tokens.len() as f64,
            status: CostStatus::Actual,
        },
    };
    // Local model: $0 pricing
    let zero_pricing = ModelUsageCosting::default();
    let emb_cost = calculate_cost(&zero_pricing, &emb_usage, CostStatus::Actual);
    let emb_usage = UsageReport {
        cost: emb_cost,
        ..emb_usage
    };
    Ok(vec![Messages::Assistant {
        id: foundation_compact::ids::new_scru128(),
        model: ModelId::Name("llamacpp".to_string(), None),
        timestamp: SystemTime::now(),
        usage: emb_usage,
        content: ModelOutput::Embedding {
            dimensions,
            values: embeddings.to_vec(),
        },
        stop_reason: StopReason::Stop,
        provider: ModelProviders::LLAMACPP,
        error_detail: None,
        signature: None,
        metadata: None,
    }])
}

/// Marshal `ModelParams` into the shim's [`MtpSampling`].
#[allow(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    clippy::cast_possible_wrap
)]
fn mtp_sampling_from_params(params: &ModelParams) -> MtpSampling {
    MtpSampling {
        temperature: params.temperature,
        top_k: params.top_k as i32,
        top_p: params.top_p,
        repeat_penalty: params.repeat_penalty,
        seed: params.seed.unwrap_or(0xFFFF_FFFF),
    }
}

/// Build the per-stream MTP engine + state when speculative (MTP) decoding is
/// engaged for `model`. Returns `None` (→ standard streaming) when MTP is not
/// configured, or when the engine fails to initialize (logged as a `warn!`).
#[allow(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    clippy::cast_possible_wrap
)]
fn build_mtp_stream_state(
    model: &LlamaModels,
    interaction: &ModelInteraction,
    params: &ModelParams,
    n_predict: i32,
) -> Option<MtpStreamState> {
    let (model_arc, config, mtp_model_slot) = {
        let inner = model.inner.lock().unwrap();
        (
            Arc::clone(&inner.model),
            inner.config.clone(),
            Arc::clone(&inner.mtp_model),
        )
    };

    let spec_cfg = config.speculative.as_ref()?;
    if spec_cfg.kind != SpeculativeKind::Mtp {
        return None;
    }
    let mtp_path = spec_cfg.mtp_model.as_ref()?;

    // Chat-templated prompt — parity with the non-streaming path.
    let prompt = if interaction.messages.is_empty() {
        interaction.system_prompt.clone().unwrap_or_default()
    } else {
        match apply_chat_template(&model_arc, interaction) {
            Ok(p) => p,
            Err(err) => {
                tracing::warn!(error = %err, "MTP stream: chat template failed — using standard streaming");
                return None;
            }
        }
    };

    // Load the shared draft model once, then build this stream's own engine from
    // it (independent contexts — concurrent streams never contend).
    let draft_model = match get_or_load_mtp_model(&mtp_model_slot, mtp_path) {
        Ok(m) => m,
        Err(err) => {
            tracing::warn!(error = %err, "MTP stream: draft model load failed — using standard streaming");
            return None;
        }
    };
    let engine = match build_mtp_engine(&model_arc, draft_model, &config, spec_cfg) {
        Ok(engine) => engine,
        Err(err) => {
            tracing::warn!(error = %err, "MTP stream: engine init failed — using standard streaming");
            return None;
        }
    };

    Some(MtpStreamState {
        engine,
        prompt,
        sampling: mtp_sampling_from_params(params),
        n_predict,
        started: false,
    })
}

/// Load-once, share the MTP draft model weights.
///
/// The slot lock is held only during the one-time load; afterward callers get a
/// cheap `Arc` clone. Concurrent first callers block on the single load rather
/// than each loading the ~100 MB GGUF independently.
fn get_or_load_mtp_model(
    slot: &Arc<Mutex<Option<Arc<LlamaMtpModel>>>>,
    mtp_path: &std::path::Path,
) -> Result<Arc<LlamaMtpModel>, infrastructure_llama_cpp::speculative::MtpError> {
    let mut guard = slot.lock().unwrap();
    if let Some(model) = guard.as_ref() {
        return Ok(Arc::clone(model));
    }
    let model = Arc::new(LlamaMtpModel::load(mtp_path)?);
    *guard = Some(Arc::clone(&model));
    Ok(model)
}

/// Build a fresh per-generation MTP engine (contexts + speculator) from the
/// shared target + draft models — cheap, and independent from any other engine.
#[allow(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    clippy::cast_possible_wrap
)]
fn build_mtp_engine(
    model: &LlamaModel,
    draft_model: Arc<LlamaMtpModel>,
    backend_config: &LlamaBackendConfig,
    spec_cfg: &SpeculativeConfig,
) -> Result<LlamaMtp, infrastructure_llama_cpp::speculative::MtpError> {
    LlamaMtp::new(
        model,
        draft_model,
        backend_config.context_length as u32,
        backend_config.batch_size as u32,
        backend_config.n_threads as i32,
        spec_cfg.n_max as i32,
    )
}

/// Run MTP speculative generation through the C++ shim wrapper.
///
/// WHY: MTP (`common/speculative.h`) is a stateful C++ engine; we own an
/// [`LlamaMtp`] that internally holds its own target + draft contexts and the
/// speculator. This helper marshals `ModelParams` → [`MtpSampling`], runs one
/// full `begin → draft → verify → accept` loop, and returns the result in the
/// standard [`Messages::Assistant`] shape used by the rest of the backend.
///
/// The MTP head GGUF path is supplied by the caller via
/// [`SpeculativeConfig::mtp_model`] — the wire-up matches the Gemma/Qwen head
/// shape (a separate draft GGUF that shares the target KV cache).
///
/// Errors here are propagated to the caller which decides whether to fall back
/// to standard decoding (spec-51 G5).
#[allow(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    clippy::cast_possible_wrap,
    clippy::cast_precision_loss
)]
fn run_mtp_generation(
    mtp_model_slot: &Arc<Mutex<Option<Arc<LlamaMtpModel>>>>,
    model: &LlamaModel,
    prompt: &str,
    params: &ModelParams,
    backend_config: &LlamaBackendConfig,
    spec_cfg: &SpeculativeConfig,
    mtp_path: &std::path::Path,
) -> GenerationResult<Vec<Messages>> {
    let sampling = mtp_sampling_from_params(params);
    let n_predict = params.max_tokens as i32;

    // Load the draft model once (shared read-only), then build a FRESH engine
    // for this generation — no lock is held during generation, so concurrent
    // generations proceed on independent engines.
    let draft_model = get_or_load_mtp_model(mtp_model_slot, mtp_path)
        .map_err(|e| GenerationError::Generic(format!("MTP model load failed: {e}")))?;
    let engine = build_mtp_engine(model, draft_model, backend_config, spec_cfg)
        .map_err(|e| GenerationError::Generic(format!("MTP init failed: {e}")))?;

    let generation = engine
        .generate(prompt, n_predict, sampling)
        .map_err(|e| GenerationError::Generic(format!("MTP generate failed: {e}")))?;

    tracing::info!(
        n_prompt_tokens = generation.stats.n_prompt_tokens,
        n_generated = generation.stats.n_generated,
        n_drafted = generation.stats.n_drafted,
        n_accepted = generation.stats.n_accepted,
        acceptance_rate = generation.acceptance_rate(),
        "MTP speculative decode completed"
    );

    let input_tokens = f64::from(generation.stats.n_prompt_tokens.max(0));
    let output_tokens = f64::from(generation.stats.n_generated.max(0));

    let zero_pricing = ModelUsageCosting::default();
    let mtp_usage = UsageReport {
        input: input_tokens,
        output: output_tokens,
        cache_read: 0.0,
        cache_write: 0.0,
        total_tokens: input_tokens + output_tokens,
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
    let mtp_cost = calculate_cost(&zero_pricing, &mtp_usage, CostStatus::Actual);
    let mtp_usage = UsageReport {
        cost: mtp_cost,
        ..mtp_usage
    };

    Ok(vec![Messages::Assistant {
        id: foundation_compact::ids::new_scru128(),
        model: ModelId::Name("llamacpp".to_string(), None),
        timestamp: SystemTime::now(),
        usage: mtp_usage,
        content: ModelOutput::Text(TextContent {
            content: generation.text,
            signature: None,
        }),
        stop_reason: StopReason::Stop,
        provider: ModelProviders::LLAMACPP,
        error_detail: None,
        signature: None,
        metadata: None,
    }])
}

/// Generate text from the input prompt using autoregressive decoding.
///
/// This function implements the main token generation loop:
/// 1. Tokenize the prompt
/// 2. Evaluate the prompt through the model
/// 3. Sample tokens using the provided sampler
/// 4. Detokenize and accumulate the output
/// 5. Check for stop conditions
#[allow(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    clippy::cast_possible_wrap,
    clippy::cast_precision_loss
)]
fn generate_text(
    model: &LlamaModel,
    ctx: &mut LlamaModelContext,
    sampler: &mut LlamaSampler,
    prompt: &str,
    params: &ModelParams,
    _spec: &ModelSpec,
) -> GenerationResult<Vec<Messages>> {
    // Note: the speculative/MTP dispatch lives in `Model::generate` (before the
    // standard `ctx`/`sampler` are built, so a successful MTP run allocates no
    // wasted context). This function is the standard decode path — and the MTP
    // fallback.

    // Tokenize the prompt
    let tokens = model
        .str_to_token(prompt, AddBos::Always)
        .map_err(Into::<GenerationError>::into)?;

    tracing::trace!("generae: prompt={prompt}");
    // Create batch and add sequence for prompt
    let mut batch = LlamaBatch::new(tokens.len(), 1);
    batch
        .add_sequence(&tokens, 0, true)
        .map_err(|e| GenerationError::Generic(format!("Failed to add tokens to batch: {e}")))?;

    // Decode the prompt
    ctx.decode(&mut batch)
        .map_err(Into::<GenerationError>::into)?;

    tracing::trace!("generate: decode prompt");

    // Generate tokens up to max_tokens or until stop token
    let max_tokens = params.max_tokens;
    let mut generated_tokens: Vec<LlamaToken> = Vec::new();
    let mut output_text = String::new();
    let start_pos = tokens.len() as i32;

    for current_pos in (start_pos..).take(max_tokens) {
        tracing::trace!("generate: current pos: {current_pos}");

        // Sample the next token (idx=0 for single sequence)
        let next_token = sampler.sample(ctx, 0);
        generated_tokens.push(next_token);

        // Check for end of sequence
        if model.is_eog_token(next_token) {
            tracing::trace!("generate: eog stopping: {current_pos}");
            break;
        }

        // Detokenize and accumulate
        let token_str = model
            .token_to_str(next_token, Special::Tokenize)
            .map_err(Into::<GenerationError>::into)?;

        tracing::trace!("generate: token: {token_str}: {current_pos}");
        output_text.push_str(&token_str);

        // Check for stop tokens
        if params
            .stop_tokens
            .iter()
            .any(|seq| output_text.contains(seq))
        {
            break;
        }

        // Create batch for single token and decode
        let mut batch = LlamaBatch::new(1, 1);
        batch
            .add(next_token, current_pos, &[0], true)
            .map_err(|e| GenerationError::Generic(format!("Failed to add token to batch: {e}")))?;

        ctx.decode(&mut batch)
            .map_err(Into::<GenerationError>::into)?;
    }

    // Calculate token counts
    let input_tokens = tokens.len() as f64;
    let output_tokens = generated_tokens.len() as f64;

    // Local model: $0 pricing
    let zero_pricing = ModelUsageCosting::default();
    let txt_usage = UsageReport {
        input: input_tokens,
        output: output_tokens,
        cache_read: 0.0,
        cache_write: 0.0,
        total_tokens: input_tokens + output_tokens,
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
    let txt_cost = calculate_cost(&zero_pricing, &txt_usage, CostStatus::Actual);
    let txt_usage = UsageReport {
        cost: txt_cost,
        ..txt_usage
    };
    // Return generated text as Assistant message
    Ok(vec![Messages::Assistant {
        id: foundation_compact::ids::new_scru128(),
        model: ModelId::Name("llamacpp".to_string(), None),
        timestamp: SystemTime::now(),
        usage: txt_usage,
        content: ModelOutput::Text(TextContent {
            content: output_text,
            signature: None,
        }),
        stop_reason: StopReason::Stop,
        provider: ModelProviders::LLAMACPP,
        error_detail: None,
        signature: None,
        metadata: None,
    }])
}

// ==================================
// LlamaBackends
// ==================================

/// Hardware backend variants for llama.cpp.
#[derive(Debug, Clone, Copy)]
pub enum LlamaBackends {
    /// CPU-only execution.
    LLamaCPU,
    /// GPU execution (CUDA or Vulkan).
    LLamaGPU,
    /// Apple Metal execution.
    LLamaMetal,
}

impl ModelProvider for LlamaBackends {
    type Config = LlamaBackendConfig;
    type Model = LlamaModels;

    fn create(self, _config: Option<Self::Config>) -> ModelProviderResult<Self>
    where
        Self: Sized,
    {
        // Initialize the llama.cpp backend
        let _backend = LlamaBackend::init_or_get()
            .map_err(|e| crate::errors::ModelProviderErrors::FailedFetching(Box::new(e)))?;

        Ok(self)
    }

    fn describe(&self) -> ModelProviderResult<crate::types::base_types::ModelProviderDescriptor> {
        Ok(crate::types::base_types::ModelProviderDescriptor {
            id: "llamacpp",
            name: "llama.cpp Local Inference",
            reasoning: false,
            api: crate::types::base_types::ModelAPI::Custom("llama-cpp".to_string()),
            provider: ModelProviders::LLAMACPP,
            base_url: None,
            inputs: crate::types::base_types::MessageType::Text,
            cost: crate::types::base_types::ModelUsageCosting {
                input: 0.0,
                output: 0.0,
                cache_read: 0.0,
                cache_write: 0.0,
            },
            context_window: 4096,
            max_tokens: 2048,
        })
    }

    fn get_model(&self, model_id: ModelId) -> ModelProviderResult<Self::Model> {
        // For now, we require a local file path
        // In a full implementation, this would check a cache first
        let model_spec = ModelSpec {
            name: format!("{model_id:?}"),
            id: model_id.clone(),
            devices: None,
            model_location: None,
            lora_location: None,
        };

        self.get_model_by_spec(model_spec)
    }

    fn get_model_by_spec(&self, model_spec: ModelSpec) -> ModelProviderResult<Self::Model> {
        // Default config (standard decoding); load_model validates the model
        // location and loads.
        self.load_model(model_spec, &LlamaBackendConfig::default())
    }

    fn get_one(
        &self,
        model_id: ModelId,
    ) -> ModelProviderResult<crate::types::base_types::ModelSpec> {
        Err(ModelProviderErrors::NotFound(format!(
            "Model {model_id:?} not found in registry"
        )))
    }

    fn get_all(
        &self,
        _model_id: ModelId,
    ) -> ModelProviderResult<Vec<crate::types::base_types::ModelSpec>> {
        Err(ModelProviderErrors::NotFound(
            "Model registry not implemented".to_string(),
        ))
    }
}

impl LlamaBackends {
    /// Load a model, applying a [`LlamaBackendConfig`] (GPU layers, context
    /// length, batch size, threads) and its optional speculative (MTP) config.
    ///
    /// When `config.speculative` is `Some`, the model is checked for MTP support
    /// (`n_layer_nextn > 0`). If the model does **not** support it, this returns
    /// an error rather than silently ignoring the request — enabling MTP on an
    /// incapable model is a configuration mistake, not a no-op (spec-51 G2).
    ///
    /// # Errors
    /// Returns [`ModelProviderErrors`] if the model location is missing, the
    /// model fails to load, or MTP was requested for a model that lacks an MTP
    /// head.
    pub fn load_model(
        &self,
        model_spec: ModelSpec,
        config: &LlamaBackendConfig,
    ) -> ModelProviderResult<LlamaModels> {
        let model_path = model_spec.model_location.as_ref().ok_or_else(|| {
            ModelProviderErrors::ModelErrors(ModelErrors::NotFound(
                "No model location specified".to_string(),
            ))
        })?;

        let backend = LlamaBackend::init_or_get().map_err(|e| {
            ModelProviderErrors::ModelErrors(ModelErrors::FailedLoading(Box::new(e)))
        })?;

        // Apply model params from config (GPU offload, …) — previously these
        // were silently dropped and defaults were used.
        let model_params = config.to_model_params();
        let model = LlamaModel::load_from_file(&backend, model_path, &model_params)
            .map_err(|e| ModelProviderErrors::ModelErrors(e.into()))?;

        // Capability gate (spec-51 G2): MTP requested → the model must have an
        // embedded MTP head (n_layer_nextn > 0) OR a separate MTP head GGUF must
        // be supplied (the Gemma/Qwen heads are separate draft models). If
        // neither, error rather than silently ignoring the request.
        if let Some(spec) = &config.speculative {
            let has_head = model.supports_mtp() || spec.mtp_model.is_some();
            if !has_head {
                return Err(ModelProviderErrors::ModelErrors(ModelErrors::NotFound(
                    format!(
                        "speculative/MTP decoding requested but model '{}' has no embedded MTP \
                         head (n_layer_nextn == 0) and no separate mtp_model path was provided",
                        model_spec.name
                    ),
                )));
            }
        }

        // Apply context params from config (context length, batch, threads).
        let context_params = config.to_context_params();

        Ok(LlamaModels::new_with_config(
            model,
            context_params,
            model_spec,
            config.clone(),
        ))
    }
}
