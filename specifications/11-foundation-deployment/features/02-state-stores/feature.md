---
workspace_name: "ewe_platform"
spec_directory: "specifications/11-foundation-deployment"
feature_directory: "specifications/11-foundation-deployment/features/02-state-stores"
this_file: "specifications/11-foundation-deployment/features/02-state-stores/feature.md"

status: pending
priority: high
created: 2026-03-26

depends_on: ["01-foundation-deployment-core"]

tasks:
  completed: 0
  uncompleted: 9
  total: 9
  completion_percentage: 0%
---


# State Stores

## Iron Law: Zero Warnings

> **All code must compile with zero warnings and pass all lints. No suppression. No exceptions.**
>
> - `cargo clippy -p foundation_db -- -D warnings -W clippy::pedantic` — zero warnings
> - `cargo doc -p foundation_db --no-deps` — zero rustdoc warnings
> - `cargo test -p foundation_db` — zero compilation warnings
> - **No `#[allow(...)]`, `#[expect(...)]`, or `#![allow(...)]` anywhere.** Fix the code, never suppress.

## Overview

Implement the state management layer inspired by alchemy's `StateStore` interface. **Implementation lives in `foundation_db` crate** since all state backends are database/storage abstractions. The `foundation_deployment` crate imports and uses these stores via a feature flag or direct dependency.

Six interchangeable backends:

1. **LibSQLStateStore** - Local embedded libSQL database (no external dependencies)
2. **TursoStateStore** - Remote Turso database with optional embedded replica for local caching
3. **JsonFileStateStore** - JSON files in `.deployment/` directory, git-friendly
4. **R2StateStore** - Cloudflare R2 object storage, S3-compatible, stores state as JSON objects via Cloudflare API
5. **D1StateStore** - Cloudflare D1 edge SQLite database, stores state via Cloudflare API
6. **SqliteStateStore** - Plain local SQLite via libsql (local-only mode, no sync)

All backends implement the same `StateStore` trait from `foundation_db`. Any backend works with any provider. The state store is the **source of truth** for what's deployed — not config files. State stores track deployment resources across all providers, enabling change detection (skip deploy if config unchanged), resource cleanup (destroy orphans), and cross-machine state sharing.

The R2 and D1 backends are particularly useful when deploying Cloudflare Workers — state lives in the same ecosystem. But they are **not** coupled to the Cloudflare provider; they can store state for any provider.

**Note on SQLite backends:** Both `LibSQLStateStore` and `SqliteStateStore` use the `libsql` crate (Turso's SQLite fork). The difference:
- `LibSQLStateStore` - Local embedded libSQL, can optionally sync to remote Turso
- `SqliteStateStore` - Local-only SQLite via libsql, no remote sync capability
- `TursoStateStore` - Remote-first Turso with optional embedded replica for caching

## Dependencies

**Implementation:** `foundation_db` crate
- Depends on `foundation_core` for `SimpleHttpClient`
- `foundation_deployment` imports `foundation_db` for state store access

**Required by:**
- `03-deployment-engine` - Engine reads/writes state during deployment
- `04-cloudflare-provider`, `05-gcp-cloud-run-provider`, `06-aws-lambda-provider` - Providers update state

## Requirements

### StateStore Trait

All methods that perform I/O return `StateStoreStream<T>` — a lazy, composable
iterator backed by `run_future_iter` for async backends, or `Vec::into_iter().map(...)`
for sync backends. This is the **default pattern for all async operations** in this
crate (see `fundamentals/00-overview.md` § Valtron Async Bridge Policy).

**WHY streams everywhere (not just multi-value ops):**
- `exec_future` eagerly collects results into `Vec<T>` inside the async block — you pay
  the full memory cost upfront, even for a single row. `run_future_iter` streams lazily.
- Returning iterators from single-value ops (`get`, `set`, `delete`) keeps the API uniform
  and composable. Callers chain, filter, and merge state store operations without special
  cases. The blocking decision lives at the caller's boundary, not inside the store.

**Exception:** `init()` is one-shot bootstrap (schema creation, directory setup). It uses
`exec_future` and returns `Result<(), DeploymentError>` directly.

```rust
// state/traits.rs

use foundation_core::valtron::ThreadedValue;

/// Lazy stream of state store results.
///
/// WHY Box<dyn>: trait methods need a common return type across all backends.
/// SQL backends produce this from `run_future_iter`, sync backends from
/// `Vec::into_iter().map(ThreadedValue::Value)`.
pub type StateStoreStream<T> = Box<dyn Iterator<Item = ThreadedValue<T, DeploymentError>> + Send>;

/// Trait for deployment state persistence backends.
/// Inspired by alchemy's StateStore interface.
///
/// All I/O methods return `StateStoreStream<T>` for composability and lazy evaluation.
/// Callers consume streams at their own boundary — the store never blocks internally.
pub trait StateStore: Send + Sync {
    /// Initialize the store (create tables, directories, etc.).
    /// This is the one exception: uses `exec_future` (one-shot bootstrap).
    fn init(&self) -> Result<(), DeploymentError>;

    /// List all resource IDs. Returns a lazy stream of IDs.
    fn list(&self) -> Result<StateStoreStream<String>, DeploymentError>;

    /// Count resources. Returns a stream yielding a single `usize`.
    fn count(&self) -> Result<StateStoreStream<usize>, DeploymentError>;

    /// Get state for a single resource. Stream yields 0 or 1 `Option<ResourceState>`.
    fn get(&self, resource_id: &str) -> Result<StateStoreStream<Option<ResourceState>>, DeploymentError>;

    /// Get state for multiple resources. Stream yields one `ResourceState` per match.
    fn get_batch(&self, ids: &[&str]) -> Result<StateStoreStream<ResourceState>, DeploymentError>;

    /// Get all resource states. Stream yields one `ResourceState` per row.
    fn all(&self) -> Result<StateStoreStream<ResourceState>, DeploymentError>;

    /// Set (create or update) state. Stream yields `()` on completion.
    fn set(&self, resource_id: &str, state: &ResourceState) -> Result<StateStoreStream<()>, DeploymentError>;

    /// Delete state. Stream yields `()` on completion.
    fn delete(&self, resource_id: &str) -> Result<StateStoreStream<()>, DeploymentError>;

    /// Sync state to a remote location (no-op for FileStateStore).
    fn sync_remote(&self) -> Result<(), DeploymentError> {
        Ok(()) // Default: no remote sync
    }
}

// =========================================================================
// Stream consumption helpers (state/helpers.rs)
// =========================================================================

/// Extract the first successful value from a state store stream.
/// Logs and skips errors. Returns `None` if the stream is empty or all errors.
pub fn collect_first<T>(stream: StateStoreStream<T>) -> Result<Option<T>, DeploymentError> {
    for item in stream {
        match item {
            ThreadedValue::Value(Ok(val)) => return Ok(Some(val)),
            ThreadedValue::Value(Err(e)) => return Err(e),
        }
    }
    Ok(None)
}

/// Collect all successful values from a state store stream.
/// Returns the first error encountered, if any.
pub fn collect_all<T>(stream: StateStoreStream<T>) -> Result<Vec<T>, DeploymentError> {
    let mut results = Vec::new();
    for item in stream {
        match item {
            ThreadedValue::Value(Ok(val)) => results.push(val),
            ThreadedValue::Value(Err(e)) => return Err(e),
        }
    }
    Ok(results)
}

/// Drive a write stream to completion, consuming all items.
/// Returns the first error, or `Ok(())` if all items succeeded.
pub fn drive_to_completion(stream: StateStoreStream<()>) -> Result<(), DeploymentError> {
    for item in stream {
        match item {
            ThreadedValue::Value(Ok(())) => {}
            ThreadedValue::Value(Err(e)) => return Err(e),
        }
    }
    Ok(())
}
```

### ResourceState

```rust
// state/types.rs

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceState {
    /// Unique resource identifier (e.g. "my-worker", "my-cloud-run-service").
    pub id: String,

    /// Resource kind (e.g. "cloudflare::worker", "gcp::cloud-run-service", "aws::lambda-function").
    pub kind: String,

    /// Provider name ("cloudflare", "gcp", "aws").
    pub provider: String,

    /// Current lifecycle status.
    pub status: StateStatus,

    /// Deployment environment (staging, production, etc.).
    pub environment: Option<String>,

    /// SHA-256 hash of the serialized input config at time of deploy.
    /// Used for change detection: if hash matches, skip deployment.
    pub config_hash: String,

    /// Provider-specific output data (deployment ID, URL, etc.).
    pub output: serde_json::Value,

    /// Serialized input config (for inspection/debugging).
    pub config_snapshot: serde_json::Value,

    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum StateStatus {
    Creating,
    Created,
    Updating,
    Deleting,
    Deleted,
    Failed { error: String },
}

impl ResourceState {
    /// Check if config has changed by comparing hashes.
    pub fn config_changed(&self, new_config_hash: &str) -> bool {
        self.config_hash != new_config_hash
    }

    /// Check if this resource needs deployment.
    pub fn needs_deploy(&self, new_config_hash: &str) -> bool {
        match &self.status {
            StateStatus::Created => self.config_changed(new_config_hash),
            StateStatus::Failed { .. } => true,
            _ => true, // Creating, Updating, Deleting all need attention
        }
    }
}
```

### FileStateStore

Sync backend — no Valtron needed. Returns `StateStoreStream<T>` built from
`Vec::into_iter().map(|v| ThreadedValue::Value(Ok(v)))` for trait consistency.

```rust
// state/file.rs

use std::path::{Path, PathBuf};
use foundation_core::valtron::ThreadedValue;

/// JSON file-based state store.
/// Stores each resource as a separate JSON file:
///   .deployment/{provider}/{stage}/{resource_id}.json
///
/// Simple, git-friendly, no dependencies. Purely synchronous — no Valtron.
/// Returns StateStoreStream for trait consistency, built from Vec iterators.
pub struct FileStateStore {
    root_dir: PathBuf,
    provider: String,
    stage: String,
}

impl FileStateStore {
    pub fn new(project_dir: &Path, provider: &str, stage: &str) -> Self {
        Self {
            root_dir: project_dir.join(".deployment").join(provider).join(stage),
            provider: provider.to_string(),
            stage: stage.to_string(),
        }
    }

    fn resource_path(&self, resource_id: &str) -> PathBuf {
        let safe_id = resource_id.replace('/', ":");
        self.root_dir.join(format!("{safe_id}.json"))
    }

    /// Wrap a single value into a StateStoreStream (sync convenience).
    fn wrap_value<T: Send + 'static>(val: T) -> StateStoreStream<T> {
        Box::new(std::iter::once(ThreadedValue::Value(Ok(val))))
    }

    /// Wrap a Vec into a StateStoreStream (sync convenience).
    fn wrap_vec<T: Send + 'static>(vals: Vec<T>) -> StateStoreStream<T> {
        Box::new(vals.into_iter().map(|v| ThreadedValue::Value(Ok(v))))
    }
}

impl StateStore for FileStateStore {
    fn init(&self) -> Result<(), DeploymentError> {
        std::fs::create_dir_all(&self.root_dir)?;
        Ok(())
    }

    fn get(&self, resource_id: &str) -> Result<StateStoreStream<Option<ResourceState>>, DeploymentError> {
        let path = self.resource_path(resource_id);
        if !path.exists() {
            return Ok(Self::wrap_value(None));
        }
        let content = std::fs::read_to_string(&path)?;
        let state: ResourceState = serde_json::from_str(&content)
            .map_err(|e| DeploymentError::StateFailed(e.to_string()))?;
        Ok(Self::wrap_value(Some(state)))
    }

    fn set(&self, resource_id: &str, state: &ResourceState) -> Result<StateStoreStream<()>, DeploymentError> {
        self.init()?;
        let path = self.resource_path(resource_id);
        let content = serde_json::to_string_pretty(state)
            .map_err(|e| DeploymentError::StateFailed(e.to_string()))?;
        std::fs::write(&path, content)?;
        Ok(Self::wrap_value(()))
    }

    fn delete(&self, resource_id: &str) -> Result<StateStoreStream<()>, DeploymentError> {
        let path = self.resource_path(resource_id);
        if path.exists() {
            std::fs::remove_file(&path)?;
        }
        Ok(Self::wrap_value(()))
    }

    fn list(&self) -> Result<StateStoreStream<String>, DeploymentError> {
        let mut ids = Vec::new();
        if self.root_dir.exists() {
            for entry in std::fs::read_dir(&self.root_dir)? {
                let entry = entry?;
                if let Some(name) = entry.path().file_stem() {
                    ids.push(name.to_string_lossy().replace(':', "/"));
                }
            }
        }
        Ok(Self::wrap_vec(ids))
    }

    // count, get_batch, all — built on list + get, returning StateStoreStream
}
```

### SqliteStateStore (Local-only via libsql)

Async backend — uses `run_future_iter` for all trait methods, `exec_future` only
for `init()`. Connection wrapped in `Arc<libsql::Connection>`, cloned into each
async block. `!Send` row iterators stay on the worker thread via `RowsIterator`.

```rust
// state/sqlite.rs

use std::path::{Path, PathBuf};
use std::sync::Arc;
use foundation_core::valtron::{run_future_iter, ThreadedValue};

/// Plain local SQLite state store using libsql (Turso's SQLite fork).
/// This is a local-only store with no remote sync capability.
///
/// Schema:
///   CREATE TABLE resources (
///     id TEXT PRIMARY KEY,
///     kind TEXT NOT NULL,
///     provider TEXT NOT NULL,
///     status TEXT NOT NULL,
///     environment TEXT,
///     config_hash TEXT NOT NULL,
///     output TEXT NOT NULL,         -- JSON
///     config_snapshot TEXT NOT NULL, -- JSON
///     created_at TEXT NOT NULL,
///     updated_at TEXT NOT NULL
///   );
///
/// Default path: `.deployment/state.db`
///
/// All trait methods use `run_future_iter` — the !Send `libsql::Rows` iterator
/// stays on the worker thread, only `Send` results cross the channel boundary.
pub struct SqliteStateStore {
    conn: Arc<libsql::Connection>,
    db_path: PathBuf,
}

impl SqliteStateStore {
    pub fn new(db_path: PathBuf) -> Result<Self, DeploymentError> {
        // Connection init uses exec_future (one-shot bootstrap)
        let conn = /* exec_future to create connection */;
        Ok(Self { conn: Arc::new(conn), db_path })
    }

    pub fn from_env(project_dir: &Path) -> Result<Self, DeploymentError> {
        let db_path = std::env::var("DEPLOYMENT_STATE_DB")
            .map(PathBuf::from)
            .unwrap_or_else(|_| project_dir.join(".deployment/state.db"));
        Self::new(db_path)
    }
}

impl StateStore for SqliteStateStore {
    fn init(&self) -> Result<(), DeploymentError> {
        // One-shot bootstrap — exec_future is acceptable here
        std::fs::create_dir_all(self.db_path.parent().unwrap())?;
        exec_future(async move {
            conn.execute_batch("PRAGMA journal_mode=WAL;").await?;
            conn.execute(
                "CREATE TABLE IF NOT EXISTS resources (...)",
                [],
            ).await?;
            Ok::<_, libsql::Error>(())
        })?;
        Ok(())
    }

    fn get(&self, resource_id: &str) -> Result<StateStoreStream<Option<ResourceState>>, DeploymentError> {
        let id = resource_id.to_string();    // Own before async boundary
        let conn = Arc::clone(&self.conn);   // Clone the Arc

        let iter = run_future_iter(
            move || async move {
                let mut stmt = conn.prepare("SELECT * FROM resources WHERE id = ?").await?;
                let rows = stmt.query([id]).await?;
                // RowsIterator: !Send, stays on worker thread
                // Yields Result<Option<ResourceState>, Error> per row
                Ok::<_, libsql::Error>(ResourceRowsIterator::new_single(rows))
            },
            None, // default queue size
            None, // default backpressure sleep
        ).map_err(|e| DeploymentError::StateFailed(format!("Valtron scheduling failed: {e}")))?;

        Ok(Box::new(iter))
    }

    fn list(&self) -> Result<StateStoreStream<String>, DeploymentError> {
        let conn = Arc::clone(&self.conn);

        let iter = run_future_iter(
            move || async move {
                let mut stmt = conn.prepare("SELECT id FROM resources").await?;
                let rows = stmt.query([]).await?;
                Ok::<_, libsql::Error>(IdRowsIterator::new(rows))
            },
            None,
            None,
        ).map_err(|e| DeploymentError::StateFailed(format!("Valtron scheduling failed: {e}")))?;

        Ok(Box::new(iter))
    }

    fn set(&self, resource_id: &str, state: &ResourceState) -> Result<StateStoreStream<()>, DeploymentError> {
        let id = resource_id.to_string();
        let state_json = serde_json::to_string(state)
            .map_err(|e| DeploymentError::StateFailed(e.to_string()))?;
        let conn = Arc::clone(&self.conn);

        let iter = run_future_iter(
            move || async move {
                conn.execute(
                    "INSERT OR REPLACE INTO resources (...) VALUES (...)",
                    [/* params from state_json */],
                ).await?;
                // Single-item iterator yielding ()
                Ok::<_, libsql::Error>(std::iter::once(Ok(())))
            },
            None,
            None,
        ).map_err(|e| DeploymentError::StateFailed(format!("Valtron scheduling failed: {e}")))?;

        Ok(Box::new(iter))
    }

    // delete, count, get_batch, all — same pattern: run_future_iter + RowsIterator
}
```

**RowsIterator pattern (shared across SQL backends):**

```rust
/// Iterator over libsql::Rows that stays on the worker thread (!Send).
/// Converts each row to a ResourceState. Polls async `rows.next()` to completion
/// before yielding — see PROPOSAL_ROWS_STREAMING.md for the full design.
pub struct ResourceRowsIterator {
    rows: libsql::Rows,
    current_future: Option<Pin<Box<dyn Future<Output = Result<Option<libsql::Row>, libsql::Error>>>>>,
}

impl Iterator for ResourceRowsIterator {
    type Item = Result<ResourceState, DeploymentError>;

    fn next(&mut self) -> Option<Self::Item> {
        // Poll rows.next() in a loop until Ready (see foundation_db LEARNINGS.md)
        // Convert row columns → ResourceState
        // Return None when rows exhausted
    }
}
```

### LibSQLStateStore (Embedded with optional Turso sync)

Same `run_future_iter` pattern as `SqliteStateStore`. Connection in `Arc<libsql::Connection>`.
Shares `ResourceRowsIterator` and `IdRowsIterator` with other SQL backends.

```rust
// state/libsql.rs

use std::path::{Path, PathBuf};
use std::sync::Arc;
use foundation_core::valtron::run_future_iter;

/// Local embedded libSQL state store with optional remote Turso sync.
///
/// Modes:
///   1. Local-only: behaves like SqliteStateStore (no sync)
///   2. Embedded replica: local file with automatic background sync to Turso
///
/// All trait methods use `run_future_iter`. Connection in `Arc<libsql::Connection>`.
/// Shares RowsIterator types with SqliteStateStore and TursoStateStore.
pub struct LibSQLStateStore {
    conn: Arc<libsql::Connection>,
    local_path: PathBuf,
    turso_url: Option<String>,
    turso_auth_token: Option<String>,
}

impl LibSQLStateStore {
    /// Local-only mode: no remote sync. Connection created via exec_future (bootstrap).
    pub fn local(local_path: &Path) -> Result<Self, DeploymentError>;

    /// Embedded replica mode: local file with automatic sync to Turso.
    pub fn with_sync(local_path: &Path, turso_url: &str, auth_token: &str) -> Result<Self, DeploymentError>;

    /// Create from environment variables:
    ///   LIBSQL_LOCAL_PATH (optional, default: .deployment/libsql.db)
    ///   LIBSQL_TURSO_URL (optional - if set, enables sync)
    ///   LIBSQL_TURSO_TOKEN (required if LIBSQL_TURSO_URL is set)
    pub fn from_env(project_dir: &Path) -> Result<Self, DeploymentError>;
}

impl StateStore for LibSQLStateStore {
    fn init(&self) -> Result<(), DeploymentError> {
        // exec_future — one-shot bootstrap (same as SqliteStateStore)
        // PRAGMA journal_mode=WAL + CREATE TABLE IF NOT EXISTS
    }

    fn get(&self, resource_id: &str) -> Result<StateStoreStream<Option<ResourceState>>, DeploymentError> {
        // Same run_future_iter pattern as SqliteStateStore::get
    }

    fn list(&self) -> Result<StateStoreStream<String>, DeploymentError> {
        // Same run_future_iter pattern as SqliteStateStore::list
    }

    fn set(&self, resource_id: &str, state: &ResourceState) -> Result<StateStoreStream<()>, DeploymentError> {
        // Same run_future_iter pattern as SqliteStateStore::set
    }

    // delete, count, get_batch, all — identical pattern to SqliteStateStore

    fn sync_remote(&self) -> Result<(), DeploymentError> {
        if self.turso_url.is_some() {
            // exec_future to trigger manual sync if needed
            // libsql handles background sync automatically
        }
        Ok(())
    }
}
```

### TursoStateStore (Remote-first with embedded replica cache)

Same `run_future_iter` pattern as other SQL backends. Only the connection setup
differs (remote Turso vs local libsql). Shares `ResourceRowsIterator`.

```rust
// state/turso.rs

use std::path::{Path, PathBuf};
use std::sync::Arc;

/// Remote-first Turso state store using libsql.
///
/// All trait methods use `run_future_iter`. Connection in `Arc<libsql::Connection>`.
/// Same schema and queries as SqliteStateStore/LibSQLStateStore — only connection
/// setup differs. Turso handles replication automatically.
pub struct TursoStateStore {
    conn: Arc<libsql::Connection>,
    local_path: Option<PathBuf>,
    turso_url: String,
    turso_auth_token: String,
}

impl TursoStateStore {
    /// Remote-only mode. Connection created via exec_future (bootstrap).
    pub fn remote(turso_url: &str, auth_token: &str) -> Result<Self, DeploymentError>;

    /// Embedded replica mode: local SQLite file syncs with Turso.
    pub fn embedded_replica(local_path: &Path, turso_url: &str, auth_token: &str) -> Result<Self, DeploymentError>;

    /// Create from environment variables:
    ///   TURSO_DATABASE_URL (required)
    ///   TURSO_AUTH_TOKEN (required)
    ///   TURSO_LOCAL_REPLICA (optional path for embedded replica)
    pub fn from_env() -> Result<Self, DeploymentError>;
}

impl StateStore for TursoStateStore {
    // Same run_future_iter pattern as SqliteStateStore for all methods.
    // init() uses exec_future (bootstrap).
    // Shares ResourceRowsIterator, IdRowsIterator with other SQL backends.
}
```

### R2StateStore

HTTP-based async backend. Uses `run_future_iter` wrapping `SimpleHttpClient` calls.
Each trait method schedules an async HTTP request via `run_future_iter` and returns
a `StateStoreStream`.

```rust
// state/r2.rs

use foundation_core::simple_http::client::SimpleHttpClient;
use foundation_core::valtron::{run_future_iter, ThreadedValue};

const CF_API_BASE: &str = "https://api.cloudflare.com/client/v4";

/// Cloudflare R2 object storage state store.
///
/// All trait methods use `run_future_iter` wrapping SimpleHttpClient HTTP calls.
/// R2 API is async — the HTTP request runs on the worker thread, only the
/// parsed result crosses the channel.
pub struct R2StateStore {
    api_token: String,
    account_id: String,
    bucket_name: String,
    prefix: String,
}

impl R2StateStore {
    pub fn new(api_token: &str, account_id: &str, bucket_name: &str, prefix: Option<&str>) -> Self;
    pub fn from_env() -> Result<Self, DeploymentError>;

    fn object_key(&self, resource_id: &str) -> String {
        let safe_id = resource_id.replace('/', ":");
        format!("{}{safe_id}.json", self.prefix)
    }
}

impl StateStore for R2StateStore {
    fn init(&self) -> Result<(), DeploymentError> {
        // R2 buckets are pre-created — exec_future to verify connectivity
        Ok(())
    }

    fn get(&self, resource_id: &str) -> Result<StateStoreStream<Option<ResourceState>>, DeploymentError> {
        let key = self.object_key(resource_id);
        let url = format!("{CF_API_BASE}/accounts/{}/r2/buckets/{}/objects/{key}",
            self.account_id, self.bucket_name);
        let token = self.api_token.clone();

        let iter = run_future_iter(
            move || async move {
                // GET object via SimpleHttpClient with Bearer auth
                // Return None on 404, parse JSON on 200
                // Yields single Result<Option<ResourceState>, Error>
                Ok::<_, DeploymentError>(std::iter::once(Ok(/* parsed or None */)))
            },
            None, None,
        ).map_err(|e| DeploymentError::StateFailed(e.to_string()))?;

        Ok(Box::new(iter))
    }

    fn set(&self, resource_id: &str, state: &ResourceState) -> Result<StateStoreStream<()>, DeploymentError> {
        let key = self.object_key(resource_id);
        let body = serde_json::to_string_pretty(state)
            .map_err(|e| DeploymentError::StateFailed(e.to_string()))?;
        let url = format!("{CF_API_BASE}/accounts/{}/r2/buckets/{}/objects/{key}",
            self.account_id, self.bucket_name);
        let token = self.api_token.clone();

        let iter = run_future_iter(
            move || async move {
                // PUT object via SimpleHttpClient with Bearer auth
                Ok::<_, DeploymentError>(std::iter::once(Ok(())))
            },
            None, None,
        ).map_err(|e| DeploymentError::StateFailed(e.to_string()))?;

        Ok(Box::new(iter))
    }

    fn list(&self) -> Result<StateStoreStream<String>, DeploymentError> {
        let prefix = self.prefix.clone();
        let url = format!("{CF_API_BASE}/accounts/{}/r2/buckets/{}/objects?prefix={prefix}",
            self.account_id, self.bucket_name);
        let token = self.api_token.clone();

        let iter = run_future_iter(
            move || async move {
                // GET listing via SimpleHttpClient, parse response
                // Return iterator of resource IDs extracted from keys
                Ok::<_, DeploymentError>(/* ids_vec.into_iter().map(Ok) */)
            },
            None, None,
        ).map_err(|e| DeploymentError::StateFailed(e.to_string()))?;

        Ok(Box::new(iter))
    }

    // delete, count, get_batch, all — same run_future_iter + SimpleHttpClient pattern
}
```

### D1StateStore

HTTP-based async backend. Uses `run_future_iter` wrapping `SimpleHttpClient` calls
to the D1 SQL-over-HTTP API. Same schema as other SQL backends.

```rust
// state/d1.rs

use foundation_core::simple_http::client::SimpleHttpClient;
use foundation_core::valtron::{run_future_iter, ThreadedValue};

const CF_API_BASE: &str = "https://api.cloudflare.com/client/v4";

/// Cloudflare D1 edge SQLite state store.
///
/// All trait methods use `run_future_iter` wrapping SimpleHttpClient HTTP calls.
/// D1 API returns JSON rows — no !Send row iterators, but we still use
/// `run_future_iter` for consistency and composability.
pub struct D1StateStore {
    api_token: String,
    account_id: String,
    database_id: String,
}

impl D1StateStore {
    pub fn new(api_token: &str, account_id: &str, database_id: &str) -> Self;
    pub fn from_env() -> Result<Self, DeploymentError>;

    fn query_url(&self) -> String {
        format!(
            "{CF_API_BASE}/accounts/{}/d1/database/{}/query",
            self.account_id, self.database_id
        )
    }
}

impl StateStore for D1StateStore {
    fn init(&self) -> Result<(), DeploymentError> {
        // exec_future — one-shot bootstrap: CREATE TABLE via D1 API
        // POST { "sql": "CREATE TABLE IF NOT EXISTS resources (...)" }
        todo!()
    }

    fn get(&self, resource_id: &str) -> Result<StateStoreStream<Option<ResourceState>>, DeploymentError> {
        let id = resource_id.to_string();
        let url = self.query_url();
        let token = self.api_token.clone();

        let iter = run_future_iter(
            move || async move {
                // POST { "sql": "SELECT * FROM resources WHERE id = ?", "params": [id] }
                // Parse D1 response: { "success": true, "result": [{ "results": [...] }] }
                // D1 returns JSON rows — no !Send issues, parse directly
                // Yield single Result<Option<ResourceState>, Error>
                Ok::<_, DeploymentError>(std::iter::once(Ok(/* parsed or None */)))
            },
            None, None,
        ).map_err(|e| DeploymentError::StateFailed(e.to_string()))?;

        Ok(Box::new(iter))
    }

    fn set(&self, resource_id: &str, state: &ResourceState) -> Result<StateStoreStream<()>, DeploymentError> {
        let id = resource_id.to_string();
        let state_json = serde_json::to_string(state)
            .map_err(|e| DeploymentError::StateFailed(e.to_string()))?;
        let url = self.query_url();
        let token = self.api_token.clone();

        let iter = run_future_iter(
            move || async move {
                // POST { "sql": "INSERT OR REPLACE INTO resources (...)", "params": [...] }
                Ok::<_, DeploymentError>(std::iter::once(Ok(())))
            },
            None, None,
        ).map_err(|e| DeploymentError::StateFailed(e.to_string()))?;

        Ok(Box::new(iter))
    }

    fn list(&self) -> Result<StateStoreStream<String>, DeploymentError> {
        let url = self.query_url();
        let token = self.api_token.clone();

        let iter = run_future_iter(
            move || async move {
                // POST { "sql": "SELECT id FROM resources" }
                // Parse D1 JSON response, extract ids
                Ok::<_, DeploymentError>(/* ids.into_iter().map(Ok) */)
            },
            None, None,
        ).map_err(|e| DeploymentError::StateFailed(e.to_string()))?;

        Ok(Box::new(iter))
    }

    // delete, count, get_batch, all — same run_future_iter + D1 HTTP pattern
}
```

### StateStore Factory

```rust
// state/mod.rs

/// Selects a state store backend based on environment configuration.
///
/// Priority:
///   1. D1 — if DEPLOYMENT_D1_DATABASE_ID is set (+ CLOUDFLARE_API_TOKEN, CLOUDFLARE_ACCOUNT_ID)
///   2. R2 — if DEPLOYMENT_R2_BUCKET is set (+ CLOUDFLARE_API_TOKEN, CLOUDFLARE_ACCOUNT_ID)
///   3. Turso — if TURSO_DATABASE_URL is set (+ TURSO_AUTH_TOKEN)
///   4. LibSQL with sync — if LIBSQL_TURSO_URL is set (+ LIBSQL_TURSO_TOKEN)
///   5. LibSQL local — if LIBSQL_LOCAL_PATH is set or .deployment/libsql.db exists
///   6. SQLite (local-only) — if DEPLOYMENT_STATE_DB is set or .deployment/state.db exists
///   7. JSON files — default fallback
pub fn create_state_store(
    project_dir: &Path,
    provider: &str,
    stage: &str,
) -> Box<dyn StateStore> {
    if std::env::var("DEPLOYMENT_D1_DATABASE_ID").is_ok() {
        Box::new(D1StateStore::from_env().expect(
            "CLOUDFLARE_API_TOKEN and CLOUDFLARE_ACCOUNT_ID must be set with DEPLOYMENT_D1_DATABASE_ID"
        ))
    } else if std::env::var("DEPLOYMENT_R2_BUCKET").is_ok() {
        Box::new(R2StateStore::from_env().expect(
            "CLOUDFLARE_API_TOKEN and CLOUDFLARE_ACCOUNT_ID must be set with DEPLOYMENT_R2_BUCKET"
        ))
    } else if std::env::var("TURSO_DATABASE_URL").is_ok() {
        Box::new(TursoStateStore::from_env().expect(
            "TURSO_AUTH_TOKEN must be set with TURSO_DATABASE_URL"
        ))
    } else if std::env::var("LIBSQL_TURSO_URL").is_ok() {
        Box::new(LibSQLStateStore::from_env(project_dir).expect(
            "LIBSQL_TURSO_TOKEN must be set with LIBSQL_TURSO_URL"
        ))
    } else if std::env::var("LIBSQL_LOCAL_PATH").is_ok()
        || project_dir.join(".deployment/libsql.db").exists()
    {
        Box::new(LibSQLStateStore::from_env(project_dir).unwrap_or_else(|_| {
            LibSQLStateStore::local(&project_dir.join(".deployment/libsql.db"))
        }))
    } else if std::env::var("DEPLOYMENT_STATE_DB").is_ok()
        || project_dir.join(".deployment/state.db").exists()
    {
        Box::new(SqliteStateStore::from_env(project_dir))
    } else {
        Box::new(JsonFileStateStore::new(project_dir, provider, stage))
    }
}
```

## Tasks

1. **Define StateStore trait and types**
   - [ ] Create `src/state/mod.rs`, `traits.rs`, `types.rs`
   - [ ] Define `StateStore` trait with all methods
   - [ ] Define `ResourceState` and `StateStatus` types
   - [ ] Implement `needs_deploy()` change detection
   - [ ] Write unit tests for change detection logic

2. **Implement JsonFileStateStore**
   - [ ] Create `src/state/file.rs`
   - [ ] Implement all `StateStore` methods
   - [ ] Handle directory creation, safe filenames
   - [ ] Write unit tests with temp directories

3. **Implement SqliteStateStore (local-only via libsql)**
   - [ ] Create `src/state/sqlite.rs`
   - [ ] Add `libsql` dependency (local-only mode)
   - [ ] Implement schema creation in `init()` with WAL mode
   - [ ] Implement all CRUD operations using libsql::Connection
   - [ ] Write unit tests with local file database

4. **Implement LibSQLStateStore (embedded with optional Turso sync)**
   - [ ] Create `src/state/libsql.rs`
   - [ ] Implement local-only mode (no sync)
   - [ ] Implement embedded replica mode with Turso sync
   - [ ] Configure via `LIBSQL_LOCAL_PATH`, `LIBSQL_TURSO_URL`, `LIBSQL_TURSO_TOKEN`
   - [ ] Implement `sync_remote()` method
   - [ ] Write integration test (requires Turso account, mark `#[ignore]`)

5. **Implement TursoStateStore (remote-first with replica cache)**
   - [ ] Create `src/state/turso.rs`
   - [ ] Implement remote-only mode (connect to Turso URL)
   - [ ] Implement embedded replica mode (local file + remote sync)
   - [ ] Configure via `TURSO_DATABASE_URL` and `TURSO_AUTH_TOKEN`
   - [ ] Write integration test (requires Turso account, mark `#[ignore]`)

6. **Implement R2StateStore**
   - [ ] Create `src/state/r2.rs`
   - [ ] Implement object key generation with prefix
   - [ ] Implement `get` — GET object, parse JSON, handle 404 as None
   - [ ] Implement `set` — PUT object as JSON
   - [ ] Implement `delete` — DELETE object
   - [ ] Implement `list` — list objects by prefix, extract resource IDs
   - [ ] All API calls via SimpleHttpClient with Bearer auth
   - [ ] Write integration test (requires R2 bucket + CF credentials, mark `#[ignore]`)

7. **Implement D1StateStore**
   - [ ] Create `src/state/d1.rs`
   - [ ] Implement `execute_sql()` helper — POST to D1 query API via SimpleHttpClient
   - [ ] Implement `init()` — CREATE TABLE via D1 API (same schema as SqliteStateStore)
   - [ ] Implement all CRUD operations via SQL-over-HTTP
   - [ ] Parse D1 API response format (`{ "success": true, "result": [...] }`)
   - [ ] Write integration test (requires D1 database + CF credentials, mark `#[ignore]`)

8. **Implement state store factory**
   - [ ] Create `create_state_store()` function
   - [ ] Auto-detect: D1 > R2 > Turso > LibSQL with sync > LibSQL local > SQLite > JSON files
   - [ ] Write unit tests for factory logic

9. **Config hashing**
   - [ ] Implement SHA-256 hashing of serialized config
   - [ ] Use `serde_json::to_string()` for canonical serialization
   - [ ] Write unit tests verifying deterministic hashing

10. **Write comprehensive tests**
   - [ ] Test round-trip for all six stores (set -> get -> verify)
   - [ ] Test list/count/delete operations
   - [ ] Test change detection (config_changed, needs_deploy)
   - [ ] Test concurrent access (libsql WAL mode)
   - [ ] Run same test suite against all local backends (trait-based testing)
   - [ ] Run same test suite against remote backends with `#[ignore]` (R2, D1, Turso, LibSQL with sync)

## Implementation Notes

### Valtron Async Bridge (MANDATORY — read `fundamentals/00-overview.md` § Valtron Async Bridge Policy)

- **`run_future_iter` is the default for ALL async I/O** — single-value AND multi-value. This avoids upfront `Vec` allocation and preserves composability. Callers pull only what they need, when they need it.
- **`exec_future` is ONLY for one-shot bootstrap** — `init()` (schema creation), connection setup in `new()`. Never in trait methods.
- **`!Send` row iterators** (`libsql::Rows`) stay on the worker thread via `RowsIterator`. Only `Send` values cross the channel. See `foundation_db/LEARNINGS.md` and `PROPOSAL_ROWS_STREAMING.md`.
- **`Send + 'static` boundary** — clone `Arc<Connection>` and convert `&str` → `String` before the async block.
- **Sync backends (JsonFileStateStore) bypass Valtron entirely** — direct `std::fs` ops. Return `StateStoreStream` via `Vec::into_iter().map(|v| ThreadedValue::Value(Ok(v)))`.
- **Stream consumption helpers** — `collect_first()`, `collect_all()`, `drive_to_completion()` in `state/helpers.rs` for caller convenience at sync boundaries.

### General

- All SQL-based stores (`SqliteStateStore`, `LibSQLStateStore`, `TursoStateStore`) use the `libsql` crate (Turso's SQLite fork)
- Four SQL-based stores (SQLite, LibSQL, Turso, D1) share the same schema — extract shared SQL into helper functions
- **Shared RowsIterator types** — `ResourceRowsIterator` and `IdRowsIterator` are shared across all libsql-based backends. D1 doesn't need them (JSON response).
- D1StateStore executes SQL over HTTP via the Cloudflare D1 API, not a direct connection
- R2StateStore is object-based (one JSON file per resource), similar to JsonFileStateStore but remote
- All libsql-based stores use WAL mode for concurrent read access
- JSON file store and R2 store use `serde_json::to_string_pretty` for human-readable state
- Config hashing must be deterministic: sort keys, normalize whitespace
- Turso handles its own replication — no manual sync code needed
- LibSQL with Turso sync handles replication automatically via libsql's embedded replica feature
- R2 and D1 stores share `CLOUDFLARE_API_TOKEN` and `CLOUDFLARE_ACCOUNT_ID` env vars with the Cloudflare provider — no extra auth setup needed if already deploying to Cloudflare
- **Error types** — `derive_more::From` + manual `Display`, central `errors.rs`. No `thiserror`.

## Dependencies

```toml
[dependencies]
libsql = "0.6"  # All SQL-based stores use libsql (Turso's SQLite fork)
sha2 = "0.10"
# R2 and D1 stores use foundation_core::simple_http::client::SimpleHttpClient — no extra deps
```

## Success Criteria

- [ ] All 10 tasks completed
- [ ] `cargo clippy -p foundation_db -- -D warnings -W clippy::pedantic` — zero warnings, zero suppression
- [ ] `cargo doc -p foundation_db --no-deps` — zero rustdoc warnings
- [ ] No `#[allow(...)]` or `#[expect(...)]` anywhere in the code
- [ ] All six stores pass the same test suite (trait-based testing)
- [ ] Change detection correctly skips unchanged resources
- [ ] libsql WAL mode works for concurrent reads
- [ ] Turso embedded replica mode works when configured
- [ ] LibSQL with sync replicates to remote Turso correctly
- [ ] R2 store reads/writes JSON objects correctly
- [ ] D1 store executes SQL-over-HTTP correctly
- [ ] Factory auto-detects correct backend from environment

## Verification

```bash
cd backends/foundation_db
cargo test state -- --nocapture
cargo test state_sqlite -- --nocapture

# Remote integrations (require credentials, mark #[ignore])
cargo test state_turso -- --ignored --nocapture
cargo test state_libsql_sync -- --ignored --nocapture
cargo test state_r2 -- --ignored --nocapture
cargo test state_d1 -- --ignored --nocapture
```

---

_Created: 2026-03-26_
