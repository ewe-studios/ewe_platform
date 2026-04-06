//! llama.cpp ModelBackend implementations.
//!
//! This module provides the llama.cpp integration for foundation_ai,
//! enabling local execution of GGUF-format models.
//!
//! # Architecture
//!
//! - [`LlamaBackends`] - Hardware variant enum (CPU/GPU/Metal) implementing `ModelProvider`
//! - [`LlamaBackendConfig`] - Configuration with builder pattern for provider initialization
//! - [`LlamaModels`] - `Model` trait implementation with interior mutability
//! - [`LlamaCppStream`] - `StreamIterator` implementation for token-by-token streaming

use infrastructure_llama_cpp::context::params::LlamaContextParams;
use infrastructure_llama_cpp::llama_backend::LlamaBackend;
use infrastructure_llama_cpp::model::params::LlamaModelParams;
use infrastructure_llama_cpp::model::{AddBos, LlamaChatMessage, LlamaChatTemplate, LlamaModel, Special};
use infrastructure_llama_cpp::sampling::LlamaSampler;
use infrastructure_llama_cpp::llama_batch::LlamaBatch;

use std::cell::RefCell;
use std::num::NonZeroU32;
use std::rc::Rc;

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
    /// KV cache type (F16, Q8_0, etc.).
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
        .map(|n| n.get())
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
    pub fn to_context_params(&self) -> LlamaContextParams {
        let mut params = LlamaContextParams::default();
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

/// Internal state for LlamaModels with interior mutability.
struct LlamaModelsInner {
    model: Rc<LlamaModel>,
    context: LlamaContextParams,
    #[allow(dead_code)]
    sampler: Option<LlamaSampler>,
    spec: ModelSpec,
    last_usage: Option<UsageReport>,
}

/// llama.cpp model wrapper implementing the Model trait.
///
/// Uses interior mutability (RefCell) so that &self methods can mutate
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
    /// Create a new LlamaModels instance.
    fn new(
        model: LlamaModel,
        context: LlamaContextParams,
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
        let backend = LlamaBackend::init().map_err(|e| {
            GenerationError::LlamaCpp(e)
        })?;

        // Get model, spec, and context params
        let (model, spec, ctx_params) = {
            let inner = self.inner.borrow();
            (Rc::clone(&inner.model), inner.spec.clone(), inner.context.clone())
        };

        // Create context
        let mut ctx = model.new_context(&backend, ctx_params).map_err(|e| {
            GenerationError::LlamaContextLoad(e)
        })?;

        // Build sampler chain from params
        let params = specs.unwrap_or_default();
        let mut sampler = build_sampler_chain(&params);

        // Check if this is an embedding request
        let is_embedding = interaction.messages.iter().any(|msg| {
            if let Messages::Assistant { content, .. } = msg {
                matches!(content, ModelOutput::Embedding { .. })
            } else {
                false
            }
        });

        // Apply chat template if messages are present
        let prompt = if !interaction.messages.is_empty() {
            // Use custom template if provided, otherwise use model's default template
            let template = if let Some(custom_template) = &interaction.chat_template {
                LlamaChatTemplate::new(custom_template).map_err(|e| {
                    GenerationError::Generic(format!("Chat template creation error: {e}"))
                })?
            } else {
                model.chat_template(None).map_err(|e| {
                    GenerationError::ChatTemplate(e)
                })?
            };

            // Convert our Messages to LlamaChatMessage
            let mut chat_messages = Vec::new();
            for msg in &interaction.messages {
                if let Messages::User { content, .. } = msg {
                    if let UserModelContent::Text(text_content) = content {
                        chat_messages.push(LlamaChatMessage::new(
                            "user".to_string(),
                            text_content.content.clone(),
                        ).map_err(|e| {
                            GenerationError::Generic(format!("Chat message error: {e}"))
                        })?);
                    }
                } else if let Messages::Assistant { content: ModelOutput::Text(text_content), .. } = msg {
                    chat_messages.push(LlamaChatMessage::new(
                        "assistant".to_string(),
                        text_content.content.clone(),
                    ).map_err(|e| {
                        GenerationError::Generic(format!("Chat message error: {e}"))
                    })?);
                }
            }

            // Apply template
            model.apply_chat_template(&template, &chat_messages, true).map_err(|e| {
                GenerationError::ApplyChatTemplate(e)
            })?
        } else {
            interaction.system_prompt.unwrap_or_default()
        };

        if is_embedding {
            // Generate embeddings
            let tokens = model.str_to_token(&prompt, AddBos::Always).map_err(|e| {
                GenerationError::Tokenization(e)
            })?;

            let mut batch = LlamaBatch::new(tokens.len(), 1);

            // Add tokens to batch
            for (i, &token) in tokens.iter().enumerate() {
                batch.add(token, i as i32, &[0], i == tokens.len() - 1).map_err(|e| {
                    GenerationError::Generic(format!("Batch add error: {e}"))
                })?;
            }

            // Encode
            ctx.encode(&mut batch).map_err(|e| {
                GenerationError::Encode(e)
            })?;

            // Get embeddings
            let embeddings = ctx.embeddings_seq_ith(0).map_err(|e| {
                GenerationError::Generic(format!("Embedding error: {e}"))
            })?;

            let dimensions = embeddings.len();
            let values = embeddings.to_vec();

            let usage = UsageReport {
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
            };

            self.inner.borrow_mut().last_usage = Some(usage.clone());

            Ok(vec![Messages::Assistant {
                model: spec.id.clone(),
                timestamp: std::time::SystemTime::now(),
                usage,
                content: ModelOutput::Embedding { dimensions, values },
                stop_reason: StopReason::Stop,
                provider: ModelProviders::LLAMACPP,
                error_detail: None,
                signature: None,
            }])
        } else {
            // Text generation
            let tokens = model.str_to_token(&prompt, AddBos::Always).map_err(|e| {
                GenerationError::Tokenization(e)
            })?;

            let mut batch = LlamaBatch::new(tokens.len(), 1);

            // Add prompt tokens to batch
            for (i, &token) in tokens.iter().enumerate() {
                batch.add(token, i as i32, &[0], i == tokens.len() - 1).map_err(|e| {
                    GenerationError::Generic(format!("Batch add error: {e}"))
                })?;
            }

            // Decode prompt
            ctx.decode(&mut batch).map_err(|e| {
                GenerationError::Decode(e)
            })?;

            let mut output_tokens = Vec::new();
            let mut pos = tokens.len() as i32;
            let max_tokens = params.max_tokens;

            // Get EOS token
            let eos_token = model.token_eos();

            // Generation loop
            for _ in 0..max_tokens {
                // Sample next token
                let new_token = sampler.sample(&ctx, -1);

                // Check for EOS
                if new_token == eos_token {
                    break;
                }

                output_tokens.push(new_token);
                sampler.accept(new_token);

                // Prepare next batch
                batch.clear();
                batch.add(new_token, pos, &[0], true).map_err(|e| {
                    GenerationError::Generic(format!("Batch add error: {e}"))
                })?;
                pos += 1;

                // Decode
                ctx.decode(&mut batch).map_err(|e| {
                    GenerationError::Decode(e)
                })?;
            }

            // Convert tokens to string
            let output_text = model.tokens_to_str(&output_tokens, Special::Tokenize)
                .unwrap_or_else(|_| String::from("[invalid tokens]"));

            let usage = UsageReport {
                input: tokens.len() as f64,
                output: output_tokens.len() as f64,
                cache_read: 0.0,
                cache_write: 0.0,
                total_tokens: (tokens.len() + output_tokens.len()) as f64,
                cost: UsageCosting {
                    currency: "USD".to_string(),
                    input: 0.0,
                    output: 0.0,
                    cache_read: 0.0,
                    cache_write: 0.0,
                    total_tokens: 0.0,
                },
            };

            self.inner.borrow_mut().last_usage = Some(usage.clone());

            Ok(vec![Messages::Assistant {
                model: spec.id.clone(),
                timestamp: std::time::SystemTime::now(),
                usage,
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
    }

    fn stream<T>(
        &self,
        _interaction: ModelInteraction,
        _specs: Option<ModelParams>,
    ) -> GenerationResult<T>
    where
        T: StreamIterator<D = Messages, P = ModelState>,
    {
        Err(GenerationError::Generic(
            "Streaming is not yet implemented for LlamaModels. \
             Use LlamaCppStream directly or call generate() instead."
                .to_string(),
        ))
    }
}

// ==================================
// LlamaCppStream
// ==================================

/// Stream iterator for token-by-token generation.
///
/// Implements `StreamIterator` to yield `Messages` one at a time
/// as tokens are generated.
pub struct LlamaCppStream {
    // Stream state would go here
    // This is a placeholder for future implementation
    _phantom: std::marker::PhantomData<Messages>,
}

impl Iterator for LlamaCppStream {
    type Item = Stream<Messages, ModelState>;

    fn next(&mut self) -> Option<Self::Item> {
        // Placeholder implementation
        // Full implementation would decode one token per call
        None
    }
}

// Note: StreamIterator is automatically implemented via blanket impl
// for any Iterator<Item = Stream<D, P>>

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

        let context_params = LlamaContextParams::default();

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
