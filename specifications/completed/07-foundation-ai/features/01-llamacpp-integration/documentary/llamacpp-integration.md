# llama.cpp Integration Documentary

## Executive Summary

This document provides a comprehensive guide to integrating llama.cpp with the EWE Platform's `foundation_ai` backend via the existing `infrastructure_llama_cpp` crate. It covers llama.cpp's architecture, API surface, GGUF model format, and detailed Rust integration patterns.

---

## Table of Contents

1. [llama.cpp Overview](#llamacpp-overview)
2. [Architecture](#architecture)
3. [GGUF Model Format](#gguf-model-format)
4. [Core API Components](#core-api-components)
5. [Rust Bindings Architecture](#rust-bindings-architecture)
6. [Model Loading](#model-loading)
7. [Context Management](#context-management)
8. [Tokenization](#tokenization)
9. [Inference](#inference)
10. [Sampling Strategies](#sampling-strategies)
11. [KV Cache Management](#kv-cache-management)
12. [Chat Templates](#chat-templates)
13. [LoRA Adapters](#lora-adapters)
14. [Embeddings](#embeddings)
15. [Multimodal Support (mtmd)](#multimodal-support-mtmd)
16. [Hardware Acceleration](#hardware-acceleration)
17. [Integration with foundation_ai](#integration-with-foundation_ai)
18. [Use Case Patterns](#use-case-patterns)

---

## llama.cpp Overview

**llama.cpp** is a highly optimized C/C++ library for running large language models (LLMs) efficiently on consumer hardware. Key characteristics:

- **Performance**: Optimized for CPU and GPU inference with quantization support
- **Portability**: Cross-platform with support for CUDA, Metal, Vulkan, and CPU-only execution
- **Model Support**: Supports GGUF format models from HuggingFace (Llama, Mistral, Qwen, Gemma, etc.)
- **Quantization**: Extensive quantization types (Q2_K through Q8_0, IQ variants) for memory efficiency

### Key Capabilities

| Capability | Description |
|------------|-------------|
| Text Generation | Autoregressive token generation with configurable sampling |
| Chat Completion | Multi-turn conversation with chat template support |
| Embeddings | Extract contextual embeddings for RAG, similarity, etc. |
| LoRA | Low-Rank Adapter support for fine-tuned model variants |
| Multimodal | Image + text processing (llava, gemma3, etc.) |

---

## Architecture

### Core Components

```
┌─────────────────────────────────────────────────────────────────┐
│                      llama.cpp Stack                            │
├─────────────────────────────────────────────────────────────────┤
│  ┌─────────────┐  ┌─────────────┐  ┌─────────────────────────┐ │
│  │ High-Level  │  │  Sampling   │  │    Chat Templates       │ │
│  │   API       │  │   Chains    │  │      (Jinja-like)       │ │
│  └─────────────┘  └─────────────┘  └─────────────────────────┘ │
├─────────────────────────────────────────────────────────────────┤
│  ┌─────────────┐  ┌─────────────┐  ┌─────────────────────────┐ │
│  │  llama_     │  │   Context   │  │      KV Cache           │ │
│  │  Model      │  │  Management │  │      Management         │ │
│  └─────────────┘  └─────────────┘  └─────────────────────────┘ │
├─────────────────────────────────────────────────────────────────┤
│  ┌─────────────┐  ┌─────────────┐  ┌─────────────────────────┐ │
│  │   ggml      │  │  Backend    │  │    Hardware-specific    │ │
│  │  Tensor     │  │  Scheduler  │  │    Kernels (CUDA, etc)  │ │
│  │  Graph      │  │             │  │                         │ │
│  └─────────────┘  └─────────────┘  └─────────────────────────┘ │
└─────────────────────────────────────────────────────────────────┘
```

### Memory Layout

```
Model File (GGUF) → Memory Mapped → VRAM/RAM Split → KV Cache → Logits Output
```

---

## GGUF Model Format

**GGUF (GPT-Generated Unified Format)** is llama.cpp's model format designed for:

- Fast loading via memory mapping
- Embedded metadata (tokenizer, chat templates, architecture)
- Quantized tensor storage

### File Structure

```
┌──────────────────┐
│ GGUF Header      │ - Magic number, version, tensor count
├──────────────────┤
│ Metadata KV      │ - Architecture, hyperparameters, tokenizer info
├──────────────────┤
│ Tensor Info      │ - Name, dimensions, type, offset for each tensor
├──────────────────┤
│ Tensor Data      │ - Actual tensor weights (possibly quantized)
└──────────────────┘
```

### Quantization Types

| Type | Size | Quality | Use Case |
|------|------|---------|----------|
| F16 | 2B/param | Lossless | High-quality inference |
| Q8_0 | ~1B/param | Near-lossless | Quality-focused |
| Q5_K_M | ~0.625B/param | Very Good | Balanced quality/size |
| Q4_K_M | ~0.5B/param | Good | Default recommendation |
| Q3_K_M | ~0.4B/param | Acceptable | Memory-constrained |
| Q2_K | ~0.3B/param | Degraded | Extreme constraints |

### Loading from HuggingFace

Models from HuggingFace typically follow this pattern:

```
https://huggingface.co/{org}/{repo}/resolve/main/{model_name}-{quant}.gguf
```

Example:
```
https://huggingface.co/Qwen/Qwen2-1.5B-Instruct-GGUF/resolve/main/qwen2-1_5b-instruct-q4_k_m.gguf
```

---

## Core API Components

### 1. Backend (`llama_backend`)

The backend must be initialized before any other operations:

```c
// C API
void llama_backend_init(void);
void llama_backend_free(void);
```

```rust
// Rust wrapper
use infrastructure_llama_cpp::llama_backend::LlamaBackend;

let backend = LlamaBackend::init()?;  // Can only be done once
// ... use backend ...
drop(backend);  // Frees backend resources
```

### 2. Model (`llama_model`)

Represents a loaded GGUF model file:

```c
struct llama_model_params {
    ggml_backend_dev_t * devices;
    int32_t n_gpu_layers;  // Layers to offload to GPU
    enum llama_split_mode split_mode;
    int32_t main_gpu;
    const float * tensor_split;
    llama_progress_callback progress_callback;
    bool vocab_only;
    bool use_mmap;
    bool use_mlock;
    // ... more options
};

struct llama_model * llama_model_load_from_file(
    const char * path_model,
    struct llama_model_params params
);
```

```rust
use infrastructure_llama_cpp::model::{LlamaModel, params::LlamaModelParams};

let params = LlamaModelParams::default()
    .with_n_gpu_layers(35);  // Offload 35 layers to GPU

let model = LlamaModel::load_from_file(&backend, "model.gguf", &params)?;
```

### 3. Context (`llama_context`)

The context holds:
- KV cache for autoregressive generation
- Current session state
- Logits from last decode

```c
struct llama_context_params {
    uint32_t n_ctx;              // Context window size
    uint32_t n_batch;            // Logical batch size
    uint32_t n_ubatch;           // Physical batch size
    uint32_t n_seq_max;          // Max sequences
    int32_t n_threads;           // CPU threads
    enum llama_rope_scaling_type rope_scaling_type;
    float rope_freq_base;
    float rope_freq_scale;
    enum ggml_type type_k;       // KV cache quantization
    enum ggml_type type_v;
    bool embeddings;             // Enable embeddings
    bool offload_kqv;            // Offload KV ops to GPU
    // ... more options
};
```

```rust
use infrastructure_llama_cpp::context::params::LlamaContextParams;

let ctx_params = LlamaContextParams::default()
    .with_n_ctx(4096)
    .with_n_batch(512)
    .with_embeddings(true);

let mut ctx = model.new_context(&backend, ctx_params)?;
```

### 4. Batch (`llama_batch`)

Batches are used to submit tokens for processing:

```c
struct llama_batch {
    int32_t n_tokens;
    llama_token * token;
    float * embd;
    llama_pos * pos;
    int32_t * n_seq_id;
    llama_seq_id ** seq_id;
    int8_t * logits;  // Which tokens output logits
};
```

```rust
use infrastructure_llama_cpp::llama_batch::LlamaBatch;

let mut batch = LlamaBatch::new(512, 1);  // capacity, sequences

// Add tokens
for (i, token) in tokens.iter().enumerate() {
    let is_last = i == tokens.len() - 1;
    batch.add(*token, i as i32, &[0], is_last)?;  // token, pos, seq_ids, logits
}

// Process batch
ctx.decode(&mut batch)?;
```

### 5. Sampler (`llama_sampler`)

Samplers select the next token from logits:

```rust
use infrastructure_llama_cpp::sampling::LlamaSampler;

// Simple greedy
let sampler = LlamaSampler::greedy();

// Temperature + Top-P + Greedy chain
let sampler = LlamaSampler::chain_simple([
    LlamaSampler::temp(0.7),
    LlamaSampler::top_p(0.9, 1),
    LlamaSampler::greedy(),
]);

// Sample token
let next_token = sampler.sample(&ctx, -1);  // -1 = last token
sampler.accept(next_token);  // Update sampler state
```

---

## Rust Bindings Architecture

The project has two layers:

### Layer 1: `infrastructure_llama_bindings` (Low-level)

Auto-generated bindings via `bindgen`:

```rust
// infrastructure/llama-bindings/src/lib.rs
include!(concat!(env!("OUT_DIR"), "/bindings.rs"));
```

Provides raw FFI bindings to all `llama_*` and `ggml_*` functions.

### Layer 2: `infrastructure_llama_cpp` (Safe Wrapper)

Idiomatic Rust API wrapping the bindings:

| Module | Purpose |
|--------|---------|
| `llama_backend` | Backend initialization |
| `model` | Model loading, tokenization, chat templates |
| `context` | Context creation, decode/encode, logits |
| `sampling` | Sampler chain construction |
| `llama_batch` | Batch construction |
| `token` | Token types and data |
| `mtmd` | Multimodal support |

---

## Model Loading

### Basic Loading

```rust
use infrastructure_llama_cpp::{
    llama_backend::LlamaBackend,
    model::{LlamaModel, params::LlamaModelParams},
};

fn load_model(backend: &LlamaBackend, path: &str) -> Result<LlamaModel> {
    let params = LlamaModelParams::default();
    LlamaModel::load_from_file(backend, path, &params)
}
```

### GPU Offloading

```rust
let params = LlamaModelParams::default()
    .with_n_gpu_layers(45)  // Offload 45 layers
    .with_main_gpu(0)       // Use GPU 0
    .with_split_mode(SplitMode::Layer);  // Split by layer
```

### Progress Callback

```rust
let params = LlamaModelParams::default()
    .with_progress_callback(|progress, _user_data| {
        println!("Loading: {:.1}%", progress * 100.0);
        true  // Continue loading
    });
```

### Model Inspection

```rust
// Get model metadata
println!("Vocab size: {}", model.n_vocab());
println!("Embedding size: {}", model.n_embd());
println!("Context length: {}", model.n_ctx_train());
println!("Layer count: {}", model.n_layer());
println!("Model size: {} GB", model.size() as f64 / 1e9);
println!("Parameters: {} B", model.n_params() as f64 / 1e9);

// Get metadata values
let architecture = model.meta_val_str("general.architecture")?;
let name = model.meta_val_str("general.name")?;
```

---

## Context Management

### Creating a Context

```rust
use infrastructure_llama_cpp::context::params::LlamaContextParams;

let ctx_params = LlamaContextParams::default()
    .with_n_ctx(8192)        // Context window
    .with_n_batch(1024)      // Batch size
    .with_n_ubatch(512)      // Physical batch
    .with_n_threads(8)       // CPU threads
    .with_embeddings(false); // Not needed for text gen

let mut ctx = model.new_context(&backend, ctx_params)?;
```

### Decode vs Encode

```rust
// For text generation (causal models)
ctx.decode(&mut batch)?;

// For embeddings/encoder models
ctx.encode(&mut batch)?;
```

### Logits Access

```rust
// Get logits for last decoded token
let logits = ctx.get_logits();  // &[f32] of size n_vocab

// Get logits for specific token index
let logits_ith = ctx.get_logits_ith(token_index);
```

### Performance Timing

```rust
ctx.reset_timings();
// ... run inference ...
let timings = ctx.timings();
println!("t/s: {:.2}", timings.t_per_second);
```

---

## Tokenization

### String to Tokens

```rust
use infrastructure_llama_cpp::model::AddBos;

let tokens = model.str_to_token("Hello, world!", AddBos::Always)?;
// Vec<LlamaToken>
```

### Tokens to String

```rust
use infrastructure_llama_cpp::model::Special;

let text = model.tokens_to_str(&tokens, Special::Tokenize)?;
```

### Special Tokens

```rust
let bos = model.token_bos();      // Beginning of sequence
let eos = model.token_eos();      // End of sequence
let nl = model.token_nl();        // Newline
let sep = model.token_sep();      // Separator

// Check if token is end-of-generation
if model.is_eog_token(token) {
    println!("Generation complete");
}
```

### Token Attributes

```rust
let attrs = model.token_attr(token);
if attrs.contains(LlamaTokenAttr::Control) {
    // This is a control token
}
```

---

## Inference

### Basic Text Generation Loop

```rust
use infrastructure_llama_cpp::{
    llama_batch::LlamaBatch,
    sampling::LlamaSampler,
    model::{AddBos, Special},
};

fn generate_text(
    model: &LlamaModel,
    ctx: &mut LlamaContext,
    prompt: &str,
    max_tokens: usize,
) -> Result<String> {
    // Tokenize prompt
    let tokens = model.str_to_token(prompt, AddBos::Always)?;

    // Create batch
    let mut batch = LlamaBatch::new(512, 1);

    // Process prompt
    for (i, token) in tokens.iter().enumerate() {
        let is_last = i == tokens.len() - 1;
        batch.add(*token, i as i32, &[0], is_last)?;
    }
    ctx.decode(&mut batch)?;

    // Generation loop
    let mut n_cur = batch.n_tokens();
    let mut sampler = LlamaSampler::chain_simple([
        LlamaSampler::temp(0.7),
        LlamaSampler::top_p(0.9, 1),
        LlamaSampler::greedy(),
    ]);
    let mut output = String::new();
    let mut decoder = encoding_rs::UTF_8.new_decoder();

    while n_cur < max_tokens {
        // Sample next token
        let token = sampler.sample(ctx, -1);
        sampler.accept(token);

        // Check for EOS
        if token == model.token_eos() {
            break;
        }

        // Convert token to text
        let bytes = model.token_to_bytes(token, Special::Tokenize)?;
        let mut piece = String::new();
        decoder.decode_to_string(&bytes, &mut piece, false)?;
        output.push_str(&piece);

        // Prepare next batch
        batch.clear();
        batch.add(token, n_cur as i32, &[0], true)?;
        ctx.decode(&mut batch)?;

        n_cur += 1;
    }

    Ok(output)
}
```

### Batched Prompting

```rust
// Multiple prompts in one batch
let prompts = vec!["Prompt 1", "Prompt 2", "Prompt 3"];
let mut batch = LlamaBatch::new(1024, prompts.len() as i32);

for (seq_id, prompt) in prompts.iter().enumerate() {
    let tokens = model.str_to_token(prompt, AddBos::Always)?;
    for (i, token) in tokens.iter().enumerate() {
        batch.add(*token, i as i32, &[seq_id as i32], i == tokens.len() - 1)?;
    }
}

ctx.decode(&mut batch)?;
```

---

## Sampling Strategies

### Available Samplers

| Sampler | Description |
|---------|-------------|
| `greedy()` | Select highest probability token |
| `dist(seed)` | Random sample by probability |
| `temp(t)` | Apply temperature scaling |
| `top_k(k)` | Limit to top k tokens |
| `top_p(p, min_keep)` | Nucleus sampling |
| `min_p(p, min_keep)` | Minimum probability threshold |
| `typical(p, min_keep)` | Locally typical sampling |
| `mirostat(seed, tau, eta)` | Mirostat 1.0 |
| `mirostat_v2(seed, tau, eta)` | Mirostat 2.0 |
| `xtc(p, t, min_keep, seed)` | XTC sampler |
| `penalties(last_n, repeat, freq, present)` | Repetition penalties |
| `grammar(model, grammar, root)` | Grammar-constrained |
| `dry(...)` | DRY repetition prevention |
| `logit_bias(n_vocab, biases)` | Apply logit biases |

### Sampler Chains

```rust
// Typical chat configuration
let sampler = LlamaSampler::chain_simple([
    LlamaSampler::temp(0.7),
    LlamaSampler::top_p(0.9, 1),
    LlamaSampler::top_k(40),
    LlamaSampler::penalties(64, 1.1, 0.1, 0.1),
    LlamaSampler::greedy(),
]);
```

### Grammar-Constrained Generation

```rust
// JSON grammar example
let grammar = r#"
root ::= object
object ::= "{" pair ("," pair)* "}"
pair ::= string ":" value
value ::= string | number | boolean | null
string ::= "\"" char* "\""
char ::= [^"\\] | "\\" ["\\/bfnrt]
number ::= "-"? [0-9]+ ("." [0-9]+)?
boolean ::= "true" | "false"
null ::= "null"
"#;

let sampler = LlamaSampler::grammar(&model, grammar, "root")?;
```

---

## KV Cache Management

### Cache Clearing

```rust
ctx.kv_cache_clear();
```

### Sequence Management

```rust
// Copy sequence
ctx.kv_cache_seq_cp(seq_id_src, seq_id_dst, start, end);

// Remove sequence
ctx.kv_cache_seq_rm(seq_id, start, end);

// Keep only specific sequence
ctx.kv_cache_seq_keep(seq_id);
```

### Cache Defragmentation

```rust
// Defragment if fragmentation > threshold
ctx.kv_cache_defrag();
```

---

## Chat Templates

### Get Template from Model

```rust
// Get default template
let template = model.chat_template(None)?;

// Get named template
let template = model.chat_template(Some("chatml"))?;
```

### Apply Template

```rust
use infrastructure_llama_cpp::model::LlamaChatMessage;

let messages = vec![
    LlamaChatMessage::new("system", "You are a helpful assistant")?,
    LlamaChatMessage::new("user", "Hello!")?,
];

let prompt = model.apply_chat_template(&template, &messages, true)?;
// Note: add_ass=true adds the opening assistant tag
```

### Built-in Templates

llama.cpp supports named templates:
- `chatml`
- `llama3`
- `mistral`
- `gemma`
- `qwen`
- And many more...

---

## LoRA Adapters

### Loading LoRA

```rust
let adapter = model.lora_adapter_init("lora.gguf")?;
```

### Applying LoRA

```rust
// Apply with scale
ctx.lora_adapter_set(&mut adapter, 1.0)?;

// Remove adapter
ctx.lora_adapter_remove(&mut adapter)?;
```

### Multiple Adapters

```rust
let adapter1 = model.lora_adapter_init("lora1.gguf")?;
let adapter2 = model.lora_adapter_init("lora2.gguf")?;

// Apply both with different scales
ctx.lora_adapter_set(&mut adapter1, 0.8)?;
ctx.lora_adapter_set(&mut adapter2, 0.2)?;
```

---

## Embeddings

### Enable Embeddings

```rust
let ctx_params = LlamaContextParams::default()
    .with_embeddings(true)
    .with_pooling_type(LLAMA_POOLING_TYPE_MEAN);

let ctx = model.new_context(&backend, ctx_params)?;
```

### Extract Embeddings

```rust
// Get embedding for sequence
let embedding = ctx.embeddings_seq_ith(0)?;

// Get embedding for specific token
let embedding = ctx.embeddings_ith(token_index)?;
```

### Reranking

```rust
// For reranking models
let ctx_params = LlamaContextParams::default()
    .with_pooling_type(LLAMA_POOLING_TYPE_RANK);
```

---

## Multimodal Support (mtmd)

The `mtmd` (multimodal) feature enables image+text processing.

### Enable Feature

```toml
# Cargo.toml
infrastructure_llama_cpp = { version = "...", features = ["mtmd"] }
```

### Load Multimodal Model

```rust
#[cfg(feature = "mtmd")]
{
    use infrastructure_llama_cpp::mtmd::{MtmhContext, MtmhInput};

    // Load context
    let mtmd_ctx = MtmhContext::new(&model)?;

    // Process image + text
    let input = MtmhInput::new()
        .with_text("Describe this image:")
        .with_image(image_bytes)?;

    let result = mtmd_ctx.process(&input)?;
}
```

---

## Hardware Acceleration

### Feature Flags

```toml
infrastructure_llama_cpp = {
    version = "...",
    features = ["cuda", "metal", "vulkan"]
}
```

### CUDA

```bash
# Build requirements
export CUDA_PATH=/usr/local/cuda
export CMAKE_PREFIX_PATH=$CUDA_PATH
```

```rust
let params = LlamaModelParams::default()
    .with_n_gpu_layers(999);  // Offload all layers
```

### Metal (Apple)

```rust
// Automatic on Apple Silicon
let params = LlamaModelParams::default()
    .with_n_gpu_layers(999);
```

### Vulkan

```bash
# Build requirements
export VULKAN_SDK=/path/to/vulkan
```

```rust
// Discover Vulkan devices
let devices = infrastructure_llama_cpp::list_llama_ggml_backend_devices();
for device in devices {
    println!("{}: {} ({})", device.index, device.name, device.backend);
}
```

---

## Integration with foundation_ai

### Current Structure

The `foundation_ai` crate has:

```
backends/foundation_ai/
├── src/
│   ├── backends/
│   │   ├── mod.rs
│   │   ├── llamacpp.rs     # LlamaBackends enum (CPU, GPU, Metal)
│   │   └── huggingface.rs  # HuggingFace model provider
│   ├── models/
│   │   ├── mod.rs
│   │   ├── model_descriptors.rs       # Model params/types
│   │   └── model_descriptors_defaults.rs
│   ├── types/
│   │   └── mod.rs          # Model, ModelBackend traits
│   └── errors/
│       └── mod.rs
```

### Type Mapping

| foundation_ai Type | llama.cpp Equivalent |
|--------------------|----------------------|
| `ModelParams` | `LlamaContextParams` |
| `ModelSpec` | `LlamaModel` + path |
| `ModelBackend` | `LlamaBackend` + `LlamaContext` |
| `Quantization` | GGUF quantization types |
| `ModelOutput::Text` | Decoded token string |

### Implementation Pattern

```rust
use infrastructure_llama_cpp::{
    llama_backend::LlamaBackend,
    model::{LlamaModel, params::LlamaModelParams},
    context::params::LlamaContextParams,
    llama_batch::LlamaBatch,
    sampling::LlamaSampler,
};

pub struct LlamaCppModel {
    backend: LlamaBackend,
    model: LlamaModel,
    ctx: LlamaContext<'static>,
}

impl Model for LlamaCppModel {
    fn text(&self, prompt: String, specs: Option<ModelParams>) -> GenerationResult<String> {
        // 1. Tokenize
        let tokens = self.model.str_to_token(&prompt, AddBos::Always)?;

        // 2. Build batch
        let mut batch = LlamaBatch::new(512, 1);
        // ... add tokens ...

        // 3. Decode
        self.ctx.decode(&mut batch)?;

        // 4. Generate
        let mut sampler = build_sampler(specs);
        // ... sampling loop ...

        Ok(output)
    }
}
```

---

## Use Case Patterns

### 1. Chat Completion Service

```rust
pub struct ChatModel {
    model: LlamaModel,
    ctx: LlamaContext<'static>,
    template: LlamaChatTemplate,
    default_sampler: LlamaSampler,
}

impl ChatModel {
    pub fn new(path: &str) -> Result<Self> {
        let backend = LlamaBackend::init()?;
        let model = LlamaModel::load_from_file(&backend, path, &params)?;
        let template = model.chat_template(None)?;
        let ctx = model.new_context(&backend, ctx_params)?;

        Ok(Self {
            model,
            ctx,
            template,
            default_sampler: LlamaSampler::chain_simple([
                LlamaSampler::temp(0.7),
                LlamaSampler::top_p(0.9),
                LlamaSampler::greedy(),
            ]),
        })
    }

    pub fn chat(&mut self, messages: &[Message]) -> Result<String> {
        // Convert messages to llama_chat_message format
        let llama_messages: Vec<_> = messages
            .iter()
            .map(|m| LlamaChatMessage::new(&m.role, &m.content))
            .collect::<Result<_>>()?;

        // Apply template
        let prompt = self.model.apply_chat_template(
            &self.template,
            &llama_messages,
            true
        )?;

        // Generate
        generate(&mut self.ctx, &prompt, &self.default_sampler)
    }
}
```

### 2. Embedding Service

```rust
pub struct EmbeddingModel {
    model: LlamaModel,
    ctx: LlamaContext<'static>,
}

impl EmbeddingModel {
    pub fn new(path: &str) -> Result<Self> {
        let backend = LlamaBackend::init()?;
        let model = LlamaModel::load_from_file(&backend, path, &params)?;

        let ctx_params = LlamaContextParams::default()
            .with_embeddings(true)
            .with_pooling_type(LLAMA_POOLING_TYPE_MEAN);

        let ctx = model.new_context(&backend, ctx_params)?;

        Ok(Self { model, ctx })
    }

    pub fn embed(&mut self, text: &str) -> Result<Vec<f32>> {
        let tokens = self.model.str_to_token(text, AddBos::Always)?;

        let mut batch = LlamaBatch::new(512, 1);
        for (i, token) in tokens.iter().enumerate() {
            batch.add(*token, i as i32, &[0], true)?;
        }

        self.ctx.encode(&mut batch)?;
        self.ctx.embeddings_seq_ith(0).map(|e| e.to_vec())
    }
}
```

### 3. Streaming Generator

```rust
pub struct StreamingGenerator {
    model: LlamaModel,
    ctx: LlamaContext<'static>,
}

impl StreamingGenerator {
    pub fn generate_stream(
        &mut self,
        prompt: &str,
        config: GenerationConfig,
    ) -> impl Iterator<Item = Result<String>> {
        // Setup
        let tokens = self.model.str_to_token(prompt, AddBos::Always).unwrap();
        let mut batch = LlamaBatch::new(512, 1);
        // ... setup batch ...

        self.ctx.decode(&mut batch).unwrap();

        let sampler = build_sampler(&config);
        let mut decoder = encoding_rs::UTF_8.new_decoder();

        // Streaming iterator
        GenerationStream::new(self, sampler, decoder)
    }
}
```

### 4. Code Completion

```rust
pub struct CodeModel {
    model: LlamaModel,
    ctx: LlamaContext<'static>,
    grammar: LlamaSampler,
}

impl CodeModel {
    pub fn new(path: &str) -> Result<Self> {
        let backend = LlamaBackend::init()?;
        let model = LlamaModel::load_from_file(&backend, path, &params)?;
        let ctx = model.new_context(&backend, ctx_params)?;

        // Grammar for valid code
        let grammar = LlamaSampler::grammar(
            &model,
            include_str!("grammars/code.ebnf"),
            "root"
        )?;

        Ok(Self { model, ctx, grammar })
    }

    pub fn complete(&mut self, prefix: &str) -> Result<String> {
        // Use grammar-constrained sampling
        let sampler = LlamaSampler::chain_simple([
            self.grammar.clone(),
            LlamaSampler::temp(0.2),  // Low temp for code
            LlamaSampler::greedy(),
        ]);

        generate(&mut self.ctx, prefix, &sampler)
    }
}
```

---

## Performance Optimization Tips

### 1. Context Size
- Use `n_ctx` matching your use case (smaller = less memory)
- For chat: 4096-8192 is typical
- For long documents: 16384-32768

### 2. Batch Size
- `n_batch` should be >= prompt length for single prompt
- For multiple prompts: sum of all prompt lengths

### 3. KV Cache Quantization
```rust
let params = LlamaContextParams::default()
    .with_type_k(GGML_TYPE_Q8_0)  // Quantize K cache
    .with_type_v(GGML_TYPE_Q8_0); // Quantize V cache
```

### 4. Thread Count
```rust
let params = LlamaContextParams::default()
    .with_n_threads(num_cpus::get())
    .with_n_threads_batch(num_cpus::get());
```

### 5. GPU Offloading
- Offload as many layers as VRAM allows
- Use `tensor_split` for multi-GPU setups

---

## Error Handling Patterns

```rust
use infrastructure_llama_cpp::{
    LlamaCppError,
    LlamaModelLoadError,
    DecodeError,
    TokenToStringError,
};

fn safe_generate(model: &LlamaModel, prompt: &str) -> Result<String, AppError> {
    match model.str_to_token(prompt, AddBos::Always) {
        Ok(tokens) => { /* process */ }
        Err(StringToTokenError::NulError(e)) => {
            Err(AppError::InvalidInput(e.to_string()))
        }
        Err(e) => Err(AppError::TokenizationFailed(e.to_string()))
    }
}
```

---

## Testing Patterns

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_model_loading() {
        let backend = LlamaBackend::init().unwrap();
        let model = LlamaModel::load_from_file(
            &backend,
            "test_models/tiny.gguf",
            &LlamaModelParams::default()
        );
        assert!(model.is_ok());
    }

    #[test]
    fn test_generation() {
        // Use a small test model
        let mut chat = ChatModel::new("test_models/tiny.gguf").unwrap();
        let response = chat.chat(&[Message::user("Hi")]).unwrap();
        assert!(!response.is_empty());
    }
}
```

---

## References

- [llama.cpp GitHub](https://github.com/ggerganov/llama.cpp)
- [llama.cpp Documentation](https://github.com/ggerganov/llama.cpp/tree/master/docs)
- [GGUF Format Specification](https://github.com/ggerganov/ggml/blob/master/docs/gguf.md)
- [llama-cpp-rs (utilityai)](https://github.com/utilityai/llama-cpp-rs)
- [HuggingFace GGUF Models](https://huggingface.co/models?library=gguf)
