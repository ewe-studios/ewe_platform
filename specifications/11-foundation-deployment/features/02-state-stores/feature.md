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

```rust
// state/traits.rs

/// Trait for deployment state persistence backends.
/// Inspired by alchemy's StateStore interface.
pub trait StateStore: Send + Sync {
    /// Initialize the store (create tables, directories, etc.).
    fn init(&self) -> Result<(), DeploymentError>;

    /// List all resource IDs in the store.
    fn list(&self) -> Result<Vec<String>, DeploymentError>;

    /// Count resources.
    fn count(&self) -> Result<usize, DeploymentError>;

    /// Get state for a single resource.
    fn get(&self, resource_id: &str) -> Result<Option<ResourceState>, DeploymentError>;

    /// Get state for multiple resources in one call.
    fn get_batch(&self, ids: &[&str]) -> Result<Vec<ResourceState>, DeploymentError>;

    /// Get all resource states.
    fn all(&self) -> Result<Vec<ResourceState>, DeploymentError>;

    /// Set (create or update) state for a resource.
    fn set(&self, resource_id: &str, state: &ResourceState) -> Result<(), DeploymentError>;

    /// Delete state for a resource.
    fn delete(&self, resource_id: &str) -> Result<(), DeploymentError>;

    /// Sync state to a remote location (no-op for FileStateStore).
    fn sync_remote(&self) -> Result<(), DeploymentError> {
        Ok(()) // Default: no remote sync
    }
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

```rust
// state/file.rs

use std::path::{Path, PathBuf};

/// JSON file-based state store.
/// Stores each resource as a separate JSON file:
///   .deployment/{provider}/{stage}/{resource_id}.json
///
/// Simple, git-friendly, no dependencies.
pub struct FileStateStore {
    root_dir: PathBuf,
    provider: String,
    stage: String,
}

impl FileStateStore {
    /// Create a store rooted at `project_dir/.deployment/{provider}/{stage}/`.
    pub fn new(project_dir: &Path, provider: &str, stage: &str) -> Self {
        Self {
            root_dir: project_dir.join(".deployment").join(provider).join(stage),
            provider: provider.to_string(),
            stage: stage.to_string(),
        }
    }

    fn resource_path(&self, resource_id: &str) -> PathBuf {
        // Replace slashes with colons in filenames (like alchemy)
        let safe_id = resource_id.replace('/', ":");
        self.root_dir.join(format!("{}.json", safe_id))
    }
}

impl StateStore for FileStateStore {
    fn init(&self) -> Result<(), DeploymentError> {
        std::fs::create_dir_all(&self.root_dir)?;
        Ok(())
    }

    fn get(&self, resource_id: &str) -> Result<Option<ResourceState>, DeploymentError> {
        let path = self.resource_path(resource_id);
        if !path.exists() {
            return Ok(None);
        }
        let content = std::fs::read_to_string(&path)?;
        let state: ResourceState = serde_json::from_str(&content)
            .map_err(|e| DeploymentError::StateFailed(e.to_string()))?;
        Ok(Some(state))
    }

    fn set(&self, resource_id: &str, state: &ResourceState) -> Result<(), DeploymentError> {
        self.init()?; // Ensure directory exists
        let path = self.resource_path(resource_id);
        let content = serde_json::to_string_pretty(state)
            .map_err(|e| DeploymentError::StateFailed(e.to_string()))?;
        std::fs::write(&path, content)?;
        Ok(())
    }

    fn delete(&self, resource_id: &str) -> Result<(), DeploymentError> {
        let path = self.resource_path(resource_id);
        if path.exists() {
            std::fs::remove_file(&path)?;
        }
        Ok(())
    }

    // ... list, count, get_batch, all implementations
}
```

### SqliteStateStore (Local-only via libsql)

```rust
// state/sqlite.rs

use std::path::{Path, PathBuf};

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
/// Uses libsql instead of rusqlite for consistency with TursoStateStore.
pub struct SqliteStateStore {
    db_path: PathBuf,
}

impl SqliteStateStore {
    pub fn new(db_path: PathBuf) -> Self {
        Self { db_path }
    }

    /// Create from environment variable DEPLOYMENT_STATE_DB,
    /// or default to `project_dir/.deployment/state.db`.
    pub fn from_env(project_dir: &Path) -> Self {
        let db_path = std::env::var("DEPLOYMENT_STATE_DB")
            .map(PathBuf::from)
            .unwrap_or_else(|_| project_dir.join(".deployment/state.db"));
        Self::new(db_path)
    }

    fn connection(&self) -> Result<libsql::Connection, DeploymentError>;
}

impl StateStore for SqliteStateStore {
    fn init(&self) -> Result<(), DeploymentError> {
        std::fs::create_dir_all(self.db_path.parent().unwrap())?;
        let conn = self.connection()?;
        conn.execute_batch("PRAGMA journal_mode=WAL;")?;
        conn.execute(
            "CREATE TABLE IF NOT EXISTS resources (
                id TEXT PRIMARY KEY,
                kind TEXT NOT NULL,
                provider TEXT NOT NULL,
                status TEXT NOT NULL,
                environment TEXT,
                config_hash TEXT NOT NULL,
                output TEXT NOT NULL,
                config_snapshot TEXT NOT NULL,
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL
            )",
            [],
        )?;
        Ok(())
    }

    // get, set, delete, list, all — standard SQL operations
    // Uses libsql::Connection instead of rusqlite::Connection
}
```

### LibSQLStateStore (Embedded with optional Turso sync)

```rust
// state/libsql.rs

use std::path::{Path, PathBuf};

/// Local embedded libSQL state store with optional remote Turso sync.
///
/// This store uses libsql's embedded database with optional sync to a remote Turso database.
/// When sync is configured, changes are automatically replicated to the remote Turso instance.
///
/// Modes:
///   1. Local-only: behaves like SqliteStateStore (no sync)
///   2. Embedded replica: local file with automatic background sync to Turso
///
/// Configure via:
///   - `LIBSQL_LOCAL_PATH` - local database path (default: `.deployment/libsql.db`)
///   - `LIBSQL_TURSO_URL` - optional Turso URL for sync (e.g., libsql://my-db.turso.io)
///   - `LIBSQL_TURSO_TOKEN` - optional Turso auth token for sync
///
/// When Turso URL and token are provided, the embedded replica syncs automatically.
pub struct LibSQLStateStore {
    /// Local database path.
    local_path: PathBuf,
    /// Optional Turso URL for sync.
    turso_url: Option<String>,
    /// Optional Turso auth token for sync.
    turso_auth_token: Option<String>,
}

impl LibSQLStateStore {
    /// Local-only mode: no remote sync.
    pub fn local(local_path: &Path) -> Self {
        Self {
            local_path: local_path.to_path_buf(),
            turso_url: None,
            turso_auth_token: None,
        }
    }

    /// Embedded replica mode: local file with automatic sync to Turso.
    pub fn with_sync(local_path: &Path, turso_url: &str, auth_token: &str) -> Self {
        Self {
            local_path: local_path.to_path_buf(),
            turso_url: Some(turso_url.to_string()),
            turso_auth_token: Some(auth_token.to_string()),
        }
    }

    /// Create from environment variables:
    ///   LIBSQL_LOCAL_PATH (optional, default: .deployment/libsql.db)
    ///   LIBSQL_TURSO_URL (optional - if set, enables sync)
    ///   LIBSQL_TURSO_TOKEN (required if LIBSQL_TURSO_URL is set)
    pub fn from_env(project_dir: &Path) -> Result<Self, DeploymentError> {
        let local_path = std::env::var("LIBSQL_LOCAL_PATH")
            .map(PathBuf::from)
            .unwrap_or_else(|_| project_dir.join(".deployment/libsql.db"));

        let turso_url = std::env::var("LIBSQL_TURSO_URL").ok();
        let turso_auth_token = std::env::var("LIBSQL_TURSO_TOKEN").ok();

        // Validate: if URL is set, token must also be set
        if turso_url.is_some() && turso_auth_token.is_none() {
            return Err(DeploymentError::ConfigInvalid {
                file: "env".into(),
                reason: "LIBSQL_TURSO_TOKEN is required when LIBSQL_TURSO_URL is set".into(),
            });
        }

        Ok(Self {
            local_path,
            turso_url,
            turso_auth_token,
        })
    }

    fn connection(&self) -> Result<libsql::Connection, DeploymentError>;
}

impl StateStore for LibSQLStateStore {
    fn init(&self) -> Result<(), DeploymentError> {
        std::fs::create_dir_all(self.local_path.parent().unwrap())?;
        let conn = self.connection()?;
        conn.execute_batch("PRAGMA journal_mode=WAL;")?;
        conn.execute(
            "CREATE TABLE IF NOT EXISTS resources (
                id TEXT PRIMARY KEY,
                kind TEXT NOT NULL,
                provider TEXT NOT NULL,
                status TEXT NOT NULL,
                environment TEXT,
                config_hash TEXT NOT NULL,
                output TEXT NOT NULL,
                config_snapshot TEXT NOT NULL,
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL
            )",
            [],
        )?;
        Ok(())
    }

    fn sync_remote(&self) -> Result<(), DeploymentError> {
        // libsql handles sync automatically when configured with Turso URL.
        // This method can be used to trigger a manual sync if needed.
        if self.turso_url.is_some() {
            // Trigger sync - libsql handles this automatically in the background
            // but we can force a sync here if needed
        }
        Ok(())
    }

    // get, set, delete, list, all — standard SQL operations via libsql
}
```

### TursoStateStore (Remote-first with embedded replica cache)

```rust
// state/turso.rs

use std::path::{Path, PathBuf};

/// Remote-first Turso state store using libsql.
///
/// This store connects to a remote Turso database. It supports an optional embedded
/// local replica that caches data for faster reads and offline operation.
///
/// Modes:
///   1. Remote-only: all reads/writes go directly to Turso
///   2. Embedded replica: local cache with automatic background sync to Turso
///
/// Turso handles replication — no manual sync needed.
/// Configure via TURSO_DATABASE_URL and TURSO_AUTH_TOKEN.
pub struct TursoStateStore {
    /// Local path for embedded replica (optional).
    local_path: Option<PathBuf>,
    /// Turso database URL (e.g., libsql://my-db-user.turso.io).
    turso_url: String,
    /// Turso auth token.
    turso_auth_token: String,
}

impl TursoStateStore {
    /// Remote-only mode: all reads/writes go to Turso directly.
    pub fn remote(turso_url: &str, auth_token: &str) -> Self {
        Self {
            local_path: None,
            turso_url: turso_url.to_string(),
            turso_auth_token: auth_token.to_string(),
        }
    }

    /// Embedded replica mode: local SQLite file syncs with Turso.
    /// Reads are local (fast), writes sync to remote.
    pub fn embedded_replica(local_path: &Path, turso_url: &str, auth_token: &str) -> Self {
        Self {
            local_path: Some(local_path.to_path_buf()),
            turso_url: turso_url.to_string(),
            turso_auth_token: auth_token.to_string(),
        }
    }

    /// Create from environment variables:
    ///   TURSO_DATABASE_URL (required)
    ///   TURSO_AUTH_TOKEN (required)
    ///   TURSO_LOCAL_REPLICA (optional path for embedded replica)
    pub fn from_env() -> Result<Self, DeploymentError> {
        let turso_url = std::env::var("TURSO_DATABASE_URL")
            .map_err(|_| DeploymentError::ConfigInvalid {
                file: "env".into(),
                reason: "TURSO_DATABASE_URL required for Turso state store".into(),
            })?;
        let turso_auth_token = std::env::var("TURSO_AUTH_TOKEN")
            .map_err(|_| DeploymentError::ConfigInvalid {
                file: "env".into(),
                reason: "TURSO_AUTH_TOKEN required for Turso state store".into(),
            })?;
        let local_path = std::env::var("TURSO_LOCAL_REPLICA")
            .map(PathBuf::from)
            .ok();

        Ok(Self {
            local_path,
            turso_url,
            turso_auth_token,
        })
    }
}

impl StateStore for TursoStateStore {
    // Same schema and queries as SqliteStateStore/LibSQLStateStore.
    // Only the connection setup differs (remote Turso vs local libsql).
    // Turso handles replication automatically.
}
```

### R2StateStore

```rust
// state/r2.rs

use foundation_core::simple_http::client::SimpleHttpClient;

const CF_API_BASE: &str = "https://api.cloudflare.com/client/v4";

/// Cloudflare R2 object storage state store.
///
/// Stores each resource as a JSON object in an R2 bucket:
///   {prefix}/{resource_id}.json
///
/// Uses the Cloudflare R2 API (S3-compatible) via SimpleHttpClient.
/// Good for teams already on Cloudflare — state lives in the same ecosystem.
///
/// Configure via:
///   CLOUDFLARE_API_TOKEN (required)
///   CLOUDFLARE_ACCOUNT_ID (required)
///   DEPLOYMENT_R2_BUCKET (required - bucket name)
///   DEPLOYMENT_R2_PREFIX (optional - key prefix, defaults to "deployment-state/")
pub struct R2StateStore {
    api_token: String,
    account_id: String,
    bucket_name: String,
    prefix: String,
}

impl R2StateStore {
    pub fn new(api_token: &str, account_id: &str, bucket_name: &str, prefix: Option<&str>) -> Self {
        Self {
            api_token: api_token.to_string(),
            account_id: account_id.to_string(),
            bucket_name: bucket_name.to_string(),
            prefix: prefix.unwrap_or("deployment-state/").to_string(),
        }
    }

    pub fn from_env() -> Result<Self, DeploymentError> {
        let api_token = std::env::var("CLOUDFLARE_API_TOKEN")
            .map_err(|_| DeploymentError::ConfigInvalid {
                file: "env".into(),
                reason: "CLOUDFLARE_API_TOKEN required for R2 state store".into(),
            })?;
        let account_id = std::env::var("CLOUDFLARE_ACCOUNT_ID")
            .map_err(|_| DeploymentError::ConfigInvalid {
                file: "env".into(),
                reason: "CLOUDFLARE_ACCOUNT_ID required for R2 state store".into(),
            })?;
        let bucket_name = std::env::var("DEPLOYMENT_R2_BUCKET")
            .map_err(|_| DeploymentError::ConfigInvalid {
                file: "env".into(),
                reason: "DEPLOYMENT_R2_BUCKET required for R2 state store".into(),
            })?;
        let prefix = std::env::var("DEPLOYMENT_R2_PREFIX").ok();
        Ok(Self::new(&api_token, &account_id, &bucket_name, prefix.as_deref()))
    }

    fn object_key(&self, resource_id: &str) -> String {
        let safe_id = resource_id.replace('/', ":");
        format!("{}{}.json", self.prefix, safe_id)
    }

    /// R2 API: GET /accounts/{account_id}/r2/buckets/{bucket}/objects/{key}
    fn get_object_url(&self, key: &str) -> String {
        format!(
            "{}/accounts/{}/r2/buckets/{}/objects/{}",
            CF_API_BASE, self.account_id, self.bucket_name, key
        )
    }
}

impl StateStore for R2StateStore {
    fn init(&self) -> Result<(), DeploymentError> {
        // R2 buckets are pre-created — just verify connectivity
        // GET /accounts/{account_id}/r2/buckets/{bucket}
        Ok(())
    }

    fn get(&self, resource_id: &str) -> Result<Option<ResourceState>, DeploymentError> {
        let key = self.object_key(resource_id);
        // GET object from R2 via SimpleHttpClient
        // Return None on 404, parse JSON on 200
        todo!()
    }

    fn set(&self, resource_id: &str, state: &ResourceState) -> Result<(), DeploymentError> {
        let key = self.object_key(resource_id);
        let body = serde_json::to_string_pretty(state)
            .map_err(|e| DeploymentError::StateFailed(e.to_string()))?;
        // PUT object to R2 via SimpleHttpClient
        // Content-Type: application/json
        todo!()
    }

    fn delete(&self, resource_id: &str) -> Result<(), DeploymentError> {
        let key = self.object_key(resource_id);
        // DELETE object from R2 via SimpleHttpClient
        todo!()
    }

    fn list(&self) -> Result<Vec<String>, DeploymentError> {
        // GET /accounts/{account_id}/r2/buckets/{bucket}/objects?prefix={prefix}
        // Parse listing response, extract resource IDs from keys
        todo!()
    }

    // count, get_batch, all — built on top of list + get
}
```

### D1StateStore

```rust
// state/d1.rs

use foundation_core::simple_http::client::SimpleHttpClient;

const CF_API_BASE: &str = "https://api.cloudflare.com/client/v4";

/// Cloudflare D1 edge SQLite state store.
///
/// Uses the Cloudflare D1 HTTP API to execute SQL queries against a D1 database.
/// Same schema as SqliteStateStore/LibSQLStateStore/TursoStateStore — it's SQLite under the hood.
///
/// D1 is Cloudflare's serverless SQLite database. State queries go through
/// the Cloudflare API, not a direct database connection.
///
/// Configure via:
///   CLOUDFLARE_API_TOKEN (required)
///   CLOUDFLARE_ACCOUNT_ID (required)
///   DEPLOYMENT_D1_DATABASE_ID (required)
pub struct D1StateStore {
    api_token: String,
    account_id: String,
    database_id: String,
}

impl D1StateStore {
    pub fn new(api_token: &str, account_id: &str, database_id: &str) -> Self {
        Self {
            api_token: api_token.to_string(),
            account_id: account_id.to_string(),
            database_id: database_id.to_string(),
        }
    }

    pub fn from_env() -> Result<Self, DeploymentError> {
        let api_token = std::env::var("CLOUDFLARE_API_TOKEN")
            .map_err(|_| DeploymentError::ConfigInvalid {
                file: "env".into(),
                reason: "CLOUDFLARE_API_TOKEN required for D1 state store".into(),
            })?;
        let account_id = std::env::var("CLOUDFLARE_ACCOUNT_ID")
            .map_err(|_| DeploymentError::ConfigInvalid {
                file: "env".into(),
                reason: "CLOUDFLARE_ACCOUNT_ID required for D1 state store".into(),
            })?;
        let database_id = std::env::var("DEPLOYMENT_D1_DATABASE_ID")
            .map_err(|_| DeploymentError::ConfigInvalid {
                file: "env".into(),
                reason: "DEPLOYMENT_D1_DATABASE_ID required for D1 state store".into(),
            })?;
        Ok(Self::new(&api_token, &account_id, &database_id))
    }

    /// Execute a SQL query against D1 via the Cloudflare API.
    /// POST /accounts/{account_id}/d1/database/{database_id}/query
    fn query_url(&self) -> String {
        format!(
            "{}/accounts/{}/d1/database/{}/query",
            CF_API_BASE, self.account_id, self.database_id
        )
    }

    /// Execute SQL via SimpleHttpClient.
    /// Request body: { "sql": "...", "params": [...] }
    /// Response: { "success": true, "result": [{ "results": [...] }] }
    fn execute_sql(
        &self,
        sql: &str,
        params: &[serde_json::Value],
    ) -> Result<Vec<serde_json::Value>, DeploymentError> {
        // POST to query_url with Bearer auth
        // Parse CfApiResponse wrapper
        todo!()
    }
}

impl StateStore for D1StateStore {
    fn init(&self) -> Result<(), DeploymentError> {
        // Create the resources table if it doesn't exist.
        // Same schema as SqliteStateStore/LibSQLStateStore/TursoStateStore.
        self.execute_sql(
            "CREATE TABLE IF NOT EXISTS resources (
                id TEXT PRIMARY KEY,
                kind TEXT NOT NULL,
                provider TEXT NOT NULL,
                status TEXT NOT NULL,
                environment TEXT,
                config_hash TEXT NOT NULL,
                output TEXT NOT NULL,
                config_snapshot TEXT NOT NULL,
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL
            )",
            &[],
        )?;
        Ok(())
    }

    fn get(&self, resource_id: &str) -> Result<Option<ResourceState>, DeploymentError> {
        let rows = self.execute_sql(
            "SELECT * FROM resources WHERE id = ?",
            &[serde_json::Value::String(resource_id.to_string())],
        )?;
        // Parse first row into ResourceState, or None if empty
        todo!()
    }

    fn set(&self, resource_id: &str, state: &ResourceState) -> Result<(), DeploymentError> {
        // INSERT OR REPLACE INTO resources ...
        todo!()
    }

    fn delete(&self, resource_id: &str) -> Result<(), DeploymentError> {
        self.execute_sql(
            "DELETE FROM resources WHERE id = ?",
            &[serde_json::Value::String(resource_id.to_string())],
        )?;
        Ok(())
    }

    fn list(&self) -> Result<Vec<String>, DeploymentError> {
        let rows = self.execute_sql("SELECT id FROM resources", &[])?;
        // Extract id strings from rows
        todo!()
    }

    // count, get_batch, all — standard SQL via execute_sql
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

- All SQL-based stores (`SqliteStateStore`, `LibSQLStateStore`, `TursoStateStore`) use the `libsql` crate (Turso's SQLite fork)
- Four SQL-based stores (SQLite, LibSQL, Turso, D1) share the same schema — extract shared SQL into helper functions
- D1StateStore executes SQL over HTTP via the Cloudflare D1 API, not a direct connection
- R2StateStore is object-based (one JSON file per resource), similar to JsonFileStateStore but remote
- All libsql-based stores use WAL mode for concurrent read access
- JSON file store and R2 store use `serde_json::to_string_pretty` for human-readable state
- Config hashing must be deterministic: sort keys, normalize whitespace
- Turso handles its own replication — no manual sync code needed
- LibSQL with Turso sync handles replication automatically via libsql's embedded replica feature
- R2 and D1 stores share `CLOUDFLARE_API_TOKEN` and `CLOUDFLARE_ACCOUNT_ID` env vars with the Cloudflare provider — no extra auth setup needed if already deploying to Cloudflare

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
