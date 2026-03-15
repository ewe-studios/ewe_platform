# 05 - Foundation AI Type System

The `foundation_ai` crate (`backends/foundation_ai/`) defines the domain-level abstractions for interacting with AI models. These types are backend-agnostic -- the same `ModelParams` and `Model` trait work whether the underlying engine is llama.cpp, an HTTP API, or any other provider.

**Crate location**: `backends/foundation_ai/`

## Module Structure

```
src/
  lib.rs
  types/mod.rs                       -- Core types and traits (596 lines)
  backends/mod.rs                    -- Backend module declarations
  backends/llamacpp.rs               -- LlamaBackends enum implementing ModelBackend
  backends/huggingface.rs            -- HuggingFace backend (stub)
  errors/mod.rs                      -- Error types
  models/mod.rs                      -- Model descriptor modules
  models/model_descriptors.rs        -- Auto-generated provider descriptors (500KB+)
  models/model_descriptors_defaults.rs -- Default descriptors
```

## Core Traits

### `ModelProvider` - Model Discovery

```rust
pub trait ModelProvider {
    /// Get a single model matching the given ID
    fn get_one(&self, model_id: ModelId) -> ModelProviderResult<ModelSpec>;

    /// Get all models matching the given ID
    fn get_all(&self, model_id: ModelId) -> ModelProviderResult<ModelSpec>;
}
```

### `Model` - Inference Interface

```rust
pub trait Model {
    /// Get model specification
    fn spec(&self) -> ModelSpec;

    /// Generate text (convenience method)
    fn text(&self, prompt: String, specs: Option<ModelParams>) -> GenerationResult<String>;

    /// Stream text output (convenience method)
    fn stream_text<T>(&self, prompt: String, specs: Option<ModelParams>) -> GenerationResult<T>
    where T: StreamIterator<String, ()>;

    /// Generate arbitrary output type
    fn generate<T>(&self, prompt: String, specs: Option<ModelParams>) -> GenerationResult<T>;

    /// Stream arbitrary output
    fn stream<T, D, P>(&self, prompt: String, specs: Option<ModelParams>) -> GenerationResult<T>
    where T: StreamIterator<D, P>;
}
```

The `text()` and `stream_text()` methods are convenience wrappers around `generate()` and `stream()`. The `StreamIterator` type from `foundation_core` supports async computation with a synchronous iteration API.

### `ModelBackend` - Backend Factory

```rust
pub trait ModelBackend {
    /// Create a Model instance from a specification
    fn get_model<T: Model>(&self, model_spec: ModelSpec) -> ModelResult<T>;
}
```

## Model Identification

### `ModelId` - How to Find a Model

```rust
pub enum ModelId {
    /// Exact model name with optional quantization
    Name(String, Option<Quantization>),

    /// Short alias (not the full name)
    Alias(String, Option<Quantization>),

    /// Model group with optional quantization
    Group(String, Option<Quantization>),

    /// Model architecture with optional quantization
    Architecture(String, Option<Quantization>),
}
```

Usage pattern:
```rust
// Find a specific model
let id = ModelId::Name("llama-2-7b-chat".into(), Some(Quantization::Q4_KM));

// Find any model in a family
let id = ModelId::Group("llama-2".into(), Some(Quantization::Q5_KM));

// Find by architecture
let id = ModelId::Architecture("llama".into(), None);
```

### `ModelSpec` - Model Specification

```rust
pub struct ModelSpec {
    pub name: String,
    pub id: ModelId,
    pub devices: Option<Vec<DeviceId>>,
    pub model_location: Option<PathBuf>,
    pub lora_location: Option<PathBuf>,
}
```

This tells the backend everything needed to load a model:
- Which model to load (`id`)
- Where to find it (`model_location`)
- Optional LoRA adapter (`lora_location`)
- Which devices to use (`devices`)

### `DeviceId` - GPU/Accelerator Identification

```rust
#[derive(Serialize, Deserialize, Debug, Copy, Clone, PartialEq, PartialOrd)]
pub struct DeviceId(u16);

impl DeviceId {
    pub fn new(id: u16) -> Self;
    pub fn get_id(&self) -> u16;
}
```

Maps to the device indices returned by `list_llama_ggml_backend_devices()` in the safe wrapper layer.

## Generation Parameters

### `ModelParams` - How to Generate

```rust
pub struct ModelParams {
    pub max_tokens: usize,           // Maximum tokens to generate
    pub temperature: f32,            // Sampling temperature
    pub top_p: f32,                  // Nucleus sampling threshold
    pub top_k: f32,                  // Top-K sampling
    pub repeat_penalty: f32,         // Repetition penalty
    pub seed: Option<u32>,           // RNG seed
    pub stop_tokens: Vec<String>,    // Stop sequences
    pub thinking_level: ThinkingLevels,
    pub cache_retention: CacheRetention,
    pub thinking_budget: Option<ThinkingBudget>,
}
```

### Mapping to llama.cpp

| `ModelParams` field | llama.cpp equivalent | Rust wrapper |
|--------------------|--------------------|--------------|
| `max_tokens` | Loop iteration count | Manual loop control |
| `temperature` | `llama_sampler_init_temp(t)` | `LlamaSampler::temp(t)` |
| `top_p` | `llama_sampler_init_top_p(p, 1)` | `LlamaSampler::top_p(p, 1)` |
| `top_k` | `llama_sampler_init_top_k(k)` | `LlamaSampler::top_k(k as i32)` |
| `repeat_penalty` | `llama_sampler_init_penalties(n, rp, 0, 0)` | `LlamaSampler::penalties(n, rp, 0.0, 0.0)` |
| `seed` | `llama_sampler_init_dist(seed)` | `LlamaSampler::dist(seed)` |
| `stop_tokens` | Manual check against `model.is_eog_token()` + string matching | Manual implementation |

### `ModelConfig` - Runtime Configuration

```rust
pub struct ModelConfig {
    pub context_length: usize,
    pub max_threads: usize,
    pub template: Option<String>,
    pub params: ModelParams,
    pub streaming: bool,
}
```

### Mapping to llama.cpp

| `ModelConfig` field | llama.cpp equivalent | Rust wrapper |
|--------------------|--------------------|--------------|
| `context_length` | `llama_context_params.n_ctx` | `LlamaContextParams::with_n_ctx(NonZeroU32::new(n))` |
| `max_threads` | `llama_context_params.n_threads` | `LlamaContextParams::with_n_threads(n)` |
| `template` | Chat template name or Jinja string | `LlamaChatTemplate::new(tmpl)` |
| `streaming` | Token-by-token output in generation loop | Manual implementation |

## Model Source

```rust
pub enum ModelSource {
    /// Remote HTTP endpoint
    HTTP(Uri),

    /// HuggingFace repository name
    HuggingFace(String),

    /// Local GGUF file path
    LocalFile(PathBuf),

    /// Local directory containing model files
    LocalDirectory(PathBuf),
}
```

For llama.cpp, the resolution path is:
1. `LocalFile` -> Direct path to GGUF file, pass to `LlamaModel::load_from_file()`
2. `LocalDirectory` -> Search for `*.gguf` files in directory
3. `HuggingFace` -> Use `hf_hub` crate to download/cache, then load as local file
4. `HTTP` -> Download to local cache, then load as local file

## Quantization

```rust
pub enum Quantization {
    None,             // F32 - no quantization
    Default,          // Use model's built-in quantization
    F16,              // Half precision
    Q2K, Q2_KS, Q2_KM, Q2_KL,   // 2-bit K-quant variants
    Q3_KS, Q3_KM,               // 3-bit K-quant variants
    Q4_0, Q4_1,                  // Legacy 4-bit
    IQ_4Nl, IQ_4Xs,             // Importance-quantized 4-bit
    Q4_KM, Q4_KS,               // 4-bit K-quant variants
    Q5_KS, Q5_KM, Q5_KL,       // 5-bit K-quant variants
    Q6_K, Q6_KM, Q6_KS, Q6_KL, // 6-bit K-quant variants
    Q8_0, Q8_1,                  // 8-bit quantization
    Ud_IQ_1M, UD_IQ_1S,         // Ultra-dense 1-bit
    UD_IQ_2M, UD_IQ_2Xxs,       // Ultra-dense 2-bit
    UD_IQ_3Xxs,                  // Ultra-dense 3-bit
    UD_Q_2KXl, UD_Q_3KXl, UD_Q_4KXl,  // Ultra-dense K-quant XL
    UD_Q_5KXl, UD_Q_6KXl, UD_Q_8KXl,  // Ultra-dense K-quant XL
    Custom(String),              // Unknown/custom format
}
```

The `Quantization` enum is used for model selection (in `ModelId`) -- it tells the system which quantization variant of a model to prefer. The actual GGUF file contains the quantization metadata.

## Model Output

```rust
pub enum ModelOutput {
    Text {
        content: String,
        signature: Option<String>,
    },
    ThinkingContent {
        thinking: String,
        signature: Option<String>,
    },
    Image {
        b64: String,
        mime_type: MimeType,
    },
    ToolCall {
        id: String,
        name: String,
        arguments: Option<HashMap<String, ArgType>>,
        signature: Option<String>,
    },
}
```

For llama.cpp text generation, the output would be `ModelOutput::Text`. For models with chain-of-thought, `ThinkingContent` captures the reasoning trace separately.

## Thinking Budget

```rust
pub struct ThinkingBudget {
    pub minimal: f64,
    pub medium: f64,
    pub low: f64,
    pub high: f64,
}

pub enum ThinkingLevels {
    Minimal, Low, Medium, High, Custom(String),
}
```

These control how much reasoning effort a model should expend. For llama.cpp, this could map to:
- Number of tokens allocated for chain-of-thought
- Temperature adjustments for the thinking phase
- Whether to use extended context for reasoning

## Cache Retention

```rust
pub enum CacheRetention {
    None,              // No KV cache retention between calls
    Short,             // Keep cache for short-term reuse
    Long,              // Keep cache for long-term reuse
    Custom(String),    // Custom retention policy
}
```

Maps to KV cache management in llama.cpp:
- `None` -> `ctx.clear_kv_cache()` between calls
- `Short` -> Keep cache but clear after N turns
- `Long` -> Persist via `ctx.save_session_file()`

## Provider System

### `KnownModelProviders`

```rust
pub enum KnownModelProviders {
    AMAZONBEDROCK, ANTHROPIC, GOOGLE, GOOGLEGEMINICLI,
    GOOGLEANTIGRAVITY, GOOGLEVERTEX, OPENAI, AZUREOPENAIRESPONSES,
    OPENAICODEX, GITHUBCOPILOT, XAI, GROQ, CEREBRAS,
    OPENROUTER, VERCELAIGATEWAY, ZAI, MISTRAL,
    MINIMAX, MINIMAXCN, HUGGINGFACE, OPENCODE, KIMICODING,
    Custom(String),
}
```

### `ModelProviderDescriptor`

```rust
pub struct ModelProviderDescriptor {
    pub id: String,                    // e.g., "claude-sonnet-4-20250514"
    pub name: String,                  // e.g., "Claude Sonnet 4"
    pub api: ModelAPI,                 // API protocol
    pub provider: String,             // Provider key
    pub base_url: String,             // API endpoint
    pub reasoning: bool,              // Supports chain-of-thought
    pub inputs: [MessageType; 2],     // Supported input types
    pub cost: ModelUsageCosting,      // Per-token pricing
    pub context_window: u32,          // Max context length
    pub max_tokens: u32,              // Max output tokens
}
```

The `model_descriptors.rs` file contains auto-generated descriptors for 100+ models across all providers.

### `ModelAPI`

```rust
pub enum ModelAPI {
    OpenAICompletions,
    OpenAIResponses,
    AzureOpenaiResponses,
    OpenaiCodexResponses,
    AnthropicMessages,
    BedrockConverseStream,
    GoogleGenerativeAi,
    GoogleGeminiCli,
    GoogleVertex,
    Custom(String),
}
```

## llama.cpp Backend Implementation

**File**: `backends/foundation_ai/src/backends/llamacpp.rs`

```rust
pub enum LlamaBackends {
    LLamaCPU,    // CPU-only inference
    LLamaGPU,    // GPU via Vulkan or CUDA
    LLamaMetal,  // Apple Metal
}

impl ModelBackend for LlamaBackends {
    fn get_model<T: Model>(&self, _model_spec: ModelSpec) -> ModelResult<T> {
        todo!()  // Implementation pending
    }
}
```

The backend is currently a skeleton with `todo!()` implementations. See [11-integration-guide.md](./11-integration-guide.md) for how to complete the implementation.

## Error Types

```rust
pub enum GenerationError {
    Failed(BoxedError),
}
pub type GenerationResult<T> = Result<T, GenerationError>;

pub enum ModelProviderErrors {
    NotFound(String),
    FailedFetching(BoxedError),
}
pub type ModelProviderResult<T> = Result<T, ModelProviderErrors>;

pub enum ModelErrors {
    NotFound(String),
    FailedLoading(BoxedError),
}
pub type ModelResult<T> = Result<T, ModelErrors>;

pub enum FoundationAIErrors {
    ModelErrors(ModelErrors),
    GenerationErrors(GenerationError),
    RegistryErrors(ModelProviderErrors),
}
pub type FoundationAIResult<T> = Result<T, FoundationAIErrors>;
```

## MimeType

```rust
pub enum MimeType {
    TextPlain, TextHtml, TextMarkdown, TextXml, TextCss,
    ApplicationJson, ApplicationXml, ApplicationOctetStream, ApplicationPdf,
    ImagePng, ImageJpeg, ImageGif, ImageWebp, ImageSvgXml, ImageBmp,
    AudioMp3, AudioWav, AudioOgg, AudioMpeg,
    VideoMp4, VideoWebm, VideoOgg,
    Custom(String),
}
```

Used in `ModelOutput::Image` to specify the format of generated images.

See [06-model-lifecycle.md](./06-model-lifecycle.md) for how these types flow through the system during inference.
