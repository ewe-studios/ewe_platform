# 08 - Advanced Features

## LoRA Adapters

LoRA (Low-Rank Adaptation) allows fine-tuning model behavior without modifying the base weights. The adapter is a small supplementary file that modifies specific weight matrices.

### Loading and Applying

```rust
// Load adapter from the model
let adapter = model.lora_adapter_init("/path/to/adapter.gguf")?;

// Apply to a context with a scale factor
ctx.lora_adapter_set(&mut adapter, 1.0)?;
// scale=1.0 means full adapter strength
// scale=0.5 means half strength (interpolation with base)

// Generate with the adapter active...

// Remove the adapter
ctx.lora_adapter_remove(&mut adapter)?;
```

### Architecture

The Rust wrapper:

```rust
// From model.rs
pub struct LlamaLoraAdapter {
    pub lora_adapter: NonNull<infrastructure_llama_bindings::llama_adapter_lora>,
}

impl LlamaModel {
    pub fn lora_adapter_init(&self, path: impl AsRef<Path>)
        -> Result<LlamaLoraAdapter, LlamaLoraAdapterInitError>;
}

// From context.rs
impl LlamaContext<'_> {
    pub fn lora_adapter_set(
        &self, adapter: &mut LlamaLoraAdapter, scale: f32,
    ) -> Result<(), LlamaLoraAdapterSetError>;

    pub fn lora_adapter_remove(
        &self, adapter: &mut LlamaLoraAdapter,
    ) -> Result<(), LlamaLoraAdapterRemoveError>;
}
```

### LoRA with `foundation_ai`

The `ModelSpec` struct supports LoRA locations:

```rust
pub struct ModelSpec {
    pub name: String,
    pub id: ModelId,
    pub devices: Option<Vec<DeviceId>>,
    pub model_location: Option<PathBuf>,
    pub lora_location: Option<PathBuf>,  // <- LoRA adapter path
}
```

### Switching Between Adapters

```rust
// Apply adapter A
ctx.lora_adapter_set(&mut adapter_a, 1.0)?;
// Generate with adapter A...

// Switch to adapter B (remove A first)
ctx.lora_adapter_remove(&mut adapter_a)?;
ctx.lora_adapter_set(&mut adapter_b, 1.0)?;
// Generate with adapter B...
```

## Grammar-Constrained Generation

Grammar samplers force the model output to conform to a formal grammar (GBNF format), enabling structured output like JSON.

### Basic Grammar

```rust
let grammar_str = r#"
root   ::= object
value  ::= object | array | string | number | "true" | "false" | "null"

object ::= "{" ws (string ":" ws value ("," ws string ":" ws value)*)? "}"
array  ::= "[" ws (value ("," ws value)*)? "]"
string ::= "\"" ([^"\\] | "\\" .)* "\""
number ::= "-"? [0-9]+ ("." [0-9]+)?

ws     ::= [ \t\n]*
"#;

let sampler = LlamaSampler::chain_simple([
    LlamaSampler::grammar(&model, grammar_str, "root")?,
    LlamaSampler::temp(0.8),
    LlamaSampler::dist(1234),
]);
```

### Lazy Grammar

Lazy grammar only enforces the grammar after trigger words or tokens are encountered. This allows free-form text before the structured output:

```rust
let sampler = LlamaSampler::grammar_lazy(
    &model,
    grammar_str,
    "root",
    ["```json", "{"],      // trigger words
    &[],                    // trigger tokens
)?;
```

### Grammar Error Handling

```rust
pub enum GrammarError {
    RootNotFound,           // Grammar root not in grammar string
    TriggerWordNullBytes,   // Trigger word contains null bytes
    GrammarNullBytes,       // Grammar string contains null bytes
    NullGrammar,            // llama.cpp returned null
}
```

## Multi-Model Multi-Device (MTMD)

The `mtmd` module (feature-gated) provides multimodal support for vision-language models. It handles text, image, and audio inputs through a unified interface.

### Configuration

```rust
use infrastructure_llama_cpp::mtmd::{MtmdContextParams, MtmdInputChunkType};

pub struct MtmdContextParams {
    pub use_gpu: bool,
    pub print_timings: bool,
    pub n_threads: i32,
    pub media_marker: CString,  // e.g., "<image>" marker in text
}

pub enum MtmdInputChunkType {
    Text,
    Image,
    Audio,
}
```

### Feature Gate

MTMD is enabled via the `mtmd` feature flag:

```toml
# Cargo.toml
[features]
mtmd = ["infrastructure_llama_bindings/mtmd"]
```

When enabled, the build system compiles the multimodal tools from `llama.cpp/tools/mtmd/` and generates bindings for `mtmd_*` functions.

## RoPE Scaling for Extended Context

RoPE (Rotary Position Embedding) scaling allows extending a model's context length beyond its training length.

### Configuration

```rust
use infrastructure_llama_cpp::context::params::{LlamaContextParams, RopeScalingType};

let ctx_params = LlamaContextParams::default()
    .with_n_ctx(NonZeroU32::new(8192))          // Extended context
    .with_rope_scaling_type(RopeScalingType::Yarn)  // YaRN scaling
    .with_rope_freq_base(10000.0)               // Base frequency
    .with_rope_freq_scale(0.5);                 // Frequency scale factor
```

### Scaling Types

```rust
pub enum RopeScalingType {
    Unspecified = -1,  // Use model default
    None = 0,          // No scaling
    Linear = 1,        // Linear interpolation
    Yarn = 2,          // YaRN (Yet another RoPE extensioN)
}
```

### RoPE Types in Models

```rust
pub enum RopeType {
    Norm,    // Standard RoPE (LLaMA, Mistral)
    NeoX,    // GPT-NeoX style
    MRope,   // Multi-head RoPE
    Vision,  // Vision transformer RoPE
}

let rope_type = model.rope_type();
```

## Flash Attention

Flash attention reduces memory usage and improves speed for attention computation:

```rust
let ctx_params = LlamaContextParams::default()
    .with_flash_attention_policy(flash_attn_type);
```

Flash attention is configured at context creation time and affects all subsequent decode operations.

## KV Cache Quantization

Reduce memory usage by quantizing the KV cache:

```rust
use infrastructure_llama_cpp::context::params::KvCacheType;

let ctx_params = LlamaContextParams::default()
    .with_type_k(KvCacheType::Q8_0)   // 8-bit quantized keys
    .with_type_v(KvCacheType::Q8_0);  // 8-bit quantized values
```

Available KV cache types include all GGML types:
- `F32`, `F16` (default) -- full precision
- `Q8_0` -- good quality-memory tradeoff
- `Q4_0`, `Q4_1` -- aggressive compression
- `BF16` -- brain float 16-bit

### Memory Impact

KV cache memory = `n_ctx * n_layer * (n_embd_head * n_head_kv) * 2 * sizeof(type)`

For a 7B model (32 layers, 32 heads, 128 dim) with 4096 context:
- F16: 4096 * 32 * 4096 * 2 * 2 = 2 GB
- Q8_0: 4096 * 32 * 4096 * 2 * 1 = 1 GB
- Q4_0: 4096 * 32 * 4096 * 2 * 0.5 = 512 MB

## KV Cache Offloading

Control whether the KV cache stays on GPU:

```rust
let ctx_params = LlamaContextParams::default()
    .with_offload_kqv(true);  // Keep KV on GPU (default)

// Set to false to keep KV cache on CPU (saves GPU VRAM)
let ctx_params = LlamaContextParams::default()
    .with_offload_kqv(false);
```

## Sliding Window Attention

Some models (e.g., Mistral) use sliding window attention. Control the behavior:

```rust
let ctx_params = LlamaContextParams::default()
    .with_swa_full(true);  // Full sliding window (default)
```

## Embedding Models

Configure for embedding extraction:

```rust
use infrastructure_llama_cpp::context::params::{LlamaContextParams, LlamaPoolingType};

let ctx_params = LlamaContextParams::default()
    .with_embeddings(true)
    .with_pooling_type(LlamaPoolingType::Mean);

let mut ctx = model.new_context(&backend, ctx_params)?;

// After decode:
let embeddings = ctx.embeddings_seq_ith(seq_id)?;
// Returns &[f32] of length n_embd
```

### Pooling Types

```rust
pub enum LlamaPoolingType {
    Unspecified,  // Use model default
    None,         // Per-token embeddings (no pooling)
    Mean,         // Average of all token embeddings
    Cls,          // CLS token embedding
    Last,         // Last token embedding
    Rank,         // Reranking score (single float)
}
```

## Reranking Models

Cross-encoder reranking models produce a relevance score for query-document pairs:

```rust
let ctx_params = LlamaContextParams::default()
    .with_embeddings(true)
    .with_pooling_type(LlamaPoolingType::Rank);

// Format: query + separator + document
let pair = format!("{query}</s><s>{document}");
let tokens = model.str_to_token(&pair, AddBos::Always)?;

let mut batch = LlamaBatch::new(2048, 1);
batch.add_sequence(&tokens, 0, false)?;

ctx.clear_kv_cache();
ctx.decode(&mut batch)?;

let score = ctx.embeddings_seq_ith(0)?;
println!("Relevance score: {:.3}", score[0]);
```

## Buffer Type Overrides (MoE on CPU)

For Mixture-of-Experts models, you can keep expert layers on CPU while the rest stays on GPU:

```rust
use std::pin::pin;

let mut model_params = pin!(LlamaModelParams::default().with_n_gpu_layers(999));

// Move all MoE layers to CPU
model_params.as_mut().add_cpu_moe_override();

// Or specify a custom pattern
model_params.as_mut().add_cpu_buft_override(c"\\.ffn_(up|down|gate)_(ch|)exps");
```

This is useful for large MoE models where expert weights exceed GPU VRAM.

## Logit Bias

Modify token probabilities during sampling:

```rust
use infrastructure_llama_cpp::token::logit_bias::LlamaLogitBias;

let biases = vec![
    LlamaLogitBias::new(LlamaToken(1), 1.5),   // Boost token 1
    LlamaLogitBias::new(LlamaToken(2), -1.0),  // Suppress token 2
    LlamaLogitBias::new(LlamaToken(3), f32::NEG_INFINITY), // Ban token 3
];

let sampler = LlamaSampler::chain_simple([
    LlamaSampler::logit_bias(model.n_vocab(), &biases),
    LlamaSampler::temp(0.8),
    LlamaSampler::dist(1234),
]);
```

## DRY (Don't Repeat Yourself) Sampler

Penalizes repeated n-grams in the output:

```rust
let sampler = LlamaSampler::dry(
    &model,
    1.5,          // multiplier (penalty strength)
    1.75,         // base (exponential base)
    2,            // allowed_length (n-grams shorter than this are OK)
    64,           // penalty_last_n (look-back window)
    ["\n", ".", "!", "?"],  // sequence breakers (reset penalty)
);
```

## Dynamic Temperature

Temperature that adapts based on the entropy of the logit distribution:

```rust
let sampler = LlamaSampler::temp_ext(
    0.8,    // base temperature
    0.1,    // delta (temperature range)
    1.0,    // exponent
);
```

## Multiple Sequences

The `n_seq_max` parameter enables managing multiple sequences in a single context:

```rust
let ctx_params = LlamaContextParams::default()
    .with_n_seq_max(4);  // Up to 4 concurrent sequences

let mut ctx = model.new_context(&backend, ctx_params)?;

// Process different sequences
batch.add(token_a, pos_a, &[0], true)?;  // Sequence 0
batch.add(token_b, pos_b, &[1], true)?;  // Sequence 1
ctx.decode(&mut batch)?;
```

## Performance Monitoring

```rust
let timings = ctx.timings();
println!("{}", timings);
// Output:
// load time = 1234.56 ms
// prompt eval time = 567.89 ms / 512 tokens (1.11 ms per token, 901.23 tokens per second)
// eval time = 2345.67 ms / 128 runs (18.33 ms per token, 54.56 tokens per second)

// Reset timings for next measurement
ctx.reset_timings();
```

See [09-hardware-backends.md](./09-hardware-backends.md) for hardware-specific configuration and [10-usecases-and-patterns.md](./10-usecases-and-patterns.md) for practical recipes.
