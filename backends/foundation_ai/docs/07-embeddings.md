# Fundamentals 07 — Embedding provider and vector integration

How text is converted to vectors for semantic search, and how vectors are
stored and queried.

---

## 1. The embedding pipeline

```
Text → EmbeddingProvider.embed() → Vec<f32> → VectorStore.insert()
                                                  ↓
Query text → EmbeddingProvider.embed() → Vec<f32> → VectorStore.search() → matches
```

## 2. EmbeddingProvider trait

```rust
#[async_trait]
pub trait EmbeddingProvider: Send + Sync {
    fn name(&self) -> &str;
    async fn embed(&self, texts: Vec<String>) -> EmbeddingResult<Vec<Vec<f32>>>;
    fn dimension(&self) -> usize;
}
```

Returns vectors normalized to unit length (cosine similarity = dot product).

## 3. EmbeddingRouter

Routes embedding requests to the right provider based on model ID:

```rust
let router = EmbeddingRouter::new()
    .register(openai_embeddings, "text-embedding-3-small")
    .register(local_embeddings, "all-MiniLM-L6-v2");
```

Fallback chain: if primary provider fails, try the next one.

## 4. Integration with VectorStore

The `VectorStore` trait stores vectors with metadata:

```rust
store.insert("namespace", VectorEntry {
    id: "doc-123".into(),
    vector: Vector::new(embedding),
    metadata: VectorMetadata { tags },
}).await?;

let matches = store.search("namespace", &query_embedding, 10).await?;
// matches: Vec<VectorMatch { id, score }>
```

## 5. Supported embedding models

| Provider | Model | Dimension |
|---|---|---|
| OpenAI | text-embedding-3-small | 1536 |
| OpenAI | text-embedding-3-large | 3072 |
| Local | all-MiniLM-L6-v2 | 384 |
| Local | bge-small-en-v1.5 | 384 |

## 6. Chunking

Long texts are split into chunks before embedding:

```rust
let chunks = chunk_text(&text, max_tokens: 500, overlap: 50);
// Each chunk gets its own vector, linked by metadata
```

Chunk size trades off:
- **Smaller chunks** → more precise matches, more vectors
- **Larger chunks** → broader context, fewer vectors
- **Overlap** → prevents information loss at boundaries
