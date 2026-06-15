# 11 - Integration Guide

This document provides a step-by-step guide for completing the integration between the `foundation_ai` domain layer (Layer 3) and the `infrastructure_llama_cpp` safe wrapper (Layer 2). The current implementation has skeleton code with `todo!()` markers that need to be filled in.

## Current State

The `foundation_ai` crate defines traits and types for backend-agnostic model interaction. The `LlamaBackends` enum in `backends/foundation_ai/src/backends/llamacpp.rs` implements `ModelBackend` but all methods are `todo!()`:

```rust
// Current state in backends/foundation_ai/src/backends/llamacpp.rs
pub enum LlamaBackends {
    LLamaCPU,
    LLamaGPU,
    LLamaMetal,
}

impl ModelBackend for LlamaBackends {
    fn get_model<T: Model>(&self, _model_spec: ModelSpec) -> ModelResult<T> {
        todo!()
    }
}

impl LlamaBackends {
    fn get_llama_cpu_model<T: Model>(
        &self,
        _model_spec: ModelSpec,
    ) -> ModelResult<T> {
        todo!()
    }
}
```

## Step 1: Implement the LlamaModel Wrapper

Create a struct that implements the `Model` trait by wrapping `infrastructure_llama_cpp` types.

### Design

```rust
use infrastructure_llama_cpp::llama_backend::LlamaBackend;
use infrastructure_llama_cpp::model::LlamaModel as LLModel;
use infrastructure_llama_cpp::context::LlamaContext;
use infrastructure_llama_cpp::context::params::LlamaContextParams;
use infrastructure_llama_cpp::sampling::LlamaSampler;
use infrastructure_llama_cpp::llama_batch::LlamaBatch;
use infrastructure_llama_cpp::model::{AddBos, Special};

use crate::types::{Model, ModelSpec, ModelParams, ModelConfig, ModelOutput, GenerationResult};
use crate::errors::GenerationError;

use std::sync::Arc;

/// A concrete Model implementation backed by llama.cpp.
pub struct LlamaCppModel {
    spec: ModelSpec,
    config: ModelConfig,
    backend: Arc<LlamaBackend>,
    model: Arc<LLModel>,
}
```

### Lifetime Challenge

The key architectural challenge is that `LlamaContext<'a>` borrows `&'a LlamaModel`, creating a lifetime dependency. This means the context cannot outlive the model. Two approaches:

**Approach A: Context per call** -- Create a fresh context for each `text()` / `generate()` call. Simple but wasteful (no KV cache reuse between calls).

**Approach B: Persistent context** -- Store the context alongside the model using `Arc<LLModel>` and unsafe lifetime extension. More complex but enables KV cache reuse.

For the initial implementation, Approach A is recommended:

```rust
impl LlamaCppModel {
    fn create_context(&self) -> Result<LlamaContext<'_>, GenerationError> {
        let ctx_params = self.translate_context_params();
        self.model
            .new_context(&self.backend, ctx_params)
            .map_err(|e| GenerationError::Failed(Box::new(e)))
    }
}
```

## Step 2: Parameter Translation

### ModelParams to Sampling Chain

Map `foundation_ai::types::ModelParams` fields to `LlamaSampler` chain elements:

```rust
impl LlamaCppModel {
    fn build_sampler(&self, params: &ModelParams) -> LlamaSampler {
        let mut samplers: Vec<LlamaSampler> = Vec::new();

        // Repetition penalty
        if params.repeat_penalty > 1.0 {
            samplers.push(LlamaSampler::penalties(
                64,                      // penalty_last_n (look-back window)
                params.repeat_penalty,   // repeat_penalty
                0.0,                     // frequency_penalty
                0.0,                     // presence_penalty
            ));
        }

        // Top-K filtering
        if params.top_k > 0.0 {
            samplers.push(LlamaSampler::top_k(params.top_k as i32));
        }

        // Top-P (nucleus) sampling
        if params.top_p < 1.0 {
            samplers.push(LlamaSampler::top_p(params.top_p, 1));
        }

        // Temperature
        if params.temperature > 0.0 {
            samplers.push(LlamaSampler::temp(params.temperature));
        }

        // Selection strategy
        if params.temperature == 0.0 {
            samplers.push(LlamaSampler::greedy());
        } else {
            samplers.push(LlamaSampler::dist(params.seed.unwrap_or(1234)));
        }

        LlamaSampler::chain_simple(samplers)
    }
}
```

### ModelConfig to Context Parameters

Map `foundation_ai::types::ModelConfig` to `LlamaContextParams`:

```rust
use std::num::NonZeroU32;

impl LlamaCppModel {
    fn translate_context_params(&self) -> LlamaContextParams {
        let mut ctx_params = LlamaContextParams::default();

        if self.config.context_length > 0 {
            ctx_params = ctx_params
                .with_n_ctx(NonZeroU32::new(self.config.context_length as u32));
        }

        if self.config.max_threads > 0 {
            let threads = self.config.max_threads as i32;
            ctx_params = ctx_params
                .with_n_threads(threads)
                .with_n_threads_batch(threads);
        }

        ctx_params
    }
}
```

### ModelSpec to Model Parameters

Map `ModelSpec` fields to `LlamaModelParams`:

```rust
use infrastructure_llama_cpp::model::params::LlamaModelParams;

impl LlamaBackends {
    fn translate_model_params(&self) -> LlamaModelParams {
        match self {
            LlamaBackends::LLamaCPU => {
                LlamaModelParams::default()
                    .with_n_gpu_layers(0)
            }
            LlamaBackends::LLamaGPU => {
                LlamaModelParams::default()
                    .with_n_gpu_layers(999)
            }
            LlamaBackends::LLamaMetal => {
                LlamaModelParams::default()
                    .with_n_gpu_layers(999)
            }
        }
    }
}
```

### Device Mapping

Map `foundation_ai::types::DeviceId` to backend device indices:

```rust
use crate::types::DeviceId;

fn map_devices(
    devices: &Option<Vec<DeviceId>>,
    model_params: LlamaModelParams,
) -> Result<LlamaModelParams, crate::errors::ModelErrors> {
    if let Some(device_ids) = devices {
        let indices: Vec<usize> = device_ids
            .iter()
            .map(|d| d.get_id() as usize)
            .collect();
        model_params
            .with_devices(&indices)
            .map_err(|e| crate::errors::ModelErrors::FailedLoading(Box::new(e)))
    } else {
        Ok(model_params)
    }
}
```

## Step 3: Implement the Model Trait

### Text Generation

```rust
impl Model for LlamaCppModel {
    fn spec(&self) -> ModelSpec {
        self.spec.clone()
    }

    fn text(
        &self,
        prompt: String,
        specs: Option<ModelParams>,
    ) -> GenerationResult<String> {
        let params = specs.unwrap_or(self.config.params.clone());
        let mut ctx = self.create_context()?;

        // Apply chat template if configured
        let formatted = if let Some(ref tmpl_name) = self.config.template {
            let template = self.model.chat_template(Some(tmpl_name))
                .map_err(|e| GenerationError::Failed(Box::new(e)))?;
            let messages = vec![
                infrastructure_llama_cpp::model::LlamaChatMessage::new(
                    "user".into(), prompt
                ).map_err(|e| GenerationError::Failed(Box::new(e)))?,
            ];
            self.model.apply_chat_template(&template, &messages, true)
                .map_err(|e| GenerationError::Failed(Box::new(e)))?
        } else {
            prompt
        };

        // Tokenize
        let tokens = self.model
            .str_to_token(&formatted, AddBos::Always)
            .map_err(|e| GenerationError::Failed(Box::new(e)))?;

        // Prefill
        let mut batch = LlamaBatch::new(512, 1);
        let last = (tokens.len() - 1) as i32;
        for (i, tok) in (0i32..).zip(tokens.iter()) {
            batch.add(*tok, i, &[0], i == last)
                .map_err(|e| GenerationError::Failed(Box::new(e)))?;
        }
        ctx.decode(&mut batch)
            .map_err(|e| GenerationError::Failed(Box::new(e)))?;

        // Sample
        let mut sampler = self.build_sampler(&params);
        let mut n_cur = batch.n_tokens();
        let mut decoder = encoding_rs::UTF_8.new_decoder();
        let mut output = String::new();

        for _ in 0..params.max_tokens {
            let token = sampler.sample(&ctx, batch.n_tokens() - 1);
            sampler.accept(token);

            if self.model.is_eog_token(token) {
                break;
            }

            // Check stop tokens
            let bytes = self.model.token_to_bytes(token, Special::Tokenize)
                .map_err(|e| GenerationError::Failed(Box::new(e)))?;
            let mut piece = String::with_capacity(32);
            decoder.decode_to_string(&bytes, &mut piece, false);
            output.push_str(&piece);

            // Check if any stop token was generated
            if params.stop_tokens.iter().any(|stop| output.ends_with(stop)) {
                // Trim the stop token from output
                for stop in &params.stop_tokens {
                    if output.ends_with(stop) {
                        let new_len = output.len() - stop.len();
                        output.truncate(new_len);
                        break;
                    }
                }
                break;
            }

            batch.clear();
            batch.add(token, n_cur, &[0], true)
                .map_err(|e| GenerationError::Failed(Box::new(e)))?;
            ctx.decode(&mut batch)
                .map_err(|e| GenerationError::Failed(Box::new(e)))?;
            n_cur += 1;
        }

        Ok(output)
    }

    fn stream_text<T>(
        &self,
        _prompt: String,
        _specs: Option<ModelParams>,
    ) -> GenerationResult<T>
    where
        T: foundation_core::types::StreamIterator<String, ()>,
    {
        // Implementation would use a channel-based StreamIterator
        // that yields tokens as they are generated
        todo!("Implement streaming")
    }

    fn generate<T>(
        &self,
        prompt: String,
        specs: Option<ModelParams>,
    ) -> GenerationResult<T> {
        // For text output, delegate to text() and wrap in ModelOutput::Text
        todo!("Implement generic generate")
    }

    fn stream<T, D, P>(
        &self,
        _prompt: String,
        _specs: Option<ModelParams>,
    ) -> GenerationResult<T>
    where
        T: foundation_core::types::StreamIterator<D, P>,
    {
        todo!("Implement generic streaming")
    }
}
```

## Step 4: Implement ModelBackend

### Model Construction

```rust
use crate::errors::{ModelErrors, ModelResult};

impl ModelBackend for LlamaBackends {
    fn get_model<T: Model>(&self, model_spec: ModelSpec) -> ModelResult<T> {
        match self {
            LlamaBackends::LLamaCPU => self.build_model(model_spec, 0),
            LlamaBackends::LLamaGPU => self.build_model(model_spec, 999),
            LlamaBackends::LLamaMetal => self.build_model(model_spec, 999),
        }
    }
}

impl LlamaBackends {
    fn build_model<T: Model>(
        &self,
        model_spec: ModelSpec,
        n_gpu_layers: u32,
    ) -> ModelResult<T> {
        // Initialize backend
        let backend = LlamaBackend::init()
            .map_err(|e| ModelErrors::FailedLoading(Box::new(e)))?;
        let backend = Arc::new(backend);

        // Configure model parameters
        let mut model_params = LlamaModelParams::default()
            .with_n_gpu_layers(n_gpu_layers);

        // Apply device selection if specified
        model_params = map_devices(&model_spec.devices, model_params)?;

        // Resolve model path
        let model_path = model_spec.model_location
            .as_ref()
            .ok_or_else(|| ModelErrors::NotFound(
                format!("No model_location in spec: {:?}", model_spec.id)
            ))?;

        // Load model
        let model = LLModel::load_from_file(&backend, model_path, &model_params)
            .map_err(|e| ModelErrors::FailedLoading(Box::new(e)))?;
        let model = Arc::new(model);

        // Load LoRA adapter if specified
        if let Some(ref lora_path) = model_spec.lora_location {
            let _adapter = model.lora_adapter_init(lora_path)
                .map_err(|e| ModelErrors::FailedLoading(Box::new(e)))?;
            // Store adapter for later use with context
        }

        // Build config with defaults
        let config = ModelConfig {
            context_length: model.n_ctx_train() as usize,
            max_threads: std::thread::available_parallelism()
                .map(|p| p.get())
                .unwrap_or(4),
            template: model.chat_template(None).ok()
                .and_then(|t| t.to_str().ok().map(|s| s.to_string())),
            params: ModelParams::default(),
            streaming: false,
        };

        let llama_model = LlamaCppModel {
            spec: model_spec,
            config,
            backend,
            model,
        };

        // This requires T to be LlamaCppModel -- see "Type Erasure" section below
        todo!("Return llama_model as T")
    }
}
```

### Type Erasure Challenge

The `ModelBackend::get_model<T: Model>` signature requires returning a generic `T`. This creates a fundamental tension: the backend knows it creates a `LlamaCppModel`, but must return it as an arbitrary `T: Model`.

**Option 1: Downcast via Any**

```rust
use std::any::Any;

pub trait Model: Any + Send + Sync {
    fn spec(&self) -> ModelSpec;
    fn text(&self, prompt: String, specs: Option<ModelParams>) -> GenerationResult<String>;
    // ... other methods ...

    fn as_any(&self) -> &dyn Any;
}

impl ModelBackend for LlamaBackends {
    fn get_model<T: Model + 'static>(&self, model_spec: ModelSpec) -> ModelResult<T> {
        let model = self.build_model_internal(model_spec)?;
        let boxed: Box<dyn Any> = Box::new(model);
        boxed.downcast::<T>()
            .map(|b| *b)
            .map_err(|_| ModelErrors::NotFound("Type mismatch".into()))
    }
}
```

**Option 2: Trait object return** (requires changing the trait)

```rust
pub trait ModelBackend {
    fn get_model(&self, model_spec: ModelSpec) -> ModelResult<Box<dyn Model>>;
}
```

Option 2 is simpler and more idiomatic for this use case. The generic `<T: Model>` signature is mainly useful when the caller knows the concrete type, which is uncommon with backend-agnostic abstractions.

## Step 5: Error Mapping

Map between `infrastructure_llama_cpp` errors and `foundation_ai` errors:

```rust
use crate::errors::{GenerationError, ModelErrors, ModelProviderErrors};
use infrastructure_llama_cpp::{LlamaCppError, DecodeError};

impl From<LlamaCppError> for ModelErrors {
    fn from(e: LlamaCppError) -> Self {
        ModelErrors::FailedLoading(Box::new(e))
    }
}

impl From<DecodeError> for GenerationError {
    fn from(e: DecodeError) -> Self {
        GenerationError::Failed(Box::new(e))
    }
}

impl From<LlamaCppError> for GenerationError {
    fn from(e: LlamaCppError) -> Self {
        GenerationError::Failed(Box::new(e))
    }
}
```

### Error Context

Provide actionable error messages:

```rust
fn load_model(path: &Path) -> ModelResult<LLModel> {
    LLModel::load_from_file(&backend, path, &model_params)
        .map_err(|e| ModelErrors::FailedLoading(Box::new(
            anyhow::anyhow!("Failed to load GGUF model at {}: {}", path.display(), e)
        )))
}
```

## Step 6: Streaming Implementation

### Channel-Based StreamIterator

```rust
use std::sync::mpsc;

pub struct TokenStream {
    receiver: mpsc::Receiver<String>,
}

impl foundation_core::types::StreamIterator<String, ()> for TokenStream {
    // Implementation depends on foundation_core's StreamIterator definition
    // Typically involves:
    // - next() -> Option<String> that reads from channel
    // - The generation loop runs in a separate thread, sending tokens
}

impl LlamaCppModel {
    fn stream_text_impl(
        &self,
        prompt: String,
        params: ModelParams,
    ) -> GenerationResult<TokenStream> {
        let (sender, receiver) = mpsc::channel();

        let model = Arc::clone(&self.model);
        let backend = Arc::clone(&self.backend);
        let config = self.config.clone();

        std::thread::spawn(move || {
            // Run generation loop, sending each token piece through the channel
            let mut ctx = model.new_context(&backend, /* params */).unwrap();
            // ... tokenize, prefill, generate loop ...
            // In the loop:
            //   sender.send(piece).ok();
        });

        Ok(TokenStream { receiver })
    }
}
```

## Step 7: Model Discovery via ModelProvider

### ModelProvider Implementation

```rust
use crate::types::{ModelProvider, ModelId, ModelSpec, Quantization};
use crate::errors::{ModelProviderErrors, ModelProviderResult};

pub struct LocalModelProvider {
    model_directory: PathBuf,
}

impl ModelProvider for LocalModelProvider {
    fn get_one(&self, model_id: ModelId) -> ModelProviderResult<ModelSpec> {
        match model_id {
            ModelId::Name(name, quant) => {
                let filename = format_model_filename(&name, &quant);
                let path = self.model_directory.join(&filename);
                if path.exists() {
                    Ok(ModelSpec {
                        name: name.clone(),
                        id: ModelId::Name(name, quant),
                        devices: None,
                        model_location: Some(path),
                        lora_location: None,
                    })
                } else {
                    Err(ModelProviderErrors::NotFound(
                        format!("Model not found: {}", filename)
                    ))
                }
            }
            ModelId::Alias(alias, quant) => {
                // Look up alias in a mapping table
                todo!("Alias resolution")
            }
            _ => Err(ModelProviderErrors::NotFound(
                "Unsupported ModelId variant".into()
            )),
        }
    }

    fn get_all(&self, _model_id: ModelId) -> ModelProviderResult<ModelSpec> {
        todo!("Return all matching models")
    }
}

fn format_model_filename(name: &str, quant: &Option<Quantization>) -> String {
    match quant {
        Some(Quantization::Q4_KM) => format!("{}.Q4_K_M.gguf", name),
        Some(Quantization::Q5_KM) => format!("{}.Q5_K_M.gguf", name),
        Some(Quantization::Q8_0) => format!("{}.Q8_0.gguf", name),
        Some(Quantization::F16) => format!("{}.F16.gguf", name),
        None | Some(Quantization::Default) => format!("{}.gguf", name),
        Some(q) => format!("{}.{:?}.gguf", name, q),
    }
}
```

### HuggingFace GGUF Provider

```rust
pub struct HuggingFaceGGUFProvider;

impl ModelProvider for HuggingFaceGGUFProvider {
    fn get_one(&self, model_id: ModelId) -> ModelProviderResult<ModelSpec> {
        match model_id {
            ModelId::Name(name, quant) => {
                let filename = format_model_filename(&name, &quant);
                let path = hf_hub::api::sync::ApiBuilder::new()
                    .with_progress(true)
                    .build()
                    .map_err(|e| ModelProviderErrors::FailedFetching(Box::new(e)))?
                    .model(name.clone())
                    .get(&filename)
                    .map_err(|e| ModelProviderErrors::FailedFetching(Box::new(e)))?;

                Ok(ModelSpec {
                    name: name.clone(),
                    id: ModelId::Name(name, quant),
                    devices: None,
                    model_location: Some(path),
                    lora_location: None,
                })
            }
            _ => Err(ModelProviderErrors::NotFound(
                "HuggingFace provider requires ModelId::Name".into()
            )),
        }
    }

    fn get_all(&self, _model_id: ModelId) -> ModelProviderResult<ModelSpec> {
        todo!()
    }
}
```

## Step 8: Resource Management

### Backend Singleton

The `LlamaBackend` can only be initialized once. Use `Arc` to share it:

```rust
use std::sync::{Arc, OnceLock};

static BACKEND: OnceLock<Arc<LlamaBackend>> = OnceLock::new();

fn get_or_init_backend() -> Result<Arc<LlamaBackend>, LlamaCppError> {
    BACKEND.get_or_try_init(|| {
        LlamaBackend::init().map(Arc::new)
    }).cloned()
}
```

### Model Caching

Cache loaded models to avoid repeated loading:

```rust
use std::collections::HashMap;
use std::sync::Mutex;

static MODEL_CACHE: OnceLock<Mutex<HashMap<String, Arc<LLModel>>>> = OnceLock::new();

fn get_or_load_model(
    backend: &LlamaBackend,
    path: &Path,
    params: &LlamaModelParams,
) -> Result<Arc<LLModel>, LlamaCppError> {
    let cache = MODEL_CACHE.get_or_init(|| Mutex::new(HashMap::new()));
    let key = path.to_string_lossy().to_string();

    let mut cache = cache.lock().unwrap();
    if let Some(model) = cache.get(&key) {
        return Ok(Arc::clone(model));
    }

    let model = LLModel::load_from_file(backend, path, params)?;
    let model = Arc::new(model);
    cache.insert(key, Arc::clone(&model));
    Ok(model)
}
```

### Cleanup

The RAII pattern ensures cleanup:

```rust
// Automatic cleanup order:
// 1. LlamaContext drops first (tied to model lifetime via 'a)
// 2. LlamaModel drops (frees weights)
// 3. LlamaBackend drops (frees global state, resets AtomicBool)

// For explicit cleanup:
drop(ctx);      // llama_free()
drop(model);    // llama_free_model()
drop(backend);  // llama_backend_free()
```

## Step 9: Testing

### Unit Testing the Integration

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parameter_translation() {
        let params = ModelParams {
            max_tokens: 100,
            temperature: 0.8,
            top_p: 0.95,
            top_k: 40.0,
            repeat_penalty: 1.1,
            seed: Some(42),
            stop_tokens: vec![],
            ..Default::default()
        };

        // Verify sampler chain construction doesn't panic
        let model = LlamaCppModel { /* ... */ };
        let _sampler = model.build_sampler(&params);
    }

    #[test]
    fn test_model_params_mapping() {
        let backend = LlamaBackends::LLamaCPU;
        let params = backend.translate_model_params();
        assert_eq!(params.n_gpu_layers(), 0);

        let backend = LlamaBackends::LLamaGPU;
        let params = backend.translate_model_params();
        assert_eq!(params.n_gpu_layers(), 999);
    }

    #[test]
    fn test_device_mapping() {
        let devices = Some(vec![DeviceId::new(0), DeviceId::new(1)]);
        let params = LlamaModelParams::default();
        // This will fail without actual GPU devices, but tests the mapping logic
        let result = map_devices(&devices, params);
        // In CI without GPUs: assert!(result.is_err());
    }
}
```

### Integration Testing

Integration tests require actual model files. Use small models for testing:

```rust
#[cfg(test)]
mod integration_tests {
    use super::*;

    // Only run with: cargo test -- --ignored
    #[test]
    #[ignore]
    fn test_full_generation_pipeline() {
        let backend = LlamaBackend::init().unwrap();
        let model_params = LlamaModelParams::default();
        let model = LlamaModel::load_from_file(
            &backend,
            "test-models/tiny-model.gguf",
            &model_params,
        ).unwrap();

        let ctx_params = LlamaContextParams::default()
            .with_n_ctx(NonZeroU32::new(256));
        let mut ctx = model.new_context(&backend, ctx_params).unwrap();

        let tokens = model.str_to_token("Hello", AddBos::Always).unwrap();
        assert!(!tokens.is_empty());

        let mut batch = LlamaBatch::new(64, 1);
        batch.add(tokens[0], 0, &[0], true).unwrap();
        ctx.decode(&mut batch).unwrap();

        let sampler = LlamaSampler::chain_simple([
            LlamaSampler::greedy(),
        ]);

        let token = sampler.sample(&ctx, 0);
        assert_ne!(token.0, -1); // Valid token returned
    }
}
```

## Implementation Checklist

1. **Create `LlamaCppModel` struct** in `backends/foundation_ai/src/backends/llamacpp.rs` (or a new file)
   - Wraps `Arc<LlamaModel>` and `Arc<LlamaBackend>`
   - Stores `ModelSpec` and `ModelConfig`

2. **Implement parameter translation**
   - `ModelParams` -> `LlamaSampler` chain
   - `ModelConfig` -> `LlamaContextParams`
   - `LlamaBackends` variant -> `LlamaModelParams`
   - `DeviceId` -> backend device index

3. **Implement `Model` trait**
   - `text()` with full generation loop
   - `stream_text()` with channel-based streaming
   - `generate()` and `stream()` for generic output

4. **Complete `ModelBackend::get_model`**
   - Model path resolution from `ModelSpec`
   - LoRA adapter loading
   - Device selection

5. **Implement error mapping**
   - `From<LlamaCppError>` for `ModelErrors` and `GenerationError`
   - Actionable error messages with context

6. **Add resource management**
   - Backend singleton pattern
   - Model caching
   - Thread-safe context creation

7. **Write tests**
   - Unit tests for parameter translation
   - Integration tests with real models (marked `#[ignore]`)

## File Structure After Implementation

```
backends/foundation_ai/src/
    backends/
        mod.rs              -- Module declarations
        llamacpp.rs         -- LlamaBackends enum + ModelBackend impl
        llamacpp_model.rs   -- LlamaCppModel struct + Model impl (new)
        huggingface.rs      -- HuggingFace backend (stub)
    types/mod.rs            -- Core types and traits (unchanged)
    errors/mod.rs           -- Error types (add From impls)
    models/
        mod.rs              -- Model descriptor modules
```

See [05-foundation-ai-types.md](./05-foundation-ai-types.md) for the trait definitions and [04-rust-safe-wrappers.md](./04-rust-safe-wrappers.md) for the complete Layer 2 API.
