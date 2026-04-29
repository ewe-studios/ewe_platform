# Resonate -- Persistence Layer

## Overview

The persistence layer provides multi-backend storage for all durable state: promises, tasks, schedules, callbacks, listeners, timeouts, and outgoing messages. A trait-based abstraction (`Db`) defines ~42 methods that each backend implements.

## Storage Abstraction

```mermaid
graph TD
    Server[Server / Oracle] --> Db[Db Trait ~42 methods]
    Db --> SQLite[SQLite Backend]
    Db --> PostgreSQL[PostgreSQL Backend]
    Db --> MySQL[MySQL Backend]
    
    SQLite --> |rusqlite| File[Single File]
    PostgreSQL --> |sqlx| PGPool[Connection Pool]
    MySQL --> |sqlx| MyPool[Connection Pool]
```

### The Db Trait

```rust
#[async_trait]
pub trait Db: Send + Sync {
    // Health
    async fn ping(&self) -> Result<()>;
    
    // Promise operations
    async fn promise_get(&self, id: &str) -> Result<Option<PromiseRecord>>;
    async fn promise_create(&self, ...) -> Result<PromiseCreateResult>;
    async fn promise_settle(&self, ...) -> Result<PromiseSettleResult>;
    async fn promise_register_callback(&self, ...) -> Result<()>;
    async fn promise_register_listener(&self, ...) -> Result<()>;
    async fn promise_search(&self, ...) -> Result<Vec<PromiseRecord>>;
    
    // Task operations
    async fn task_get(&self, id: &str) -> Result<Option<TaskRecord>>;
    async fn task_create(&self, ...) -> Result<TaskCreateResult>;
    async fn task_acquire(&self, ...) -> Result<TaskAcquireResult>;
    async fn task_suspend(&self, ...) -> Result<()>;
    async fn task_fulfill(&self, ...) -> Result<()>;
    async fn task_release(&self, ...) -> Result<()>;
    async fn task_heartbeat(&self, ...) -> Result<()>;
    async fn task_halt(&self, ...) -> Result<()>;
    async fn task_continue(&self, ...) -> Result<()>;
    async fn task_fence_create(&self, ...) -> Result<()>;
    async fn task_fence_settle(&self, ...) -> Result<()>;
    async fn task_search(&self, ...) -> Result<Vec<TaskRecord>>;
    async fn task_compute_preload(&self, ...) -> Result<Vec<PromiseRecord>>;
    
    // Schedule operations
    async fn schedule_get(&self, id: &str) -> Result<Option<ScheduleRecord>>;
    async fn schedule_create(&self, ...) -> Result<()>;
    async fn schedule_delete(&self, id: &str) -> Result<()>;
    async fn schedule_search(&self, ...) -> Result<Vec<ScheduleRecord>>;
    
    // Timeout operations
    async fn get_expired_promise_timeouts(&self, now: i64) -> Result<Vec<String>>;
    async fn get_expired_task_timeouts(&self, now: i64) -> Result<Vec<TaskTimeout>>;
    async fn get_expired_schedule_timeouts(&self, now: i64) -> Result<Vec<String>>;
    async fn process_schedule_timeout(&self, ...) -> Result<()>;
    
    // Outgoing messages
    async fn take_outgoing(&self, batch_size: usize) -> Result<(Vec<Execute>, Vec<Unblock>)>;
    
    // Debug
    async fn debug_reset(&self) -> Result<()>;
    async fn snap(&self) -> Result<Snapshot>;
}
```

### Error Types

```rust
pub enum StorageError {
    Backend(String),        // Generic database error
    Serialization,          // Transaction conflict (retryable)
    InvalidInput(String),   // Constraint violation (400 error)
}
```

## SQLite Backend

The default backend. Single-file database using rusqlite with WAL mode.

### Database Schema (12 Tables)

```mermaid
erDiagram
    promises {
        text id PK
        text state
        text param_headers
        blob param_data
        text value_headers
        blob value_data
        text tags
        integer timeout_at
        integer created_at
        integer settled_at
    }
    
    tasks {
        text id PK
        text state
        integer version
    }
    
    callbacks {
        text awaited_id FK
        text awaiter_id FK
        integer ready
    }
    
    listeners {
        text promise_id FK
        text address
    }
    
    promise_timeouts {
        integer timeout_at
        text id FK
    }
    
    task_timeouts {
        integer timeout_at
        text id
        integer timeout_type
        text process_id
        integer ttl
    }
    
    outgoing_execute {
        text id PK
        text version
        text address
    }
    
    outgoing_unblock {
        text promise_id
        text address
    }
    
    schedules {
        text id PK
        text cron
        text promise_id
        integer promise_timeout
        text promise_param_headers
        blob promise_param_data
        text promise_tags
        integer created_at
        integer next_run_at
        integer last_run_at
    }
    
    schedule_timeouts {
        integer timeout_at
        text id FK
    }
    
    promises ||--o{ tasks : "id"
    promises ||--o{ callbacks : "awaited_id"
    promises ||--o{ callbacks : "awaiter_id"
    promises ||--o{ listeners : "promise_id"
    promises ||--o{ promise_timeouts : "id"
```

### SQLite Pragmas

```sql
PRAGMA journal_mode = WAL;      -- Write-Ahead Logging for concurrency
PRAGMA busy_timeout = 5000;     -- Wait up to 5s for locks
PRAGMA foreign_keys = ON;       -- Enforce referential integrity
PRAGMA synchronous = NORMAL;    -- Balance safety and performance
```

### Generated Columns (Tags)

The `promises` table uses generated columns for efficient querying:

```sql
CREATE TABLE promises (
    id TEXT PRIMARY KEY,
    state TEXT NOT NULL,
    -- ... data columns ...
    tags TEXT NOT NULL DEFAULT '{}',
    
    -- Generated columns from tags JSON
    target TEXT GENERATED ALWAYS AS (json_extract(tags, '$."resonate:target"')),
    origin TEXT GENERATED ALWAYS AS (json_extract(tags, '$."resonate:origin"')),
    branch TEXT GENERATED ALWAYS AS (json_extract(tags, '$."resonate:branch"')),
    timer BOOLEAN NOT NULL GENERATED ALWAYS AS (COALESCE(json_extract(tags, '$."resonate:timer'), '') = 'true') STORED
);

CREATE INDEX idx_promises_target ON promises(target);
CREATE INDEX idx_promises_timeout ON promises(timeout_at) WHERE state = 'pending';
```

### Concurrency Model

```rust
pub struct SqliteStorage {
    conn: Arc<Mutex<Connection>>,
}

impl SqliteStorage {
    async fn execute<F, R>(&self, f: F) -> Result<R>
    where
        F: FnOnce(&Connection) -> Result<R> + Send + 'static,
        R: Send + 'static,
    {
        let conn = self.conn.clone();
        tokio::task::block_in_place(|| {
            let guard = conn.lock().unwrap_or_else(|e| e.into_inner());
            f(&guard)
        })
    }
}
```

SQLite runs synchronously within `block_in_place()` on the tokio threadpool. A single Mutex-wrapped connection provides serialized access. The `unwrap_or_else` pattern recovers from poisoned mutexes (if a previous operation panicked).

### Settlement Chain (Atomic)

The settlement chain runs as a single SQLite transaction:

```sql
BEGIN;

-- 1. Settle promise
UPDATE promises SET state = ?, value_headers = ?, value_data = ?, settled_at = ?
WHERE id = ? AND state = 'pending';

-- 2. Delete promise timeout
DELETE FROM promise_timeouts WHERE id = ?;

-- 3. Fulfill dependent tasks
UPDATE tasks SET state = 'fulfilled' WHERE id = ?;
DELETE FROM task_timeouts WHERE id = ?;
DELETE FROM callbacks WHERE awaiter_id = ?;

-- 4. Mark callbacks ready and resume
UPDATE callbacks SET ready = 1 WHERE awaited_id = ?;

-- For each suspended task with ALL callbacks ready:
-- (No unready callbacks remain)
UPDATE tasks SET state = 'pending' WHERE id = ? AND state = 'suspended';
INSERT INTO outgoing_execute (id, version, address) VALUES (?, ?, ?);

-- 5. Notify listeners
INSERT INTO outgoing_unblock (promise_id, address)
    SELECT ?, address FROM listeners WHERE promise_id = ?;
DELETE FROM listeners WHERE promise_id = ?;

COMMIT;
```

## PostgreSQL Backend

Uses `sqlx` with async connection pooling. Key differences from SQLite:

### Connection Pool

```rust
pub struct PostgresStorage {
    pool: PgPool,
}

// Configuration
let pool = PgPoolOptions::new()
    .max_connections(config.pool_size)  // default: 10
    .connect(&config.url)
    .await?;
```

### CTE-Based Operations

PostgreSQL uses Common Table Expressions for atomic multi-step operations:

```sql
WITH settled AS (
    UPDATE promises SET state = $2, value_headers = $3, value_data = $4, settled_at = $5
    WHERE id = $1 AND state = 'pending'
    RETURNING id
),
timeout_deleted AS (
    DELETE FROM promise_timeouts WHERE id IN (SELECT id FROM settled)
),
callbacks_updated AS (
    UPDATE callbacks SET ready = true WHERE awaited_id IN (SELECT id FROM settled)
    RETURNING awaiter_id
),
tasks_resumed AS (
    UPDATE tasks SET state = 'pending'
    WHERE id IN (
        SELECT awaiter_id FROM callbacks_updated
        WHERE NOT EXISTS (
            SELECT 1 FROM callbacks WHERE awaiter_id = callbacks_updated.awaiter_id AND ready = false
        )
    )
    RETURNING id
)
INSERT INTO outgoing_execute (id, version, address)
SELECT t.id, t.version, p.target
FROM tasks_resumed tr
JOIN tasks t ON t.id = tr.id
JOIN promises p ON p.id = t.id;
```

### Row-Level Locking (FOR UPDATE)

PostgreSQL uses `SELECT ... FOR UPDATE` to lock tasks during acquisition, preventing concurrent workers from acquiring the same task:

```sql
-- Acquire: lock the task row, bump version, set timeout
UPDATE tasks SET state = 'acquired', version = version + 1
WHERE id = $1 AND state = 'pending' AND version = $2
RETURNING *;
```

The version check acts as an optimistic lock — if another worker already bumped the version, the UPDATE returns zero rows and acquisition fails.

### Serialization Conflict Retry

```rust
const MAX_RETRIES: u32 = 3;

async fn with_retry<F>(&self, f: F) -> Result<T> {
    for attempt in 0..MAX_RETRIES {
        match f().await {
            Ok(v) => return Ok(v),
            Err(e) if is_serialization_error(&e) => {
                // PostgreSQL error codes: 40001, 40P01
                tokio::time::sleep(Duration::from_millis(10 * 2u64.pow(attempt))).await;
                continue;
            }
            Err(e) => return Err(e),
        }
    }
    Err(StorageError::Serialization)
}
```

## MySQL Backend

Similar to PostgreSQL, uses `sqlx` with connection pooling. Adapted for MySQL syntax differences:

- No `RETURNING` clause — uses separate SELECT after INSERT
- Different error codes for serialization conflicts
- `ON DUPLICATE KEY UPDATE` instead of `ON CONFLICT`

## Timeout Tables

Three separate timeout tables enable efficient scanning:

### Promise Timeouts

```sql
CREATE TABLE promise_timeouts (
    timeout_at INTEGER NOT NULL,
    id TEXT PRIMARY KEY REFERENCES promises(id)
);

-- Scan: find all expired
SELECT id FROM promise_timeouts WHERE timeout_at <= ?;
```

### Task Timeouts

```sql
CREATE TABLE task_timeouts (
    timeout_at INTEGER NOT NULL,
    id TEXT NOT NULL,
    timeout_type INTEGER NOT NULL,  -- 0 = retry, 1 = lease
    process_id TEXT,
    ttl INTEGER NOT NULL,
    PRIMARY KEY (id)
);

-- type 0 (retry): task was released, waiting for retry delay
-- type 1 (lease): task is acquired, lease will expire
```

### Schedule Timeouts

```sql
CREATE TABLE schedule_timeouts (
    timeout_at INTEGER NOT NULL,
    id TEXT PRIMARY KEY REFERENCES schedules(id)
);

-- When timeout fires: create new promise from schedule template
-- Then compute next_run_at and insert new timeout
```

## Preload Mechanism

When a task is acquired, the server returns all resolved promises in the same branch. This enables replay without network round-trips:

```sql
-- Compute preload: all resolved promises sharing the same branch
SELECT * FROM promises
WHERE branch = (SELECT branch FROM promises WHERE id = ?)
  AND state IN ('resolved', 'rejected', 'rejected_canceled', 'rejected_timedout');
```

The SDK caches these locally so that replayed `ctx.run()` calls return immediately.

## Backend Selection

```toml
# SQLite (default, good for dev and low-medium traffic)
[storage]
type = "sqlite"
[storage.sqlite]
path = "resonate.db"

# PostgreSQL (recommended for production)
[storage]
type = "postgres"
[storage.postgres]
url = "postgres://user:pass@localhost:5432/resonate"
pool_size = 20

# MySQL
[storage]
type = "mysql"
[storage.mysql]
url = "mysql://user:pass@localhost:3306/resonate"
pool_size = 20
```

### Backend Comparison

| Feature | SQLite | PostgreSQL | MySQL |
|---------|--------|-----------|-------|
| Setup | Zero (single file) | Requires server | Requires server |
| Concurrency | Single writer | High (MVCC) | High (InnoDB) |
| Scaling | Vertical only | Horizontal (replicas) | Horizontal (replicas) |
| Best for | Dev, single-node | Production, multi-node | Production, multi-node |
| Connection model | Mutex-wrapped | Pool (async) | Pool (async) |
| Transaction isolation | Serialized | Read Committed | Read Committed |

## Source Paths

| File | Purpose |
|------|---------|
| `src/persistence/mod.rs` | Db trait, Storage enum, error types |
| `src/persistence/persistence_sqlite.rs` | SQLite implementation |
| `src/persistence/persistence_postgres.rs` | PostgreSQL implementation |
| `src/persistence/persistence_mysql.rs` | MySQL implementation |
