# 12 - llama.cpp Examples: Comprehensive Deep Dive

## Overview

The llama.cpp `examples/` directory contains 30+ reference implementations demonstrating the full spectrum of inference capabilities. This document provides an exhaustive analysis of each meaningful example, extracting exact API patterns, initialization sequences, memory management strategies, and architectural decisions that inform the `foundation_ai` integration.

---

## 1. Simple Text Generation (`examples/simple/simple.cpp`)

**Source**: 224 lines of self-contained code — zero dependency on `common/` utilities.

### What It Demonstrates
The absolute minimal path from model file to generated text. No chat templates, no complex sampling, no common utilities. This is the canonical "how does llama.cpp actually work" reference.

### Complete Initialization Sequence

```cpp
// Step 1: Load dynamic compute backends (CUDA, Metal, Vulkan, CPU)
ggml_backend_load_all();

// Step 2: Configure and load model
llama_model_params model_params = llama_model_default_params();
model_params.n_gpu_layers = ngl;  // 99 = offload everything to GPU
llama_model * model = llama_model_load_from_file(model_path.c_str(), model_params);

// Step 3: Get vocabulary handle (separate from model)
const llama_vocab * vocab = llama_model_get_vocab(model);

// Step 4: Configure and create context
llama_context_params ctx_params = llama_context_default_params();
ctx_params.n_ctx = n_prompt + n_predict - 1;  // Exact size needed
ctx_params.n_batch = n_prompt;                  // Process entire prompt in one batch
ctx_params.no_perf = false;                     // Enable performance counters
llama_context * ctx = llama_init_from_model(model, ctx_params);

// Step 5: Create sampler chain with single greedy sampler
auto sparams = llama_sampler_chain_default_params();
sparams.no_perf = false;
llama_sampler * smpl = llama_sampler_chain_init(sparams);
llama_sampler_chain_add(smpl, llama_sampler_init_greedy());
```

### Two-Phase Tokenization Pattern

This is a critical pattern that every llama.cpp consumer must implement:

```cpp
// Phase 1: Probe for required buffer size (pass NULL, returns negative count)
const int n_prompt = -llama_tokenize(vocab, prompt.c_str(), prompt.size(), NULL, 0, true, true);

// Phase 2: Allocate and tokenize
std::vector<llama_token> prompt_tokens(n_prompt);
if (llama_tokenize(vocab, prompt.c_str(), prompt.size(), 
                   prompt_tokens.data(), prompt_tokens.size(), true, true) < 0) {
    // error
}
```

Parameters: `add_special=true` (add BOS), `parse_special=true` (recognize special tokens in text).

### The Generation Loop (Line-by-Line)

```cpp
// Create initial batch from all prompt tokens at once
llama_batch batch = llama_batch_get_one(prompt_tokens.data(), prompt_tokens.size());

// Handle encoder-decoder models (T5, BART, etc.)
if (llama_model_has_encoder(model)) {
    llama_encode(ctx, batch);  // Run encoder
    llama_token decoder_start = llama_model_decoder_start_token(model);
    if (decoder_start == LLAMA_TOKEN_NULL) {
        decoder_start = llama_vocab_bos(vocab);
    }
    batch = llama_batch_get_one(&decoder_start, 1);  // Decoder starts with start token
}

// Main autoregressive loop
for (int n_pos = 0; n_pos + batch.n_tokens < n_prompt + n_predict; ) {
    // 1. Evaluate batch through transformer
    llama_decode(ctx, batch);
    n_pos += batch.n_tokens;

    // 2. Sample next token from last position's logits
    //    idx=-1 means "last token in batch"
    new_token_id = llama_sampler_sample(smpl, ctx, -1);

    // 3. Check end-of-generation (EOS, EOT, or model-specific EOG tokens)
    if (llama_vocab_is_eog(vocab, new_token_id)) break;

    // 4. Detokenize for output
    char buf[128];
    int n = llama_token_to_piece(vocab, new_token_id, buf, sizeof(buf), 0, true);
    printf("%s", std::string(buf, n).c_str());
    fflush(stdout);  // Streaming output

    // 5. Create single-token batch for next iteration
    batch = llama_batch_get_one(&new_token_id, 1);
}
```

### Resource Cleanup Order

```cpp
llama_sampler_free(smpl);   // Free sampler chain
llama_free(ctx);             // Free context (KV cache, etc.)
llama_model_free(model);     // Free model weights
// ggml backends freed implicitly
```

### Key Insight: `llama_batch_get_one`
This is a lightweight batch constructor — it creates a batch that references the token array directly (no copy). It assumes:
- All tokens belong to sequence 0
- Positions are sequential starting from 0
- Only the last token has logits enabled

### Foundation_ai Integration Notes
- Our `LlamaModels::generate()` should follow this exact loop
- The `llama_batch_get_one` shortcut works for single-sequence generation
- For multi-sequence/parallel generation, use `llama_batch_init()` + `common_batch_add()` instead
- The encoder-decoder path must be supported for T5-style models

---

## 2. Interactive Chat (`examples/simple-chat/simple-chat.cpp`)

**Source**: 211 lines, still no `common/` dependency (except for `llama.h`).

### What It Demonstrates
Multi-turn conversational chat with:
- Chat template discovery from model metadata (GGUF embedded Jinja2 templates)
- Incremental prompt formatting (only tokenize the new part each turn)
- Stateful KV cache management across turns
- Production-quality sampler chain (min_p → temperature → distribution)

### Chat Template System (The Core Innovation)

```cpp
// Get the model's built-in chat template (stored in GGUF metadata)
const char * tmpl = llama_model_chat_template(model, /* name */ nullptr);
// name=nullptr gets the default template
// name="rerank" gets a specialized reranking template if available

// Format messages using the template
std::vector<llama_chat_message> messages;
messages.push_back({"user", strdup(user.c_str())});

// Apply template to all messages, with generation prompt
std::vector<char> formatted(llama_n_ctx(ctx));
int new_len = llama_chat_apply_template(
    tmpl,              // Template string (Jinja2-style)
    messages.data(),   // Array of {role, content} structs
    messages.size(),   // Number of messages
    true,              // add_ass = true → append assistant prompt marker
    formatted.data(),  // Output buffer
    formatted.size()   // Buffer size
);
```

### Incremental Formatting Strategy

This is a crucial optimization — instead of re-tokenizing the entire conversation each turn:

```cpp
int prev_len = 0;

// Each turn:
// 1. Format ALL messages (including new one)
int new_len = llama_chat_apply_template(tmpl, messages.data(), messages.size(), true, ...);

// 2. Extract ONLY the new part (delta from previous formatting)
std::string prompt(formatted.begin() + prev_len, formatted.begin() + new_len);

// 3. Generate response
std::string response = generate(prompt);

// 4. Track where we are for next turn
messages.push_back({"assistant", strdup(response.c_str())});
prev_len = llama_chat_apply_template(tmpl, messages.data(), messages.size(), false, nullptr, 0);
// add_ass=false here because we don't want the assistant prompt after storing a complete response
```

### Sampler Chain for Chat

```cpp
llama_sampler * smpl = llama_sampler_chain_init(llama_sampler_chain_default_params());
llama_sampler_chain_add(smpl, llama_sampler_init_min_p(0.05f, 1));     // Filter unlikely tokens
llama_sampler_chain_add(smpl, llama_sampler_init_temp(0.8f));           // Add randomness
llama_sampler_chain_add(smpl, llama_sampler_init_dist(LLAMA_DEFAULT_SEED)); // Sample from distribution
```

**Order matters**: min_p filters first (removes tokens below 5% of max probability), temperature scales remaining logits, distribution samples from the result.

### Context Window Management

```cpp
const bool is_first = llama_memory_seq_pos_max(llama_get_memory(ctx), 0) == -1;
// Returns -1 when sequence 0 has no tokens yet

// Check context overflow before decoding
int n_ctx_used = llama_memory_seq_pos_max(llama_get_memory(ctx), 0) + 1;
if (n_ctx_used + batch.n_tokens > n_ctx) {
    // Context full — must handle (truncate, shift, or error)
    exit(0);
}
```

### BOS Token Handling

```cpp
// First turn: add BOS (is_first=true)
// Subsequent turns: no BOS (already in KV cache from first turn)
const int n_prompt_tokens = -llama_tokenize(vocab, prompt.c_str(), prompt.size(), NULL, 0, is_first, true);
```

### Foundation_ai Integration Notes
- Our `ModelInteraction` → `LlamaChatTemplate` pipeline maps directly to this
- The incremental formatting pattern is essential for efficiency — don't re-tokenize history
- `llama_model_chat_template()` returns NULL if no template in GGUF; must handle fallback
- The `prev_len` tracking pattern should be encapsulated in our chat state management

---

## 3. Embeddings (`examples/embedding/embedding.cpp`)

**Source**: 415 lines using `common/` utilities extensively.

### What It Demonstrates
Production-grade embedding extraction with:
- Batch processing of multiple texts in parallel
- All pooling strategies (NONE, MEAN, CLS, LAST, RANK)
- OpenAI-compatible JSON output format
- Cosine similarity computation
- Reranking support with classification labels

### Embedding-Specific Initialization

```cpp
params.embedding = true;  // CRITICAL: Switches context to embedding mode

// Unified KV cache for arbitrary number of sequences
if (params.n_parallel == 1) {
    params.kv_unified = true;
    params.n_parallel = llama_max_parallel_sequences();  // Get hardware max
}

// Batch size should equal context size for full utilization
params.n_batch = params.n_ctx;

// Non-causal models (BERT-style) require n_ubatch == n_batch
if (params.attention_type != LLAMA_ATTENTION_TYPE_CAUSAL) {
    params.n_ubatch = params.n_batch;
}
```

### Multi-Sequence Batching

```cpp
// Each input text gets its own sequence ID
static void batch_add_seq(llama_batch & batch, const std::vector<int32_t> & tokens, llama_seq_id seq_id) {
    for (size_t i = 0; i < tokens.size(); i++) {
        common_batch_add(batch, tokens[i], i, { seq_id }, true);
        //                      token_id, position, seq_ids, compute_logits
    }
}

// Note: compute_logits=true for ALL tokens in embedding mode
// This is different from generation where only the last token needs logits
```

### Batch Decode with Pooling

```cpp
static void batch_decode(llama_context * ctx, llama_batch & batch, float * output, 
                          int n_seq, int n_embd_out, int embd_norm) {
    const enum llama_pooling_type pooling_type = llama_pooling_type(ctx);
    
    // CRITICAL: Clear KV cache before each batch of sequences
    // Embeddings don't need prior context
    llama_memory_clear(llama_get_memory(ctx), true);
    
    llama_decode(ctx, batch);
    
    for (int i = 0; i < batch.n_tokens; i++) {
        if (!batch.logits[i]) continue;
        
        const float * embd = nullptr;
        int embd_pos = 0;
        
        if (pooling_type == LLAMA_POOLING_TYPE_NONE) {
            // Per-token embeddings (no pooling)
            embd = llama_get_embeddings_ith(ctx, i);
            embd_pos = i;
        } else {
            // Sequence-level pooled embedding
            embd = llama_get_embeddings_seq(ctx, batch.seq_id[i][0]);
            embd_pos = batch.seq_id[i][0];
        }
        
        // Normalize: L2 (default), L1, max, p-norm, or none
        common_embd_normalize(embd, out, n_embd_out, embd_norm);
    }
}
```

### Output Dimension Detection

```cpp
const int n_embd_out = llama_model_n_embd_out(model);  // Embedding dimension

// For classification/ranking models:
const uint32_t n_cls_out = llama_model_n_cls_out(model);  // Number of classes
const char * label = llama_model_cls_label(model, i);       // Class label string
```

### Batch Overflow Handling

```cpp
// Process inputs in chunks that fit within n_batch
int e = 0;  // embeddings stored so far
int s = 0;  // sequences in current batch
for (int k = 0; k < n_prompts; k++) {
    auto & inp = inputs[k];
    
    // Flush batch if adding this input would overflow
    if (batch.n_tokens + inp.size() > n_batch || s >= n_seq_max) {
        batch_decode(ctx, batch, emb + e * n_embd_out, s, n_embd_out, norm);
        e += (pooling == NONE) ? batch.n_tokens : s;
        s = 0;
        common_batch_clear(batch);
    }
    
    batch_add_seq(batch, inp, s);
    s += 1;
}
// Process final batch
batch_decode(ctx, batch, emb + e * n_embd_out, s, n_embd_out, norm);
```

### Reranking Support

```cpp
// Reranking uses LLAMA_POOLING_TYPE_RANK with query-document pairs
if (pooling_type == LLAMA_POOLING_TYPE_RANK && prompt.find(cls_sep) != std::string::npos) {
    // Try model's rerank template first
    const char * rerank_prompt = llama_model_chat_template(model, "rerank");
    if (rerank_prompt != nullptr) {
        // Template with {query} and {document} placeholders
        string_replace_all(final_prompt, "{query}", query);
        string_replace_all(final_prompt, "{document}", doc);
    } else {
        // Fallback: concatenate with EOS/SEP tokens between
        final_prompt = query + eos_token + sep_token + doc;
    }
}
```

### Foundation_ai Integration Notes
- `ModelOutput::Embedding { dimensions, values }` must handle all pooling types
- Batch processing is fundamental — our API should accept `Vec<String>` inputs
- `llama_memory_clear()` between batches is mandatory for embeddings
- The `n_ubatch = n_batch` constraint for non-causal models must be enforced
- Reranking is a distinct use case — our API should support it via `ModelInteraction`

---

## 4. Parallel Request Handling (`examples/parallel/parallel.cpp`)

**Source**: 521 lines — simulates a multi-client inference server.

### What It Demonstrates
The canonical pattern for serving multiple concurrent inference requests:
- System prompt sharing via KV cache copying
- Continuous batching (new requests inserted while others generate)
- Per-client sampling with independent state
- Sequence-based KV cache management
- Batch view slicing for large batches
- Dynamic batch size recovery on KV cache overflow

### Client State Structure

```cpp
struct client {
    int32_t id = 0;
    llama_seq_id seq_id = -1;  // -1 = idle
    llama_token sampled;
    int32_t n_past = 0;        // Position in KV cache
    int32_t n_prompt = 0;
    int32_t n_decoded = 0;
    int32_t i_batch = -1;      // Position in current batch
    std::string input, prompt, response;
    struct common_sampler * smpl = nullptr;  // Per-client sampler
};
```

### System Prompt Sharing (KV Cache Copy)

```cpp
// Evaluate system prompt once in sequence 0
for (int32_t i = 0; i < n_tokens_system; ++i) {
    common_batch_add(batch, tokens_system[i], i, { 0 }, false);
}
llama_decode(ctx, batch);

// Copy sequence 0's KV cache to all client sequences
auto * mem = llama_get_memory(ctx);
for (int32_t i = 1; i <= n_clients; ++i) {
    llama_memory_seq_cp(mem, 0, i, -1, -1);
    // src_seq=0, dst_seq=i, p0=-1 (start), p1=-1 (end) = copy ALL positions
}
```

### The Main Server Loop

```cpp
while (true) {
    common_batch_clear(batch);
    
    // 1. Add sampled tokens from active clients to batch
    for (auto & client : clients) {
        if (client.seq_id == -1) continue;  // Skip idle clients
        client.i_batch = batch.n_tokens;     // Remember position in batch
        common_batch_add(batch, client.sampled, client.n_past++, { client.id + 1 }, true);
        client.n_decoded += 1;
    }
    
    // 2. Insert new requests (continuous batching)
    if (cont_batching || batch.n_tokens == 0) {
        for (auto & client : clients) {
            if (client.seq_id == -1 && g_seq_id < n_seq) {
                // Tokenize new request
                std::vector<llama_token> tokens = common_tokenize(ctx, client.prompt, false);
                for (size_t i = 0; i < tokens.size(); ++i) {
                    common_batch_add(batch, tokens[i], client.n_past++, { client.id + 1 }, false);
                }
                // Only last token needs logits
                batch.logits[batch.n_tokens - 1] = true;
                client.i_batch = batch.n_tokens - 1;
            }
        }
    }
    
    // 3. Process batch in chunks (batch view slicing)
    for (int32_t i = 0; i < batch.n_tokens; i = i_next) {
        const int32_t n_tokens = std::min(n_batch, batch.n_tokens - i);
        
        // Create a VIEW into the batch (no copy, just pointer offsets)
        llama_batch batch_view = {
            n_tokens,
            batch.token    + i,
            nullptr,               // no embeddings
            batch.pos      + i,
            batch.n_seq_id + i,
            batch.seq_id   + i,
            batch.logits   + i,
        };
        
        const int ret = llama_decode(ctx, batch_view);
        if (ret != 0) {
            if (n_batch == 1 || ret < 0) {
                // Unrecoverable
                return 1;
            }
            // KV cache full — retry with smaller batch
            n_cache_miss += 1;
            n_batch /= 2;
            continue;
        }
        
        i_next = i + n_tokens;
        n_batch = params.n_batch;  // Restore original batch size on success
        
        // 4. Sample for each client whose logit position falls in this sub-batch
        for (auto & client : clients) {
            if (client.i_batch < i || client.i_batch >= i + n_tokens) continue;
            
            // Note: logits index is relative to batch_view, not original batch
            const llama_token id = common_sampler_sample(client.smpl, ctx, client.i_batch - i);
            common_sampler_accept(client.smpl, id, true);
            
            client.response += common_token_to_piece(ctx, id);
            client.sampled = id;
            
            // Check completion conditions
            if (llama_vocab_is_eog(vocab, id) || client.n_decoded >= n_predict) {
                // Clean up: remove client's KV cache, restore system prompt
                llama_memory_seq_rm(mem, client.id + 1, -1, -1);
                llama_memory_seq_cp(mem, 0, client.id + 1, -1, -1);
                client.seq_id = -1;  // Mark idle
            }
        }
    }
}
```

### Sequence Lifecycle Management

```
New Request  → llama_memory_seq_cp(mem, 0, client_id, -1, -1)    // Inherit system prompt
Generation   → Normal decode + sample loop (tokens accumulate in KV cache)
Completion   → llama_memory_seq_rm(mem, client_id, -1, -1)       // Free all KV cache
             → llama_memory_seq_cp(mem, 0, client_id, -1, -1)    // Reset with system prompt
Idle         → client.seq_id = -1                                  // Available for next request
```

### Foundation_ai Integration Notes
- This is the pattern for `LlamaBackends` serving multiple concurrent requests
- System prompt sharing is critical for chat applications (saves ~50% KV cache per request)
- The batch view slicing pattern avoids allocations while respecting `n_batch` limits
- Per-client samplers enable independent generation parameters (temperature, etc.)
- The dynamic batch size halving is a practical KV cache pressure relief valve

---

## 5. Batched Parallel Generation (`examples/batched/batched.cpp`)

**Source**: 265 lines — simpler than `parallel/`, focused on divergent generation from shared prompt.

### What It Demonstrates
Generate N independent completions from the same prompt, each with different sampling. Useful for:
- Best-of-N sampling (generate N, pick best)
- Beam search approximation
- A/B testing different sampling parameters

### Backend Sampling (Experimental GPU-Side Sampling)

```cpp
// Create per-sequence sampler configs
std::vector<llama_sampler_seq_config> sampler_configs;
for (int32_t i = 0; i < n_parallel; ++i) {
    llama_sampler * smpl = llama_sampler_chain_init(sparams);
    llama_sampler_chain_add(smpl, llama_sampler_init_top_k(params.sampling.top_k));
    llama_sampler_chain_add(smpl, llama_sampler_init_top_p(params.sampling.top_p, min_keep));
    llama_sampler_chain_add(smpl, llama_sampler_init_temp(params.sampling.temp));
    llama_sampler_chain_add(smpl, llama_sampler_init_dist(params.sampling.seed));
    
    sampler_configs.push_back({ i, smpl });  // { seq_id, sampler }
}

// Optional: Pass samplers to context for GPU-accelerated sampling
if (params.sampling.backend_sampling) {
    ctx_params.samplers = sampler_configs.data();
    ctx_params.n_samplers = sampler_configs.size();
}
```

### Shared Prompt + Divergent Generation

```cpp
// All sequences share the same prompt tokens
std::vector<llama_seq_id> seq_ids(n_parallel);
for (int32_t i = 0; i < n_parallel; ++i) seq_ids[i] = i;

// Initial batch: same tokens assigned to ALL sequence IDs
for (size_t i = 0; i < tokens_list.size(); ++i) {
    common_batch_add(batch, tokens_list[i], i, seq_ids, false);
    //                                          ^^^^^^^ all sequences!
}
// Only compute logits for last prompt token
batch.logits[batch.n_tokens - 1] = true;

llama_decode(ctx, batch);

// After prompt: each sequence diverges independently
// Track each sequence's position in the batch
std::vector<int32_t> i_batch(n_parallel, batch.n_tokens - 1);

while (n_cur <= n_predict) {
    common_batch_clear(batch);
    
    for (int32_t i = 0; i < n_parallel; ++i) {
        if (i_batch[i] < 0) continue;  // Finished
        
        // Sample from sequence i's logits position
        const llama_token new_token = llama_sampler_sample(
            sampler_configs[i].sampler, ctx, i_batch[i]);
        
        if (llama_vocab_is_eog(vocab, new_token)) {
            i_batch[i] = -1;
            continue;
        }
        
        // Each sequence gets its own token at the same position
        i_batch[i] = batch.n_tokens;
        common_batch_add(batch, new_token, n_cur, { i }, true);
        //                                         ^^^ only this sequence
    }
    
    if (batch.n_tokens == 0) break;
    n_cur += 1;
    llama_decode(ctx, batch);
}
```

### KV Cache Size Calculation

```cpp
// Shared prompt + divergent generations
const int n_kv_req = tokens.size() + (n_predict - tokens.size()) * n_parallel;
// Example: 10 prompt tokens + (100 - 10) * 4 parallel = 10 + 360 = 370 KV entries
```

### Foundation_ai Integration Notes
- Exposes "best-of-N" or "multiple completions" as a generation parameter
- Shared prompt evaluation is automatic when tokens are assigned to multiple seq_ids
- Backend sampling could offer GPU-accelerated sampling in future
- `llama_batch_init(size, 0, n_parallel)` — third param is `n_seq_max` per token

---

## 6. Speculative Decoding — Simple (`examples/speculative-simple/`)

**Source**: 270 lines — uses `common/speculative.h` helper abstractions.

### What It Demonstrates
Accelerated inference using a small draft model to propose tokens, verified by the large target model. 2-3x speedup when draft model has good acceptance rate.

### Dual Model Setup

```cpp
// Load target model (large, accurate)
auto llama_init_tgt = common_init_from_params(params);
model_tgt = llama_init_tgt->model();
ctx_tgt = llama_init_tgt->context();

// Load draft model (small, fast)
auto params_dft = params;
params_dft.n_parallel = 1;
params_dft.n_ctx = params_spec.n_ctx;
params_dft.n_batch = llama_n_ctx_seq(ctx_tgt);
params_dft.model = params_spec.mparams_dft;
params_dft.n_gpu_layers = params_spec.n_gpu_layers;

model_dft.reset(llama_model_load_from_file(params_dft.model.path.c_str(), mparams_dft));
params.speculative.model_dft = model_dft.get();
```

### Speculative Decoding Loop

```cpp
// Initialize speculator with draft model context
struct common_speculative * spec = common_speculative_init(params.speculative, ctx_tgt);
common_speculative_begin(spec, prompt_tgt);

while (true) {
    // Step 1: Draft tokens using small model
    llama_tokens draft = common_speculative_draft(spec, params_spec, prompt_tgt, id_last);
    
    // Step 2: Build target batch: [last_accepted, draft0, draft1, ..., draftN-1]
    common_batch_clear(batch_tgt);
    common_batch_add(batch_tgt, id_last, n_past++, { 0 }, true);
    for (size_t i = 0; i < draft.size(); ++i) {
        common_batch_add(batch_tgt, draft[i], n_past + i, { 0 }, true);
    }
    
    // Step 3: Evaluate ALL tokens in target model in single forward pass
    llama_decode(ctx_tgt, batch_tgt);
    
    // Step 4: Verify draft tokens against target model
    // Returns accepted tokens — always at least 1 (the non-speculative sample)
    const auto ids = common_sampler_sample_and_accept_n(smpl, ctx_tgt, draft);
    
    // Step 5: Update state
    n_past += ids.size() - 1;
    n_drafted += draft.size();
    n_accept += ids.size() - 1;
    
    // Step 6: Output accepted tokens
    for (size_t i = 0; i < ids.size(); ++i) {
        prompt_tgt.push_back(id_last);
        id_last = ids[i];
        if (llama_vocab_is_eog(vocab, id_last)) { has_eos = true; break; }
        printf("%s", common_token_to_piece(ctx_tgt, id_last).c_str());
    }
    
    // Step 7: Clean up rejected tokens from KV cache
    llama_memory_seq_rm(llama_get_memory(ctx_tgt), 0, n_past, -1);
    
    if (n_predict_reached || has_eos) break;
}

// Final metrics
LOG_INF("n_drafted = %d\n", n_drafted);
LOG_INF("n_accept  = %d\n", n_accept);
LOG_INF("accept    = %.3f%%\n", 100.0f * n_accept / n_drafted);
```

### How Verification Works

The key insight: draft tokens are evaluated by the target model in a **single batch**. For each draft token position, the target model's logits are compared against the draft model's prediction:

1. If the target model would have sampled the same token → **accept** (free token!)
2. If not → **reject**, sample from target model's distribution at this position, discard remaining draft tokens

This preserves the target model's output distribution exactly — speculative decoding is mathematically equivalent to normal decoding, just faster.

### Foundation_ai Integration Notes
- Speculative decoding is purely a performance optimization — transparent to the API consumer
- Requires two compatible models (same vocabulary)
- Our `LlamaBackendConfig` should support optional draft model configuration
- The `common_speculative_*` helpers encapsulate the complexity well

---

## 7. Full Speculative Decoding (`examples/speculative/speculative.cpp`)

### What It Adds Over Simple
- **Tree-based drafting**: Multiple parallel draft branches
- **Stochastic acceptance**: `r <= p_target / p_draft` preserves target distribution
- **Branch splitting**: When confidence drops below threshold, fork into new branches
- **Complex sequence management**: Per-branch KV cache copies/removals

This is significantly more complex (~500 lines) and demonstrates the most advanced speculative decoding strategy.

---

## 8. Retrieval / RAG Pipeline (`examples/retrieval/retrieval.cpp`)

### What It Demonstrates
Complete document retrieval pipeline:
1. Read files → chunk by separator → tokenize chunks
2. Batch-encode all chunks into embeddings
3. Interactive query: encode query → cosine similarity search → return top-K

### Chunking Strategy

```cpp
struct chunk {
    std::string filename;
    size_t filepos;
    std::string textdata;
    std::vector<llama_token> tokens;
    std::vector<float> embedding;
};

// Split on separator (e.g., "."), accumulate until chunk_size exceeded
static std::vector<chunk> chunk_file(const std::string & filename, 
                                      int chunk_size, 
                                      const std::string & separator) {
    // Reads file in 1024-byte buffers
    // Splits on separator
    // Accumulates text until chunk_size exceeded
    // Leftovers appended to last chunk
}
```

### Similarity Search

```cpp
// Flat array: embeddings[chunk_id * n_embd] → embeddings[(chunk_id+1) * n_embd]
// Query encoded with same model, same normalization
float sim = common_embd_similarity_cos(emb_query, emb_chunk, n_embd);
// Sort by similarity, return top-K
```

### Foundation_ai Integration Notes
- RAG is a first-class use case for embeddings
- Our embedding API should support batch processing with chunked input
- Cosine similarity should be a utility function in `foundation_ai`

---

## 9. State Save/Load (`examples/save-load-state/save-load-state.cpp`)

### What It Demonstrates
KV cache serialization for session persistence, cross-context state transfer, and deterministic replay.

### Full Context State

```cpp
// Save: entire context state (KV cache + RNG + metadata)
llama_state_save_file(ctx, "state.bin", tokens, n_tokens);

// Load: into a NEW context (must be same model)
llama_context * ctx2 = llama_init_from_model(model, ctx_params);
llama_state_load_file(ctx2, "state.bin", tokens_out, capacity, &count);
// Now ctx2 can continue generation exactly where ctx left off
```

### Per-Sequence State (Fine-Grained)

```cpp
// Extract just one sequence's KV cache
size_t size = llama_state_seq_get_size(ctx, seq_id);
std::vector<uint8_t> buf(size);
llama_state_seq_get_data(ctx, buf.data(), buf.size(), seq_id);

// Restore to a DIFFERENT sequence in the same or different context
llama_memory_clear(llama_get_memory(ctx3), true);
llama_state_seq_set_data(ctx3, buf.data(), buf.size(), new_seq_id);
```

### Deterministic Replay

The example verifies that:
1. `result0` (direct generation)
2. `result1` (generation after full state save/load)
3. `result2` (generation after per-sequence state save/load to different seq_id)

All produce **identical output** when using the same seed and sampler.

### Foundation_ai Integration Notes
- State save/load enables conversation caching, session persistence, and checkpoint/resume
- Our API should expose `save_state()` / `load_state()` on `LlamaModels`
- Per-sequence state is useful for branching conversations (fork a conversation to explore different paths)

---

## 10. Lookahead Decoding (`examples/lookahead/lookahead.cpp`)

### Key Architecture
- Self-speculative: uses the model's own n-gram predictions as drafts
- No draft model required
- Parameters: W (lookahead window, e.g., 15), N (n-gram size, e.g., 5), G (max verification n-grams, e.g., 15)
- Needs `W + G + 1` parallel sequences
- Uses sparse logits computation — only compute where needed
- N-gram ring buffer stores observed patterns for candidate matching

---

## 11. Prompt Lookup Decoding (`examples/lookup/lookup.cpp`)

### Key Architecture
- Pure table-lookup drafting — no neural network for drafts
- Three cache tiers: context (from prompt), static (pre-computed file), dynamic (learned)
- `common_ngram_cache_draft()` searches caches for matching n-grams
- Cache persistence: `common_ngram_cache_save()` / `common_ngram_cache_load()`
- Highly effective for repetitive/templated content

---

## 12. Eval Callback (`examples/eval-callback/eval-callback.cpp`)

### Key Architecture
- Register callback via `params.cb_eval = callback_fn`
- Callback fires for each node in the computation graph during `llama_decode()`
- Provides: node name, tensor shapes, data types, actual tensor values
- Useful for debugging attention patterns, profiling layer performance
- Our API could expose this as an optional diagnostic mode

---

## 13. Training / Fine-tuning (`examples/training/finetune.cpp`)

### Key Architecture
- `common_opt_dataset_init()` creates training dataset from tokenized text
- `llama_opt_init()` initializes optimizer (SGD, Adam)
- `llama_opt_epoch()` runs one training pass with progress callbacks
- Requires F32 KV cache (for gradients)
- `llama_model_save_to_file()` persists fine-tuned weights
- Significant memory requirements — F32 model + F32 KV cache

---

## 14. Diffusion LLM (`examples/diffusion/diffusion-cli.cpp`)

### Key Architecture
- Non-autoregressive generation: `llama_set_causal_attn(ctx, false)`
- Mask token initialization: sequence starts as `[input_tokens, MASK, MASK, ..., MASK]`
- Iterative refinement: each step unmasques highest-confidence positions
- Confidence algorithms: CONFIDENCE_BASED, ENTROPY_BASED, MARGIN_BASED, RANDOM
- Transfer schedules: TIMESTEP_BASED (Dream), BLOCK_BASED (LLaDA)
- Full sequence decoded each step (not incremental)

```cpp
enum diffusion_algorithm { ORIGIN, ENTROPY_BASED, MARGIN_BASED, RANDOM, CONFIDENCE_BASED };
enum transfer_schedule { TIMESTEP_BASED, BLOCK_BASED };

// Example confidence calculation
static float calculate_confidence(const llama_token_data_array & cur_p, 
                                   diffusion_algorithm alg, std::mt19937 & rng) {
    switch (alg) {
        case CONFIDENCE_BASED: return cur_p.data[cur_p.selected].p;
        case ENTROPY_BASED: {
            float entropy = 0.0f;
            for (size_t i = 0; i < cur_p.size; i++) {
                float prob = cur_p.data[i].p;
                entropy += prob * logf(prob + 1e-10f);
            }
            return -entropy;
        }
        case MARGIN_BASED: /* top1_prob - top2_prob */ break;
        case RANDOM: /* uniform random */ break;
    }
}
```

---

## 15. ReAct Pattern (`examples/reason-act.sh`)

### What It Demonstrates
A minimal Reason-Act agent loop using llama-cli's interactive mode.

```bash
./llama-cli $MODEL --color \
    -f ./prompts/reason-act.txt \      # External prompt file with ReAct format
    -i --interactive-first \            # Start in interactive mode
    --top_k 10000 --temp 0.2 \         # Low temperature for reasoning
    --repeat_penalty 1 \               # No repeat penalty
    -t 7 -c 2048 \                     # 7 threads, 2048 context
    -r "Question:" -r "Observation:" \ # Reverse prompts: hand control back to user
    --in-prefix " " \                  # Space before user input
    -n -1                              # Unlimited generation
```

The reverse prompt (`-r`) feature pauses generation when the model outputs "Question:" or "Observation:", allowing external tool execution before continuing. This is the simplest form of tool-augmented generation.

---

## API Pattern Summary

| Example | Initialization | Batching | Sampling | Memory Mgmt | Key Innovation |
|---------|---------------|----------|----------|-------------|----------------|
| simple | Manual 5-step | `batch_get_one` | Greedy chain | None | Minimal baseline |
| simple-chat | Manual + template | `batch_get_one` | min_p+temp+dist | Stateful KV | Incremental formatting |
| embedding | `common_init` | Multi-seq `batch_init` | None | Clear per batch | Pooling strategies |
| parallel | `common_init` | Batch view slicing | Per-client samplers | Seq copy/remove | System prompt sharing |
| batched | `common_init` | Multi-seq shared prompt | Per-seq samplers | Shared prompt KV | Backend sampling |
| speculative-simple | Dual model init | Draft+target batch | `sample_and_accept_n` | Seq remove rejected | Draft acceleration |
| speculative | Dual model init | Tree branching | Stochastic acceptance | Branch copy/prune | Tree speculation |
| retrieval | `common_init` + embed | Multi-chunk batching | None | Clear per batch | Chunked RAG |
| save-load-state | Dual context | Standard | Standard | State serialize | Session persistence |
| lookahead | Many sequences | Sparse logits | N-gram verification | Complex seq ops | Self-speculation |
| lookup | Standard | With cache drafts | Standard | Standard | Table-lookup drafts |
| training | F32 KV cache | Dataset batches | N/A | F32 requirement | Gradient computation |
| diffusion | Non-causal attn | Full sequence | Confidence-based | Mask tracking | Non-autoregressive |

---

_Created: 2026-04-07_
_Source: Direct analysis of llama.cpp examples/ source code_
