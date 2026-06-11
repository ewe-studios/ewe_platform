# Decision 07: Vector Search Strategy

**Status:** Proposed  
**Date:** 2026-06-11  
**Context:** Specification 36 — Agentic API for foundation_ai

## Problem

Semantic recall requires vector similarity search over embedded message content, observations, and reflections. The search must work across multiple storage backends (in-memory, SQLite/Turso, fjall) and ideally be WASM-compatible for browser-based agents.

## Decision

Vector search is implemented as a **pluggable trait hierarchy** in `foundation_db`, with multiple backend implementations. The Context API uses the vector search trait via trait objects, allowing the backend to be selected at runtime.

### Trait Hierarchy

```rust
pub trait VectorStore: Send + Sync {
    /// Insert a vector with associated metadata
    fn insert(&self, id: &str, vector: &[f32], metadata: VectorMetadata);
    
    /// Insert multiple vectors (batched)
    fn insert_batch(&self, entries: &[VectorEntry]);
    
    /// Query for nearest neighbors
    fn query(&self, vector: &[f32>, top_k: usize) -> Vec<VectorMatch>;
    
    /// Delete entries by ID
    fn delete(&self, ids: &[&str]);
    
    /// Flush pending writes (for persistent stores)
    fn flush(&self) -> Result<()>;
    
    /// Get store statistics
    fn stats(&self) -> VectorStoreStats;
}

pub struct VectorEntry {
    pub id: String,
    pub vector: Vec<f32>,
    pub metadata: VectorMetadata,
}

pub struct VectorMatch {
    pub id: String,
    pub score: f32,
    pub metadata: VectorMetadata,
}

pub struct VectorMetadata {
    pub session_id: SessionId,
    pub message_id: Option<Scru128>,
    pub content_type: String,    // "message", "observation", "reflection"
    pub text_content: String,    // original text for display
    pub created_at: u128,        // scru128 timestamp
}
```

### Backend Implementations

| Backend | Environment | Persistence | WASM | Use Case |
|---------|-------------|-------------|------|----------|
| **InMemoryVectorStore** | All | No | ✅ | Development, testing, short-lived sessions |
| **TursoVectorStore** | Native + WASM | Yes (libsql) | ✅ | Production sessions with persistence |
| **FjallVectorStore** | Native | Yes (LSM) | ❌ | Native-only sessions with local persistence |

### In-Memory Store

Simple flat-scan implementation for development and testing:

```rust
pub struct InMemoryVectorStore {
    entries: RwLock<HashMap<String, VectorEntry>>,
    dimension: u16,
}
```

- **Query:** Linear scan with cosine similarity — O(n) but fine for small datasets (<10k vectors)
- **Insert:** O(1) HashMap insert
- **WASM-compatible:** No platform-specific dependencies

### Turso/libsql Vector Store

Uses Turso's native vector extension for indexed search:

```sql
-- Vector table schema
CREATE TABLE vectors (
    id TEXT PRIMARY KEY,
    session_id TEXT NOT NULL,
    message_id TEXT,
    content_type TEXT NOT NULL,
    text_content TEXT,
    created_at INTEGER,
    embedding BLOB  -- f32 array serialized to blob
);

-- Create vector index
CREATE VEC0 vector_index ON vectors (embedding);
```

- **Query:** HNSW or IVF index for O(log n) search
- **Insert:** Standard SQL insert + index update
- **WASM-compatible:** Turso (libsql) works in WASM environments
- **Persistence:** Vectors survive restarts

### Fjall Vector Store (Native Only)

Uses fjall's LSM key-value store with custom vector indexing:

```rust
pub struct FjallVectorStore {
    keyspace: Keyspace,
    vector_space: Keyspace,  // separate keyspace for vector data
    index: IvfIndex,         // IVF index built from vectors
}
```

- **Query:** IVF index for approximate nearest neighbor search
- **Insert:** LSM write → index rebuild on flush
- **NOT WASM-compatible:** fjall requires native file system
- **Persistence:** Vectors survive restarts via LSM

### Index Strategy

| Store Size | Index Type | Rationale |
|------------|------------|-----------|
| < 1,000 vectors | Flat scan | Index overhead > benefit |
| 1,000 - 100,000 vectors | IVF (Inverted File Index) | Good balance of accuracy and speed |
| > 100,000 vectors | HNSW (Hierarchical Navigable Small World) | Best query speed for large datasets |

**Decision:** Start with **flat scan** for all stores. Add IVF indexing when stores exceed 1,000 vectors. This is the simplest correct approach and can be measured and optimized later.

### Dimension Consistency

All vectors in a store must have the same dimension:

```rust
impl VectorStore for InMemoryVectorStore {
    fn insert(&self, id: &str, vector: &[f32], metadata: VectorMetadata) {
        assert_eq!(
            vector.len() as u16, self.dimension,
            "Vector dimension mismatch: expected {}, got {}",
            self.dimension, vector.len()
        );
        // ...
    }
}
```

This is enforced at the store level — mixing dimensions is a runtime error.

### Context API Integration

The Context API owns a `VectorStore` instance:

```rust
pub struct ContextProvider {
    session_id: SessionId,
    working_memory: Arc<WorkingMemory>,
    observation_memory: Arc<ObservationMemory>,
    reflection_memory: Arc<ReflectionMemory>,
    vector_store: Arc<dyn VectorStore>,
    embedding_provider: Arc<dyn EmbeddingProvider>,
}

impl ContextProvider {
    /// Semantic search across all memory tiers
    pub fn semantic_recall(&self, query: &str, limit: usize) -> Vec<MemoryResult> {
        let embedding = self.embedding_provider.embed(query, "default")?;
        self.vector_store.query(&embedding.data, limit)
            .into_iter()
            .map(|m| self.resolve_match(m))
            .collect()
    }
}
```

### Shared Module Location

Vector search modules should be added to `foundation_db`:

```
foundation_db/
├── src/
│   ├── vector/
│   │   ├── mod.rs          — pub mod + re-exports
│   │   ├── types.rs        — VectorEntry, VectorMatch, VectorMetadata
│   │   ├── error.rs        — VectorError, VectorResult
│   │   ├── traits.rs       — VectorStore trait
│   │   ├── in_memory.rs    — InMemoryVectorStore
│   │   ├── turso.rs        — TursoVectorStore (behind feature flag)
│   │   └── fjall.rs        — FjallVectorStore (behind feature flag)
```

Feature flags:
- `vector-in-memory` — always available (default)
- `vector-turso` — requires turso/libsql dependency
- `vector-fjall` — requires fjall dependency

### Reference Sources

Vector database source code available for study:
- `/home/darkvoid/Boxxed/@formulas/src.rust/src.FileSystemAPIs/src.VectorDB/` — general reference
- `/home/darkvoid/Boxxed/@formulas/src.rust/src.FileSystemAPIs/src.VectorDB/src.Chroma` — Chroma implementation patterns

## Rationale

**Why a trait hierarchy instead of a single implementation?**  
- Different environments need different backends (WASM vs native)
- Production needs persistence; development needs simplicity
- Trait allows swapping backends without changing the Context API

**Why start with flat scan?**  
- Agent sessions typically have <1,000 messages/observations
- Flat scan is simple, correct, and easy to debug
- Indexing can be added when profiling shows it's needed

**Why Turso/libsql as the primary persistent backend?**  
- WASM-compatible (critical for browser-based agents)
- Native vector extension (no external index service needed)
- ACID guarantees for vector data
- Already in use for foundation_db

## Alternatives Considered

### External vector database (Pinecone, Weaviate, Qdrant)
- **Pros:** Managed, scalable, advanced indexing
- **Cons:** External dependency, network latency, data privacy, not WASM-compatible
- **Rejected because:** Adds infrastructure complexity; in-process stores are sufficient for agent session scale

### Pure HNSW from the start
- **Pros:** Fast query performance
- **Cons:** Complex implementation, memory overhead, overkill for small datasets
- **Deferred to:** Optimization phase — start with flat scan, add HNSW when needed

### Embed vectors directly in the message store
- **Pros:** Co-located data, single storage layer
- **Cons:** Message store format (NDJSON) is not ideal for vector search; mixes concerns
- **Rejected because:** Vector search requires specialized indexing — better as a separate store
