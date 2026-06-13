# Decision 12: Database and Auth Integration

**Status:** Proposed  
**Date:** 2026-06-11  
**Context:** Specification 36 — Agentic API for foundation_ai

## Problem

**TODO**: Locally we dont need auth, we can add a always allowed authentication implementation for local or where auth is disabled.

The agentic API needs persistent storage for sessions, messages, and memory snapshots. It also needs authentication and authorization for:
- Session access control (who can read/write a session) **TODO**, the Agent owns the session, unsure exactly why this is important, can you elaborate more with clarity?
- Tool call permission gating (which tools a user can invoke)
- Model access control (which models a user can use)
- Usage tracking and billing (token consumption per user/org)

The platform already has `foundation_db` and `foundation_auth` — the agentic API must integrate with these rather than building its own storage and auth layers.

## Decision

**All storage is built directly on foundation_db's existing capabilities.** The agentic API does not define its own storage traits or abstraction layer — it consumes foundation_db's primitives (key-value store, document store, vector store) directly. This means every storage backend foundation_db supports (SQLite, disk, Cloudflare D1/Turso, in-memory, etc.) automatically works for the agentic API with zero additional code.

### Storage Mapping

| Agentic Concern | foundation_db Primitive | Operations |
|-----------------|------------------------|------------|
| **Session metadata** | Key-value store (`KVStore`) | `set(session_key, metadata)`, `get(session_key)`, `list(prefix)`, `delete(session_key)` |
| **Messages** | Document store or append-only log | `append(session_key, message)`, `scan(session_key, limit)`, `scan_all(session_key)` |
| **Memory snapshots** | Key-value store (`KVStore`) | `set(memory_key, snapshot)`, `get(memory_key)` |
| **Vector embeddings** | Vector store (`VectorStore`) | `insert(id, vector, metadata)`, `query(vector, top_k)`, `delete(ids)` |

### Storage Keys

All keys are namespaced by SessionId to ensure isolation:

```
session:{session_id}:metadata       → SessionMetadata (JSON)
session:{session_id}:messages       → append-only message log
session:{session_id}:memory:working → WorkingMemory snapshot (JSON)
session:{session_id}:memory:observation → ObservationMemory snapshot (JSON)
session:{session_id}:memory:reflection → ReflectionMemory snapshot (JSON)
session:{session_id}:vectors        → vector store namespace
```

This key structure means:
- **Listing sessions** = `kv.list("session:")` — prefix scan
- **Loading a session** = `kv.get("session:{id}:metadata")`
- **Getting recent messages** = `doc.scan("session:{id}:messages", limit=N)`
- **Session deletion** = delete all keys with prefix `session:{session_id}:`

### Architecture

```
foundation_db (persistence layer — already exists)
├── KVStore — key-value operations (get, set, list, delete)
├── DocumentStore — document/append operations (append, scan)
├── VectorStore — vector similarity search (insert, query, delete)
└── Backends: SQLite, Turso/D1, in-memory, disk, Cloudflare, etc.

Agentic API (consumer — no storage layer of its own)
├── Message API → uses foundation_db::DocumentStore directly
├── Context API → uses foundation_db::KVStore directly
├── EmbeddingProvider → uses foundation_db::VectorStore directly
└── Session Manager → uses foundation_db::KVStore directly
```

### No Intermediate Abstraction Layer

The agentic API does **not** define its own `SessionStorage`, `MessageStorage`, or `MemoryStorage` traits. Instead:

```rust
// DIRECT usage — no intermediate trait
pub struct MessageApi {
    session_id: SessionId,
    doc_store: Arc<DocumentStore>,  // foundation_db type, directly used
    write_buffer: Arc<ConcurrentQueue<QueuedMessage>>,
    broadcaster: Arc<Broadcaster<MessageEvent>>,
}

impl MessageApi {
    pub fn append(&self, message: Message) {
        self.write_buffer.push(QueuedMessage { message });
    }

    pub fn flush(&self) -> Result<()> {
        let messages = drain_queue(&self.write_buffer);
        for msg in messages {
            // Direct foundation_db call — no trait indirection
            self.doc_store.append(
                &format!("session:{}:messages", self.session_id),
                msg.to_json()?,
            )?;
        }
        Ok(())
    }

    pub fn recent(&self, n: usize) -> Result<Vec<Message>> {
        let key = format!("session:{}:messages", self.session_id);
        let docs = self.doc_store.scan(&key, n)?;
        docs.into_iter()
            .map(|d| Message::from_json(&d.content))
            .collect()
    }
}
```

**Why no intermediate traits:**
- foundation_db's types (`KVStore`, `DocumentStore`, `VectorStore`) are already trait-backed and swappable
- Adding another trait layer duplicates abstraction without adding value
- Testing can mock foundation_db's types directly (they're already trait-based)
- Fewer layers = easier to understand, fewer things to maintain

### Auth Integration

```
foundation_auth (auth layer)
├── Authentication — verify user identity (JWT, session cookies, API keys)
├── Authorization — check user permissions (RBAC, ABAC)
└── Usage Tracking — record token consumption per user/org

Agentic API (consumer)
├── Session access → auth.check(user, "session:{id}:write")
├── Tool call gating → auth.check(user, "tool:{name}:execute")
├── Model access → auth.check(user, "model:{id}:use")
└── Usage tracking → auth.record_usage(user, TokenUsage { ... })
```

### Auth Trait

```rust
// In foundation_ai::agentic::auth
pub trait AuthProvider: Send + Sync {
    /// Verify user identity and return authenticated user info
    fn authenticate(&self, credentials: &Credentials) -> Result<AuthenticatedUser>;
    
    /// Check if user has permission to perform action
    fn authorize(&self, user: &AuthenticatedUser, permission: &Permission) -> Result<bool>;
    
    /// Record usage for billing/tracking
    fn record_usage(&self, user: &AuthenticatedUser, usage: TokenUsage) -> Result<()>;
    
    /// Get user's allowed models
    fn allowed_models(&self, user: &AuthenticatedUser) -> Result<Vec<String>>;
    
    /// Get user's allowed tools
    fn allowed_tools(&self, user: &AuthenticatedUser) -> Result<Vec<String>>;
}

pub struct Permission {
    pub resource: String,    // "session", "tool", "model"
    pub resource_id: String, // session_id, tool_name, model_id
    pub action: String,      // "read", "write", "execute", "use"
}

pub struct TokenUsage {
    pub model_id: String,
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub cached_tokens: u64,
    pub timestamp: u128,
}
```

### Auth Flow

```
User request: POST /api/sessions/{session_id}/messages
├── Extract credentials from request (JWT, API key, session cookie)
├── foundation_auth.authenticate(credentials)
│   └── Returns AuthenticatedUser or error
├── foundation_auth.authorize(user, Permission {
│       resource: "session",
│       resource_id: session_id,
│       action: "write",
│   })
│   └── Returns true/false — reject if false
├── Check tool permissions (if request includes tool calls)
│   └── For each tool call: authorize(user, Permission { resource: "tool", ... })
├── Execute request (via foundation_db primitives)
├── foundation_auth.record_usage(user, TokenUsage { ... })
└── Return response
```

### Session Access Control

Sessions are owned by the user who created them:

```rust
pub struct SessionMetadata {
    pub session_id: SessionId,
    pub owner_id: UserId,
    pub created_at: u128,
    pub status: SessionStatus,
    pub model: String,
    pub shared_with: Vec<UserId>,  // users with read access
    pub name: Option<String>,      // user-friendly name
}
```

Access rules:
- **Owner:** Full read/write/delete access
- **Shared with:** Read access only (can view messages, memories)
- **Others:** No access (403 Forbidden)

### Tool Call Permission Gating

```rust
impl ToolCallManager {
    pub fn submit(&self, calls: Vec<ToolCallRequest>, user: &AuthenticatedUser) -> Result<()> {
        for call in &calls {
            if !self.auth.authorize(user, &Permission {
                resource: "tool".to_string(),
                resource_id: call.tool_name.clone(),
                action: "execute".to_string(),
            })? {
                return Err(AgenticError::ToolNotAuthorized {
                    tool_name: call.tool_name.clone(),
                    user_id: user.id.clone(),
                });
            }
        }
        self.submit_authorized(calls)
    }
}
```

### Backend Inheritance

Because the agentic API uses foundation_db directly, every backend foundation_db supports works automatically:

| Backend | Environment | How It Works |
|---------|-------------|--------------|
| **SQLite** | Native | foundation_db's SQLite backend — all KV, document, vector ops via SQLite |
| **Turso/D1** | Native + WASM | foundation_db's Turso backend — works in browser and native |
| **In-memory** | All | foundation_db's in-memory backend — development, testing |
| **Disk (fjall)** | Native | foundation_db's fjall backend — local persistence |
| **Cloudflare D1** | WASM (CF Workers) | foundation_db's D1 backend — serverless sessions |
| **Cloudflare KV** | WASM (CF Workers) | foundation_db's CF KV backend — session metadata + memory snapshots |

No additional code is needed in the agentic API to support any of these backends — switching is a matter of configuring foundation_db.

### Feature Flags

```toml
# foundation_ai/Cargo.toml
[features]
agentic = [
    "foundation_db",
    "foundation_auth",
]
```

The agentic API's feature flags simply enable foundation_db and foundation_auth. Backend selection is controlled by foundation_db's own feature flags.

## Rationale

**Why no intermediate storage traits?**  
- foundation_db's types are already abstracted behind traits — adding another layer is double abstraction
- Every storage operation the agentic API needs (append, scan, get, set, vector query) maps directly to foundation_db operations
- Fewer layers = simpler code, easier to understand and maintain
- foundation_db handles backend-specific optimizations (indexing, batching, caching) — no need to re-implement

**Why direct foundation_db usage instead of wrapping?**  
- foundation_db is designed to be consumed directly — its API is already ergonomic
- Wrapping would require maintaining a parallel API that mirrors foundation_db's types
- When foundation_db adds a new backend or feature, the agentic API automatically benefits without changes

**Why auth trait instead of direct usage?**  
- Auth is a different concern — foundation_auth may not expose a trait interface the agentic API can consume directly
- The `AuthProvider` trait allows testing with mock auth without needing a full foundation_auth setup
- This is the one place where an intermediate abstraction is justified — auth is a security boundary

## Alternatives Considered

### Define agentic-specific storage traits
- **Pros:** Complete isolation from foundation_db changes
- **Cons:** Double abstraction, maintenance burden, no functional benefit since foundation_db is already trait-backed
- **Rejected because:** foundation_db's types are already swappable via their own trait system

### Built-in storage (no foundation_db)
- **Pros:** No external dependency
- **Cons:** Duplicates storage logic, inconsistent with platform standards, no multi-backend support
- **Rejected because:** foundation_db already provides everything needed

### No auth
- **Pros:** Simplest
- **Cons:** No access control, no usage tracking, no billing
- **Rejected because:** Platform serves multiple users — auth is required
