# 02 - llama.cpp Core C API Reference

This document covers the complete C API surface of llama.cpp as exposed through the FFI bindings in `infrastructure/llama-bindings/`. All functions listed here are available via `infrastructure_llama_bindings::*` in Rust code.

## Initialization and Backend

### `llama_backend_init()`

```c
void llama_backend_init(void);
```

Initializes the llama.cpp backend. Must be called before any other llama function. This sets up global state, including the thread pool and backend registries. The Rust wrapper enforces single initialization via `LlamaBackend::init()`.

### `llama_backend_free()`

```c
void llama_backend_free(void);
```

Frees the llama.cpp backend. Called automatically by `LlamaBackend::drop()`.

### `llama_numa_init(strategy)`

```c
void llama_numa_init(enum ggml_numa_strategy strategy);
```

Initializes NUMA (Non-Uniform Memory Access) support. Strategies:
- `GGML_NUMA_STRATEGY_DISABLED` - No NUMA awareness
- `GGML_NUMA_STRATEGY_DISTRIBUTE` - Distribute across NUMA nodes
- `GGML_NUMA_STRATEGY_ISOLATE` - Isolate to local NUMA node
- `GGML_NUMA_STRATEGY_NUMACTL` - Use numactl configuration
- `GGML_NUMA_STRATEGY_MIRROR` - Mirror across NUMA nodes

### Capability Queries

```c
bool llama_supports_mmap(void);       // Can memory-map model files?
bool llama_supports_mlock(void);      // Can lock model memory?
bool llama_supports_gpu_offload(void); // GPU backend available?
size_t llama_max_devices(void);       // Maximum number of devices
int64_t llama_time_us(void);          // Current time in microseconds
```

### Backend Device Enumeration

```c
size_t ggml_backend_dev_count(void);
ggml_backend_dev_t ggml_backend_dev_get(size_t index);
void ggml_backend_dev_get_props(ggml_backend_dev_t dev, struct ggml_backend_dev_props * props);
const char * ggml_backend_reg_name(ggml_backend_reg_t reg);
ggml_backend_reg_t ggml_backend_dev_backend_reg(ggml_backend_dev_t dev);
```

The device properties struct contains:
```c
struct ggml_backend_dev_props {
    const char * name;         // "CPU", "Vulkan0", "CUDA0", etc.
    const char * description;  // "NVIDIA GeForce RTX 3080"
    size_t memory_total;       // Total device memory in bytes
    size_t memory_free;        // Available device memory in bytes
    enum ggml_backend_dev_type type_; // CPU, GPU, ACCEL, IGPU
};
```

## Model Loading

### `llama_model_default_params()`

```c
struct llama_model_params llama_model_default_params(void);
```

Returns default model parameters:
- `n_gpu_layers = 999` (offload all layers)
- `split_mode = LLAMA_SPLIT_MODE_LAYER`
- `main_gpu = 0`
- `use_mmap = true`
- `use_mlock = false`
- `vocab_only = false`

### `llama_model_params` Structure

```c
struct llama_model_params {
    int32_t n_gpu_layers;    // Number of layers to offload to GPU (-1 = all)
    enum llama_split_mode split_mode; // How to split across GPUs
    int32_t main_gpu;        // Main GPU for scratch/small tensors
    const float * tensor_split; // Proportion of model per GPU
    ggml_backend_sched_eval_callback progress_callback;
    void * progress_callback_user_data;
    const struct llama_model_kv_override * kv_overrides; // GGUF KV overrides
    const struct llama_model_tensor_buft_override * tensor_buft_overrides;
    const ggml_backend_dev_t * devices;
    bool vocab_only;         // Only load vocabulary (no weights)
    bool use_mmap;           // Use memory mapping
    bool use_mlock;          // Lock model memory (prevent swapping)
};
```

### `llama_load_model_from_file(path, params)`

```c
struct llama_model * llama_load_model_from_file(
    const char * path_model,
    struct llama_model_params params
);
```

Loads a GGUF model file. Returns `NULL` on failure. The Rust wrapper maps this to:

```rust
LlamaModel::load_from_file(&backend, path, &params) -> Result<LlamaModel, LlamaModelLoadError>
```

### `llama_free_model(model)`

```c
void llama_free_model(struct llama_model * model);
```

Frees the model. Called automatically by `LlamaModel::drop()`.

### Split Modes

```c
enum llama_split_mode {
    LLAMA_SPLIT_MODE_NONE  = 0, // Single GPU
    LLAMA_SPLIT_MODE_LAYER = 1, // Split layers across GPUs (default)
    LLAMA_SPLIT_MODE_ROW   = 2, // Split rows with tensor parallelism
};
```

## Model Queries

### Architecture and Size

```c
int32_t  llama_n_embd(const struct llama_model * model);        // Embedding dimension
int32_t  llama_model_n_layer(const struct llama_model * model);  // Number of layers
int32_t  llama_model_n_head(const struct llama_model * model);   // Attention heads
int32_t  llama_model_n_head_kv(const struct llama_model * model);// KV attention heads
int32_t  llama_n_ctx_train(const struct llama_model * model);    // Training context length
uint64_t llama_model_size(const struct llama_model * model);     // Total tensor bytes
uint64_t llama_model_n_params(const struct llama_model * model); // Parameter count
```

### Architecture Detection

```c
bool llama_model_has_encoder(const struct llama_model * model);
bool llama_model_has_decoder(const struct llama_model * model);
bool llama_model_is_recurrent(const struct llama_model * model); // RWKV, Mamba
int32_t llama_model_rope_type(const struct llama_model * model); // RoPE variant
```

RoPE types:
```c
enum llama_rope_type {
    LLAMA_ROPE_TYPE_NONE   = -1,
    LLAMA_ROPE_TYPE_NORM   =  0, // Standard RoPE
    LLAMA_ROPE_TYPE_NEOX   =  1, // GPT-NeoX style
    LLAMA_ROPE_TYPE_MROPE  =  2, // Multi-head RoPE
    LLAMA_ROPE_TYPE_VISION =  4, // Vision transformer RoPE
};
```

### Metadata Access

```c
int32_t llama_model_meta_count(const struct llama_model * model);

int32_t llama_model_meta_key_by_index(
    const struct llama_model * model,
    int32_t i,
    char * buf, size_t buf_size
);

int32_t llama_model_meta_val_str(
    const struct llama_model * model,
    const char * key,
    char * buf, size_t buf_size
);

int32_t llama_model_meta_val_str_by_index(
    const struct llama_model * model,
    int32_t i,
    char * buf, size_t buf_size
);
```

All metadata functions return the number of bytes written, or a negative value on error. If the buffer is too small, they return the required size (negative).

### Chat Templates

```c
const char * llama_model_chat_template(
    const struct llama_model * model,
    const char * name  // NULL for default template
);

int32_t llama_chat_apply_template(
    const char * tmpl,
    const struct llama_chat_message * chat,
    size_t n_msg,
    bool add_ass,      // Add assistant opening tag
    char * buf,
    int32_t length
);
```

The `llama_chat_message` structure:
```c
struct llama_chat_message {
    const char * role;    // "system", "user", "assistant"
    const char * content; // Message text
};
```

## Context Creation and Management

### `llama_context_default_params()`

```c
struct llama_context_params llama_context_default_params(void);
```

Returns default context parameters:
- `n_ctx = 512`
- `n_batch = 2048`
- `n_ubatch = 512`
- `n_seq_max = 1`
- `n_threads = 4`
- `n_threads_batch = 4`
- `rope_scaling_type = LLAMA_ROPE_SCALING_UNSPECIFIED`
- `embeddings = false`
- `offload_kqv = true`
- `swa_full = true`

### `llama_context_params` Structure

```c
struct llama_context_params {
    uint32_t n_ctx;          // Context size (0 = model default)
    uint32_t n_batch;        // Max tokens per logical batch
    uint32_t n_ubatch;       // Max tokens per physical batch
    uint32_t n_seq_max;      // Max sequences for recurrent models
    int32_t  n_threads;      // Threads for generation
    int32_t  n_threads_batch;// Threads for prompt processing

    enum llama_rope_scaling_type rope_scaling_type;
    float rope_freq_base;    // RoPE base frequency
    float rope_freq_scale;   // RoPE frequency scale

    enum ggml_type type_k;   // KV cache key type (default: F16)
    enum ggml_type type_v;   // KV cache value type (default: F16)

    enum llama_flash_attn_type flash_attn_type;
    int32_t pooling_type;    // Pooling type for embeddings

    bool embeddings;         // Enable embedding extraction
    bool offload_kqv;        // Offload KV cache to GPU
    bool swa_full;           // Full sliding window attention

    ggml_backend_sched_eval_callback cb_eval;
    void * cb_eval_user_data;
};
```

### `llama_new_context_with_model(model, params)`

```c
struct llama_context * llama_new_context_with_model(
    struct llama_model * model,
    struct llama_context_params params
);
```

Creates an inference context from a model. Returns `NULL` on failure. The context allocates the KV cache and is ready for inference.

### `llama_free(ctx)`

```c
void llama_free(struct llama_context * ctx);
```

### Context Queries

```c
uint32_t llama_n_ctx(const struct llama_context * ctx);    // Context size
uint32_t llama_n_batch(const struct llama_context * ctx);  // Logical batch size
uint32_t llama_n_ubatch(const struct llama_context * ctx); // Physical batch size
```

## Tokenization

### Vocabulary

```c
const struct llama_vocab * llama_model_get_vocab(const struct llama_model * model);
int32_t llama_n_vocab(const struct llama_vocab * vocab);
enum llama_vocab_type llama_vocab_type(const struct llama_vocab * vocab);
```

Vocabulary types:
```c
enum llama_vocab_type {
    LLAMA_VOCAB_TYPE_SPM = 1, // SentencePiece
    LLAMA_VOCAB_TYPE_BPE = 2, // Byte Pair Encoding
};
```

### Token Operations

```c
// String to tokens
int32_t llama_tokenize(
    const struct llama_vocab * vocab,
    const char * text,
    int32_t text_len,
    llama_token * tokens,
    int32_t n_tokens_max,
    bool add_special,  // Add BOS/EOS
    bool parse_special // Parse special tokens in text
);

// Token to string
int32_t llama_token_to_piece(
    const struct llama_vocab * vocab,
    llama_token token,
    char * buf,
    int32_t length,
    int32_t lstrip,
    bool special
);
```

`llama_tokenize` returns the number of tokens written, or a negative value equal to the number of tokens needed if the buffer is too small.

### Special Tokens

```c
llama_token llama_token_bos(const struct llama_vocab * vocab); // Beginning of sequence
llama_token llama_token_eos(const struct llama_vocab * vocab); // End of sequence
llama_token llama_token_nl(const struct llama_vocab * vocab);  // Newline
llama_token llama_token_sep(const struct llama_vocab * vocab); // Separator
bool llama_token_is_eog(const struct llama_vocab * vocab, llama_token token); // End of generation
```

### Token Attributes

```c
enum llama_token_attr llama_token_get_attr(
    const struct llama_vocab * vocab,
    llama_token token
);
```

Attributes (flags):
```c
enum llama_token_attr {
    LLAMA_TOKEN_ATTR_UNKNOWN      = 0,
    LLAMA_TOKEN_ATTR_UNUSED       = 1,
    LLAMA_TOKEN_ATTR_NORMAL       = 2,
    LLAMA_TOKEN_ATTR_CONTROL      = 4,
    LLAMA_TOKEN_ATTR_USER_DEFINED = 8,
    LLAMA_TOKEN_ATTR_BYTE         = 16,
    LLAMA_TOKEN_ATTR_NORMALIZED   = 32,
    LLAMA_TOKEN_ATTR_LSTRIP       = 64,
    LLAMA_TOKEN_ATTR_RSTRIP       = 128,
    LLAMA_TOKEN_ATTR_SINGLE_WORD  = 256,
};
```

## Batch Processing

### `llama_batch` Structure

```c
struct llama_batch {
    int32_t n_tokens;      // Current number of tokens
    llama_token * token;   // Token IDs [n_tokens]
    float * embd;          // Embeddings (alternative to tokens) [n_tokens * embd]
    llama_pos * pos;       // Token positions [n_tokens]
    int32_t * n_seq_id;    // Number of sequence IDs per token [n_tokens]
    llama_seq_id ** seq_id;// Sequence IDs [n_tokens][n_seq_id]
    int8_t * logits;       // Whether to compute logits [n_tokens]
};
```

### Batch Functions

```c
// Allocate a batch
struct llama_batch llama_batch_init(
    int32_t n_tokens_alloc,
    int32_t embd,       // 0 for token-based, >0 for embedding-based
    int32_t n_seq_max   // Max sequences per token
);

// Get a batch for a single sequence (no allocation)
struct llama_batch llama_batch_get_one(
    llama_token * tokens,
    int32_t n_tokens
);

// Free batch memory
void llama_batch_free(struct llama_batch batch);
```

## Encode / Decode (Inference)

### `llama_decode(ctx, batch)`

```c
int32_t llama_decode(
    struct llama_context * ctx,
    struct llama_batch batch
);
```

Runs the forward pass on the batch tokens. This is the core inference function for decoder-only models (most LLMs). Returns:
- `0` - Success
- `1` - No KV cache slot available (context full)
- `-1` - No tokens in batch
- Other negative values - Unknown errors

### `llama_encode(ctx, batch)`

```c
int32_t llama_encode(
    struct llama_context * ctx,
    struct llama_batch batch
);
```

Encoder forward pass (for encoder-decoder models like T5). Same return codes as `llama_decode`.

### Logit Access

```c
// Logits for the last token with logits enabled
float * llama_get_logits(struct llama_context * ctx);

// Logits for token at position i
float * llama_get_logits_ith(struct llama_context * ctx, int32_t i);
```

Both return a pointer to an array of `n_vocab` floats.

### Embedding Access

```c
// Embeddings for the i-th token
float * llama_get_embeddings_ith(struct llama_context * ctx, int32_t i);

// Embeddings for the i-th sequence (pooled)
float * llama_get_embeddings_seq(struct llama_context * ctx, llama_seq_id seq_id);
```

Returns a pointer to `n_embd` floats. Requires `embeddings = true` in context params.

### Pooling Types

```c
enum llama_pooling_type {
    LLAMA_POOLING_TYPE_UNSPECIFIED = -1,
    LLAMA_POOLING_TYPE_NONE = 0, // No pooling, per-token embeddings
    LLAMA_POOLING_TYPE_MEAN = 1, // Mean of all token embeddings
    LLAMA_POOLING_TYPE_CLS  = 2, // CLS token embedding
    LLAMA_POOLING_TYPE_LAST = 3, // Last token embedding
    LLAMA_POOLING_TYPE_RANK = 4, // Rank/reranking score
};
```

## Sampling

### Sampler Chain

```c
struct llama_sampler * llama_sampler_chain_init(
    struct llama_sampler_chain_params params
);

void llama_sampler_chain_add(
    struct llama_sampler * chain,
    struct llama_sampler * smpl
);

// Sample a token
llama_token llama_sampler_sample(
    struct llama_sampler * smpl,
    struct llama_context * ctx,
    int32_t idx  // Index of token whose logits to sample from
);

// Accept a token (update internal state)
void llama_sampler_accept(struct llama_sampler * smpl, llama_token token);

// Reset sampler state
void llama_sampler_reset(struct llama_sampler * smpl);

// Get random seed
uint32_t llama_sampler_get_seed(const struct llama_sampler * smpl);

// Free sampler
void llama_sampler_free(struct llama_sampler * smpl);
```

### Sampler Initialization Functions

```c
// Selection samplers (must be last in chain)
struct llama_sampler * llama_sampler_init_greedy(void);
struct llama_sampler * llama_sampler_init_dist(uint32_t seed);

// Temperature
struct llama_sampler * llama_sampler_init_temp(float t);
struct llama_sampler * llama_sampler_init_temp_ext(float t, float delta, float exponent);

// Top-K
struct llama_sampler * llama_sampler_init_top_k(int32_t k);

// Top-P (Nucleus)
struct llama_sampler * llama_sampler_init_top_p(float p, size_t min_keep);

// Min-P
struct llama_sampler * llama_sampler_init_min_p(float p, size_t min_keep);

// Top-n-sigma
struct llama_sampler * llama_sampler_init_top_n_sigma(float n);

// Typical sampling
struct llama_sampler * llama_sampler_init_typical(float p, size_t min_keep);

// XTC (experimental)
struct llama_sampler * llama_sampler_init_xtc(float p, float t, size_t min_keep, uint32_t seed);

// Mirostat
struct llama_sampler * llama_sampler_init_mirostat(
    int32_t n_vocab, uint32_t seed, float tau, float eta, int32_t m
);
struct llama_sampler * llama_sampler_init_mirostat_v2(uint32_t seed, float tau, float eta);

// Repetition penalty
struct llama_sampler * llama_sampler_init_penalties(
    int32_t penalty_last_n,
    float penalty_repeat,
    float penalty_freq,
    float penalty_present
);

// DRY (Don't Repeat Yourself)
struct llama_sampler * llama_sampler_init_dry(
    const struct llama_vocab * vocab,
    int32_t n_ctx_train,
    float dry_multiplier,
    float dry_base,
    int32_t dry_allowed_length,
    int32_t dry_penalty_last_n,
    const char ** seq_breakers,
    size_t num_breakers
);

// Grammar
struct llama_sampler * llama_sampler_init_grammar(
    const struct llama_vocab * vocab,
    const char * grammar_str,
    const char * grammar_root
);

struct llama_sampler * llama_sampler_init_grammar_lazy(
    const struct llama_vocab * vocab,
    const char * grammar_str,
    const char * grammar_root,
    const char ** trigger_words,
    size_t num_trigger_words,
    const llama_token * trigger_tokens,
    size_t num_trigger_tokens
);

// Logit bias
struct llama_sampler * llama_sampler_init_logit_bias(
    int32_t n_vocab,
    int32_t n_logit_bias,
    const struct llama_logit_bias * logit_bias
);
```

## KV Cache Management

The KV cache stores the key-value attention state. These functions manipulate it for efficient multi-turn conversation and sequence management.

```c
// Get the memory (KV cache) object
llama_memory_t llama_get_memory(const struct llama_context * ctx);

// Remove tokens from a sequence
bool llama_memory_seq_rm(
    llama_memory_t mem,
    llama_seq_id seq_id,  // -1 = all sequences
    llama_pos p0,         // Start position (-1 = beginning)
    llama_pos p1          // End position (-1 = end)
);

// Copy a sequence
void llama_memory_seq_cp(
    llama_memory_t mem,
    llama_seq_id seq_id_src,
    llama_seq_id seq_id_dst,
    llama_pos p0,
    llama_pos p1
);

// Keep only one sequence, remove all others
void llama_memory_seq_keep(llama_memory_t mem, llama_seq_id seq_id);

// Add a delta to token positions
void llama_memory_seq_add(
    llama_memory_t mem,
    llama_seq_id seq_id,
    llama_pos p0,
    llama_pos p1,
    llama_pos delta
);

// Divide positions by a factor
void llama_memory_seq_div(
    llama_memory_t mem,
    llama_seq_id seq_id,
    llama_pos p0,
    llama_pos p1,
    int d
);

// Get the max position in a sequence
llama_pos llama_memory_seq_pos_max(llama_memory_t mem, llama_seq_id seq_id);

// Clear entire cache
void llama_memory_clear(llama_memory_t mem, bool data);
```

## LoRA Adapters

```c
// Initialize a LoRA adapter
struct llama_adapter_lora * llama_adapter_lora_init(
    struct llama_model * model,
    const char * path_lora
);

// Set active LoRA adapters
int32_t llama_set_adapters_lora(
    struct llama_context * ctx,
    struct llama_adapter_lora ** adapters,
    size_t n_adapters,
    float * scales
);

// Free a LoRA adapter
void llama_adapter_lora_free(struct llama_adapter_lora * adapter);
```

## Session Management

```c
// Save session to file
bool llama_save_session_file(
    struct llama_context * ctx,
    const char * path_session,
    const llama_token * tokens,
    size_t n_token_count
);

// Load session from file
bool llama_load_session_file(
    struct llama_context * ctx,
    const char * path_session,
    llama_token * tokens_out,
    size_t n_token_capacity,
    size_t * n_token_count_out
);

// State serialization
size_t llama_get_state_size(const struct llama_context * ctx);
size_t llama_copy_state_data(struct llama_context * ctx, uint8_t * dst);
size_t llama_set_state_data(struct llama_context * ctx, const uint8_t * src);
```

## Performance Monitoring

```c
struct llama_perf_context_data llama_perf_context(const struct llama_context * ctx);
void llama_perf_context_reset(struct llama_context * ctx);
```

The performance data structure:
```c
struct llama_perf_context_data {
    double t_start_ms;    // Start time
    double t_load_ms;     // Model load time
    double t_p_eval_ms;   // Prompt evaluation time
    double t_eval_ms;     // Token generation time
    int32_t n_p_eval;     // Prompt tokens evaluated
    int32_t n_eval;       // Tokens generated
    int32_t n_reused;     // KV cache tokens reused
};
```

## Logging

```c
void llama_log_set(ggml_log_callback log_callback, void * user_data);
void ggml_log_set(ggml_log_callback log_callback, void * user_data);

// Log levels
enum ggml_log_level {
    GGML_LOG_LEVEL_NONE  = 0,
    GGML_LOG_LEVEL_DEBUG = 1,
    GGML_LOG_LEVEL_INFO  = 2,
    GGML_LOG_LEVEL_WARN  = 3,
    GGML_LOG_LEVEL_ERROR = 4,
    GGML_LOG_LEVEL_CONT  = 5, // Continuation of previous log
};
```

The Rust wrapper provides `send_logs_to_tracing(LogOptions)` to redirect llama.cpp logs into the Rust `tracing` ecosystem. This is implemented with separate log states for llama.cpp and GGML to prevent log interleaving:

```rust
use infrastructure_llama_cpp::{send_logs_to_tracing, LogOptions};

send_logs_to_tracing(LogOptions::default().with_logs_enabled(true));
```

See [04-rust-safe-wrappers.md](./04-rust-safe-wrappers.md) for the complete Rust API mapping.
