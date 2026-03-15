# 04 - Rust Safe Wrappers (`infrastructure_llama_cpp`)

The safe wrapper crate provides idiomatic Rust APIs around the raw FFI bindings. Every unsafe call is encapsulated, memory is managed via RAII, and errors are expressed through typed enums.

**Crate name**: `infrastructure_llama_cpp`
**Source location**: `infrastructure/llama-cpp/src/`

## Module Structure

```
src/
  lib.rs              -- Error types, utility functions, device enumeration, logging
  llama_backend.rs    -- LlamaBackend singleton
  model.rs            -- LlamaModel, LlamaLoraAdapter, chat templates, tokenization
  model/params.rs     -- LlamaModelParams, LlamaSplitMode
  model/params/kv_overrides.rs -- GGUF metadata overrides
  context.rs          -- LlamaContext, encode/decode, logits, embeddings
  context/params.rs   -- LlamaContextParams, RopeScalingType, KvCacheType, pooling
  context/kv_cache.rs -- KV cache operations (seq_rm, seq_cp, seq_add, etc.)
  context/session.rs  -- Session save/load, state serialization
  llama_batch.rs      -- LlamaBatch input builder
  sampling.rs         -- LlamaSampler chain and strategies
  token.rs            -- LlamaToken wrapper
  token/data.rs       -- LlamaTokenData (id, logit, probability)
  token/data_array.rs -- LlamaTokenDataArray
  token/logit_bias.rs -- LlamaLogitBias
  token_type.rs       -- LlamaTokenAttr flags
  timing.rs           -- LlamaTimings
  log.rs              -- Log state management (internal)
  mtmd.rs             -- Multi-model multi-device (feature-gated)
```

## `LlamaBackend` - Global Initialization

**File**: `src/llama_backend.rs`

The backend must be initialized before any other operation. It enforces singleton semantics via an `AtomicBool`.

```rust
pub struct LlamaBackend {}

impl LlamaBackend {
    // Initialize without NUMA
    pub fn init() -> crate::Result<LlamaBackend>;

    // Initialize with NUMA strategy
    pub fn init_numa(strategy: NumaStrategy) -> crate::Result<LlamaBackend>;

    // Capability queries
    pub fn supports_gpu_offload(&self) -> bool;
    pub fn supports_mmap(&self) -> bool;
    pub fn supports_mlock(&self) -> bool;

    // Suppress all llama.cpp logging
    pub fn void_logs(&mut self);
}

// Drop calls llama_backend_free and resets the AtomicBool
impl Drop for LlamaBackend { ... }
```

Key behavior:
- `init()` returns `Err(LlamaCppError::BackendAlreadyInitialized)` if called twice
- After `drop()`, `init()` can be called again
- The backend reference is required by `LlamaModel::load_from_file()` and `model.new_context()` as a proof-of-initialization

### NUMA Strategies

```rust
pub enum NumaStrategy {
    DISABLED, DISTRIBUTE, ISOLATE, NUMACTL, MIRROR, COUNT,
}
```

## `LlamaModel` - Model Management

**File**: `src/model.rs`

Wraps `llama_model` with RAII and provides tokenization, metadata queries, and context creation.

```rust
#[repr(transparent)]
pub struct LlamaModel {
    pub model: NonNull<infrastructure_llama_bindings::llama_model>,
}

unsafe impl Send for LlamaModel {}
unsafe impl Sync for LlamaModel {}
```

### Loading

```rust
impl LlamaModel {
    pub fn load_from_file(
        _: &LlamaBackend,
        path: impl AsRef<Path>,
        params: &LlamaModelParams,
    ) -> Result<Self, LlamaModelLoadError>;
}
```

### Model Properties

```rust
impl LlamaModel {
    pub fn n_ctx_train(&self) -> u32;    // Training context length
    pub fn n_vocab(&self) -> i32;        // Vocabulary size
    pub fn vocab_type(&self) -> VocabType; // BPE or SPM
    pub fn n_embd(&self) -> c_int;       // Embedding dimension
    pub fn n_layer(&self) -> u32;        // Layer count
    pub fn n_head(&self) -> u32;         // Attention heads
    pub fn n_head_kv(&self) -> u32;      // KV attention heads
    pub fn size(&self) -> u64;           // Total tensor bytes
    pub fn n_params(&self) -> u64;       // Parameter count
    pub fn is_recurrent(&self) -> bool;  // RWKV, Mamba, etc.
    pub fn rope_type(&self) -> Option<RopeType>; // Norm, NeoX, MRope, Vision
}
```

### Tokenization

```rust
impl LlamaModel {
    // Tokenize a string
    pub fn str_to_token(
        &self, str: &str, add_bos: AddBos,
    ) -> Result<Vec<LlamaToken>, StringToTokenError>;

    // Detokenize a single token
    pub fn token_to_str(
        &self, token: LlamaToken, special: Special,
    ) -> Result<String, TokenToStringError>;

    // Detokenize to bytes (handles partial UTF-8)
    pub fn token_to_bytes(
        &self, token: LlamaToken, special: Special,
    ) -> Result<Vec<u8>, TokenToStringError>;

    // Detokenize multiple tokens
    pub fn tokens_to_str(
        &self, tokens: &[LlamaToken], special: Special,
    ) -> Result<String, TokenToStringError>;

    // Iterate all tokens in vocabulary
    pub fn tokens(&self, special: Special)
        -> impl Iterator<Item = (LlamaToken, Result<String, TokenToStringError>)>;
}

pub enum AddBos { Always, Never }
pub enum Special { Tokenize, Plaintext }
```

### Special Tokens

```rust
impl LlamaModel {
    pub fn token_bos(&self) -> LlamaToken;          // Beginning of sequence
    pub fn token_eos(&self) -> LlamaToken;          // End of sequence
    pub fn token_nl(&self) -> LlamaToken;           // Newline
    pub fn token_sep(&self) -> LlamaToken;          // Separator
    pub fn decode_start_token(&self) -> LlamaToken; // Decoder start
    pub fn is_eog_token(&self, token: LlamaToken) -> bool; // End of generation
    pub fn token_attr(&self, token: LlamaToken) -> LlamaTokenAttrs; // Token attributes
}
```

### Metadata

```rust
impl LlamaModel {
    pub fn meta_count(&self) -> i32;
    pub fn meta_val_str(&self, key: &str) -> Result<String, MetaValError>;
    pub fn meta_key_by_index(&self, index: i32) -> Result<String, MetaValError>;
    pub fn meta_val_str_by_index(&self, index: i32) -> Result<String, MetaValError>;
}
```

### Chat Templates

```rust
impl LlamaModel {
    // Retrieve the model's built-in chat template
    pub fn chat_template(&self, name: Option<&str>)
        -> Result<LlamaChatTemplate, ChatTemplateError>;

    // Apply template to messages
    pub fn apply_chat_template(
        &self,
        tmpl: &LlamaChatTemplate,
        chat: &[LlamaChatMessage],
        add_ass: bool,  // Add assistant opening tag
    ) -> Result<String, ApplyChatTemplateError>;
}

pub struct LlamaChatTemplate(CString);
pub struct LlamaChatMessage { role: CString, content: CString }

impl LlamaChatMessage {
    pub fn new(role: String, content: String) -> Result<Self, NewLlamaChatMessageError>;
}
```

### LoRA Adapters

```rust
impl LlamaModel {
    pub fn lora_adapter_init(&self, path: impl AsRef<Path>)
        -> Result<LlamaLoraAdapter, LlamaLoraAdapterInitError>;
}

pub struct LlamaLoraAdapter {
    pub lora_adapter: NonNull<infrastructure_llama_bindings::llama_adapter_lora>,
}
```

### Context Creation

```rust
impl LlamaModel {
    pub fn new_context<'a>(
        &'a self,
        _: &LlamaBackend,
        params: LlamaContextParams,
    ) -> Result<LlamaContext<'a>, LlamaContextLoadError>;
}
```

Note the lifetime `'a` -- the context borrows the model, preventing the model from being dropped while the context exists.

## `LlamaModelParams` - Model Loading Configuration

**File**: `src/model/params.rs`

```rust
pub struct LlamaModelParams {
    pub params: infrastructure_llama_bindings::llama_model_params,
    kv_overrides: Vec<...>,
    buft_overrides: Vec<...>,
    devices: Pin<Box<[ggml_backend_dev_t; 16]>>,
}
```

### Builder Pattern

```rust
impl LlamaModelParams {
    // Defaults: n_gpu_layers=999, split_mode=Layer, use_mmap=true
    fn default() -> Self;

    pub fn with_n_gpu_layers(self, n: u32) -> Self;
    pub fn with_main_gpu(self, gpu: i32) -> Self;
    pub fn with_vocab_only(self, v: bool) -> Self;
    pub fn with_use_mlock(self, v: bool) -> Self;
    pub fn with_split_mode(self, mode: LlamaSplitMode) -> Self;
    pub fn with_devices(self, devices: &[usize]) -> Result<Self, LlamaCppError>;

    // KV override (requires Pin)
    pub fn append_kv_override(self: Pin<&mut Self>, key: &CStr, value: ParamOverrideValue);

    // Buffer type override (move MoE to CPU)
    pub fn add_cpu_moe_override(self: Pin<&mut Self>);
    pub fn add_cpu_buft_override(self: Pin<&mut Self>, pattern: &CStr);
}

pub enum LlamaSplitMode {
    None,   // Single GPU
    Layer,  // Split layers across GPUs (default)
    Row,    // Split rows with tensor parallelism
}
```

## `LlamaContext` - Inference Context

**File**: `src/context.rs`

```rust
pub struct LlamaContext<'a> {
    pub context: NonNull<infrastructure_llama_bindings::llama_context>,
    pub model: &'a LlamaModel,
    initialized_logits: Vec<i32>,
    embeddings_enabled: bool,
}
```

### Core Inference

```rust
impl LlamaContext<'_> {
    // Decode (autoregressive generation)
    pub fn decode(&mut self, batch: &mut LlamaBatch) -> Result<(), DecodeError>;

    // Encode (encoder-decoder models)
    pub fn encode(&mut self, batch: &mut LlamaBatch) -> Result<(), EncodeError>;

    // Context properties
    pub fn n_ctx(&self) -> u32;
    pub fn n_batch(&self) -> u32;
    pub fn n_ubatch(&self) -> u32;
}
```

### Logit Access

```rust
impl LlamaContext<'_> {
    // Logits for the last token
    pub fn get_logits(&self) -> &[f32];

    // Logits for position i (panics if not initialized)
    pub fn get_logits_ith(&self, i: i32) -> &[f32];

    // Convenience: iterate over (token, logit) pairs
    pub fn candidates(&self) -> impl Iterator<Item = LlamaTokenData>;
    pub fn candidates_ith(&self, i: i32) -> impl Iterator<Item = LlamaTokenData>;

    // Get a LlamaTokenDataArray ready for sampling
    pub fn token_data_array(&self) -> LlamaTokenDataArray;
    pub fn token_data_array_ith(&self, i: i32) -> LlamaTokenDataArray;
}
```

### Embedding Access

```rust
impl LlamaContext<'_> {
    // Per-sequence embeddings (pooled)
    pub fn embeddings_seq_ith(&self, i: i32) -> Result<&[f32], EmbeddingsError>;

    // Per-token embeddings
    pub fn embeddings_ith(&self, i: i32) -> Result<&[f32], EmbeddingsError>;
}
```

### LoRA Management

```rust
impl LlamaContext<'_> {
    pub fn lora_adapter_set(
        &self, adapter: &mut LlamaLoraAdapter, scale: f32,
    ) -> Result<(), LlamaLoraAdapterSetError>;

    pub fn lora_adapter_remove(
        &self, adapter: &mut LlamaLoraAdapter,
    ) -> Result<(), LlamaLoraAdapterRemoveError>;
}
```

### Performance Timings

```rust
impl LlamaContext<'_> {
    pub fn reset_timings(&mut self);
    pub fn timings(&mut self) -> LlamaTimings;
}

pub struct LlamaTimings {
    pub fn t_start_ms(&self) -> f64;
    pub fn t_load_ms(&self) -> f64;
    pub fn t_p_eval_ms(&self) -> f64;  // Prompt evaluation time
    pub fn t_eval_ms(&self) -> f64;    // Generation time
    pub fn n_p_eval(&self) -> i32;     // Prompt tokens evaluated
    pub fn n_eval(&self) -> i32;       // Tokens generated
}
```

### KV Cache Operations

**File**: `src/context/kv_cache.rs`

```rust
impl LlamaContext<'_> {
    // Clear entire cache
    pub fn clear_kv_cache(&mut self);

    // Remove tokens from a sequence
    pub fn clear_kv_cache_seq(
        &mut self, src: Option<u32>, p0: Option<u32>, p1: Option<u32>,
    ) -> Result<bool, KvCacheConversionError>;

    // Copy cache between sequences
    pub fn copy_kv_cache_seq(
        &mut self, src: i32, dest: i32, p0: Option<u32>, p1: Option<u32>,
    ) -> Result<(), KvCacheConversionError>;

    // Keep only one sequence
    pub fn llama_kv_cache_seq_keep(&mut self, seq_id: i32);

    // Shift positions
    pub fn kv_cache_seq_add(
        &mut self, seq_id: i32, p0: Option<u32>, p1: Option<u32>, delta: i32,
    ) -> Result<(), KvCacheConversionError>;

    // Divide positions
    pub fn kv_cache_seq_div(
        &mut self, seq_id: i32, p0: Option<u32>, p1: Option<u32>, d: NonZeroU8,
    ) -> Result<(), KvCacheConversionError>;

    // Query max position
    pub fn kv_cache_seq_pos_max(&self, seq_id: i32) -> i32;
}
```

### Session Management

**File**: `src/context/session.rs`

```rust
impl LlamaContext<'_> {
    pub fn save_session_file(
        &self, path: impl AsRef<Path>, tokens: &[LlamaToken],
    ) -> Result<(), SaveSessionError>;

    pub fn load_session_file(
        &mut self, path: impl AsRef<Path>, max_tokens: usize,
    ) -> Result<Vec<LlamaToken>, LoadSessionError>;

    pub fn get_state_size(&self) -> usize;
    pub unsafe fn copy_state_data(&self, dest: *mut u8) -> usize;
    pub unsafe fn set_state_data(&mut self, src: &[u8]) -> usize;
}
```

## `LlamaContextParams` - Context Configuration

**File**: `src/context/params.rs`

```rust
pub struct LlamaContextParams {
    pub context_params: infrastructure_llama_bindings::llama_context_params,
}

unsafe impl Send for LlamaContextParams {}
unsafe impl Sync for LlamaContextParams {}
```

### Builder Pattern

```rust
impl LlamaContextParams {
    // Defaults: n_ctx=512, n_batch=2048, n_ubatch=512, n_threads=4
    fn default() -> Self;

    pub fn with_n_ctx(self, n_ctx: Option<NonZeroU32>) -> Self;
    pub fn with_n_batch(self, n_batch: u32) -> Self;
    pub fn with_n_ubatch(self, n_ubatch: u32) -> Self;
    pub fn with_n_threads(self, n_threads: i32) -> Self;
    pub fn with_n_threads_batch(self, n_threads: i32) -> Self;
    pub fn with_embeddings(self, enabled: bool) -> Self;
    pub fn with_offload_kqv(self, enabled: bool) -> Self;
    pub fn with_swa_full(self, enabled: bool) -> Self;
    pub fn with_n_seq_max(self, n: u32) -> Self;
    pub fn with_rope_scaling_type(self, t: RopeScalingType) -> Self;
    pub fn with_rope_freq_base(self, f: f32) -> Self;
    pub fn with_rope_freq_scale(self, f: f32) -> Self;
    pub fn with_pooling_type(self, t: LlamaPoolingType) -> Self;
    pub fn with_flash_attention_policy(self, p: llama_flash_attn_type) -> Self;
    pub fn with_type_k(self, t: KvCacheType) -> Self;
    pub fn with_type_v(self, t: KvCacheType) -> Self;
    pub fn with_cb_eval(self, cb: ggml_backend_sched_eval_callback) -> Self;
}
```

### Supporting Enums

```rust
pub enum RopeScalingType { Unspecified, None, Linear, Yarn }
pub enum LlamaPoolingType { Unspecified, None, Mean, Cls, Last, Rank }
pub enum KvCacheType { F32, F16, Q4_0, Q4_1, ..., BF16, TQ1_0, MXFP4, Unknown(u32) }
```

## `LlamaBatch` - Input Builder

**File**: `src/llama_batch.rs`

```rust
pub struct LlamaBatch<'a> {
    allocated: usize,
    pub initialized_logits: Vec<i32>,
    pub llama_batch: llama_batch,
    phantom: PhantomData<&'a [LlamaToken]>,
}
```

### API

```rust
impl LlamaBatch<'_> {
    // Allocate a new batch
    pub fn new(n_tokens: usize, n_seq_max: i32) -> Self;

    // Wrap existing tokens (no allocation)
    pub fn get_one(tokens: &[LlamaToken]) -> Result<Self, BatchAddError>;

    // Add a single token
    pub fn add(
        &mut self, token: LlamaToken, pos: llama_pos,
        seq_ids: &[i32], logits: bool,
    ) -> Result<(), BatchAddError>;

    // Add a sequence of tokens
    pub fn add_sequence(
        &mut self, tokens: &[LlamaToken], seq_id: i32, logits_all: bool,
    ) -> Result<(), BatchAddError>;

    // Clear without deallocating
    pub fn clear(&mut self);

    pub fn n_tokens(&self) -> i32;
}
```

Key behavior of `add_sequence`: The last token in the sequence always has logits enabled (regardless of `logits_all`), because you always need logits from the final position to continue generation.

## `LlamaSampler` - Sampling Chain

**File**: `src/sampling.rs`

```rust
pub struct LlamaSampler {
    pub sampler: *mut infrastructure_llama_bindings::llama_sampler,
}
```

### Core Operations

```rust
impl LlamaSampler {
    // Sample a token from the context at position idx
    pub fn sample(&mut self, ctx: &LlamaContext, idx: i32) -> LlamaToken;

    // Apply sampler to a token data array
    pub fn apply(&self, data_array: &mut LlamaTokenDataArray);

    // Accept a token (update internal state)
    pub fn accept(&mut self, token: LlamaToken);
    pub fn accept_many(&mut self, tokens: impl IntoIterator<Item = impl Borrow<LlamaToken>>);

    // Reset sampler state
    pub fn reset(&mut self);

    // Get the random seed
    pub fn get_seed(&self) -> u32;
}
```

### Chain Construction

```rust
impl LlamaSampler {
    // Build a sampling chain (order matters!)
    pub fn chain(samplers: impl IntoIterator<Item = Self>, no_perf: bool) -> Self;
    pub fn chain_simple(samplers: impl IntoIterator<Item = Self>) -> Self;
}
```

### Strategy Constructors

```rust
impl LlamaSampler {
    // Selection (must be last in chain)
    pub fn greedy() -> Self;
    pub fn dist(seed: u32) -> Self;

    // Temperature
    pub fn temp(t: f32) -> Self;
    pub fn temp_ext(t: f32, delta: f32, exponent: f32) -> Self;

    // Top-K
    pub fn top_k(k: i32) -> Self;

    // Top-P (Nucleus)
    pub fn top_p(p: f32, min_keep: usize) -> Self;

    // Min-P
    pub fn min_p(p: f32, min_keep: usize) -> Self;

    // Top-n-sigma
    pub fn top_n_sigma(n: f32) -> Self;

    // Typical
    pub fn typical(p: f32, min_keep: usize) -> Self;

    // XTC
    pub fn xtc(p: f32, t: f32, min_keep: usize, seed: u32) -> Self;

    // Mirostat
    pub fn mirostat(n_vocab: i32, seed: u32, tau: f32, eta: f32, m: i32) -> Self;
    pub fn mirostat_v2(seed: u32, tau: f32, eta: f32) -> Self;

    // Repetition penalty
    pub fn penalties(last_n: i32, repeat: f32, freq: f32, present: f32) -> Self;

    // DRY (Don't Repeat Yourself)
    pub fn dry(model: &LlamaModel, multiplier: f32, base: f32,
               allowed_length: i32, penalty_last_n: i32,
               seq_breakers: impl IntoIterator<Item = impl AsRef<[u8]>>) -> Self;

    // Grammar-constrained generation
    pub fn grammar(model: &LlamaModel, grammar_str: &str, grammar_root: &str)
        -> Result<Self, GrammarError>;
    pub fn grammar_lazy(model: &LlamaModel, grammar_str: &str, grammar_root: &str,
                        trigger_words: ..., trigger_tokens: &[LlamaToken])
        -> Result<Self, GrammarError>;

    // Logit bias
    pub fn logit_bias(n_vocab: i32, biases: &[LlamaLogitBias]) -> Self;
}
```

### Typical Sampling Chain

```rust
let sampler = LlamaSampler::chain_simple([
    LlamaSampler::top_k(40),
    LlamaSampler::top_p(0.95, 1),
    LlamaSampler::temp(0.8),
    LlamaSampler::dist(1234),  // Must be last (or greedy())
]);
```

## Token Types

### `LlamaToken`

**File**: `src/token.rs`

```rust
#[repr(transparent)]
#[derive(Copy, Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub struct LlamaToken(pub infrastructure_llama_bindings::llama_token); // i32
```

### `LlamaTokenData`

**File**: `src/token/data.rs`

```rust
pub struct LlamaTokenData {
    // id: LlamaToken, logit: f32, p: f32
}
```

### `LlamaTokenAttrs`

**File**: `src/token_type.rs`

Uses `enumflags2` for bitflag support:

```rust
#[bitflags]
#[repr(u32)]
pub enum LlamaTokenAttr {
    Unknown, Unused, Normal, Control, UserDefined,
    Byte, Normalized, LStrip, RStrip, SingleWord,
}

pub struct LlamaTokenAttrs(pub BitFlags<LlamaTokenAttr>);
```

## Error Types

**File**: `src/lib.rs`

```rust
pub enum LlamaCppError {
    BackendAlreadyInitialized,
    ChatTemplateError(ChatTemplateError),
    DecodeError(DecodeError),
    EncodeError(EncodeError),
    LlamaModelLoadError(LlamaModelLoadError),
    LlamaContextLoadError(LlamaContextLoadError),
    BatchAddError(BatchAddError),
    EmbeddingError(EmbeddingsError),
    BackendDeviceNotFound(usize),
    MaxDevicesExceeded(usize),
}

pub enum DecodeError { NoKvCacheSlot, NTokensZero, Unknown(c_int) }
pub enum EncodeError { NoKvCacheSlot, NTokensZero, Unknown(c_int) }
pub enum EmbeddingsError { NotEnabled, LogitsNotEnabled, NonePoolType }
pub enum LlamaModelLoadError { NullError(NulError), NullResult, PathToStrError(PathBuf) }
pub enum GrammarError { RootNotFound, TriggerWordNullBytes, GrammarNullBytes, NullGrammar }
```

## Thread Safety

- `LlamaModel`: `Send + Sync` -- can be shared across threads
- `LlamaContextParams`: `Send + Sync`
- `LlamaContext`: Not `Send` or `Sync` by default (contains mutable FFI state)
- `LlamaBackend`: Enforces global singleton via `AtomicBool`

## Utility Functions

```rust
pub fn llama_time_us() -> i64;           // Time in microseconds
pub fn ggml_time_us() -> i64;            // GGML time in microseconds
pub fn max_devices() -> usize;           // Maximum device count
pub fn mmap_supported() -> bool;         // mmap support check
pub fn mlock_supported() -> bool;        // mlock support check
pub fn llama_supports_mlock() -> bool;   // mlock support check
pub fn list_llama_ggml_backend_devices() -> Vec<LlamaBackendDevice>;
pub fn send_logs_to_tracing(options: LogOptions); // Redirect logs to tracing
```

See [05-foundation-ai-types.md](./05-foundation-ai-types.md) for how these types map to the domain layer.
