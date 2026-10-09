# 07 - Inference Pipeline

This document details the inference pipeline from tokenization through sampling, covering the mechanics of how tokens flow through the transformer and how the KV cache enables efficient autoregressive generation.

## Pipeline Overview

```
Input Text
    |
    v
[Tokenization] model.str_to_token()
    |
    v
Token IDs: [1, 15043, 590, 1024, ...]
    |
    v
[Batch Construction] LlamaBatch::add()
    |
    v
llama_batch { token, pos, seq_id, logits }
    |
    v
[Prefill / Decode] ctx.decode(&mut batch)
    |
    v
Transformer forward pass (attention, FFN, RoPE)
    |
    v
KV Cache updated with new key/value pairs
    |
    v
[Logit Extraction] ctx.get_logits() / ctx.candidates()
    |
    v
float[n_vocab] logit distribution
    |
    v
[Sampling] sampler.sample(&ctx, idx)
    |
    v
Selected token ID
    |
    v
[Detokenization] model.token_to_bytes(token)
    |
    v
Output bytes -> text
```

## Tokenization

### How Tokenization Works

The model's vocabulary defines how text is broken into tokens. Two main algorithms are supported:

**BPE (Byte Pair Encoding)**: Used by GPT-2, LLaMA 3, Mistral
- Starts with individual bytes/characters
- Iteratively merges the most frequent adjacent pairs
- Merge rules are stored in the GGUF file

**SPM (SentencePiece Model)**: Used by LLaMA 1/2, Gemma
- Treats text as a raw byte stream
- Uses unigram language model for subword segmentation
- Vocabulary and scores are stored in the GGUF file

```rust
// Check which tokenizer the model uses
match model.vocab_type() {
    VocabType::BPE => println!("Byte Pair Encoding"),
    VocabType::SPM => println!("SentencePiece Model"),
}
```

### Special Token Handling

```rust
// AddBos::Always prepends the beginning-of-sequence token
let tokens = model.str_to_token("Hello", AddBos::Always)?;
// Result: [BOS, Hello_token, ...]

// AddBos::Never skips it (use when you've already added it)
let tokens = model.str_to_token("Hello", AddBos::Never)?;
// Result: [Hello_token, ...]
```

The `str_to_token` implementation uses a two-pass approach: first attempt with an estimated buffer size, then retry with the exact size if needed:

```rust
let tokens_estimation = std::cmp::max(8, (str.len() / 2) + usize::from(add_bos));
let mut buffer: Vec<LlamaToken> = Vec::with_capacity(tokens_estimation);

let size = llama_tokenize(vocab_ptr, c_string.as_ptr(), ...);

if size.is_negative() {
    // Buffer too small, retry with exact size
    buffer.reserve_exact(-size as usize);
    llama_tokenize(vocab_ptr, c_string.as_ptr(), ...);
}
```

## Batch Construction

### Single-Token Batches

For autoregressive generation, each step processes a single token:

```rust
batch.clear();
batch.add(token, position, &[0], true)?;
//         ^      ^          ^     ^
//         |      |          |     └ logits=true (we need output)
//         |      |          └ sequence IDs
//         |      └ position in the sequence
//         └ token ID
```

### Prompt Prefill Batches

For the initial prompt, all tokens are processed at once but only the last token needs logits:

```rust
let last = (tokens.len() - 1) as i32;
for (i, token) in (0i32..).zip(tokens.iter()) {
    batch.add(*token, i, &[0], i == last)?;
}
```

This is efficient because:
1. All prompt tokens are processed in parallel (batched matrix multiplication)
2. KV pairs for all positions are stored in the cache
3. Only the last token's logits are computed (saving memory)

### Multi-Sequence Batches

For embedding models or parallel generation:

```rust
// Add sequence 0
batch.add_sequence(&tokens_0, 0, false)?;
// Add sequence 1
batch.add_sequence(&tokens_1, 1, false)?;
// Process both at once
ctx.decode(&mut batch)?;

// Get embeddings per sequence
let emb_0 = ctx.embeddings_seq_ith(0)?;
let emb_1 = ctx.embeddings_seq_ith(1)?;
```

### `LlamaBatch::get_one` for Simple Cases

```rust
// Zero-allocation wrapper around existing token slice
let batch = LlamaBatch::get_one(&tokens)?;
ctx.decode(&mut batch)?;
```

This does not allocate internal buffers -- it creates a `llama_batch` pointing directly to the token slice. The last token automatically has logits enabled.

## The Forward Pass (`ctx.decode`)

### What Happens Inside

When `ctx.decode(&mut batch)` is called:

1. **Token Embedding**: Each token ID is looked up in the embedding matrix to get a vector
2. **Position Encoding (RoPE)**: Rotary position embeddings are applied based on the `pos` values in the batch
3. **Attention Layers**: For each transformer layer:
   - **Query/Key/Value projection**: Input is projected to Q, K, V matrices
   - **KV Cache Update**: New K and V vectors are stored at their positions in the cache
   - **Attention Computation**: Q attends to all cached K/V pairs (including from previous decodes)
   - **Output Projection**: Attention output is projected back
4. **Feed-Forward Network**: MLP/MoE layer processes each position
5. **Final Norm + Logit Projection**: The last layer's output is normalized and projected to vocabulary size

### Return Values

```rust
match ctx.decode(&mut batch) {
    Ok(()) => { /* Success - logits are ready */ }
    Err(DecodeError::NoKvCacheSlot) => {
        // Context is full, need to manage cache
    }
    Err(DecodeError::NTokensZero) => {
        // Empty batch
    }
    Err(DecodeError::Unknown(code)) => {
        // Other error
    }
}
```

## KV Cache Mechanics

### How the KV Cache Works

The KV cache stores the key and value projections from attention computation:

```
Position:  0    1    2    3    4    5    ...    n_ctx-1
          [K0] [K1] [K2] [K3] [K4] [K5]  ...  [empty]
          [V0] [V1] [V2] [V3] [V4] [V5]  ...  [empty]
```

Each decode call adds new K/V entries at the specified positions. Subsequent tokens attend to all cached entries, avoiding recomputation of previous tokens.

### Cache Full: Position Shifting

When the cache fills up, shift positions to make room:

```rust
let n_ctx = ctx.n_ctx() as i32;
let n_keep = 256;  // Keep first N tokens (system prompt)
let n_discard = 128;  // Remove this many tokens

// Remove tokens in range [n_keep, n_keep + n_discard)
ctx.clear_kv_cache_seq(Some(0), Some(n_keep as u32), Some((n_keep + n_discard) as u32))?;

// Shift remaining positions down
ctx.kv_cache_seq_add(0, Some((n_keep + n_discard) as u32), None, -n_discard)?;
```

### Multi-Turn Conversation with Cache Reuse

For chat applications, keep the KV cache between turns:

```rust
// Turn 1: Process system + user message
let tokens_turn1 = model.str_to_token(&formatted_turn1, AddBos::Always)?;
let mut batch = LlamaBatch::new(512, 1);
batch.add_sequence(&tokens_turn1, 0, false)?;
ctx.decode(&mut batch)?;
// ... generate response, remember total position ...

// Turn 2: Only process new user message (cache has previous context)
let tokens_turn2 = model.str_to_token(&new_user_message, AddBos::Never)?;
batch.clear();
for (i, token) in tokens_turn2.iter().enumerate() {
    let pos = (previous_total_position + i) as i32;
    let is_last = i == tokens_turn2.len() - 1;
    batch.add(*token, pos, &[0], is_last)?;
}
ctx.decode(&mut batch)?;
// ... generate response ...
```

### Sequence Forking

Create parallel generation paths from a shared prefix:

```rust
// Process shared prefix on sequence 0
batch.add_sequence(&prefix_tokens, 0, false)?;
ctx.decode(&mut batch)?;

// Fork: copy sequence 0's cache to sequence 1
let prefix_len = prefix_tokens.len() as i32;
ctx.copy_kv_cache_seq(0, 1, Some(0), Some(prefix_len as u32))?;

// Now generate independently on both sequences
```

## Logit Extraction

After decode, logits are available for tokens that had `logits=true`:

```rust
// For the last token (most common case)
let logits: &[f32] = ctx.get_logits();
assert_eq!(logits.len(), model.n_vocab() as usize);

// For a specific position
let logits_i: &[f32] = ctx.get_logits_ith(position);

// As structured data for sampling
let candidates: LlamaTokenDataArray = ctx.token_data_array();
```

The `initialized_logits` vector in `LlamaContext` tracks which positions have valid logits, and `get_logits_ith` panics if you request an uninitialized position.

## Sampling

### Sampling Chain Execution Order

The sampling chain applies transformations in order. A typical chain:

```rust
let sampler = LlamaSampler::chain_simple([
    // 1. Repetition penalty (modifies logits based on history)
    LlamaSampler::penalties(64, 1.1, 0.0, 0.0),

    // 2. Top-K (keep only top K candidates)
    LlamaSampler::top_k(40),

    // 3. Top-P / Nucleus (keep candidates summing to P probability mass)
    LlamaSampler::top_p(0.95, 1),

    // 4. Temperature (scale logits)
    LlamaSampler::temp(0.8),

    // 5. Selection (must be last)
    LlamaSampler::dist(1234),  // Random weighted selection
    // or: LlamaSampler::greedy()  // Argmax
]);
```

### Sampling Strategies Explained

**Temperature**: Scales logits by `1/t`. Lower = more deterministic, higher = more random. At `t=0`, equivalent to greedy.

**Top-K**: Keeps only the K highest-probability tokens. Removes long-tail noise.

**Top-P (Nucleus)**: Keeps the smallest set of tokens whose cumulative probability >= P. Adapts to the distribution shape.

**Min-P**: Keeps tokens with probability >= P * max_probability. Filters relative to the best candidate.

**Mirostat**: Maintains a target "surprise" level (cross-entropy). Dynamically adjusts the candidate set to keep generation at a consistent perplexity.

**DRY**: "Don't Repeat Yourself" -- penalizes tokens that would create repeated n-grams. Uses sequence breakers (e.g., newline, period) to reset the penalty.

### Using `sampler.sample()`

```rust
// Sample from the last decoded position
let token = sampler.sample(&ctx, batch.n_tokens() - 1);

// Update internal state (for repetition tracking, grammar state, etc.)
sampler.accept(token);
```

`sampler.sample()` combines the apply-and-select steps: it extracts logits from the context, applies all chained transformations, and returns the selected token.

## Detokenization

### Token to Text

```rust
// To string (fails on invalid UTF-8)
let text = model.token_to_str(token, Special::Tokenize)?;

// To bytes (handles partial UTF-8 sequences)
let bytes = model.token_to_bytes(token, Special::Tokenize)?;
```

### Handling Partial UTF-8

Some tokens may produce partial UTF-8 sequences (e.g., multi-byte characters split across tokens). Use `encoding_rs::Decoder` for correct streaming output:

```rust
let mut decoder = encoding_rs::UTF_8.new_decoder();
let mut output = String::with_capacity(32);

let bytes = model.token_to_bytes(token, Special::Tokenize)?;
let _result = decoder.decode_to_string(&bytes, &mut output, false);
print!("{}", output);
```

The decoder buffers incomplete byte sequences and emits complete characters.

## Embedding Pipeline

For embedding models, the pipeline differs:

```rust
// 1. Configure for embeddings
let ctx_params = LlamaContextParams::default()
    .with_embeddings(true)
    .with_pooling_type(LlamaPoolingType::Mean);

// 2. Create context and batch
let mut ctx = model.new_context(&backend, ctx_params)?;
let mut batch = LlamaBatch::new(ctx.n_ctx() as usize, 1);

// 3. Add text as sequence
let tokens = model.str_to_token(text, AddBos::Always)?;
batch.add_sequence(&tokens, 0, false)?;

// 4. Process (using decode, not encode, for decoder-only embedding models)
ctx.clear_kv_cache();
ctx.decode(&mut batch)?;

// 5. Extract embeddings
let embeddings = ctx.embeddings_seq_ith(0)?;
// embeddings.len() == model.n_embd()
```

## Reranking Pipeline

For cross-encoder reranking models:

```rust
let ctx_params = LlamaContextParams::default()
    .with_embeddings(true)
    .with_pooling_type(LlamaPoolingType::Rank);

let mut ctx = model.new_context(&backend, ctx_params)?;

// Format query-document pairs
for (i, doc) in documents.iter().enumerate() {
    let pair = format!("{query}</s><s>{doc}");
    let tokens = model.str_to_token(&pair, AddBos::Always)?;

    let mut batch = LlamaBatch::new(2048, 1);
    batch.add_sequence(&tokens, i as i32, false)?;

    ctx.clear_kv_cache();
    ctx.decode(&mut batch)?;

    let score = ctx.embeddings_seq_ith(i as i32)?;
    println!("Document {}: score = {:.3}", i, score[0]);
}
```

## Performance Considerations

### Prompt Processing (Prefill)

- All prompt tokens are processed in a single batched operation
- Parallelism is controlled by `n_threads_batch`
- Larger `n_batch` allows more tokens per forward pass
- Memory usage scales with `n_ctx * n_embd * 2 * sizeof(kv_type)` for the KV cache

### Token Generation

- Each token requires a full forward pass but only processes 1 token
- KV cache avoids recomputing attention for previous tokens
- Parallelism is controlled by `n_threads`
- Generation speed is typically measured in tokens/second

### Timing

```rust
let timings = ctx.timings();
println!("Prompt: {:.1}ms ({} tokens, {:.1} t/s)",
    timings.t_p_eval_ms(),
    timings.n_p_eval(),
    1000.0 * timings.n_p_eval() as f64 / timings.t_p_eval_ms());
println!("Generation: {:.1}ms ({} tokens, {:.1} t/s)",
    timings.t_eval_ms(),
    timings.n_eval(),
    1000.0 * timings.n_eval() as f64 / timings.t_eval_ms());
```

See [08-advanced-features.md](./08-advanced-features.md) for LoRA, grammar, and multimodal extensions.
