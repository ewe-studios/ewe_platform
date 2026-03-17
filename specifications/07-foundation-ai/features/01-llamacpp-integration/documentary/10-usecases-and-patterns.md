# 10 - Use Cases and Patterns

This document provides practical, copy-paste-ready patterns for common use cases with the Rust llama.cpp integration. Each pattern uses real types from the codebase and follows the idioms established in the `examples/llama-cpp/` directory.

## Text Generation (Chat Completion)

The canonical text generation pattern, based on `examples/llama-cpp/simple/src/main.rs`.

### Minimal Generation

```rust
use infrastructure_llama_cpp::llama_backend::LlamaBackend;
use infrastructure_llama_cpp::model::LlamaModel;
use infrastructure_llama_cpp::model::params::LlamaModelParams;
use infrastructure_llama_cpp::model::{AddBos, Special};
use infrastructure_llama_cpp::context::params::LlamaContextParams;
use infrastructure_llama_cpp::llama_batch::LlamaBatch;
use infrastructure_llama_cpp::sampling::LlamaSampler;
use std::num::NonZeroU32;

let backend = LlamaBackend::init()?;
let model_params = LlamaModelParams::default();
let model = LlamaModel::load_from_file(&backend, "model.gguf", &model_params)?;

let ctx_params = LlamaContextParams::default()
    .with_n_ctx(NonZeroU32::new(2048));
let mut ctx = model.new_context(&backend, ctx_params)?;

let tokens = model.str_to_token("Hello my name is", AddBos::Always)?;

// Prefill: process all prompt tokens
let mut batch = LlamaBatch::new(512, 1);
let last = (tokens.len() - 1) as i32;
for (i, tok) in (0i32..).zip(tokens.iter()) {
    batch.add(*tok, i, &[0], i == last)?;
}
ctx.decode(&mut batch)?;

// Generate
let mut sampler = LlamaSampler::chain_simple([
    LlamaSampler::dist(1234),
    LlamaSampler::greedy(),
]);

let mut n_cur = batch.n_tokens();
let mut decoder = encoding_rs::UTF_8.new_decoder();

for _ in 0..256 {
    let token = sampler.sample(&ctx, batch.n_tokens() - 1);
    sampler.accept(token);

    if model.is_eog_token(token) {
        break;
    }

    let bytes = model.token_to_bytes(token, Special::Tokenize)?;
    let mut piece = String::with_capacity(32);
    decoder.decode_to_string(&bytes, &mut piece, false);
    print!("{}", piece);

    batch.clear();
    batch.add(token, n_cur, &[0], true)?;
    ctx.decode(&mut batch)?;
    n_cur += 1;
}
```

### With Chat Template

```rust
use infrastructure_llama_cpp::model::{LlamaChatMessage, LlamaChatTemplate};

let template = model.chat_template(None)?;
let messages = vec![
    LlamaChatMessage::new("system".into(), "You are a helpful assistant.".into())?,
    LlamaChatMessage::new("user".into(), "What is Rust?".into())?,
];

// add_ass=true appends the assistant turn prefix
let formatted = model.apply_chat_template(&template, &messages, true)?;
let tokens = model.str_to_token(&formatted, AddBos::Never)?;
// ... proceed with batch construction and generation loop
```

### With Quality Sampling

```rust
let sampler = LlamaSampler::chain_simple([
    // Repetition penalty over last 64 tokens
    LlamaSampler::penalties(64, 1.1, 0.0, 0.0),
    // Top-K: keep only top 40 candidates
    LlamaSampler::top_k(40),
    // Top-P (nucleus): keep candidates summing to 95% probability
    LlamaSampler::top_p(0.95, 1),
    // Temperature: 0.8 for some creativity
    LlamaSampler::temp(0.8),
    // Random weighted selection with seed
    LlamaSampler::dist(1234),
]);
```

### Streaming Output

The generation loop above already streams token-by-token. The key is using `encoding_rs::UTF_8.new_decoder()` to handle multi-byte UTF-8 characters that may be split across tokens:

```rust
let mut decoder = encoding_rs::UTF_8.new_decoder();

// In the generation loop:
let bytes = model.token_to_bytes(token, Special::Tokenize)?;
let mut piece = String::with_capacity(32);
let _result = decoder.decode_to_string(&bytes, &mut piece, false);
print!("{}", piece);
std::io::stdout().flush()?;
```

The decoder buffers incomplete byte sequences internally. When a token produces the first byte of a multi-byte character and the next token produces the remaining bytes, the decoder emits the complete character only when all bytes are available.

## Embedding Extraction

Based on `examples/llama-cpp/embeddings/src/main.rs`. Requires an embedding model (e.g., `BAAI/bge-small-en-v1.5`).

### Single Text Embedding

```rust
use infrastructure_llama_cpp::context::params::LlamaContextParams;

let ctx_params = LlamaContextParams::default()
    .with_n_threads_batch(std::thread::available_parallelism()?.get().try_into()?)
    .with_embeddings(true);

let mut ctx = model.new_context(&backend, ctx_params)?;

let tokens = model.str_to_token("Hello world", AddBos::Always)?;
let mut batch = LlamaBatch::new(ctx.n_ctx() as usize, 1);
batch.add_sequence(&tokens, 0, false)?;

ctx.clear_kv_cache();
ctx.decode(&mut batch)?;

let embeddings = ctx.embeddings_seq_ith(0)?;
// embeddings is &[f32] of length model.n_embd()
```

### Batch Embedding (Multiple Texts)

Process multiple texts by assigning each a different sequence ID:

```rust
let texts = vec!["First document", "Second document", "Third document"];
let mut all_embeddings = Vec::new();

for (seq_id, text) in texts.iter().enumerate() {
    let tokens = model.str_to_token(text, AddBos::Always)?;
    let mut batch = LlamaBatch::new(ctx.n_ctx() as usize, 1);
    batch.add_sequence(&tokens, seq_id as i32, false)?;

    ctx.clear_kv_cache();
    ctx.decode(&mut batch)?;

    let embedding = ctx.embeddings_seq_ith(seq_id as i32)?;
    all_embeddings.push(normalize(embedding));
}

fn normalize(input: &[f32]) -> Vec<f32> {
    let magnitude = input.iter().fold(0.0, |acc, &val| val.mul_add(val, acc)).sqrt();
    input.iter().map(|&val| val / magnitude).collect()
}
```

### Semantic Search

Compute cosine similarity between a query embedding and document embeddings:

```rust
fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
    assert_eq!(a.len(), b.len());
    let (dot, norm_a, norm_b) = a.iter().zip(b.iter()).fold(
        (0.0f64, 0.0f64, 0.0f64),
        |(dot, na, nb), (&ai, &bi)| {
            let ai = ai as f64;
            let bi = bi as f64;
            (dot + ai * bi, na + ai * ai, nb + bi * bi)
        },
    );
    if norm_a == 0.0 || norm_b == 0.0 {
        return 0.0;
    }
    (dot / (norm_a.sqrt() * norm_b.sqrt())) as f32
}

// After computing embeddings for query and documents:
let mut scores: Vec<(usize, f32)> = doc_embeddings
    .iter()
    .enumerate()
    .map(|(i, doc_emb)| (i, cosine_similarity(&query_embedding, doc_emb)))
    .collect();

scores.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap());

for (idx, score) in &scores[..5] {
    println!("Document {}: score = {:.4}", idx, score);
}
```

## Cross-Encoder Reranking

Based on `examples/llama-cpp/reranker/src/main.rs`. Requires a cross-encoder model (e.g., `BAAI/bge-reranker-v2-m3`).

### Basic Reranking

```rust
use infrastructure_llama_cpp::context::params::{LlamaContextParams, LlamaPoolingType};

let ctx_params = LlamaContextParams::default()
    .with_n_threads_batch(std::thread::available_parallelism()?.get().try_into()?)
    .with_embeddings(true)
    .with_pooling_type(LlamaPoolingType::Rank);

let mut ctx = model.new_context(&backend, ctx_params)?;

let query = "What is machine learning?";
let documents = vec![
    "Machine learning is a subset of artificial intelligence.",
    "The weather today is sunny and warm.",
    "Deep learning uses neural networks with many layers.",
];

let mut scores = Vec::new();

for (i, doc) in documents.iter().enumerate() {
    // Format: query + EOS + BOS + document
    let pair = format!("{query}</s><s>{doc}");
    let tokens = model.str_to_token(&pair, AddBos::Always)?;

    let mut batch = LlamaBatch::new(2048, 1);
    batch.add_sequence(&tokens, 0, false)?;

    ctx.clear_kv_cache();
    ctx.decode(&mut batch)?;

    let score = ctx.embeddings_seq_ith(0)?;
    scores.push((i, score[0]));
}

// Sort by relevance score (descending)
scores.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap());

for (idx, score) in &scores {
    println!("Document {}: score = {:.3} -- {}", idx, score, documents[*idx]);
}
```

### Batch Reranking

When the model context is large enough, multiple query-document pairs can be processed in a single batch:

```rust
let mut batch = LlamaBatch::new(2048, 1);
let mut max_seq_id = 0;
let mut output = Vec::new();

for (i, doc) in documents.iter().enumerate() {
    let pair = format!("{query}</s><s>{doc}");
    let tokens = model.str_to_token(&pair, AddBos::Always)?;

    // Flush batch if adding this pair would exceed capacity
    if (batch.n_tokens() as usize + tokens.len()) > 2048 {
        ctx.clear_kv_cache();
        ctx.decode(&mut batch)?;
        for seq in 0..max_seq_id {
            let emb = ctx.embeddings_seq_ith(seq)?;
            output.push(emb[0]);
        }
        batch.clear();
        max_seq_id = 0;
    }

    batch.add_sequence(&tokens, max_seq_id, false)?;
    max_seq_id += 1;
}

// Process final batch
if batch.n_tokens() > 0 {
    ctx.clear_kv_cache();
    ctx.decode(&mut batch)?;
    for seq in 0..max_seq_id {
        let emb = ctx.embeddings_seq_ith(seq)?;
        output.push(emb[0]);
    }
}
```

## Grammar-Constrained JSON Output

Force the model to produce valid JSON using GBNF grammar:

```rust
let json_grammar = r#"
root   ::= object
value  ::= object | array | string | number | "true" | "false" | "null"

object ::= "{" ws (string ":" ws value ("," ws string ":" ws value)*)? "}"
array  ::= "[" ws (value ("," ws value)*)? "]"
string ::= "\"" ([^"\\] | "\\" .)* "\""
number ::= "-"? [0-9]+ ("." [0-9]+)?

ws     ::= [ \t\n]*
"#;

let sampler = LlamaSampler::chain_simple([
    LlamaSampler::grammar(&model, json_grammar, "root")?,
    LlamaSampler::temp(0.8),
    LlamaSampler::dist(1234),
]);
```

### Lazy Grammar (Free-Form + Structured)

Allow the model to reason in free-form text before producing structured output:

```rust
let sampler = LlamaSampler::grammar_lazy(
    &model,
    json_grammar,
    "root",
    ["```json", "{"],      // Trigger words that activate grammar enforcement
    &[],                    // Trigger tokens (empty = none)
)?;
```

With lazy grammar, the model can write "Here is the result:" followed by a JSON block. The grammar only kicks in after one of the trigger words is generated.

### Typed JSON Schema

For a specific JSON schema (e.g., an API response):

```rust
let api_response_grammar = r#"
root   ::= "{" ws "\"status\":" ws status "," ws "\"data\":" ws data "," ws "\"message\":" ws string "}"
status ::= "\"ok\"" | "\"error\""
data   ::= object | "null"
object ::= "{" ws (string ":" ws value ("," ws string ":" ws value)*)? "}"
value  ::= object | array | string | number | "true" | "false" | "null"
array  ::= "[" ws (value ("," ws value)*)? "]"
string ::= "\"" ([^"\\] | "\\" .)* "\""
number ::= "-"? [0-9]+ ("." [0-9]+)?
ws     ::= [ \t\n]*
"#;

let sampler = LlamaSampler::chain_simple([
    LlamaSampler::grammar(&model, api_response_grammar, "root")?,
    LlamaSampler::temp(0.3),  // Low temperature for deterministic structure
    LlamaSampler::dist(1234),
]);
```

## Multi-Turn Conversation

### With KV Cache Reuse

Reuse the KV cache between conversation turns to avoid reprocessing the entire history:

```rust
let template = model.chat_template(None)?;
let mut all_messages = Vec::new();
let mut total_tokens = 0i32;

// System message
all_messages.push(
    LlamaChatMessage::new("system".into(), "You are a helpful assistant.".into())?
);

// Turn 1: Process full prompt
all_messages.push(
    LlamaChatMessage::new("user".into(), "Hello!".into())?
);
let formatted = model.apply_chat_template(&template, &all_messages, true)?;
let tokens = model.str_to_token(&formatted, AddBos::Always)?;

let mut batch = LlamaBatch::new(512, 1);
let last = (tokens.len() - 1) as i32;
for (i, tok) in (0i32..).zip(tokens.iter()) {
    batch.add(*tok, i, &[0], i == last)?;
}
ctx.decode(&mut batch)?;

// Generate response for turn 1...
let response_1 = generate_response(&model, &mut ctx, &mut batch, &sampler, &mut total_tokens)?;

// Turn 2: Only process new tokens (KV cache has previous context)
all_messages.push(
    LlamaChatMessage::new("assistant".into(), response_1)?
);
all_messages.push(
    LlamaChatMessage::new("user".into(), "Tell me more.".into())?
);

// Re-format to get the new tokens added by the template
let formatted_2 = model.apply_chat_template(&template, &all_messages, true)?;
let all_tokens_2 = model.str_to_token(&formatted_2, AddBos::Never)?;

// Only process tokens beyond what we already have in the KV cache
let new_tokens = &all_tokens_2[total_tokens as usize..];

batch.clear();
let last_new = (new_tokens.len() - 1) as i32;
for (i, tok) in new_tokens.iter().enumerate() {
    let pos = total_tokens + i as i32;
    batch.add(*tok, pos, &[0], i as i32 == last_new)?;
}
ctx.decode(&mut batch)?;

// Generate response for turn 2...
```

### Context Window Management

When the KV cache fills up, shift positions to make room while preserving the system prompt:

```rust
let n_ctx = ctx.n_ctx() as i32;
let n_keep = 256;     // Keep first N tokens (system prompt)
let n_discard = 128;  // Discard this many oldest non-system tokens

if total_tokens + new_tokens_count > n_ctx {
    // Remove tokens in range [n_keep, n_keep + n_discard)
    ctx.clear_kv_cache_seq(
        Some(0),
        Some(n_keep as u32),
        Some((n_keep + n_discard) as u32),
    )?;

    // Shift remaining positions down by n_discard
    ctx.kv_cache_seq_add(0, Some((n_keep + n_discard) as u32), None, -n_discard)?;

    total_tokens -= n_discard;
}
```

## LoRA Adapter Switching

### Load and Apply

```rust
let adapter = model.lora_adapter_init("/path/to/adapter.gguf")?;

// Apply with full strength
ctx.lora_adapter_set(&mut adapter, 1.0)?;
// Generate with adapter active...

// Apply with half strength (blend with base model)
ctx.lora_adapter_set(&mut adapter, 0.5)?;
// Generate with partial adapter...
```

### Dynamic Switching

Switch between multiple LoRA adapters for different tasks:

```rust
let creative_adapter = model.lora_adapter_init("creative.gguf")?;
let technical_adapter = model.lora_adapter_init("technical.gguf")?;

// Creative writing mode
ctx.lora_adapter_set(&mut creative_adapter, 1.0)?;
// Generate creative text...

// Switch to technical mode
ctx.lora_adapter_remove(&mut creative_adapter)?;
ctx.lora_adapter_set(&mut technical_adapter, 1.0)?;
// Generate technical text...

// Remove all adapters (back to base model)
ctx.lora_adapter_remove(&mut technical_adapter)?;
```

### LoRA via ModelSpec

Using the `foundation_ai` type system:

```rust
use foundation_ai::types::{ModelSpec, ModelId, Quantization};

let spec = ModelSpec {
    name: "llama-2-7b-chat-lora".into(),
    id: ModelId::Name("llama-2-7b-chat".into(), Some(Quantization::Q4_KM)),
    devices: None,
    model_location: Some("/models/llama-2-7b-chat.Q4_K_M.gguf".into()),
    lora_location: Some("/models/my-adapter.gguf".into()),
};
```

## Session Persistence

### Save and Restore

Save the KV cache state to disk for resuming later:

```rust
// After processing some tokens
let processed_tokens: Vec<LlamaToken> = /* tokens processed so far */;
ctx.save_session_file("session.bin", &processed_tokens)?;

// Later, in a new process:
let mut ctx = model.new_context(&backend, ctx_params)?;
let cached_tokens = ctx.load_session_file("session.bin", 4096)?;
// KV cache is restored -- continue from where we left off
```

### Binary State Serialization

For custom persistence (e.g., sending over network):

```rust
// Serialize
let state_size = ctx.get_state_size();
let mut state_buffer = vec![0u8; state_size];
let written = unsafe { ctx.copy_state_data(state_buffer.as_mut_ptr()) };
state_buffer.truncate(written);

// Deserialize (in a fresh context of the same model)
let read = unsafe { ctx.set_state_data(&state_buffer) };
```

## Model Inspection

### Metadata Queries

```rust
println!("Parameters: {}", model.n_params());
println!("Embedding dim: {}", model.n_embd());
println!("Layers: {}", model.n_layer());
println!("Heads: {} (KV: {})", model.n_head(), model.n_head_kv());
println!("Vocab size: {}", model.n_vocab());
println!("Context length: {}", model.n_ctx_train());
println!("Model size: {} MB", model.size() / 1024 / 1024);
println!("Vocab type: {:?}", model.vocab_type());
println!("RoPE type: {:?}", model.rope_type());
println!("Recurrent: {}", model.is_recurrent());
```

### GGUF Metadata

```rust
let meta_count = model.meta_count();
for i in 0..meta_count {
    if let Ok(key) = model.meta_key_by_index(i) {
        if let Ok(val) = model.meta_val_str_by_index(i) {
            println!("{}: {}", key, val);
        }
    }
}
```

### Device Enumeration

```rust
use infrastructure_llama_cpp::list_llama_ggml_backend_devices;

let devices = list_llama_ggml_backend_devices();
for (i, dev) in devices.iter().enumerate() {
    println!("Device {}: {} ({}) -- {} MiB free",
        i, dev.name, dev.description,
        dev.memory_free / 1024 / 1024);
}
```

## Mirostat Sampling

Mirostat maintains a target "surprise" level (perplexity), producing more consistent output quality:

```rust
// Mirostat v2 (recommended)
let sampler = LlamaSampler::chain_simple([
    LlamaSampler::mirostat_v2(
        1234,   // seed
        5.0,    // tau (target surprise -- lower = more focused)
        0.1,    // eta (learning rate)
    ),
]);

// Mirostat v1
let sampler = LlamaSampler::chain_simple([
    LlamaSampler::mirostat(
        1234,   // seed
        5.0,    // tau
        0.1,    // eta
        100,    // m (number of tokens considered)
    ),
]);
```

## DRY (Don't Repeat Yourself) Sampling

Penalize repeated n-grams to reduce repetitive output:

```rust
let sampler = LlamaSampler::chain_simple([
    LlamaSampler::dry(
        &model,
        1.5,          // multiplier (penalty strength)
        1.75,         // base (exponential base for penalty growth)
        2,            // allowed_length (n-grams shorter than this are OK)
        64,           // penalty_last_n (look-back window)
        ["\n", ".", "!", "?"],  // sequence breakers (reset penalty)
    ),
    LlamaSampler::temp(0.8),
    LlamaSampler::dist(1234),
]);
```

## Performance Monitoring

```rust
use infrastructure_llama_cpp::ggml_time_us;

let t_start = ggml_time_us();

// ... generation loop ...

let t_end = ggml_time_us();
let duration = std::time::Duration::from_micros((t_end - t_start) as u64);

eprintln!(
    "Decoded {} tokens in {:.2}s, speed {:.2} t/s",
    n_tokens,
    duration.as_secs_f32(),
    n_tokens as f32 / duration.as_secs_f32()
);

// Detailed timings from llama.cpp
let timings = ctx.timings();
println!("{}", timings);
// Output:
// load time = 1234.56 ms
// prompt eval time = 567.89 ms / 512 tokens (...)
// eval time = 2345.67 ms / 128 runs (...)

// Reset timings for next measurement
ctx.reset_timings();
```

## Model Source Resolution

Using `foundation_ai` types to resolve model paths:

```rust
use foundation_ai::types::ModelSource;
use hf_hub::api::sync::ApiBuilder;

let source = ModelSource::HuggingFace("TheBloke/Llama-2-7B-Chat-GGUF".into());

let path = match source {
    ModelSource::LocalFile(path) => path,
    ModelSource::LocalDirectory(dir) => {
        // Find .gguf files in directory
        std::fs::read_dir(&dir)?
            .filter_map(|e| e.ok())
            .find(|e| e.path().extension().map_or(false, |ext| ext == "gguf"))
            .map(|e| e.path())
            .ok_or_else(|| anyhow::anyhow!("No .gguf file found in {:?}", dir))?
    }
    ModelSource::HuggingFace(repo) => {
        ApiBuilder::new()
            .with_progress(true)
            .build()?
            .model(repo)
            .get("model.Q4_K_M.gguf")?
    }
    ModelSource::HTTP(uri) => {
        // Download to cache and return path
        todo!("HTTP download not yet implemented")
    }
};
```

## Error Handling Patterns

### Decode Error Recovery

```rust
match ctx.decode(&mut batch) {
    Ok(()) => { /* Success */ }
    Err(DecodeError::NoKvCacheSlot) => {
        // Context is full -- shift or clear the KV cache
        let n_keep = 256;
        let n_discard = 128;
        ctx.clear_kv_cache_seq(
            Some(0), Some(n_keep as u32), Some((n_keep + n_discard) as u32)
        )?;
        ctx.kv_cache_seq_add(0, Some((n_keep + n_discard) as u32), None, -n_discard)?;
        // Retry decode
        ctx.decode(&mut batch)?;
    }
    Err(DecodeError::NTokensZero) => {
        // Empty batch -- nothing to do
    }
    Err(e) => return Err(e.into()),
}
```

### Graceful Backend Initialization

```rust
let backend = match LlamaBackend::init() {
    Ok(b) => b,
    Err(LlamaCppError::BackendAlreadyInitialized) => {
        // This is fine in tests or multi-component systems
        // The backend was already initialized elsewhere
        eprintln!("warning: backend already initialized");
        // Cannot obtain a second LlamaBackend handle -- this is a design limitation.
        // The backend must be initialized once and shared.
        return Err(anyhow::anyhow!("Backend already initialized"));
    }
    Err(e) => return Err(e.into()),
};
```

## Logging Configuration

```rust
use infrastructure_llama_cpp::{send_logs_to_tracing, LogOptions};

// Enable llama.cpp logs routed through Rust's tracing framework
tracing_subscriber::fmt::init();
send_logs_to_tracing(LogOptions::default().with_logs_enabled(true));

// Or silence all llama.cpp output
let mut backend = LlamaBackend::init()?;
backend.void_logs();
```

See [06-model-lifecycle.md](./06-model-lifecycle.md) for the end-to-end lifecycle and [11-integration-guide.md](./11-integration-guide.md) for implementing the `foundation_ai` backend.
