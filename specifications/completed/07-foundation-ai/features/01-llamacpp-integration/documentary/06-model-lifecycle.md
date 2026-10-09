# 06 - Model Lifecycle: From Download to Inference

This document traces the complete lifecycle of a model from acquisition to inference to cleanup, showing how data flows through all three layers.

## Phase 1: Model Acquisition

### From HuggingFace

Using the `hf_hub` crate (as shown in `examples/llama-cpp/simple/src/main.rs`):

```rust
use hf_hub::api::sync::ApiBuilder;

let model_path = ApiBuilder::new()
    .with_progress(true)
    .build()?
    .model("TheBloke/Llama-2-7B-Chat-GGUF".to_string())
    .get("llama-2-7b-chat.Q4_K_M.gguf")?;
```

This downloads the GGUF file to `~/.cache/huggingface/hub/` and returns the local path.

### From Local File

```rust
let model_path = PathBuf::from("/path/to/model.gguf");
```

### Via `foundation_ai` Types

```rust
use foundation_ai::types::ModelSource;

let source = ModelSource::HuggingFace("TheBloke/Llama-2-7B-Chat-GGUF".into());
// or
let source = ModelSource::LocalFile(PathBuf::from("/path/to/model.gguf"));
```

## Phase 2: Backend Initialization

### Layer 2: Safe Wrapper

```rust
use infrastructure_llama_cpp::llama_backend::LlamaBackend;
use infrastructure_llama_cpp::{send_logs_to_tracing, LogOptions};

// Optionally configure logging
send_logs_to_tracing(LogOptions::default().with_logs_enabled(true));

// Initialize the backend (singleton)
let backend = LlamaBackend::init()?;

// Check capabilities
if backend.supports_gpu_offload() {
    println!("GPU offload available");
}
```

### What Happens Internally

1. `LlamaBackend::init()` atomically sets `LLAMA_BACKEND_INITIALIZED` to `true`
2. Calls `llama_backend_init()` which initializes the thread pool and backend registry
3. Returns `Err(BackendAlreadyInitialized)` on second call
4. On `drop()`, calls `llama_backend_free()` and resets the atomic flag

## Phase 3: Model Loading

### Layer 2: Configure Model Parameters

```rust
use infrastructure_llama_cpp::model::params::{LlamaModelParams, LlamaSplitMode};
use std::pin::pin;

let model_params = LlamaModelParams::default()
    .with_n_gpu_layers(999)        // Offload all layers to GPU
    .with_split_mode(LlamaSplitMode::Layer);  // Split across GPUs by layer

// Optional: KV overrides (requires pinning)
let mut model_params = pin!(model_params);
let key = CString::new("llama.context_length")?;
model_params.as_mut().append_kv_override(
    key.as_c_str(),
    ParamOverrideValue::Int(8192),
);

// Optional: Select specific GPU devices
let model_params = LlamaModelParams::default()
    .with_devices(&[0, 1])?;  // Use GPU 0 and GPU 1

// Optional: Move MoE layers to CPU
let mut model_params = pin!(LlamaModelParams::default().with_n_gpu_layers(999));
model_params.as_mut().add_cpu_moe_override();
```

### Layer 2: Load the Model

```rust
use infrastructure_llama_cpp::model::LlamaModel;

let model = LlamaModel::load_from_file(&backend, &model_path, &model_params)?;
```

### What Happens Internally

1. `path` is converted to `CString`
2. `llama_load_model_from_file(cstr, params)` is called, which:
   - Opens the GGUF file
   - Reads the header and metadata
   - Memory-maps the tensor data (if `use_mmap = true`)
   - Allocates GPU memory and transfers layers (based on `n_gpu_layers`)
   - Initializes the tokenizer from GGUF metadata
3. The returned pointer is wrapped in `NonNull<llama_model>`
4. On `drop()`, `llama_free_model()` releases all resources

### Layer 2: Inspect the Model

```rust
println!("Model loaded:");
println!("  Parameters: {}", model.n_params());
println!("  Embedding dim: {}", model.n_embd());
println!("  Layers: {}", model.n_layer());
println!("  Heads: {} (KV: {})", model.n_head(), model.n_head_kv());
println!("  Vocab size: {}", model.n_vocab());
println!("  Context length: {}", model.n_ctx_train());
println!("  Model size: {} MB", model.size() / 1024 / 1024);
println!("  Vocab type: {:?}", model.vocab_type());
println!("  RoPE type: {:?}", model.rope_type());
println!("  Recurrent: {}", model.is_recurrent());

// Chat template
if let Ok(template) = model.chat_template(None) {
    println!("  Chat template: {:?}", template.to_str());
}
```

## Phase 4: Context Creation

### Layer 2: Configure Context Parameters

```rust
use infrastructure_llama_cpp::context::params::LlamaContextParams;
use std::num::NonZeroU32;

let ctx_params = LlamaContextParams::default()
    .with_n_ctx(NonZeroU32::new(4096))     // Context window size
    .with_n_batch(2048)                     // Logical batch size
    .with_n_ubatch(512)                     // Physical batch size
    .with_n_threads(8)                      // Threads for generation
    .with_n_threads_batch(8)                // Threads for prompt processing
    .with_offload_kqv(true);                // Keep KV cache on GPU
```

For embeddings:
```rust
let ctx_params = LlamaContextParams::default()
    .with_embeddings(true)
    .with_pooling_type(LlamaPoolingType::Mean);
```

For reranking:
```rust
let ctx_params = LlamaContextParams::default()
    .with_embeddings(true)
    .with_pooling_type(LlamaPoolingType::Rank);
```

### Layer 2: Create the Context

```rust
let mut ctx = model.new_context(&backend, ctx_params)?;
```

### What Happens Internally

1. `llama_new_context_with_model(model_ptr, params)` is called
2. The KV cache is allocated (size = `n_ctx * n_embd * 2 * sizeof(type_k)`)
3. The computation graph is prepared for the model architecture
4. Backend schedulers are initialized for the available devices
5. The returned context pointer is wrapped with lifetime `'a` tied to `&'a LlamaModel`

### Layer 3 Mapping

```rust
// foundation_ai ModelConfig -> llama-cpp params
fn translate_config(config: &ModelConfig) -> LlamaContextParams {
    LlamaContextParams::default()
        .with_n_ctx(NonZeroU32::new(config.context_length as u32))
        .with_n_threads(config.max_threads as i32)
        .with_n_threads_batch(config.max_threads as i32)
}
```

## Phase 5: Tokenization

```rust
use infrastructure_llama_cpp::model::{AddBos, Special};

let prompt = "Hello, how are you?";
let tokens = model.str_to_token(prompt, AddBos::Always)?;

println!("Tokenized {} chars into {} tokens", prompt.len(), tokens.len());
for token in &tokens {
    let text = model.token_to_str(*token, Special::Tokenize)?;
    println!("  {} -> {:?}", token, text);
}
```

### With Chat Templates

```rust
use infrastructure_llama_cpp::model::{LlamaChatMessage, LlamaChatTemplate};

let template = model.chat_template(None)?;
let messages = vec![
    LlamaChatMessage::new("system".into(), "You are helpful.".into())?,
    LlamaChatMessage::new("user".into(), "Hello!".into())?,
];

let formatted = model.apply_chat_template(&template, &messages, true)?;
let tokens = model.str_to_token(&formatted, AddBos::Never)?;
```

## Phase 6: Inference (Text Generation)

### Batch Creation and Initial Decode

```rust
use infrastructure_llama_cpp::llama_batch::LlamaBatch;

let mut batch = LlamaBatch::new(512, 1);

// Add all prompt tokens, only request logits for the last one
let last_idx = (tokens.len() - 1) as i32;
for (i, token) in (0_i32..).zip(tokens.iter()) {
    batch.add(*token, i, &[0], i == last_idx)?;
}

// Process the prompt (prefill)
ctx.decode(&mut batch)?;
```

### Sampling Setup

```rust
use infrastructure_llama_cpp::sampling::LlamaSampler;

// Build a sampling chain from ModelParams
let sampler = LlamaSampler::chain_simple([
    LlamaSampler::top_k(40),
    LlamaSampler::top_p(0.95, 1),
    LlamaSampler::temp(0.8),
    LlamaSampler::penalties(64, 1.1, 0.0, 0.0),
    LlamaSampler::dist(1234),
]);
```

### Generation Loop

```rust
let mut n_cur = batch.n_tokens();
let max_tokens = 256;
let mut decoder = encoding_rs::UTF_8.new_decoder();
let mut output = String::new();

for _ in 0..max_tokens {
    // Sample next token
    let token = sampler.sample(&ctx, batch.n_tokens() - 1);
    sampler.accept(token);

    // Check for end of generation
    if model.is_eog_token(token) {
        break;
    }

    // Detokenize
    let bytes = model.token_to_bytes(token, Special::Tokenize)?;
    let mut piece = String::with_capacity(32);
    decoder.decode_to_string(&bytes, &mut piece, false);
    output.push_str(&piece);

    // If streaming, output the piece here
    print!("{}", piece);
    std::io::stdout().flush()?;

    // Prepare next decode
    batch.clear();
    batch.add(token, n_cur, &[0], true)?;
    ctx.decode(&mut batch)?;

    n_cur += 1;
}
```

## Phase 7: Embedding Extraction

```rust
let ctx_params = LlamaContextParams::default()
    .with_embeddings(true)
    .with_n_threads_batch(num_cpus::get() as i32);

let mut ctx = model.new_context(&backend, ctx_params)?;

let tokens = model.str_to_token("Hello world", AddBos::Always)?;
let mut batch = LlamaBatch::new(ctx.n_ctx() as usize, 1);
batch.add_sequence(&tokens, 0, false)?;

ctx.decode(&mut batch)?;

// Get pooled embeddings for sequence 0
let embeddings = ctx.embeddings_seq_ith(0)?;
println!("Embedding dimension: {}", embeddings.len());
```

## Phase 8: Session Persistence

### Save Session

```rust
let processed_tokens: Vec<LlamaToken> = /* tokens processed so far */;
ctx.save_session_file("session.bin", &processed_tokens)?;
```

### Load Session

```rust
let mut ctx = model.new_context(&backend, ctx_params)?;
let cached_tokens = ctx.load_session_file("session.bin", 4096)?;

// Continue from where we left off -- KV cache is restored
// Feed any new tokens after the cached prefix
```

### State Serialization

For custom serialization (e.g., sending state over network):

```rust
let state_size = ctx.get_state_size();
let mut state_buffer = vec![0u8; state_size];
let written = unsafe { ctx.copy_state_data(state_buffer.as_mut_ptr()) };
state_buffer.truncate(written);

// Later, restore:
let read = unsafe { ctx.set_state_data(&state_buffer) };
```

## Phase 9: Cleanup

Cleanup is automatic via RAII:

```rust
// Explicit cleanup order (or let them drop naturally):
drop(ctx);      // llama_free() - frees context and KV cache
drop(model);    // llama_free_model() - frees model weights
drop(backend);  // llama_backend_free() - frees global state
```

The lifetime system ensures correct ordering:
- `LlamaContext<'a>` borrows `&'a LlamaModel`, so the context is always dropped before the model
- `LlamaBackend` is required by both `load_from_file` and `new_context`, but not stored, so it can be dropped independently

## Complete End-to-End Example

This mirrors the actual `examples/llama-cpp/simple/src/main.rs`:

```rust
use infrastructure_llama_cpp::llama_backend::LlamaBackend;
use infrastructure_llama_cpp::model::LlamaModel;
use infrastructure_llama_cpp::model::params::LlamaModelParams;
use infrastructure_llama_cpp::model::{AddBos, Special};
use infrastructure_llama_cpp::context::params::LlamaContextParams;
use infrastructure_llama_cpp::llama_batch::LlamaBatch;
use infrastructure_llama_cpp::sampling::LlamaSampler;
use std::num::NonZeroU32;

fn main() -> anyhow::Result<()> {
    // 1. Initialize
    let backend = LlamaBackend::init()?;

    // 2. Load model
    let model_params = LlamaModelParams::default();
    let model = LlamaModel::load_from_file(
        &backend, "model.gguf", &model_params
    )?;

    // 3. Create context
    let ctx_params = LlamaContextParams::default()
        .with_n_ctx(NonZeroU32::new(2048));
    let mut ctx = model.new_context(&backend, ctx_params)?;

    // 4. Tokenize
    let tokens = model.str_to_token("Hello my name is", AddBos::Always)?;

    // 5. Process prompt
    let mut batch = LlamaBatch::new(512, 1);
    let last = (tokens.len() - 1) as i32;
    for (i, tok) in (0i32..).zip(tokens.iter()) {
        batch.add(*tok, i, &[0], i == last)?;
    }
    ctx.decode(&mut batch)?;

    // 6. Generate
    let mut sampler = LlamaSampler::chain_simple([
        LlamaSampler::dist(1234),
        LlamaSampler::greedy(),
    ]);

    let mut n_cur = batch.n_tokens();
    let mut decoder = encoding_rs::UTF_8.new_decoder();

    for _ in 0..32 {
        let token = sampler.sample(&ctx, batch.n_tokens() - 1);
        sampler.accept(token);

        if model.is_eog_token(token) { break; }

        let bytes = model.token_to_bytes(token, Special::Tokenize)?;
        let mut s = String::with_capacity(32);
        decoder.decode_to_string(&bytes, &mut s, false);
        print!("{}", s);

        batch.clear();
        batch.add(token, n_cur, &[0], true)?;
        ctx.decode(&mut batch)?;
        n_cur += 1;
    }

    println!("\n{}", ctx.timings());
    Ok(())
}
```

See [07-inference-pipeline.md](./07-inference-pipeline.md) for detailed coverage of the inference mechanics.
