---
workspace_name: "ewe_platform"
spec_directory: "specifications/52-tauri-foundation-platform"
feature_directory: "specifications/52-tauri-foundation-platform/features/F08-mutation-queue"
this_file: "specifications/52-tauri-foundation-platform/features/F08-mutation-queue/feature.md"

status: pending
priority: medium
created: 2026-07-17

depends_on:
  - "F01-session-backbone"

tasks:
  completed: 0
  uncompleted: 4
  total: 4
  completion_percentage: 0%
---

# F08 — LWW mutation queue (MVP)

## Overview

Implement the offline mutation queue with last-write-wins (LWW) conflict
resolution. Mutations enqueued locally in SQLite when offline, replayed in
order when connectivity returns. MVP uses LWW (timestamp comparison, server
wins ties). Custom conflict resolvers are post-MVP.

[Decision 12](../decisions/12-mutation-queue-and-conflict.md) defines the full
model. MVP ships LWW default only.

## Dependencies

Depends on:
- `F01-session-backbone` — Queue lives on the session, replay on connectivity

Required by:
- `F09-walking-skeleton` — Offline mutation in end-to-end test

## Requirements

### 1. `MutationQueue`

```rust
// foundation_platform/src/mutation.rs

pub struct MutationQueue {
    db: foundation_db::Database,
}

struct Mutation {
    id: Uuid,           // Idempotency key
    created_at: DateTime<Utc>,
    mutation_type: String,  // e.g., "order_update", "item_delete"
    payload: serde_json::Value,
    status: MutationStatus,
}

enum MutationStatus {
    Pending,
    Applied(Uuid),
    Conflict(String),
    Failed(String),
    RequiresUserResolution,  // post-MVP
}
```

### 2. Enqueue

```rust
impl MutationQueue {
    pub async fn enqueue(&self, mutation: Mutation) -> Result<()> {
        mutation.validate_local()?;
        self.db.insert("mutations", &mutation).await
    }
}
```

### 3. Replay (LWW)

```rust
impl MutationQueue {
    pub async fn replay(&self, session: &PlatformSession) -> Result<ReplayResult> {
        let pending = self.db.query(
            "SELECT * FROM mutations WHERE status = 'pending' ORDER BY created_at"
        ).await?;

        let mut results = Vec::new();
        for mutation in pending {
            let result = session.remote()
                .post("/api/mutations", &mutation).await;

            match result {
                Ok(_) => {
                    self.db.update_status(&mutation.id, MutationStatus::Applied).await?;
                    results.push(ReplayStatus::Applied(mutation.id));
                }
                Err(e) if e.is_conflict() => {
                    // MVP: LWW — server response wins
                    self.db.update_status(&mutation.id, MutationStatus::Conflict(
                        e.server_state.clone()
                    )).await?;
                    results.push(ReplayStatus::Conflict(mutation.id, e.server_state));
                }
                Err(e) => {
                    self.db.update_status(&mutation.id, MutationStatus::Failed(e.to_string())).await?;
                    results.push(ReplayStatus::Failed(mutation.id, e.to_string()));
                    break; // stop on non-retryable error
                }
            }
        }
        Ok(ReplayResult { results })
    }
}
```

### 4. Replay triggers

Replay fires on connectivity restore:

```rust
// In PlatformSession:
session.on_connectivity_change(|session, online| {
    if online {
        let queue = session.mutation_queue();
        tokio::spawn(async move {
            queue.replay(&session).await;
        });
    }
});
```

### 5. Idempotency

Every mutation carries a UUID. Server deduplicates by UUID. The platform
generates UUIDs — the user doesn't need to.

## Tasks

### Queue storage
- [ ] Create `src/mutation.rs` with `MutationQueue` struct
- [ ] Create SQLite schema for mutations table
- [ ] Implement `enqueue()` with local validation
- [ ] Implement `replay()` with LWW conflict resolution
- [ ] Test: mutation enqueued, replayed on connectivity restore

### Replay triggers
- [ ] Wire replay to connectivity change events
- [ ] Test: offline mutations replay when connectivity returns

### Idempotency
- [ ] UUID generation per mutation
- [ ] Server-side dedup contract documented

### Durability
- [ ] SQLite WAL for crash-safe queue storage
- [ ] Test: mutations survive app kill

## Verification Commands

```bash
cargo test --package foundation_platform -- mutation
```
