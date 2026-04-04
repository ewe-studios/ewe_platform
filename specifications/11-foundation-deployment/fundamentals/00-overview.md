# Foundation Deployment - Architecture Overview

**Created:** 2026-03-26
**Specification:** `specifications/11-foundation-deployment`

---

## Iron Law: Zero Warnings

> **All code must compile with zero warnings and pass all lints. No suppression. No exceptions.**
>
> This is an iron law of the project. It applies to every feature, every crate, every module.
>
> - `cargo build` — zero warnings
> - `cargo clippy -- -D warnings` — zero clippy warnings (all treated as errors)
> - `cargo clippy -- -W clippy::pedantic` — pedantic lints enabled, all resolved without suppression
> - `cargo doc --no-deps` — zero rustdoc warnings
> - `cargo test` — zero warnings during compilation
>
> **No `#[allow(...)]`, no `#[expect(...)]`, no `#![allow(...)]` anywhere.**
> If clippy or the compiler flags something, fix the code. If a lint is genuinely wrong
> for a specific case, refactor until it isn't — do not suppress it.
>
> **Verification (must pass before any commit):**
> ```bash
> cargo clippy -p foundation_deployment -- -D warnings -W clippy::pedantic
> cargo doc -p foundation_deployment --no-deps 2>&1 | grep -c "warning" | grep -q "^0$"
> cargo test -p foundation_deployment 2>&1 | grep -c "warning" | grep -q "^0$"
> ```

## Async Runtime Policy

> **Valtron is the only async runtime used by `foundation_deployment`.**
> tokio, async-std, smol, and all other async runtimes are **banned** from the
> deployment tool crate. No `async fn`, no `.await`, no `#[tokio::main]` in
> `foundation_deployment` or any of its workspace dependencies.
>
> **Generated template code** (feature 07) and **standalone example crates** (feature 09)
> are the user's deployed applications — they use whatever async runtime their target
> framework requires (axum→tokio, lambda_http→tokio, worker→wasm async). These are:
> - Feature-gated behind `template-cloudflare`, `template-gcp`, `template-aws`
> - In isolated modules with `#[cfg(feature = "...")]` at the module level
> - Example crates are standalone (not workspace members), never compiled by default
>
> **Verification**: `cargo build -p foundation_deployment` (no features) must produce
> zero tokio symbols. `cargo tree -p foundation_deployment` must show no tokio dependency.

## Valtron Async Bridge Policy

> **`run_future_iter` is the default pattern for ALL async operations.**
>
> Both `turso` and `libsql` expose async-only APIs. To bridge these into Valtron's
> synchronous `Iterator`-based world, two patterns exist. **`run_future_iter` is the
> default; `exec_future` is the exception.**
>
> | Pattern | When to Use |
> |---------|-------------|
> | **`run_future_iter`** | **Default for all async operations** — single-value AND multi-value. Returns `impl Iterator<Item = ThreadedValue<T, E>>`. Callers compose, chain, and lazily consume. No upfront `Vec` allocation. |
> | **`exec_future`** | **Only for one-shot bootstrap** — DB connection init, migrations, schema setup at startup. Never in trait methods. |
>
> **WHY `run_future_iter` even for single-value ops:**
> 1. **Memory efficiency** — `exec_future` collects all results into a `Vec<T>` inside the
>    async block before returning. `run_future_iter` streams results lazily — you only
>    pull what you need, when you need it. No wasted allocation.
> 2. **Composability** — callers get a consistent `Iterator` interface they can chain with
>    `map`, `filter`, `take`, `zip`, etc. Blocking decisions are pushed to the caller's
>    boundary (`collect()`, `next()`, `for` loop), not buried in the storage layer.
> 3. **Uniformity** — one pattern for everything means less cognitive overhead and fewer
>    code paths to maintain.
>
> **Applying `run_future_iter` (reference: `foundation_db` LEARNINGS.md):**
>
> ```rust
> use foundation_core::valtron::{run_future_iter, ThreadedValue};
>
> fn get(&self, resource_id: &str) -> Result<impl Iterator<Item = ThreadedValue<Option<ResourceState>, DeploymentError>>, DeploymentError> {
>     let id = resource_id.to_string();       // Own the data before async boundary
>     let conn = Arc::clone(&self.conn);       // Clone the Arc
>
>     let iter = run_future_iter(
>         move || async move {
>             let mut stmt = conn.prepare("SELECT * FROM resources WHERE id = ?").await?;
>             let rows = stmt.query([id]).await?;
>             Ok::<_, libsql::Error>(RowsIterator::new(rows))  // !Send stays on worker thread
>         },
>         None, // default queue size (16)
>         None, // default backpressure sleep (10ms)
>     ).map_err(|e| DeploymentError::StateFailed(format!("Valtron scheduling failed: {e}")))?;
>
>     Ok(iter)
> }
> ```
>
> **Hard constraints from `foundation_db` learnings:**
> - **`!Send` row iterators** (`turso::Rows`, `libsql::Rows`) must stay on the worker
>   thread. They never cross the async boundary. `run_future_iter` handles this: the
>   `RowsIterator` lives on the worker, only `Send` values cross the channel.
> - **`Send + 'static`** — all data captured by the async block must be owned. Clone
>   `Arc<Connection>` and convert `&str` → `String` before the async block.
> - **Three-level errors** — Valtron scheduling failure, empty stream, backend error.
>   Map all to `DeploymentError` variants.
>
> **Sync backends bypass Valtron entirely:**
> `JsonFileStateStore` uses direct `Mutex` locks and `std::fs` operations. No Valtron
> needed. It still returns iterators for multi-value methods (trait consistency), built
> from `Vec::into_iter().map(...)`.
>
> **Connection sharing:**
> SQL backends wrap the connection in `Arc<Connection>`, created once in `new()`, cloned
> into each async block.

## Core Concepts

### 1. API-First Deployment

The deployment tool calls provider REST APIs **directly** using `SimpleHttpClient` and valtron state machines from `foundation_core`. No CLI tools (`wrangler`, `gcloud`, `aws`) are required for deployment.

| Provider | API | Auth Method |
|----------|-----|-------------|
| Cloudflare | `api.cloudflare.com/client/v4` | Bearer token |
| GCP Cloud Run | `run.googleapis.com/v2` | OAuth2 / Service Account JSON |
| AWS Lambda | `lambda.{region}.amazonaws.com` | SigV4 signing |

Each provider is an API client that knows how to:
- Upload/update deployment artifacts
- Capture state from API responses (deployment IDs, URLs, versions, bindings)
- Manage secrets, environment variables, and resource bindings
- Tear down resources

### 2. State Store as Source of Truth

The **state store** — not config files — is the source of truth for what's deployed. It records:

- Resource identity (name, provider, kind)
- Lifecycle status (`creating`, `created`, `updating`, `deleting`, `failed`)
- Config hash at time of deploy (for change detection)
- API response data (deployment IDs, URLs, versions, resource bindings)

Five interchangeable backends implement the same `StateStore` trait:

| Backend | How It Works | When To Use |
|---------|-------------|-------------|
| **Turso** | SQLite via Turso with embedded replicas and remote sync | Teams, CI/CD, cross-machine shared state |
| **SQLite** | Plain local `.db` file | Single machine, no external dependencies |
| **JSON files** | One `.json` file per resource in `.deployment/` | Simplest, git-friendly, human-readable |
| **R2** | JSON objects in Cloudflare R2 bucket via Cloudflare API | Remote state for Cloudflare-centric teams |
| **D1** | SQLite-over-HTTP via Cloudflare D1 API | Edge-native SQL state for Cloudflare-centric teams |

There is no special relationship between any state store and any provider. Any backend works with any provider.

### 3. Config Files Are Generated Artifacts

Provider config files (`wrangler.toml`, `service.yaml`, `template.yaml`) are **generated when needed** for local dev tooling:

```
ewe_platform generate-config --target cloudflare
# Writes wrangler.toml so `wrangler dev` works locally

ewe_platform generate-config --target gcp
# Writes service.yaml so `gcloud run services replace` works locally

ewe_platform generate-config --target aws
# Writes template.yaml so `sam local start-api` works locally
```

The deployment tool itself does NOT read these files to deploy. It uses its own state + API calls.

### 4. Provider Model

Every cloud target implements the `DeploymentProvider` trait:

```rust
pub trait DeploymentProvider {
    type Config: DeserializeOwned + Serialize;
    type Resources: Debug;

    fn name(&self) -> &str;
    fn validate(&self, config: &Self::Config) -> Result<(), DeploymentError>;
    fn build(&self, config: &Self::Config, env: Option<&str>) -> Result<BuildOutput, DeploymentError>;
    fn deploy(&self, config: &Self::Config, env: Option<&str>, dry_run: bool) -> Result<DeploymentResult, DeploymentError>;
    fn logs(&self, config: &Self::Config, env: Option<&str>) -> Result<(), DeploymentError>;
    fn destroy(&self, config: &Self::Config, env: Option<&str>) -> Result<(), DeploymentError>;
    fn status(&self, config: &Self::Config, env: Option<&str>) -> Result<Self::Resources, DeploymentError>;
    fn generate_config(&self, config: &Self::Config, output_dir: &Path) -> Result<(), DeploymentError>;
}
```

`Config` here is the **tool's own config** for that provider (account ID, worker name, region, resource settings), not a provider config file. It may be stored in the project's `deploy.toml` or passed programmatically.

### 5. Deployment Engine

A valtron `StateMachine` orchestrates the deployment lifecycle. The planner implements
`StateMachine` and is wrapped in `StateMachineTask` to produce a `TaskIterator`,
then driven via `valtron::execute()`.

```
Validate -> Build -> Deploy (API call) -> Capture State -> Verify -> Complete
                                                             |        ^  |
                                                             |   Delay/  [on failure]
                                                             |   Retry   |
                                                             +----------Rollback -> Failed
```

**Key Valtron patterns used:**
- `StateTransition::Continue(next)` — advance to next phase
- `StateTransition::Complete(Ok(result))` — terminal success
- `StateTransition::Complete(Err(e))` — terminal failure (NOT `StateTransition::Error`, which is silently swallowed by `StateMachineTask`)
- `StateTransition::Delay(duration, next)` — health-check retries with native executor backoff

The engine:
- Hashes the config and checks state store for changes (skip if unchanged)
- Calls `provider.build()` then `provider.deploy()` which makes API calls
- Captures the API response into the state store
- Verifies the deployment is healthy (with configurable retries via `Delay`)
- Rolls back on failure using provider-specific strategies
- **Caller must hold a `PoolGuard`** (from `valtron::initialize_pool()`) — library code does not initialize the pool

---

## Crate Architecture

```
backends/foundation_deployment/
|-- Cargo.toml
|-- src/
|   |-- lib.rs                    # Re-exports, provider registry
|   |-- error.rs                  # DeploymentError enum
|   |
|   |-- core/                     # Provider-agnostic primitives
|   |   |-- mod.rs
|   |   |-- traits.rs             # DeploymentProvider trait
|   |   |-- types.rs              # BuildOutput, DeploymentResult, DeployProgress
|   |   +-- process.rs            # ProcessExecutor (for build steps only)
|   |
|   |-- state/                    # State management
|   |   |-- mod.rs                # StateStore trait + factory
|   |   |-- types.rs              # ResourceState, StateStatus
|   |   |-- sqlite.rs             # SqliteStateStore (libsql, local-only)
|   |   |-- libsql.rs             # LibSQLStateStore (embedded with optional Turso sync)
|   |   |-- turso.rs              # TursoStateStore (remote-first with replica cache)
|   |   |-- file.rs               # JsonFileStateStore
|   |   |-- r2.rs                 # R2StateStore (Cloudflare R2 via API)
|   |   +-- d1.rs                 # D1StateStore (Cloudflare D1 via API)
|   |
|   |-- engine/                   # Deployment orchestration
|   |   |-- mod.rs
|   |   |-- planner.rs            # DeploymentPlanner (valtron StateMachine)
|   |   +-- rollback.rs           # Rollback strategies
|   |
|   |-- providers/                # Provider implementations + spec fetchers
|   |   |-- mod.rs                # Provider registry (all providers registered here)
|   |   |-- openapi.rs            # Shared OpenAPI 3.x extraction utilities
|   |   |-- standard/
|   |   |   |-- mod.rs
|   |   |   +-- fetch.rs          # Generic HTTP fetch (curl-based, used by standard providers)
|   |   |-- cloudflare/
|   |   |   |-- mod.rs
|   |   |   |-- provider.rs       # CloudflareProvider impl (DeploymentProvider trait)
|   |   |   |-- fetch.rs          # Git-clone based spec fetcher
|   |   |   +-- resources/        # Auto-generated resource types
|   |   |-- gcp/
|   |   |   |-- mod.rs
|   |   |   |-- provider.rs       # GcpCloudRunProvider impl (DeploymentProvider trait)
|   |   |   |-- fetch.rs          # Two-stage Discovery API fetcher
|   |   |   +-- resources/        # Auto-generated resource types (one .rs per API)
|   |   |-- aws/
|   |   |   |-- mod.rs
|   |   |   |-- provider.rs       # AwsLambdaProvider impl (DeploymentProvider trait)
|   |   |   +-- resources/        # Auto-generated resource types
|   |   |-- fly_io/
|   |   |   |-- mod.rs
|   |   |   |-- fetch.rs          # Spec fetcher (delegates to standard::fetch)
|   |   |   +-- resources/        # Auto-generated resource types
|   |   |-- planetscale/
|   |   |   |-- mod.rs
|   |   |   |-- fetch.rs          # Spec fetcher
|   |   |   +-- resources/
|   |   |-- prisma_postgres/
|   |   |   |-- mod.rs
|   |   |   |-- fetch.rs          # Spec fetcher
|   |   |   +-- resources/
|   |   |-- supabase/
|   |   |   |-- mod.rs
|   |   |   |-- fetch.rs          # Spec fetcher
|   |   |   +-- resources/
|   |   |-- mongodb_atlas/
|   |   |   |-- mod.rs
|   |   |   |-- fetch.rs          # Spec fetcher
|   |   |   +-- resources/
|   |   |-- neon/
|   |   |   |-- mod.rs
|   |   |   |-- fetch.rs          # Spec fetcher
|   |   |   +-- resources/
|   |   +-- stripe/
|   |       |-- mod.rs
|   |       |-- fetch.rs          # Spec fetcher
|   |       +-- resources/
|   |
|   +-- template/                 # Template generation
|       |-- mod.rs
|       +-- compose.rs            # Language x Provider matrix
|
+-- tests/
    |-- state_tests.rs
    |-- engine_tests.rs
    +-- provider_tests.rs
```

---

## State Store Model

### StateStore Trait

All methods that touch async I/O return `StateStoreStream<T>` — a lazy iterator
backed by `run_future_iter`. Callers pull results on demand; no upfront `Vec`
allocation. The stream type aliases are defined in `state/types.rs`:

```rust
use foundation_core::valtron::ThreadedValue;

/// Lazy stream of state store results. Backed by `run_future_iter` for SQL/HTTP
/// backends, or `Vec::into_iter().map(...)` for sync backends (JSON files).
pub type StateStoreStream<T> = Box<dyn Iterator<Item = ThreadedValue<T, DeploymentError>> + Send>;

pub trait StateStore: Send + Sync {
    /// Initialize the store (create tables, directories, etc.).
    /// Uses `exec_future` internally — this is one-shot bootstrap, not a trait query.
    fn init(&self) -> Result<(), DeploymentError>;

    /// List all resource IDs. Returns a lazy stream.
    fn list(&self) -> Result<StateStoreStream<String>, DeploymentError>;

    /// Get state for a single resource. Returns a stream yielding 0 or 1 items.
    fn get(&self, resource_id: &str) -> Result<StateStoreStream<Option<ResourceState>>, DeploymentError>;

    /// Set (create or update) state. Returns a stream signaling completion.
    fn set(&self, resource_id: &str, state: &ResourceState) -> Result<StateStoreStream<()>, DeploymentError>;

    /// Delete state. Returns a stream signaling completion.
    fn delete(&self, resource_id: &str) -> Result<StateStoreStream<()>, DeploymentError>;

    /// Get all resource states. Returns a lazy stream of resources.
    fn all(&self) -> Result<StateStoreStream<ResourceState>, DeploymentError>;

    /// Sync state to a remote location (no-op for FileStateStore).
    fn sync_remote(&self) -> Result<(), DeploymentError> {
        Ok(()) // Default: no remote sync
    }
}
```

**Consuming streams at the boundary:**

```rust
use foundation_core::valtron::ThreadedValue;

// Single-value: pull first result
let state = store.get("my-worker")?
    .find_map(|v| match v {
        ThreadedValue::Value(Ok(val)) => Some(val),
        _ => None,
    })
    .flatten(); // Option<Option<ResourceState>> → Option<ResourceState>

// Multi-value: collect lazily
let ids: Vec<String> = store.list()?
    .filter_map(|v| match v {
        ThreadedValue::Value(Ok(id)) => Some(id),
        ThreadedValue::Value(Err(e)) => { tracing::warn!("stream error: {e}"); None }
    })
    .collect();
```

### ResourceState

```rust
pub struct ResourceState {
    pub id: String,
    pub kind: String,                        // "cloudflare::worker", "gcp::cloud-run-service"
    pub provider: String,                    // "cloudflare", "gcp", "aws"
    pub status: StateStatus,
    pub environment: Option<String>,
    pub config_hash: String,                 // For change detection
    pub output: serde_json::Value,           // Captured API response data
    pub config_snapshot: serde_json::Value,  // Input config at time of deploy
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}
```

### Change Detection

Hash the deployment config, compare to `config_hash` in state. If unchanged and status is `Created`, skip deployment.

---

## Deployment Flow

All providers follow the same flow — the only difference is which API gets called:

```
1. Load config (from deploy.toml, env vars, or programmatic input)
2. Hash config, check state store
3. If unchanged and status=Created: skip (return cached result)
4. Build artifacts (cargo build, docker build, etc.)
5. Call provider API to deploy (PUT worker, create Cloud Run service, update Lambda code)
6. Capture API response into state store
7. Verify deployment is healthy (hit URL, check status endpoint)
8. Update state to Created
```

### Cloudflare Workers

```
Build: worker-build --release (or wasm-pack build)
API:   PUT /accounts/{id}/workers/scripts/{name}  (upload script)
       PUT /accounts/{id}/workers/scripts/{name}/secrets  (set secrets)
State: Capture deployment_id, worker URL, version tag
```

### GCP Cloud Run

```
Build: docker build + push to Artifact Registry
API:   POST /v2/projects/*/locations/*/services  (create/update service)
       Wait for operation to complete
State: Capture revision name, service URL, traffic split
```

### AWS Lambda

```
Build: cargo lambda build --release (or sam build)
API:   PUT /functions/{name}/code  (upload zip)
       POST /functions/{name}/versions  (publish version)
       PUT /functions/{name}/aliases/{alias}  (update alias)
State: Capture function ARN, version, alias, API Gateway URL
```

---

## Template Composition

Templates generate a project with:
- Language-specific build setup (Cargo.toml, Dockerfile, etc.)
- Provider-specific deployment config (stored in `deploy.toml` or equivalent)
- mise.toml with provider-agnostic tasks

Generation: `ewe_platform generate --lang rust --target cloudflare`

Config files for local dev tools are generated on demand via `generate_config`.

---

## Mise Task Model

Provider-agnostic task names:

```toml
[tasks.deploy]
run = "ewe_platform deploy"

[tasks.deploy_staging]
run = "ewe_platform deploy --env staging"

[tasks.status]
run = "ewe_platform status"

[tasks.logs]
run = "ewe_platform logs"

[tasks.destroy]
run = "ewe_platform destroy"

[tasks.generate_config]
description = "Generate provider config files for local dev"
run = "ewe_platform generate-config"
```

---

_Related Specifications:_
- `specifications/02-build-http-client` - SimpleHttpClient used by all provider API clients
- `specifications/08-valtron-async-iterators` - Valtron patterns used by deployment engine

_Inspiration:_
- alchemy IaC framework - StateStore interface, resource lifecycle, provider model

---

_Created: 2026-03-26_
