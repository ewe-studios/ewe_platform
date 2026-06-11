# Decision 06: Embedding API Design

**Status:** Proposed  
**Date:** 2026-06-11  
**Context:** Specification 36 — Agentic API for foundation_ai

## Problem

Embeddings are required for semantic recall across the memory hierarchy (ObservationMemory, Message API). Embedding generation is expensive (model inference), and identical texts are frequently embedded multiple times. Embedding models produce vectors of different dimensions, making cross-model mixing dangerous.

## Decision

The Embedding API is a shared, queue-backed service with LRU caching, dimension isolation, and lazy generation. It runs as a valtron task and is accessed via `Arc`-shared state.

### Architecture

```
EmbeddingProvider { inner: Arc<EmbeddingProviderInner> }
├── request_queue: Arc<ConcurrentQueue<EmbeddingRequest>>
├── cache: Arc<LruCache<EmbeddingKey, EmbeddingVector>>
├── dimension_registry: DimensionRegistry
├── generation_task: Entry  // valtron task that processes requests
└── cache_backing: Option<FjallKeyspace>  // optional persistent cache
```

### LRU Cache

| Property | Value | Rationale |
|----------|-------|-----------|
| Max size | 1,000 entries | Bounded memory — 1,000 embeddings × ~768 floats × 4 bytes ≈ 3MB |
| Eviction policy | LRU (least recently used) | Most recently used embeddings are most likely to be reused |
| Key | `(text_hash, model_id)` | Same text + same model = cache hit |
| Value | `EmbeddingVector { dimensions: u16, data: Vec<f32> }` | Dimensions included to prevent cross-model mixing |

### Dimension Isolation

Each embedding model produces vectors of a specific dimension. Mixing vectors from different models breaks similarity computation.

```rust
pub struct EmbeddingKey {
    pub text_hash: u64,       // fast hash of input text
    pub model_id: String,     // embedding model identifier
}

pub struct EmbeddingVector {
    pub dimensions: u16,      // must match model's expected dimension
    pub data: Vec<f32>,
    pub model_id: String,     // provenance tracking
}

pub struct DimensionRegistry {
    // Maps model_id → expected dimension
    // Panics or returns error on dimension mismatch at query time
    expected: HashMap<String, u16>,
}
```

**Invariant:** When querying or inserting vectors, the dimension must match the model's expected dimension. The registry enforces this at runtime.

### Request Processing

Embedding requests are processed by a dedicated valtron task:

```
User code: embedding_provider.embed("some text", model_id)
├── Check cache (text_hash + model_id)
│   ├── Hit → return cached vector immediately
│   └── Miss → enqueue EmbeddingRequest
├── EmbeddingRequest enqueued to ConcurrentQueue
│   └── Contains: text, model_id, response_channel
├── Generation valtron task:
│   ├── Dequeues request
│   ├── Runs embedding model inference
│   ├── Validates dimension against registry
│   ├── Caches result (if cache has space)
│   └── Sends result via response channel
└── Caller receives embedding vector
```

### Cache Backing (Optional)

For persistent caching across restarts, the LRU cache can be backed by **fjall** (LSM key-value store):

| Component | Role |
|-----------|------|
| `LruCache` in memory | Fast path — hot embeddings |
| `FjallKeyspace` on disk | Cold storage — evicted entries |
| Lookup flow | Memory → if miss → fjall → if miss → generate |

**When to use persistent backing:**
- Long-running sessions where embeddings are generated once and reused across restarts
- Memory-constrained environments where cache size must be small

**When to skip persistent backing:**
- Short-lived sessions (embeddings not reused)
- Memory-abundant environments (cache can hold everything)

### Text Tokenization Strategy

The caching strategy should consider how text is chunked for embedding:

| Strategy | Pros | Cons |
|----------|------|------|
| **Whole text** | Simple, exact cache hits | Long texts waste cache space; partial matches not found |
| **Sentence-level** | Better cache reuse for overlapping content | Requires sentence boundary detection |
| **Token-chunked** | Maximizes cache reuse | Loses cross-chunk semantic context |

**Decision:** Start with **whole-text** embedding and caching. If cache hit rates are low, investigate sentence-level chunking as an optimization. This is the simplest correct approach and can be refined later.

### API Contract

```rust
pub trait EmbeddingProvider {
    /// Generate or retrieve an embedding for the given text
    fn embed(&self, text: &str, model_id: &str) -> Result<EmbeddingVector>;
    
    /// Generate embeddings for multiple texts (batched)
    fn embed_batch(&self, texts: &[String], model_id: &str) -> Result<Vec<EmbeddingVector>>;
    
    /// Register a model's expected dimension
    fn register_model(&self, model_id: &str, dimensions: u16);
    
    /// Get cache statistics (for debugging/monitoring)
    fn cache_stats(&self) -> CacheStats;
    
    /// Clear the cache (for testing/maintenance)
    fn clear_cache(&self);
}

pub struct CacheStats {
    pub hits: u64,
    pub misses: u64,
    pub size: usize,
    pub max_size: usize,
}
```

### Valtron Task Integration

The embedding generation task runs as a valtron task:

```
Embedding generation valtron task:
├── Dequeues EmbeddingRequest from ConcurrentQueue
├── Generates embedding (model inference — may take 10-100ms)
├── Caches result in LRU cache
├── Sends result via response channel
└── Returns TaskStatus::Pending(EmbeddingProgress) while generating
    └── Returns TaskStatus::Ready(EmbeddingVector) when complete
```

This allows:
- **Progress reporting** — the agent loop knows embedding is in progress
- **Cancellation** — if the session ends, the generation task can be cancelled
- **Batching** — multiple requests can be batched into a single model inference call

## Rationale

**Why a queue-backed service instead of direct calls?**  
- Embedding generation is expensive — queue allows batching
- Queue provides natural backpressure — requests wait if generation is busy
- Decouples request submission from generation — caller doesn't block on model inference

**Why LRU cache with 1,000 entries?**  
- 1,000 entries ≈ 3MB for 768-dim embeddings — negligible memory cost
- LRU ensures hot embeddings stay cached, cold ones are evicted
- Bounded size prevents unbounded memory growth

**Why dimension isolation via registry?**  
- Mixing embeddings from different models breaks similarity computation (cosine similarity on mismatched dimensions is meaningless)
- Registry provides runtime validation — dimension mismatch is caught early with a clear error

**Why start with whole-text embedding?**  
- Simplest correct approach
- Sentence-level and token-chunked strategies are optimizations that can be added later
- Cache hit rates can be measured and used to justify more complex chunking

## Alternatives Considered

### No cache (generate every time)
- **Pros:** Simplest, no cache invalidation
- **Cons:** Repeated embedding of same text wastes compute and adds latency
- **Rejected because:** Embedding generation is 10-100ms — cache hits save significant time

### External embedding service (OpenAI, etc.)
- **Pros:** No local model needed, managed scaling
- **Cons:** Network latency, API costs, privacy concerns, not WASM-compatible
- **Supported as a backend:** The EmbeddingProvider trait can have an HTTP backend that calls external APIs, but the default is local

### Vector-based cache key (hash the embedding itself)
- **Pros:** Caches based on content, not text
- **Cons:** Same text always produces same embedding for a given model — text hash is sufficient
- **Rejected because:** Text hash is simpler and produces identical cache behavior
