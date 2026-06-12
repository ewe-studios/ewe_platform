# Decision 13: DocumentStore Trait and Backends

**Status:** Proposed  
**Date:** 2026-06-12  
**Context:** Specification 36 — Agentic API for foundation_ai, cross-platform document storage

## Problem

Multiple components need append-only, scan-based document storage:
- Message API — append messages, scan recent N, scan all for replay
- Memory snapshots — append working/observation/reflection memory, scan latest
- Session metadata — append session events, scan history

The storage must work across SQLite, Turso/D1, filesystem (via VFS), Cloudflare KV, and in-memory, using the same trait interface.

## Decision

The **DocumentStore** trait is added to `foundation_db` as a cross-platform abstraction for append-only document storage with scan semantics. Each backend implements the trait using its native storage primitives.

**When the DocumentStore feature is implemented, ALL backends below are implemented. No partial delivery.**

### Trait

```rust
pub trait DocumentStore: Send + Sync {
    /// Append a document to a collection. Returns the created Document with assigned ID.
    fn append<V: Serialize>(&self, key: &str, content: V) -> StorageResult<StorageItemStream<'_, Document>>;
    
    /// Scan the last N documents from a collection, newest-first.
    fn scan<V: DeserializeOwned>(&self, key: &str, limit: usize) -> StorageResult<StorageItemStream<'_, V>>;
    
    /// Scan all documents from a collection, oldest-first.
    fn scan_all<V: DeserializeOwned>(&self, key: &str) -> StorageResult<StorageItemStream<'_, V>>;
    
    /// Delete a specific document by ID.
    fn delete(&self, key: &str, doc_id: &str) -> StorageResult<StorageItemStream<'_, ()>>;
    
    /// Delete all documents in a collection. Returns count deleted.
    fn delete_all(&self, key: &str) -> StorageResult<StorageItemStream<'_, u64>>;
    
    /// Count documents in a collection.
    fn count(&self, key: &str) -> StorageResult<StorageItemStream<'_, u64>>;
}
```

### Backend Implementations

| Backend | Location | Notes |
|---------|----------|-------|
| **SQL (SQLite/Turbo/D1)** | `foundation_db` | Uses `QueryStore` — INSERT + SELECT with ORDER BY |
| **Memory** | `foundation_db` | In-memory HashMap with sequence counter |
| **VFS (Filesystem)** | `foundation_nativeapis` | One NDJSON file per collection; append = write line; scan = read last N lines |
| **Cloudflare KV** | `foundation_db` (wasm) | KV key = `{collection}:{doc_id}` → JSON value; list keys with prefix, sort, take N |
| **Cloudflare D1** | `foundation_db` (wasm) | Same as SQL backend, uses `AsyncDocumentStore` trait |

### VFS-Based DocumentStore

When implemented in `foundation_nativeapis`, the VFS-backed DocumentStore will:

- **Storage layout:** One NDJSON file per collection at `{vfs_root}/{encoded_key}.jsonl`
- **Append:** Write one JSON line to end of file
- **Scan (last N):** Seek to end, read backwards N lines, parse JSON
- **Scan all:** Read file from start, parse each line
- **Delete:** Rewrite file excluding deleted document IDs (or append tombstone)
- **Uses VFS traits:** `VfsFile` for read/write, `VfsDirectory` for listing collections

```
vfs_root/
├── session%3Aabc123%3Amessages.jsonl    # Messages for session abc123
├── session%3Aabc123%3Amemory%3Aworking.jsonl  # Working memory snapshots
└── session%3Adef456%3Amessages.jsonl    # Messages for session def456
```

### Cloudflare KV DocumentStore

For WASM/Cloudflare Workers environments:

- **Storage layout:** KV keys = `doc:{collection}:{doc_id}` → JSON value
- **Append:** Generate doc_id, `kv.put(key, json)`, also append to list key `list:{collection}`
- **Scan (last N):** Get `list:{collection}`, sort by timestamp, take last N, fetch each
- **Scan all:** Get `list:{collection}`, sort, fetch all
- **Delete:** Delete both `doc:{collection}:{doc_id}` and remove from list
- **Uses:** `AsyncDocumentStore` trait (KV is async-only in WASM)

### SQL Schema

```sql
CREATE TABLE IF NOT EXISTS documents (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    collection_key TEXT NOT NULL,
    doc_id TEXT NOT NULL,          -- scru128 or similar
    content TEXT NOT NULL,          -- JSON-encoded document
    metadata TEXT NOT NULL DEFAULT '{}',
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);
CREATE INDEX idx_documents_collection_key ON documents(collection_key);
CREATE INDEX idx_documents_collection_created ON documents(collection_key, created_at);
CREATE UNIQUE INDEX idx_documents_collection_doc_id ON documents(collection_key, doc_id);
```

### Document Structure

```rust
pub struct Document {
    pub id: String,              // Unique within collection (scru128)
    pub content: String,         // JSON-encoded content
    pub metadata: serde_json::Value,  // Created_at, sequence, etc.
}
```

### Why a Separate Trait Instead of Using KeyValueStore?

- **Append semantics:** KeyValueStore has `set` (overwrite), DocumentStore has `append` (add to collection)
- **Scan semantics:** KeyValueStore has `list_keys` (unordered), DocumentStore has `scan` (ordered, limited)
- **Document structure:** Documents have IDs, content, and metadata — more structured than raw key-value pairs
- **Deletion granularity:** DocumentStore deletes individual documents by ID, not entire collections

## Implications

- **Message API** (Decision 02) uses DocumentStore for message persistence
- **Context Memory** (Decision 03) uses DocumentStore for memory snapshots
- **Session management** (Decision 01) uses DocumentStore for session metadata
- **foundation_ai** can build higher-level abstractions on top of DocumentStore without caring about the backend
