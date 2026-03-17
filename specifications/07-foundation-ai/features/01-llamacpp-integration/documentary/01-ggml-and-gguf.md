# 01 - GGML Tensor Library and GGUF File Format

## GGML: The Tensor Computation Engine

GGML (Georgi Gerganov Machine Learning) is a C-based tensor computation library designed specifically for efficient LLM inference on consumer hardware. Unlike training-focused frameworks (PyTorch, TensorFlow), GGML is optimized purely for inference with minimal memory overhead.

### Core Design Principles

1. **No dynamic memory allocation during inference**: All memory is pre-allocated
2. **Quantization-native**: Tensor types are designed around quantized data from the start
3. **Computation graph model**: Operations build a graph first, then execute it
4. **Backend abstraction**: Same computation graph runs on CPU, CUDA, Metal, or Vulkan
5. **Memory-mapped model loading**: Models load nearly instantly via mmap

### Tensor Types (`ggml_type`)

GGML defines a comprehensive set of tensor data types. The Rust bindings expose these through `infrastructure_llama_bindings::ggml_type` constants, and the safe wrapper maps them via `KvCacheType` in `infrastructure/llama-cpp/src/context/params.rs`:

#### Full-Precision Types

| Type | Bits/Element | Description |
|------|-------------|-------------|
| `GGML_TYPE_F32` | 32 | IEEE 754 single precision |
| `GGML_TYPE_F64` | 64 | IEEE 754 double precision |
| `GGML_TYPE_F16` | 16 | IEEE 754 half precision |
| `GGML_TYPE_BF16` | 16 | Brain floating point (exponent range of f32) |

#### Integer Types

| Type | Bits/Element | Description |
|------|-------------|-------------|
| `GGML_TYPE_I8` | 8 | Signed 8-bit integer |
| `GGML_TYPE_I16` | 16 | Signed 16-bit integer |
| `GGML_TYPE_I32` | 32 | Signed 32-bit integer |
| `GGML_TYPE_I64` | 64 | Signed 64-bit integer |

#### Legacy Quantization Types

| Type | Bits/Weight | Block Size | Description |
|------|------------|------------|-------------|
| `GGML_TYPE_Q4_0` | 4.5 | 32 | 4-bit quantization, symmetric, single scale per block |
| `GGML_TYPE_Q4_1` | 5.0 | 32 | 4-bit quantization with min value offset |
| `GGML_TYPE_Q5_0` | 5.5 | 32 | 5-bit quantization, symmetric |
| `GGML_TYPE_Q5_1` | 6.0 | 32 | 5-bit quantization with min value offset |
| `GGML_TYPE_Q8_0` | 8.5 | 32 | 8-bit quantization, symmetric (used for intermediates) |
| `GGML_TYPE_Q8_1` | 9.0 | 32 | 8-bit quantization with min value offset |

#### K-Quant Types (Recommended)

K-quant types use mixed-precision quantization with importance-based bit allocation:

| Type | Bits/Weight | Description |
|------|------------|-------------|
| `GGML_TYPE_Q2_K` | ~2.6 | 2-bit quantization with 4-bit super-blocks |
| `GGML_TYPE_Q3_K` | ~3.4 | 3-bit quantization with 6-bit super-blocks |
| `GGML_TYPE_Q4_K` | ~4.5 | 4-bit quantization with 6-bit super-blocks |
| `GGML_TYPE_Q5_K` | ~5.5 | 5-bit quantization with 6-bit super-blocks |
| `GGML_TYPE_Q6_K` | ~6.6 | 6-bit quantization with 8-bit super-blocks |
| `GGML_TYPE_Q8_K` | ~8.5 | 8-bit quantization for intermediate accumulation |

#### IQ (Importance-Quantized) Types

Ultra-low bitrate quantization using information-theoretic optimization:

| Type | Bits/Weight | Description |
|------|------------|-------------|
| `GGML_TYPE_IQ1_S` | ~1.5 | 1-bit with super-blocks, extreme compression |
| `GGML_TYPE_IQ1_M` | ~1.75 | 1-bit mixed precision |
| `GGML_TYPE_IQ2_XXS` | ~2.1 | 2-bit extra-extra-small |
| `GGML_TYPE_IQ2_XS` | ~2.3 | 2-bit extra-small |
| `GGML_TYPE_IQ2_S` | ~2.5 | 2-bit small |
| `GGML_TYPE_IQ3_XXS` | ~3.1 | 3-bit extra-extra-small |
| `GGML_TYPE_IQ3_S` | ~3.4 | 3-bit small |
| `GGML_TYPE_IQ4_NL` | ~4.5 | 4-bit non-linear quantization |
| `GGML_TYPE_IQ4_XS` | ~4.3 | 4-bit extra-small |

#### Ternary and Specialized Types

| Type | Bits/Weight | Description |
|------|------------|-------------|
| `GGML_TYPE_TQ1_0` | ~1.7 | Ternary quantization (-1, 0, 1) |
| `GGML_TYPE_TQ2_0` | ~2.0 | Ternary quantization variant |
| `GGML_TYPE_MXFP4` | 4 | Microsoft MX floating point 4-bit |

### Rust Mapping: `KvCacheType`

The safe wrapper in `infrastructure/llama-cpp/src/context/params.rs` maps these to the `KvCacheType` enum, which is used for configuring the KV cache data types:

```rust
pub enum KvCacheType {
    Unknown(infrastructure_llama_bindings::ggml_type),
    F32, F16, Q4_0, Q4_1, Q5_0, Q5_1, Q8_0, Q8_1,
    Q2_K, Q3_K, Q4_K, Q5_K, Q6_K, Q8_K,
    IQ2_XXS, IQ2_XS, IQ3_XXS, IQ1_S, IQ4_NL, IQ3_S, IQ2_S, IQ4_XS,
    I8, I16, I32, I64, F64, IQ1_M, BF16, TQ1_0, TQ2_0, MXFP4,
}
```

The `Unknown` variant preserves forward compatibility -- when llama.cpp adds new types, they pass through FFI as raw values without breaking the Rust wrapper.

### Memory Management

GGML uses a region-based memory allocator:

1. **Context allocation**: A `ggml_context` is created with a fixed memory pool
2. **Tensor creation**: Tensors are allocated from the context's memory pool
3. **No individual deallocation**: When the context is freed, all its tensors are freed together
4. **Memory mapping**: For model weights, `mmap` maps the GGUF file directly into memory, avoiding copies

The Rust FFI layer supports mmap through `LlamaModelParams`:

```rust
let params = LlamaModelParams::default(); // use_mmap defaults to true
// Or explicitly:
// params.use_mmap() returns true by default
```

### Computation Graphs

GGML constructs computation graphs before execution:

1. **Graph construction**: Operations like `ggml_mul_mat`, `ggml_add`, `ggml_norm` create graph nodes
2. **Graph optimization**: The graph is analyzed for parallelism and memory reuse
3. **Graph execution**: A backend scheduler dispatches operations to CPU/GPU
4. **No autograd**: GGML graphs are inference-only (no backward pass)

### Backend System

GGML's backend system allows the same computation to run on different hardware. The Rust bindings expose backend device enumeration:

```rust
// From infrastructure/llama-cpp/src/lib.rs
pub fn list_llama_ggml_backend_devices() -> Vec<LlamaBackendDevice> { ... }

pub struct LlamaBackendDevice {
    pub index: usize,
    pub name: String,          // e.g., "Vulkan0"
    pub description: String,   // e.g., "NVIDIA GeForce RTX 3080"
    pub backend: String,       // e.g., "Vulkan", "CUDA", "CPU"
    pub memory_total: usize,   // bytes
    pub memory_free: usize,    // bytes
    pub device_type: LlamaBackendDeviceType,
}

pub enum LlamaBackendDeviceType {
    Cpu, Accelerator, Gpu, IntegratedGpu, Unknown,
}
```

## GGUF: The Model File Format

### File Structure

A GGUF file has the following binary layout:

```
+-------------------+
| Magic Number      |  4 bytes: "GGUF" (0x46475547)
+-------------------+
| Version           |  4 bytes: uint32 (currently 3)
+-------------------+
| Tensor Count      |  8 bytes: uint64
+-------------------+
| Metadata KV Count |  8 bytes: uint64
+-------------------+
| Metadata KV Pairs |  Variable length
|   - Key (string)  |
|   - Value type     |
|   - Value data     |
+-------------------+
| Tensor Info        |  For each tensor:
|   - Name (string)  |
|   - N dimensions   |
|   - Dimensions     |
|   - Type (ggml_type)|
|   - Offset         |
+-------------------+
| Alignment Padding  |
+-------------------+
| Tensor Data        |  Raw tensor weights (bulk of file)
+-------------------+
```

### Metadata Types

GGUF metadata values can be one of these types:

| Type ID | Type | Description |
|---------|------|-------------|
| 0 | UINT8 | Unsigned 8-bit integer |
| 1 | INT8 | Signed 8-bit integer |
| 2 | UINT16 | Unsigned 16-bit integer |
| 3 | INT16 | Signed 16-bit integer |
| 4 | UINT32 | Unsigned 32-bit integer |
| 5 | INT32 | Signed 32-bit integer |
| 6 | FLOAT32 | 32-bit float |
| 7 | BOOL | Boolean |
| 8 | STRING | Length-prefixed UTF-8 string |
| 9 | ARRAY | Typed array of values |
| 10 | UINT64 | Unsigned 64-bit integer |
| 11 | INT64 | Signed 64-bit integer |
| 12 | FLOAT64 | 64-bit float |

### Common Metadata Keys

GGUF files typically contain these metadata sections:

**General metadata:**
- `general.architecture` - Model architecture (e.g., "llama", "mistral", "gpt2")
- `general.name` - Human-readable model name
- `general.file_type` - Quantization type used
- `general.quantization_version` - Quantization format version

**Architecture-specific parameters:**
- `{arch}.context_length` - Maximum context length
- `{arch}.embedding_length` - Embedding dimension (n_embd)
- `{arch}.block_count` - Number of transformer layers
- `{arch}.attention.head_count` - Number of attention heads
- `{arch}.attention.head_count_kv` - Number of KV attention heads (GQA)
- `{arch}.rope.dimension_count` - RoPE embedding dimensions
- `{arch}.rope.freq_base` - RoPE frequency base

**Tokenizer data:**
- `tokenizer.ggml.model` - Tokenizer type ("llama", "gpt2", "bert")
- `tokenizer.ggml.tokens` - Token vocabulary (string array)
- `tokenizer.ggml.scores` - Token scores (float array)
- `tokenizer.ggml.token_type` - Token types (int array)
- `tokenizer.ggml.merges` - BPE merge rules (string array)
- `tokenizer.ggml.bos_token_id` - Beginning-of-sequence token ID
- `tokenizer.ggml.eos_token_id` - End-of-sequence token ID
- `tokenizer.chat_template` - Jinja2 chat template string

### Accessing Metadata from Rust

The `LlamaModel` wrapper provides methods to query GGUF metadata:

```rust
// From infrastructure/llama-cpp/src/model.rs

// Get metadata by key
let arch = model.meta_val_str("general.architecture")?;

// Iterate all metadata
let count = model.meta_count(); // total number of KV pairs
for i in 0..count {
    let key = model.meta_key_by_index(i)?;
    let value = model.meta_val_str_by_index(i)?;
}

// Structured queries
let n_embd = model.n_embd();              // embedding dimension
let n_layer = model.n_layer();            // number of layers
let n_head = model.n_head();              // attention heads
let n_head_kv = model.n_head_kv();        // KV attention heads
let n_ctx_train = model.n_ctx_train();    // training context length
let n_params = model.n_params();          // total parameters
let size = model.size();                  // total tensor size in bytes
let vocab_type = model.vocab_type();      // BPE or SPM
let rope_type = model.rope_type();        // Norm, NeoX, MRope, Vision
let is_recurrent = model.is_recurrent();  // RWKV, Mamba, etc.
```

### KV Override System

You can override GGUF metadata at load time without modifying the file. This is useful for experimenting with context lengths or other parameters:

```rust
use std::pin::pin;
use infrastructure_llama_cpp::model::params::LlamaModelParams;
use infrastructure_llama_cpp::model::params::kv_overrides::ParamOverrideValue;

let mut model_params = pin!(LlamaModelParams::default());

// Override the context length stored in the GGUF
let key = CString::new("llama.context_length").unwrap();
model_params.as_mut().append_kv_override(
    key.as_c_str(),
    ParamOverrideValue::Int(4096),
);
```

`ParamOverrideValue` supports three types:

```rust
pub enum ParamOverrideValue {
    Int(i64),
    Float(f64),
    Bool(bool),
}
```

## Quantization Formats in Detail

### How Quantization Works

Quantization reduces model size by representing weights with fewer bits. The process:

1. **Block-based**: Weights are divided into blocks (typically 32 or 256 elements)
2. **Scale factor**: Each block stores a scale factor in higher precision (f16 or f32)
3. **Quantized values**: Individual weights are stored as low-bit integers
4. **Dequantization**: At inference time: `weight = quantized_value * scale + offset`

### Quality vs. Size Tradeoffs

Rough guidelines for a 7B parameter model:

| Quantization | File Size | Quality Loss | Speed |
|-------------|-----------|-------------|-------|
| F16 | ~14 GB | Baseline | Slowest |
| Q8_0 | ~7.5 GB | Negligible | Fast |
| Q6_K | ~5.5 GB | Very small | Fast |
| Q5_K_M | ~4.8 GB | Small | Fast |
| Q4_K_M | ~4.1 GB | Moderate | Fastest |
| Q3_K_M | ~3.3 GB | Noticeable | Fast |
| Q2_K | ~2.7 GB | Significant | Fast |
| IQ2_XXS | ~2.1 GB | Large | Fast |

### Mapping to `foundation_ai::Quantization`

The `Quantization` enum in `backends/foundation_ai/src/types/mod.rs` maps to GGUF quantization types:

```rust
pub enum Quantization {
    None,           // No quantization (F32)
    Default,        // Use model's default
    F16,            // GGML_TYPE_F16
    Q2K,            // GGML_TYPE_Q2_K
    Q2_KS,          // Q2_K small variant
    Q2_KM,          // Q2_K medium variant
    Q2_KL,          // Q2_K large variant
    Q3_KS,          // Q3_K small
    Q3_KM,          // Q3_K medium
    Q4_0,           // GGML_TYPE_Q4_0
    Q4_1,           // GGML_TYPE_Q4_1
    IQ_4Nl,         // GGML_TYPE_IQ4_NL
    IQ_4Xs,         // GGML_TYPE_IQ4_XS
    Q4_KM,          // Q4_K medium
    Q4_KS,          // Q4_K small
    Q5_KS,          // Q5_K small
    Q5_KM,          // Q5_K medium
    Q5_KL,          // Q5_K large
    Q6_K,           // GGML_TYPE_Q6_K
    Q6_KM,          // Q6_K medium
    Q6_KS,          // Q6_K small
    Q6_KL,          // Q6_K large
    Q8_0,           // GGML_TYPE_Q8_0
    Q8_1,           // GGML_TYPE_Q8_1
    Ud_IQ_1M,       // Ultra-dense IQ 1-bit mixed
    UD_IQ_1S,       // Ultra-dense IQ 1-bit small
    UD_IQ_2M,       // Ultra-dense IQ 2-bit mixed
    UD_IQ_2Xxs,     // Ultra-dense IQ 2-bit extra-extra-small
    UD_IQ_3Xxs,     // Ultra-dense IQ 3-bit extra-extra-small
    UD_Q_2KXl,      // Ultra-dense Q2_K extra-large
    UD_Q_3KXl,      // Ultra-dense Q3_K extra-large
    UD_Q_4KXl,      // Ultra-dense Q4_K extra-large
    UD_Q_5KXl,      // Ultra-dense Q5_K extra-large
    UD_Q_6KXl,      // Ultra-dense Q6_K extra-large
    UD_Q_8KXl,      // Ultra-dense Q8_K extra-large
    Custom(String),  // Custom or unknown quantization
}
```

The `S/M/L/XL` suffixes in the `foundation_ai` `Quantization` enum refer to variants within a quantization family that trade off between model size and quality. These correspond to different configurations of which layers get quantized to what precision -- "Small" uses more aggressive quantization, "Large" preserves more precision in important layers.

## How to Load and Interpret GGUF Files

### Loading Pipeline

```rust
use infrastructure_llama_cpp::llama_backend::LlamaBackend;
use infrastructure_llama_cpp::model::LlamaModel;
use infrastructure_llama_cpp::model::params::LlamaModelParams;

// 1. Initialize backend
let backend = LlamaBackend::init()?;

// 2. Configure model parameters
let model_params = LlamaModelParams::default()
    .with_n_gpu_layers(999)    // offload all layers to GPU
    .with_use_mlock(false);    // don't lock in RAM

// 3. Load model (reads GGUF header, mmaps tensor data)
let model = LlamaModel::load_from_file(&backend, "model.gguf", &model_params)?;

// 4. Inspect model metadata
println!("Parameters: {}", model.n_params());
println!("Embedding dim: {}", model.n_embd());
println!("Layers: {}", model.n_layer());
println!("Vocab size: {}", model.n_vocab());
println!("Training context: {}", model.n_ctx_train());
println!("Model size: {} bytes", model.size());
```

### Inspecting Metadata Programmatically

```rust
// Enumerate all GGUF metadata
let count = model.meta_count();
for i in 0..count {
    match (model.meta_key_by_index(i), model.meta_val_str_by_index(i)) {
        (Ok(key), Ok(val)) => println!("{}: {}", key, val),
        _ => continue,
    }
}
```

See [02-llama-cpp-core-api.md](./02-llama-cpp-core-api.md) for the complete C API and [04-rust-safe-wrappers.md](./04-rust-safe-wrappers.md) for the full Rust API.
