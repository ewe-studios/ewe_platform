# Fundamentals 07 — Embeddings and vector integration

How text becomes vectors, where this crate uses them, and how they meet a
`VectorStore`. Source: `src/agentic/embedding.rs`.

---

## 1. The pipeline

```
text ─► TextChunker ─► chunks ─► (LRU cache → cold cache → model) per chunk
                                         │
                                         ▼
                          mean of chunk vectors ─► EmbeddingVector
```

Embeddings are produced by an ordinary `Model` behind the `ProviderRouter`:
the provider receives a `ModelInteraction` marked as an embedding request and
answers with `ModelOutput::Embedding { dimensions, values }`. Backends that
support this today: OpenAI-compatible (`/v1/embeddings`) and llama.cpp.

## 2. Types

```rust
pub struct EmbeddingVector {
    pub dimensions: u16,
    pub data: Vec<f32>,
    pub model_id: String,
}

pub trait EmbeddingProvider: Send + Sync {
    fn embed(&self, text: &str, model_id: &str) -> Result<EmbeddingVector, EmbeddingError>;
    fn embed_batch(&self, texts: &[String], model_id: &str)
        -> Result<Vec<EmbeddingVector>, EmbeddingError>;
    fn register_model(&self, model_id: &str, dimensions: u16);
    fn cache_stats(&self) -> CacheStats;   // hits, misses, evictions, size
    fn clear_cache(&self);
}
```

`EmbeddingError` covers generation and routing failures, dimension overflow
(more than `u16::MAX`), and dimension mismatches between chunks.

## 3. `CachedEmbeddingProvider`

The one implementation:

```rust
use foundation_ai::agentic::{CachedEmbeddingProvider, SentenceChunker, NoopColdCache};

let embedder = CachedEmbeddingProvider::new(
    router.clone(),                 // routes the embedding model id
    Box::new(SentenceChunker),      // or WholeTextChunker
    Box::new(NoopColdCache),        // or your own ColdCache (KV, disk, …)
    1000,                           // LRU capacity
);
// Same thing with those defaults:
let embedder = CachedEmbeddingProvider::with_defaults(router.clone());

let v = embedder.embed("The cat sat on the mat.", "text-embedding-3-small")?;
```

How `embed` works:

1. The chunker splits the text (`SentenceChunker` on sentence boundaries,
   `WholeTextChunker` not at all).
2. Each chunk is looked up in the in-memory LRU, then the `ColdCache`, then
   generated through the router and written to both caches. The cache key is
   (text hash, model id, epoch).
3. Multi-chunk texts are **mean-pooled** into one vector. You always get one
   `EmbeddingVector` per input text.

`embed_batch` calls `embed` for each text.

## 4. Where the agent uses embeddings

**Semantic recall.** Give the session an embedder and
`ContextProvider::search_from_memory` — what the `search_context` tool calls —
ranks prior messages by cosine similarity instead of keyword matching. (Normal
per-turn context assembly does not use the embedder; it takes the most recent
messages.)

```rust
let agent = AgentSession::builder(router.clone())
    .with_embedder(Arc::new(CachedEmbeddingProvider::with_defaults(router)), "text-embedding-3-small")
    // search_context, built inside build() from the session's own context
    // provider — so it uses the session's stores and embedder:
    .with_toolshed(ToolShed::new().tools(ToolPreset::search_context()))
    .build()?;
```

Without an embedder, `search_context` falls back to keyword matching.
`SearchMode::Graph` has no session knowledge graph to search, so it logs a
warning and falls back to hybrid recall.

**Tool discovery.** With an embedder on the session, `build()` indexes every
tool in the session's `ToolShed` with `ToolDiscovery::in_memory(embedder,
model)`, and the built-in `shed` meta-tool searches by embedding (filling up
with name/description matches). `ToolDiscovery::new(vector_store, embedder,
model)` uses a vector store you provide; `ToolCallManager::enable_discovery`
attaches one to a manager.

## 5. Storing vectors yourself

`VectorStore`, `VectorEntry` and `VectorMatch` live in `foundation_vectors`.
To index your own documents, embed with the provider and insert the
`EmbeddingVector::data` into your store; see that crate's docs for the exact
API and distance metrics.

## 6. Choosing chunking

- `SentenceChunker` (the default) keeps each piece to a natural unit, then
  averages — good for long passages where you want one vector per document.
- `WholeTextChunker` sends the text as-is — use it for short texts, or when
  the model's own context handles long inputs better than averaging does.
- If you need **one vector per chunk** (classic RAG indexing), chunk the text
  yourself and call `embed` on each piece.
