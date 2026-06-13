# Decision 07: Vector Search Strategy

**Status:** Proposed  
**Date:** 2026-06-11  
**Updated:** 2026-06-12  
**Context:** Specification 36 — Agentic API for foundation_ai

## Problem

Semantic recall requires vector similarity search over embedded message content, observations, and reflections. The search must work across multiple storage backends (in-memory, SQLite, Turso, fjall, Cloudflare D1/KV) and be WASM-compatible.

## Decision

Vector search is split across **two crates**:

| Crate | Responsibility |
|-------|---------------|
| **`foundation_vectors`** | Core algorithms — cosine similarity, euclidean distance, flat scan, IVF, HNSW |
| **`foundation_db`** | Storage implementations — trait definition, per-backend persistence, native vector support wrapping |

### `foundation_vectors` — Algorithms (Owned by Us)

All vector search algorithms are implemented in Rust in `foundation_vectors`, regardless of whether a backend has native vector support. This gives us:

- **Consistent behavior** across all backends — same algorithm, same results
- **Fallback path** — if a backend's native vector support is unavailable, we fall back to our algorithms
- **WASM compatibility** — pure Rust algorithms work everywhere
- **Algorithm control** — we choose distance metrics, indexing strategies, and optimization paths

**Algorithms to implement:**
- **Flat scan** — brute-force cosine/euclidean similarity over all vectors
- **IVF (Inverted File Index)** — cluster-based approximate nearest neighbor for 1k-100k vectors
- **HNSW (Hierarchical Navigable Small World)** — graph-based ANN for >100k vectors

**TODO**: What of BM25 (BM25 (a classic keyword search algorithm) with vector search creates a hybrid search pipeline) This approach captures exact keyword matches and complex semantic intent together. It is highly recommended to A/B test this blend using Reciprocal Rank Fusion (RRF) to merge result. ost production systems combine these retrieval methods using one of the following approaches:Parallel Retrieval & Fusion: Run the BM25 and vector searches simultaneously to retrieve the top N results from each list. Merge the two lists using Reciprocal Rank Fusion (RRF) to combine rankings without worrying about score normalization.Re-ranking: Use a cross-encoder (like Cohere Rerank) as a final pass over your combined results to optimize the top outputs for precision.Alpha Weighting: Use weighted sums if your platform supports it (e.g., in Weaviate), where an α value of 1.0 is pure vector search and 0.0 is pure BM25.

**Distance metrics:**
- Cosine similarity (primary for text embeddings)
- Euclidean / L2 distance
- Dot product

### `foundation_db` — Storage Trait and Backends

The `VectorStore` trait in `foundation_db` defines the storage contract. Each backend implementation decides whether to use native vector support or store pre-generated embeddings for our algorithms to search.

#### Trait

```rust
pub trait VectorStore: Send + Sync {
    /// Insert a vector with associated metadata
    fn insert(&self, id: &str, vector: &[f32], metadata: VectorMetadata) -> Result<()>;
    
    /// Insert multiple vectors (batched)
    fn insert_batch(&self, entries: &[VectorEntry]) -> Result<()>;
    
    /// Query for nearest neighbors using foundation_vectors algorithms
    fn query(&self, vector: &[f32], top_k: usize) -> Result<Vec<VectorMatch>>;
    
    /// Delete entries by ID
    fn delete(&self, ids: &[&str]) -> Result<()>;
    
    /// Flush pending writes (for persistent stores)
    fn flush(&self) -> Result<()>;
    
    /// Get store statistics
    fn stats(&self) -> VectorStoreStats;
}
```

#### Backend Storage Strategies

| Backend | Storage Strategy | Native Vector Used? | Research Required |
|---------|-----------------|-------------------|-------------------|
| **In-Memory** | Store vectors in `Vec<VectorEntry>`, search via `foundation_vectors::flat_scan` | No | — |
| **Turso/libSQL** | Store vectors as BLOBs via `vector()` function; use `vector_top_k()` for native search, fall back to `foundation_vectors` if native unavailable | Yes (DiskANN) | Verify Turso Rust client exposes `vector_top_k()`, test D1 compatibility |
| **SQLite** | Store vectors as BLOBs; use `sqlite-vec` extension if available, fall back to `foundation_vectors::flat_scan` | Optional (sqlite-vec) | Research sqlite-vec extension loading in Rust, WASM compatibility |
| **Fjall (LSM KV)** | Store vectors as serialized byte arrays in fjall keyspaces; build IVF index on top | No | Research fjall keyspace design for efficient vector retrieval, IVF index persistence |
| **Cloudflare D1** | Store vectors as BLOBs in D1; compute similarity via SQL expressions or fetch + `foundation_vectors` | No | Research D1 SQL functions for cosine similarity, performance of fetch-all + client-side search |
| **Cloudflare KV** | Store vectors as JSON/B64 in KV; fetch all + client-side `foundation_vectors` search | No | Research KV list + bulk get performance, batching strategy for large vector sets |

### Data Flow

```
EmbeddingProvider generates [f32] vector
    │
    ▼
VectorStore.insert(id, vector, metadata)
    │
    ├── Turso: stores via vector() BLOB, indexed by DiskANN
    ├── SQLite: stores as BLOB, optionally indexed by sqlite-vec
    ├── In-Memory: stores in Vec<VectorEntry>
    ├── Fjall: stores in keyspace as serialized bytes
    ├── D1: stores as BLOB via SQL INSERT
    └── KV: stores as JSON via kv.put()
    
VectorStore.query(vector, top_k)
    │
    ├── Turso: calls vector_top_k() → returns matches
    ├── SQLite: calls sqlite-vec VEC0 → returns matches
    ├── Others: fetches stored vectors → foundation_vectors::flat_scan/IVF/HNSW → returns matches
```

### Dimension Consistency

All vectors in a store must have the same dimension. The `VectorStore` enforces this:

```rust
pub struct VectorStoreConfig {
    pub dimensions: u16,
    pub distance_metric: DistanceMetric,
}
```

Mismatched dimensions at insert time return an error.

### When the VectorStore Feature Is Implemented

**ALL backends listed above are implemented. No partial delivery.** Each backend's implementation includes:

1. Storage of pre-generated embeddings (the `insert` path)
2. Query using either native vector support (if available) or `foundation_vectors` algorithms
3. Proper dimension validation and consistency
4. Flush/persistence semantics
5. Tests covering insert, query, delete, and dimension mismatch scenarios

### Research Required Before Feature Implementation

The feature specification must include detailed research on:

- **Turso**: Rust client API for `vector_top_k()`, DiskANN configuration options, index rebuild behavior
- **sqlite-vec**: Extension loading in Rust, WASM compatibility, `vec0` virtual table schema design
- **Fjall**: Keyspace design for vector + metadata co-location, IVF index serialization/deserialization
- **D1**: SQL expressions for cosine similarity (`1 - vector_cosine_distance()`), query performance with large vector sets
- **KV**: Bulk get API limits, list-with-prefix performance, optimal batching strategy

## Alternatives Considered

### External vector database (Pinecone, Weaviate, Qdrant)
- **Pros:** Managed, scalable, advanced indexing
- **Cons:** External dependency, network latency, data privacy, not WASM-compatible
- **Rejected because:** Adds infrastructure complexity; in-process stores are sufficient for agent session scale

### Pure native-only (no `foundation_vectors` algorithms)
- **Pros:** Leverages database-native indexing
- **Cons:** Only Turso has native support; all other backends would need our algorithms anyway
- **Rejected because:** We need algorithms for non-native backends regardless — might as well own them

### Embed vectors directly in the message store
- **Pros:** Co-located data, single storage layer
- **Cons:** Message store format (NDJSON) is not ideal for vector search; mixes concerns
- **Rejected because:** Vector search requires specialized indexing — better as a separate store
