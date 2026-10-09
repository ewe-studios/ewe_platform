---
workspace_name: "ewe_platform"
spec_directory: "specifications/52-tauri-foundation-platform"
feature_directory: "specifications/52-tauri-foundation-platform/features/F08-mutation-queue"
this_file: "specifications/52-tauri-foundation-platform/features/F08-mutation-queue/feature.md"

status: completed
priority: medium
created: 2026-07-17
updated: 2026-07-21

depends_on:
  - "F01-session-backbone"

tasks:
  completed: 5
  uncompleted: 0
  total: 5
  completion_percentage: 100%
---

# F08 — LWW mutation queue (MVP)

## Overview

Implement offline mutation queue with last-write-wins default. Mutations
enqueued in SQLite when offline, replayed in FIFO order on connectivity
restore. UUID-based idempotency. Custom conflict resolvers are post-MVP.

[Decision 12](../decisions/12-mutation-queue-and-conflict.md). MVP = LWW only.

---

## Part A — MutationQueue

```rust
// foundation_platform/src/mutation.rs

pub struct MutationQueue { db: foundation_db::Database }

pub struct Mutation {
    pub id: Uuid,
    pub created_at: i64,
    pub mutation_type: String,
    pub payload: serde_json::Value,
    pub status: MutationStatus,
}

pub enum MutationStatus { Pending, Applied, Conflict(String), Failed(String) }

impl MutationQueue {
    pub async fn enqueue(&self, m: Mutation) -> Result<()> { ... }
    pub async fn replay(&self, session: &PlatformSession) -> Result<ReplayResult> { ... }
}
```

### A.2 — Replay triggers

```rust
session.on_connectivity_change(|session, online| {
    if online { tokio::spawn(async { session.mutation_queue().replay(&session).await; }); }
});
```

### A.3 — Replay lifecycle

```
Offline → mutations enqueued (UUID + created_at + validated locally)
Reconnected → worker replays FIFO → server deduplicates by UUID
  Success → deleted from queue
  Conflict → LWW (server wins, mutation flagged)
  Non-retryable error → replay stops
```

### A.4 — Idempotency

Each mutation carries a UUID. Server deduplicates. Platform generates UUIDs.

---

## Verification
```bash
cargo test --package foundation_platform -- mutation
```
