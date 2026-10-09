# 12 — Mutation queue and conflict resolution

**Date:** 2026-07-17
**Status:** Resolved

## Decision

When the user performs an action offline, mutations are enqueued locally in
SQLite. Each mutation has a UUID for idempotency. On connectivity restore, the
queue replays in order. The platform provides queue storage, replay triggers,
and conflict detection primitives. The user defines what goes in the queue,
how conflicts resolve, and what mutations are valid offline.

This was split from [decision 05](05-offline-and-sync.md) — cache tiers are a
separate concern. Workers that drive mutation replay are covered in
[decision 11](11-background-workers.md).

## Table of Contents

1. [Mutation queue](#mutation-queue)
2. [Conflict resolution model](#conflict-resolution-model)
3. [Replay lifecycle](#replay-lifecycle)
4. [What the platform provides vs what the user provides](#what-the-platform-provides-vs-what-the-user-provides)

---

## Mutation queue

```rust
struct MutationQueue {
    db: foundation_db::Database,
}

impl MutationQueue {
    /// Enqueue a mutation while offline. Validated locally before enqueue.
    async fn enqueue(&self, mutation: Mutation) -> Result<()> {
        mutation.validate_local()?;
        self.db.insert("mutations", &mutation).await
    }

    /// Replay all pending mutations. Called on connectivity restore.
    /// Order-preserving, idempotent (each mutation has a UUID).
    async fn replay(&self, session: &PlatformSession) -> Result<ReplayResult> {
        let pending = self.db.query(
            "SELECT * FROM mutations ORDER BY created_at"
        ).await?;
        let mut results = Vec::new();
        for mutation in pending {
            let result = session.remote()
                .post("/api/mutations", &mutation).await;
            match result {
                Ok(_) => {
                    self.db.delete("mutations", &mutation.id).await?;
                    results.push(ReplayStatus::Applied(mutation.id));
                }
                Err(e) if e.is_conflict() => {
                    results.push(ReplayStatus::Conflict(
                        mutation.id, e.detail
                    ));
                }
                Err(e) => {
                    results.push(ReplayStatus::Failed(
                        mutation.id, e.to_string()
                    ));
                    break; // stop replay on non-retryable error
                }
            }
        }
        Ok(ReplayResult { results })
    }
}
```

Queue storage is local SQLite. Each mutation has:
- **UUID** — idempotency key. The server deduplicates by UUID.
- **Created-at timestamp** — ordering for replay.
- **Type** — the mutation kind (create, update, delete, action).
- **Payload** — the mutation data.
- **Status** — `pending`, `replayed`, `conflict`, `failed`.

---

## Conflict resolution model

The platform provides primitives. The user defines the strategy per mutation
type.

### Platform primitives

| Primitive | Description |
|---|---|
| **Version vectors** | Per-entity causal ordering. `{entity_id: clock_value}`. Concurrent mutations are detectable. |
| **Timestamp comparison** | `last_modified` on each mutation. Server provides `server_timestamp` on conflict. |
| **Last-write-wins (LWW)** | Default strategy. Timestamp comparison, server wins ties. |

### User-defined strategies

Users can override the default LWW strategy per mutation type:

```rust
#[mutation_conflict(strategy = "custom")]
fn resolve_order_conflict(
    local: &Mutation,
    server: &Mutation,
    context: &ConflictContext,
) -> ConflictResolution {
    if local.payload.priority > server.payload.priority {
        ConflictResolution::UseLocal
    } else {
        ConflictResolution::UseServer
    }
}
```

Common resolution patterns:
- **Merge** — combine fields from both mutations.
- **Server wins** — discard local, accept server state.
- **Local wins** — re-apply local mutation on top of server state.
- **Ask user** — flag for UI resolution, don't block replay.

**Post-MVP.** Conflict resolver routing, client-side race handling, and the
"ask user" UX flow are deferred. For MVP, LWW (last-write-wins) with
timestamp comparison covers the common case. The full resolution model
(registrered resolvers by mutation type, `#[mutation_conflict]` dispatch,
in-background user resolution) is specified here but implementation is
post-MVP.

---

## Replay lifecycle

```
App goes offline
  │
  ├── User performs actions
  ├── Mutations validated locally
  ├── Enqueued in SQLite with UUID
  │
  ▼ Connectivity restored
Online (reconnected)
  │
  ├── Worker receives SyncEvent::Online
  ├── Queue replays in order (oldest first)
  ├── Server deduplicates by UUID
  │
  ├── Success → mutation deleted from queue
  ├── Conflict → conflict resolver invoked
  │     ├── Resolved → replayed mutation applied
  │     └── Unresolved → queued for user resolution
  └── Non-retryable error → replay stops, error surfaced
```

Replay is triggered by connectivity restore events from the session backbone
([decision 03](03-session-backbone-transport.md)).
The worker ([decision 11](11-background-workers.md)) handles the actual replay
execution — the queue is passive storage; the worker is the active driver.

Queue storage uses `foundation_db`'s SQLite backend with WAL journaling — the
same database infrastructure used by the cache ([decision 05](05-offline-and-sync.md)).
Durability is inherent: SQLite WAL survives app kill.

---

## What the platform provides vs what the user provides

| Layer | Platform provides | User provides |
|---|---|---|
| **Queue storage** | SQLite schema (mutations table). `enqueue()`, `replay()`, `delete()`. Replay triggers on connectivity change. | What goes in the queue. |
| **Conflict detection** | Version vectors, timestamp comparison, LWW default. Conflict surfaced to user code. | Custom conflict resolution per mutation type. |
| **Idempotency** | UUID generation. Server-side dedup contract (UUID in request header). | None — platform handles it. |
| **Validation** | Schema validation (well-formed JSON, required fields). | Business logic validation (`validate_local()`). |
| **Replay ordering** | FIFO by `created_at`. Guaranteed order. | Whether replay should stop on first error or continue. |
| **Durability** | SQLite WAL. Survives app kill. | None — platform handles it. |
