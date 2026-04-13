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
use infrastructure_llama_cpp::model::params::LlamaModelParams;
use infrastructure_llama_cpp::model::{AddBos, LlamaChatMessage, LlamaChatTemplate, LlamaModel, Special};
use infrastructure_llama_cpp::sampling::LlamaSampler;
use infrastructure_llama_cpp::llama_batch::LlamaBatch;
use infrastructure_llama_cpp::token::LlamaToken;

use std::cell::RefCell;
use std::num::NonZeroU32;
use std::rc::Rc;
use std::time::SystemTime;

use foundation_core::valtron::{Stream, StreamIterator};

use crate::backends::llamacpp_helpers::build_sampler_chain;
use crate::errors::{GenerationError, GenerationResult, ModelErrors, ModelProviderErrors, ModelProviderResult};
use crate::types::{
    KVCacheType, Messages, Model, ModelId, ModelInteraction, ModelOutput,
    ModelParams, ModelProvider, ModelSpec, ModelProviders, ModelState, SplitMode, StopReason,
    TextContent, UsageCosting, UsageReport, UserModelContent,
};

// ==================================
// LlamaBackendConfig
// ==================================

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
        }
    }
}

/// Get the number of CPUs available (cross-platform).
#[must_use]
fn num_cpus() -> usize {
    std::thread::available_parallelism()
        .map(std::num::NonZeroUsize::get)
        .unwrap_or(4)
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
    #[must_use]
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss, clippy::cast_possible_wrap)]
    pub fn to_context_params(&self) -> LlamaModelContextParams {
        let mut params = LlamaModelContextParams::default();
        params = params.with_n_ctx(NonZeroU32::new(self.context_length as u32));
        params = params.with_n_batch(self.batch_size as u32);
        params = params.with_n_threads(self.n_threads as i32);
        params = params.with_embeddings(true); // Enable embeddings
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
    model: Rc<LlamaModel>,
    context: LlamaModelContextParams,
    #[allow(dead_code)]
    sampler: Option<LlamaSampler>,
    spec: ModelSpec,
    last_usage: Option<UsageReport>,
}

/// `llama.cpp` model wrapper implementing the `Model` trait.
///
/// Uses interior mutability (`RefCell`) so that `&self` methods can mutate
/// the context and sampler during generation.
pub struct LlamaModels {
    inner: Rc<RefCell<LlamaModelsInner>>,
}

impl Clone for LlamaModels {
    fn clone(&self) -> Self {
        Self {
            inner: Rc::clone(&self.inner),
        }
    }
}

impl LlamaModels {
    /// Create a new `LlamaModels` instance.
    fn new(
        model: LlamaModel,
        context: LlamaModelContextParams,
        spec: ModelSpec,
    ) -> Self {
        Self {
            inner: Rc::new(RefCell::new(LlamaModelsInner {
                model: Rc::new(model),
                context,
                sampler: None,
                spec,
                last_usage: None,
            })),
        }
    }

    /// Get the model spec.
    #[must_use]
    pub fn spec(&self) -> ModelSpec {
        self.inner.borrow().spec.clone()
    }
}

impl Model for LlamaModels {
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
        // Get backend
        let backend = LlamaBackend::init().map_err(GenerationError::LlamaCpp)?;

        // Get model, spec, and context params
        let (model, spec, ctx_params) = {
            let inner = self.inner.borrow();
            (Rc::clone(&inner.model), inner.spec.clone(), inner.context.clone())
        };

        // Create context
        let mut ctx = model.new_context(&backend, ctx_params).map_err(GenerationError::LlamaContextLoad)?;

        // Build sampler chain from params
        let params = specs.unwrap_or_default();
        let mut sampler = build_sampler_chain(&params);

        // Check if this is an embedding request
        let is_embedding = is_embedding_request(&interaction.messages);

        // Apply chat template if messages are present
        let prompt = if interaction.messages.is_empty() {
            interaction.system_prompt.unwrap_or_default()
        } else {
            apply_chat_template(&model, &interaction)?
        };

        if is_embedding {
            generate_embeddings(&model, &mut ctx, &prompt, &spec)
        } else {
            generate_text(&model, &mut ctx, &mut sampler, &prompt, &params, &spec)
        }
    }

    fn stream(
        &self,
        interaction: ModelInteraction,
        specs: Option<ModelParams>,
    ) -> GenerationResult<impl StreamIterator<D = Messages, P = ModelState>> {
        LlamaCppStream::new(self.clone(), interaction, specs)
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
    inner: Rc<RefCell<LlamaCppStreamInner>>,
}

/// Internal stream state - uses Clone for context
struct LlamaCppStreamInner {
    /// Reference to the model (cloned from `LlamaModels`)
    model: LlamaModels,
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
        interaction: ModelInteraction,
        specs: Option<ModelParams>,
    ) -> GenerationResult<Self> {
        // Initialize backend upfront - this is where we can properly report errors
        let backend = LlamaBackend::init()
            .map_err(|e| GenerationError::Generic(format!("Failed to initialize llama.cpp backend: {e}")))?;

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

        // Extract prompt from interaction (simplified - just use system prompt for now)
        let prompt = interaction.system_prompt.unwrap_or_default();

        Ok(Self {
            inner: Rc::new(RefCell::new(LlamaCppStreamInner {
                model,
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
        let mut inner = self.inner.borrow_mut();

        // Check if finished
        if inner.finished {
            return None;
        }

        // First call - return Init and mark prompt as needing evaluation
        if !inner.prompt_evaluated {
            inner.prompt_evaluated = true;
            return Some(Stream::Init);
        }

        // Create backend and context on second call if not exists
        if inner.backend.is_none() {
            let Ok(backend) = LlamaBackend::init() else {
                inner.finished = true;
                return Some(Stream::Pending(ModelState::Finished));
            };

            let (model, ctx_params) = {
                let model_inner = inner.model.inner.borrow();
                (Rc::clone(&model_inner.model), model_inner.context.clone())
            };

            let ctx = match model.new_context(&backend, ctx_params) {
                Ok(ctx) => {
                    // Safety: We're transmuting the lifetime to 'static.
                    // This is safe because:
                    // 1. The context is stored in the same struct as the backend
                    // 2. Both are dropped together when the stream is dropped
                    // 3. The context only references the model which outlives the stream
                    unsafe {
                        std::mem::transmute::<LlamaModelContext<'_>, LlamaModelContext<'static>>(ctx)
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

        // Check max tokens first (before any borrows)
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
            let model_inner = inner.model.inner.borrow();
            Rc::clone(&model_inner.model)
        };

        // Check if sampler exists
        if inner.sampler.is_none() {
            inner.finished = true;
            return None;
        }

        // On first token generation, tokenize and evaluate the prompt
        if inner.tokens_generated == 0 {
            // Extract prompt early to avoid borrow conflicts
            let prompt = inner.prompt.take().unwrap_or_default();
            let Ok(tokens) = model.str_to_token(&prompt, AddBos::Always) else {
                inner.finished = true;
                return Some(Stream::Pending(ModelState::Finished));
            };
            inner.input_tokens = tokens.len();

            // Create batch and evaluate prompt
            let mut batch = LlamaBatch::new(tokens.len(), 1);
            if batch.add_sequence(&tokens, 0, true).is_err() {
                inner.finished = true;
                return Some(Stream::Pending(ModelState::Finished));
            }

            // Decode with cloned context
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

        // Check for end of sequence
        if model.is_eog_token(next_token) {
            inner.finished = true;
            return Some(Stream::Pending(ModelState::Finished));
        }

        // Detokenize
        #[allow(clippy::manual_unwrap_or_default)] // Explicit error handling is clearer here
        let token_str = match model.token_to_str(next_token, Special::Tokenize) {
            Ok(s) => s,
            Err(_) => String::new(),
        };

        inner.tokens_generated += 1;
        inner.current_pos += 1;

        // Return token as Messages::Assistant
        Some(Stream::Next(Messages::Assistant {
            model: ModelId::Name("llamacpp".to_string(), None),
            timestamp: SystemTime::now(),
            usage: UsageReport {
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
                },
            },
            content: ModelOutput::Text(TextContent {
                content: token_str,
                signature: None,
            }),
            stop_reason: StopReason::Stop,
            provider: ModelProviders::LLAMACPP,
            error_detail: None,
            signature: None,
        }))
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

/// Apply a chat template to the interaction messages.
///
/// Uses a custom template if provided, otherwise falls back to the model's default.
fn apply_chat_template(
    model: &LlamaModel,
    interaction: &ModelInteraction,
) -> GenerationResult<String> {
    // Convert Messages to LlamaChatMessage format
    let chat_messages: Vec<LlamaChatMessage> = interaction
        .messages
        .iter()
        .filter_map(|msg| {
            let (role, content_str) = match msg {
                Messages::User { content, .. } => {
                    let text = match content {
                        UserModelContent::Text(TextContent { content, .. }) => content.clone(),
                        UserModelContent::Image(_) => String::new(), // Skip images for now
                    };
                    ("user", text)
                }
                Messages::Assistant { content, .. } => {
                    let text = match content {
                        ModelOutput::Text(TextContent { content, .. }) => content.clone(),
                        ModelOutput::ToolCall { name, arguments, .. } => {
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
            if content_str.is_empty() {
                None
            } else {
                LlamaChatMessage::new(role.to_string(), content_str).ok()
            }
        })
        .collect();

    // Get chat template (use custom if provided, otherwise default)
    let template = if let Some(custom_template) = &interaction.chat_template {
        LlamaChatTemplate::new(custom_template)
            .map_err(|e| GenerationError::Generic(format!("Failed to create chat template: {e}")))?
    } else {
        model
            .chat_template(None)
            .map_err(GenerationError::ChatTemplate)?
    };

    // Apply chat template
    let prompt = model
        .apply_chat_template(&template, &chat_messages, true)
        .map_err(GenerationError::ApplyChatTemplate)?;

    Ok(prompt)
}

/// Generate embeddings from the input prompt.
#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss, clippy::cast_precision_loss)]
fn generate_embeddings(
    model: &LlamaModel,
    ctx: &mut LlamaModelContext,
    prompt: &str,
    _spec: &ModelSpec,
) -> GenerationResult<Vec<Messages>> {
    // Tokenize the prompt
    let tokens = model
        .str_to_token(prompt, AddBos::Always)
        .map_err(GenerationError::Tokenization)?;

    // Create batch and add sequence
    let mut batch = LlamaBatch::new(tokens.len(), 1);
    batch
        .add_sequence(&tokens, 0, false)
        .map_err(|e| GenerationError::Generic(format!("Failed to add tokens to batch: {e}")))?;

    // Encode to get embeddings
    ctx.encode(&mut batch)
        .map_err(GenerationError::Encode)?;

    // Get embeddings for the first sequence
    let embeddings = ctx
        .embeddings_seq_ith(0)
        .map_err(GenerationError::Embeddings)?;

    // Return embeddings as Assistant message
    let dimensions = embeddings.len();
    Ok(vec![Messages::Assistant {
        model: ModelId::Name("llamacpp".to_string(), None),
        timestamp: SystemTime::now(),
        usage: UsageReport {
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
                total_tokens: 0.0,
            },
        },
        content: ModelOutput::Embedding {
            dimensions,
            values: embeddings.to_vec(),
        },
        stop_reason: StopReason::Stop,
        provider: ModelProviders::LLAMACPP,
        error_detail: None,
        signature: None,
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
#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss, clippy::cast_possible_wrap, clippy::cast_precision_loss)]
fn generate_text(
    model: &LlamaModel,
    ctx: &mut LlamaModelContext,
    sampler: &mut LlamaSampler,
    prompt: &str,
    params: &ModelParams,
    _spec: &ModelSpec,
) -> GenerationResult<Vec<Messages>> {
    // Tokenize the prompt
    let tokens = model
        .str_to_token(prompt, AddBos::Always)
        .map_err(GenerationError::Tokenization)?;

    // Create batch and add sequence for prompt
    let mut batch = LlamaBatch::new(tokens.len(), 1);
    batch
        .add_sequence(&tokens, 0, true)
        .map_err(|e| GenerationError::Generic(format!("Failed to add tokens to batch: {e}")))?;

    // Decode the prompt
    ctx.decode(&mut batch)
        .map_err(GenerationError::Decode)?;

    // Generate tokens up to max_tokens or until stop token
    let max_tokens = params.max_tokens;
    let mut generated_tokens: Vec<LlamaToken> = Vec::new();
    let mut output_text = String::new();
    let mut current_pos = tokens.len() as i32;

    for _ in 0..max_tokens {
        // Sample the next token (idx=0 for single sequence)
        let next_token = sampler.sample(ctx, 0);
        generated_tokens.push(next_token);

        // Check for end of sequence
        if model.is_eog_token(next_token) {
            break;
        }

        // Detokenize and accumulate
        let token_str = model
            .token_to_str(next_token, Special::Tokenize)
            .map_err(GenerationError::TokenToString)?;
        output_text.push_str(&token_str);

        // Check for stop tokens
        if params.stop_tokens.iter().any(|seq| output_text.contains(seq)) {
            break;
        }

        // Create batch for single token and decode
        let mut batch = LlamaBatch::new(1, 1);
        batch
            .add(next_token, current_pos, &[0], true)
            .map_err(|e| GenerationError::Generic(format!("Failed to add token to batch: {e}")))?;

        ctx.decode(&mut batch)
            .map_err(GenerationError::Decode)?;

        current_pos += 1;
    }

    // Calculate token counts
    let input_tokens = tokens.len() as f64;
    let output_tokens = generated_tokens.len() as f64;

    // Return generated text as Assistant message
    Ok(vec![Messages::Assistant {
        model: ModelId::Name("llamacpp".to_string(), None),
        timestamp: SystemTime::now(),
        usage: UsageReport {
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
            },
        },
        content: ModelOutput::Text(TextContent {
            content: output_text,
            signature: None,
        }),
        stop_reason: StopReason::Stop,
        provider: ModelProviders::LLAMACPP,
        error_detail: None,
        signature: None,
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

    fn create(
        self,
        _config: Option<Self::Config>,
        _credential: Option<foundation_auth::AuthCredential>,
    ) -> ModelProviderResult<Self>
    where
        Self: Sized,
    {
        // Initialize the llama.cpp backend
        let _backend = LlamaBackend::init().map_err(|e| {
            crate::errors::ModelProviderErrors::FailedFetching(Box::new(e))
        })?;

        Ok(self)
    }

    fn describe(&self) -> ModelProviderResult<crate::types::ModelProviderDescriptor> {
        let descriptor = crate::types::ModelProviderDescriptor {
            id: "llamacpp".to_string(),
            name: "llama.cpp Local Inference".to_string(),
            reasoning: false,
            api: crate::types::ModelAPI::Custom("llama-cpp".to_string()),
            provider: ModelProviders::LLAMACPP,
            base_url: None,
            inputs: crate::types::MessageType::Text,
            cost: crate::types::ModelUsageCosting {
                input: 0.0,
                output: 0.0,
                cache_read: 0.0,
                cache_write: 0.0,
            },
            context_window: 4096,
            max_tokens: 2048,
        };
        Ok(descriptor)
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
        // Get model location
        let model_path = model_spec.model_location.as_ref()
            .ok_or_else(|| {
                ModelProviderErrors::ModelErrors(
                    ModelErrors::NotFound("No model location specified".to_string())
                )
            })?;

        // Load model
        let backend = LlamaBackend::init().map_err(|e| {
            ModelProviderErrors::ModelErrors(
                ModelErrors::FailedLoading(Box::new(e))
            )
        })?;

        let model_params = LlamaModelParams::default();
        let model = LlamaModel::load_from_file(&backend, model_path, &model_params)
            .map_err(|e| {
                ModelProviderErrors::ModelErrors(ModelErrors::LlamaModelLoad(e))
            })?;

        let context_params = LlamaModelContextParams::default();

        Ok(LlamaModels::new(model, context_params, model_spec))
    }

    fn get_one(&self, model_id: ModelId) -> ModelProviderResult<crate::types::ModelSpec> {
        Err(ModelProviderErrors::NotFound(
            format!("Model {model_id:?} not found in registry")
        ))
    }

    fn get_all(&self, _model_id: ModelId) -> ModelProviderResult<crate::types::ModelSpec> {
        Err(ModelProviderErrors::NotFound(
            "Model registry not implemented".to_string()
        ))
    }
}
