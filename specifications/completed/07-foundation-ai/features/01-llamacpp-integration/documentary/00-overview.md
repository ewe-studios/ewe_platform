# 00 - Overview: llama.cpp and the Rust AI Platform Integration

## What is llama.cpp?

llama.cpp is a high-performance C/C++ library for running Large Language Model (LLM) inference on consumer hardware. Originally created by Georgi Gerganov, it provides:

- CPU and GPU inference for transformer-based models
- The GGML tensor computation library for optimized math operations
- The GGUF file format for storing quantized model weights
- Support for 30+ quantization formats reducing model sizes from gigabytes to megabytes
- Hardware acceleration via CUDA, Metal, Vulkan, and CPU SIMD instructions

## Core Components

### GGML - Tensor Computation Library

GGML is the underlying tensor library that powers all computation in llama.cpp. It provides:

- Custom tensor types optimized for quantized inference
- Computation graph construction and execution
- Memory management with memory-mapped file support
- Backend abstraction for CPU, CUDA, Metal, and Vulkan
- SIMD-optimized kernels (AVX2, AVX512, NEON)

See [01-ggml-and-gguf.md](./01-ggml-and-gguf.md) for a deep dive.

### GGUF - Model File Format

GGUF (GGML Unified Format) is the binary file format used to store model weights and metadata. Key properties:

- Self-describing with typed key-value metadata
- Supports all 30+ quantization formats
- Memory-mappable for fast loading
- Contains tokenizer data, chat templates, and architecture information

See [01-ggml-and-gguf.md](./01-ggml-and-gguf.md) for format details.

### Supported Model Architectures

llama.cpp supports a wide range of transformer architectures:

- **LLaMA family**: LLaMA, LLaMA 2, LLaMA 3, Code Llama
- **Mistral family**: Mistral, Mixtral (MoE)
- **GPT variants**: GPT-2, GPT-J, GPT-NeoX, StarCoder
- **Phi family**: Phi-2, Phi-3
- **Gemma**: Gemma, Gemma 2
- **Qwen**: Qwen, Qwen2
- **RWKV**: Recurrent models
- **Embedding models**: BERT, Nomic-Embed, BGE
- **Vision models**: LLaVA, multi-modal via mtmd
- **And many others**: Falcon, MPT, Bloom, etc.

## Three-Layer Integration Architecture

This platform integrates llama.cpp through a three-layer architecture that progressively abstracts the C API into idiomatic Rust:

```
+---------------------------------------------------------------+
|                    Layer 3: Domain Types                       |
|                 backends/foundation_ai/                        |
|                                                               |
|  ModelProvider, Model, ModelBackend traits                     |
|  ModelParams, ModelConfig, ModelSource, Quantization           |
|  ModelOutput (Text, ThinkingContent, Image, ToolCall)          |
|  KnownModelProviders (20+ providers)                          |
|  LlamaBackends enum (CPU, GPU, Metal)                         |
+---------------------------------------------------------------+
                              |
                    maps to / translates
                              |
+---------------------------------------------------------------+
|                Layer 2: Safe Rust Wrappers                     |
|                infrastructure/llama-cpp/                       |
|                                                               |
|  LlamaBackend     - Global initialization (singleton)         |
|  LlamaModel       - NonNull<llama_model>, RAII, Send+Sync     |
|  LlamaContext     - NonNull<llama_context>, lifetime-bound     |
|  LlamaBatch       - Input batch builder                       |
|  LlamaSampler     - Sampling chain (top-k, top-p, temp, etc.) |
|  LlamaToken       - repr(transparent) i32 wrapper             |
|  KV Cache ops     - seq_rm, seq_cp, seq_keep, seq_add         |
|  Session mgmt     - save/load session files                   |
|  MTMD             - Multi-model multi-device (feature-gated)  |
+---------------------------------------------------------------+
                              |
                      unsafe FFI calls
                              |
+---------------------------------------------------------------+
|               Layer 1: Raw FFI Bindings                       |
|             infrastructure/llama-bindings/                     |
|                                                               |
|  bindgen-generated from llama.h (81KB header)                 |
|  build.rs: CMake compilation of llama.cpp C++ source          |
|  Feature flags: cuda, metal, vulkan, openmp, mtmd             |
|  Platform support: Linux, macOS, Windows, Android             |
|  All llama_* and ggml_* C functions/types exposed             |
+---------------------------------------------------------------+
                              |
                    compiles / links
                              |
+---------------------------------------------------------------+
|                    llama.cpp C/C++ Source                      |
|                                                               |
|  llama.cpp/src/     - Core LLM inference                      |
|  llama.cpp/ggml/    - GGML tensor library                     |
|  llama.cpp/common/  - Shared utilities                        |
|  llama.cpp/tools/   - Multimodal, server tools                |
+---------------------------------------------------------------+
```

### Layer 1: Raw FFI Bindings (`infrastructure_llama_bindings`)

The bottom layer uses `bindgen` to automatically generate Rust FFI declarations from `llama.h`. The `build.rs` (1000+ lines) orchestrates:

1. **bindgen configuration**: Allowlists `llama_*` and `ggml_*` symbols, derives `PartialEq`, handles platform-specific clang args (MSVC include paths, Android NDK sysroot)
2. **CMake compilation**: Builds the entire llama.cpp C++ codebase with configurable backends
3. **Feature-gated backends**: `cuda`, `metal`, `vulkan`, `openmp`, `mtmd`
4. **Platform-specific linking**: Framework linking on macOS (Metal, Accelerate), stdc++ on Linux, MSVC runtime on Windows, Android NDK libraries
5. **CPU feature detection**: Maps Rust target features (avx2, avx512, neon) to GGML CMake flags

See [03-rust-ffi-bindings.md](./03-rust-ffi-bindings.md) for details.

### Layer 2: Safe Rust Wrappers (`infrastructure_llama_cpp`)

The middle layer wraps every unsafe C call in a safe Rust API with proper ownership semantics:

- **RAII**: `LlamaModel`, `LlamaContext`, `LlamaBatch`, `LlamaSampler` all implement `Drop` to free C resources
- **Lifetime tracking**: `LlamaContext<'a>` borrows `&'a LlamaModel`, preventing use-after-free
- **Thread safety**: `LlamaModel` is `Send + Sync`; `LlamaContextParams` is `Send + Sync`
- **Error types**: Comprehensive error enums (`LlamaCppError`, `DecodeError`, `EncodeError`, etc.) with `thiserror`
- **Builder pattern**: `LlamaModelParams::default().with_n_gpu_layers(1000).with_split_mode(LlamaSplitMode::Layer)`

See [04-rust-safe-wrappers.md](./04-rust-safe-wrappers.md) for the full API.

### Layer 3: Domain Types (`foundation_ai`)

The top layer defines the platform's AI model abstraction, independent of any specific backend:

- **Traits**: `ModelProvider`, `Model`, `ModelBackend` define the contract for model discovery, inference, and streaming
- **Type system**: `ModelParams`, `ModelConfig`, `Quantization`, `ModelSource`, `ModelOutput` provide a rich vocabulary for configuring and interacting with models
- **Provider registry**: `KnownModelProviders` enumerates 20+ cloud and local providers
- **Backend implementations**: `LlamaBackends` enum (CPU, GPU, Metal) implements `ModelBackend` using Layer 2

See [05-foundation-ai-types.md](./05-foundation-ai-types.md) for the type system.

## Data Flow: From Model File to Generated Text

```
GGUF File on disk
    |
    v
LlamaModel::load_from_file()     -- mmap or read into memory
    |
    v
LlamaModel                       -- query metadata, tokenizer, params
    |
    v
model.new_context(params)         -- allocate KV cache, set thread count
    |
    v
LlamaContext                      -- ready for inference
    |
    v
model.str_to_token(prompt)        -- tokenize input text
    |
    v
LlamaBatch::new() + batch.add()   -- prepare input batch
    |
    v
ctx.decode(&mut batch)            -- forward pass through transformer
    |
    v
ctx.get_logits() / candidates()   -- extract output logit distribution
    |
    v
LlamaSampler::sample()            -- apply sampling strategy
    |
    v
model.token_to_str(token)         -- detokenize to text
    |
    v
Output text (streamed token-by-token)
```

See [06-model-lifecycle.md](./06-model-lifecycle.md) and [07-inference-pipeline.md](./07-inference-pipeline.md) for detailed walkthroughs.

## Working Examples

The project includes working examples in `examples/llama-cpp/`:

| Example | Description | Key APIs Used |
|---------|-------------|---------------|
| `simple/` | Text generation with sampling | `LlamaModel`, `LlamaContext`, `LlamaBatch`, `LlamaSampler` |
| `embeddings/` | Embedding extraction | `LlamaContextParams::with_embeddings(true)`, `ctx.embeddings_seq_ith()` |
| `reranker/` | Document reranking | `LlamaPoolingType::Rank`, batch processing multiple sequences |
| `mtmd/` | Multimodal (vision) | `MtmdContext`, `MtmdInputChunkType::Image` |

## Document Index

| Document | Contents |
|----------|----------|
| [01 - GGML and GGUF](./01-ggml-and-gguf.md) | Tensor library, file format, quantization formats |
| [02 - llama.cpp Core API](./02-llama-cpp-core-api.md) | Complete C API reference |
| [03 - Rust FFI Bindings](./03-rust-ffi-bindings.md) | build.rs, bindgen, feature flags, platform support |
| [04 - Rust Safe Wrappers](./04-rust-safe-wrappers.md) | LlamaModel, LlamaContext, sampling, RAII patterns |
| [05 - Foundation AI Types](./05-foundation-ai-types.md) | Domain types, traits, type mappings |
| [06 - Model Lifecycle](./06-model-lifecycle.md) | End-to-end: download to inference to cleanup |
| [07 - Inference Pipeline](./07-inference-pipeline.md) | Tokenization through sampling, batching, KV cache |
| [08 - Advanced Features](./08-advanced-features.md) | LoRA, grammar, multi-modal, RoPE scaling |
| [09 - Hardware Backends](./09-hardware-backends.md) | CPU SIMD, CUDA, Metal, Vulkan, OpenMP |
| [10 - Use Cases and Patterns](./10-usecases-and-patterns.md) | Practical recipes with code |
| [11 - Integration Guide](./11-integration-guide.md) | Implementing ModelBackend for llama.cpp |
