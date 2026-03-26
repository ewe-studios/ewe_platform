# Foundation Deployment - Architecture Overview

**Created:** 2026-03-26
**Specification:** `specifications/11-foundation-deployment`

---

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
|   |   |-- turso.rs              # TursoStateStore (libsql)
|   |   |-- sqlite.rs             # SqliteStateStore (rusqlite)
|   |   |-- file.rs               # JsonFileStateStore
|   |   |-- r2.rs                 # R2StateStore (Cloudflare R2 via API)
|   |   +-- d1.rs                 # D1StateStore (Cloudflare D1 via API)
|   |
|   |-- engine/                   # Deployment orchestration
|   |   |-- mod.rs
|   |   |-- planner.rs            # DeploymentPlanner (valtron StateMachine)
|   |   +-- rollback.rs           # Rollback strategies
|   |
|   |-- providers/                # Provider API clients
|   |   |-- mod.rs                # Provider registry
|   |   |-- cloudflare/
|   |   |   |-- mod.rs            # CloudflareProvider impl
|   |   |   |-- api.rs            # Cloudflare REST API via SimpleHttpClient
|   |   |   +-- types.rs          # CF-specific types
|   |   |-- gcp/
|   |   |   |-- mod.rs            # GcpCloudRunProvider impl
|   |   |   |-- api.rs            # Cloud Run Admin API via SimpleHttpClient
|   |   |   +-- types.rs          # GCP-specific types
|   |   +-- aws/
|   |       |-- mod.rs            # AwsLambdaProvider impl
|   |       |-- api.rs            # Lambda API via SimpleHttpClient
|   |       |-- sigv4.rs          # AWS Signature V4 signing
|   |       +-- types.rs          # AWS-specific types
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

```rust
pub trait StateStore: Send + Sync {
    fn init(&self) -> Result<(), DeploymentError>;
    fn list(&self) -> Result<Vec<String>, DeploymentError>;
    fn get(&self, resource_id: &str) -> Result<Option<ResourceState>, DeploymentError>;
    fn set(&self, resource_id: &str, state: &ResourceState) -> Result<(), DeploymentError>;
    fn delete(&self, resource_id: &str) -> Result<(), DeploymentError>;
    fn all(&self) -> Result<Vec<ResourceState>, DeploymentError>;
}
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
