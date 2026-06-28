# Fundamentals 07 — Embedding provider and vector integration

How text is converted to vectors for semantic search, and how vectors are
stored and queried.

---

## 1. The embedding pipeline

```
Text → EmbeddingProvider.embed(text, model_id) → EmbeddingVector → VectorStore.insert()
                                                                          ↓
Query text → EmbeddingProvider.embed(query, model_id) → EmbeddingVector → VectorStore.search() → matches
```

## 2. EmbeddingProvider trait

```rust
pub trait EmbeddingProvider: Send + Sync {
    fn embed(&self, text: &str, model_id: &str) -> Result<EmbeddingVector, EmbeddingError>;
    fn embed_batch(&self, texts: &[String], model_id: &str) -> Result<Vec<EmbeddingVector>, EmbeddingError>;
    fn register_model(&self, model_id: &str, dimensions: u16);
    fn cache_stats(&self) -> CacheStats;
    fn clear_cache(&self);
}
```

Key methods:
- **`embed()`** — embed a single text string for a given model
- **`embed_batch()`** — embed multiple texts efficiently
- **`register_model()`** — register a model's dimension count
- **`cache_stats()`** — cache hit/miss statistics

`EmbeddingVector` is `Vec<f32>` — vectors are typically normalized for
cosine similarity (dot product = cosine when normalized).

## 3. TextChunker — splitting text before embedding

```rust
pub trait TextChunker: Send + Sync {
    fn chunk(&self, text: &str) -> Vec<String>;
}
```

Built-in chunkers:
- **`WholeTextChunker`** — returns the entire text as one chunk
- **`SentenceChunker`** — splits on sentence boundaries

The `EmbeddingProvider` implementation uses a chunker internally:

```rust
let provider = EmbeddingProviderImpl::new(router, SentenceChunker, NoopColdCache, 1000);
```

## 4. Caching

The embedding provider includes an LRU cache:

```rust
pub struct CacheStats {
    pub hits: u64,
    pub misses: u64,
    pub evictions: u64,
}
```

Cache key is `(text_hash, model_id, epoch)` — same text + same model = cache
hit. Useful for repeated queries on the same corpus.

## 5. Integration with VectorStore

The `VectorStore` trait (in foundation_vectors) stores vectors with metadata:

```rust
store.insert("namespace", VectorEntry {
    id: "doc-123".into(),
    vector: Vector::new(embedding),
    metadata: VectorMetadata { tags },
})?;

let matches = store.search("namespace", &query_embedding, 10)?;
// matches: Vec<VectorMatch { id, score }>
```

## 6. Supported embedding models

| Provider | Model | Dimension |
|---|---|---|
| OpenAI | text-embedding-3-small | 1536 |
| OpenAI | text-embedding-3-large | 3072 |
| Local | all-MiniLM-L6-v2 | 384 |
| Local | bge-small-en-v1.5 | 384 |

## 7. Chunking strategy

Long texts are split into chunks before embedding:

```rust
// Using SentenceChunker (built-in)
let provider = EmbeddingProviderImpl::new(router, SentenceChunker, cache, max_entries);

// Each sentence gets its own vector
let embedding = provider.embed("Long text with multiple sentences.", "model-id")?;
```

Chunk size trades off:
- **Smaller chunks** → more precise matches, more vectors
- **Larger chunks** → broader context, fewer vectors
- **Sentence boundaries** → natural semantic units
