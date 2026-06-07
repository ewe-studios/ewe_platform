# Feature 14: Cedar Policy Engine

## Description

A standalone `foundation_cedar` crate that wraps the `cedar-policy` engine with multi-source policy storage, entity providers, and authorization middleware. Provides a reusable authorization layer that powers the spec-38 IdP server, `foundation_http` middleware, or any standalone application.

The engine separates **policy storage** (where policies live) from **entity providers** (where user/resource data comes from) from the **authorizer** (the Cedar evaluation core). Each has pluggable backends.

## Why Cedar

Cedar is a language for defining permissions as policies. It provides:
- **Fine-grained access control** — RBAC, ABAC, ReBAC in one language
- **Formal verification** — policies can be validated against schemas
- **Fast evaluation** — sub-millisecond authorization decisions
- **Template support** — parameterized policies for multi-tenant setups
- **Partial evaluation** — handle missing entity data gracefully
- **WASM compatible** — runs in Cloudflare Workers, browser edge environments

## Reference Sources

Reference sources listed above. Git repo links to be added when repos are created.
The `connectrpc-cedar` example in this org (see path above) shows real Cedar middleware
usage — study it for billing/account context propagation patterns, but adapt to the
ewe_platform way.

- **cedar-policy crate** (v4.11.0): `/home/darkvoid/Boxxed/@formulas/src.rust/src.auth/src.CedarPolicy/cedar/cedar-policy/`
- **cedar-local-agent**: `/home/darkvoid/Boxxed/@formulas/src.rust/src.auth/src.CedarPolicy/cedar-local-agent/` — Amazon's async `SimplePolicySetProvider` / `SimpleEntityProvider` pattern
- **cedar-examples**: `/home/darkvoid/Boxxed/@formulas/src.rust/src.auth/src.CedarPolicy/cedar-examples/` — Rust hello world, TinyTodo, use cases
- **connectrpc-cedar**: `/home/darkvoid/Boxxed/@formulas/src.rust/src.gedweb/cf-connectrpc-middleware/crates/connectrpc-cedar/` — real Cedar middleware in this org (learn from this, but ensure we do this the ewe_platform way, interested in how it carries the billing account information as well)
- **multitenant schema**: `cf-connectrpc-middleware/examples/multitenant-policies/multitenant.cedarschema`
- **Cedar docs**: https://docs.cedarpolicy.com/

---

## Crate: `foundation_cedar`

### Location

`backends/foundation_cedar/` — a standalone crate under `backends/`.

### Feature Flags

**TODO**: I want to elevate `HttpPolicyFetcher` to more than just a git fallback meaning we can have a GitRepoHttpPolicyFetcher that knows how to interact with github/gitlab/etc over http. 
But also a standard `HttpPolicyFetcher` that can be used for any HTTP endpoint which will present a set of expected or with a mapping for where to fetch specific policy files from.

Git storage capability is implemented in `foundation_nativeapis` as a generic
`PolicyFetcher` trait with `GitPolicyFetcher` (gix-based) and `HttpPolicyFetcher`
(HTTP raw-file fallback) implementations. This crate consumes it via feature flags
for reusability and extendability.

```toml
[dependencies]
cedar-policy = { version = "4.11", default-features = true }

# Policy storage backends (native only)
r2-storage = ["foundation_db/r2"]       # Cloudflare R2 via foundation_db
d1-storage = ["foundation_db/d1"]       # Cloudflare D1 via foundation_db
local-file-storage = []                  # Local filesystem policy directory
git-storage = ["dep:gix"]               # Git repository as policy source

# Wasm policy storage backends
wasm-d1-storage = ["foundation_db/wasm-bindgen-storage"]  # D1 via wasm-bindgen
wasm-r2-storage = ["foundation_db/wasm-bindgen-storage"]  # R2 via wasm-bindgen

# Partial evaluation (experimental cedar feature)
partial-eval = ["cedar-policy/partial-eval"]

# Schema validation (always on by default)
schema-validation = []

# foundation_http middleware integration
http-middleware = ["foundation_http"]

# Wasm-compatible git transport (uses fetch instead of TCP)
wasm-git-fetch = ["dep:gix-protocol", "dep:gix-fetch"]

[features]
default = ["schema-validation"]
native = ["local-file-storage", "r2-storage", "d1-storage", "git-storage"]
wasm = ["wasm-d1-storage", "wasm-r2-storage"]
full = ["native", "http-middleware", "partial-eval"]
```

---

## Architecture

```
foundation_cedar/
├── src/
│   ├── lib.rs                          # Re-exports, crate root
│   │
│   ├── core/                           # Always compiled, cross-platform
│   │   ├── mod.rs
│   │   ├── engine.rs                   # CedarEngine — schema + policy set + authorizer
│   │   ├── authorizer.rs               # Evaluation: is_authorized, is_authorized_partial
│   │   ├── request.rs                  # CedarRequest builder (principal, action, resource, context)
│   │   ├── response.rs                 # CedarResponse wrapper (decision, diagnostics, reasons)
│   │   ├── schema.rs                   # Schema loading, validation helpers
│   │   ├── errors.rs                   # CedarError enum
│   │   │
│   │   └── policy/                     # Policy management types
│   │       ├── mod.rs
│   │       ├── policy_set.rs           # PolicySetSource — where policies come from
│   │       ├── entity_provider.rs      # EntityProvider trait + EntitySetSource
│   │       ├── policy_format.rs        # PolicyFormat (Cedar text, JSON)
│   │       └── validation.rs           # Policy validation against schema
│   │
│   ├── storage/                        # Policy storage backends
│   │   ├── mod.rs                      # PolicyStore trait definitions
│   │   ├── traits.rs                   # PolicyStore (sync) + AsyncPolicyStore
│   │   │
│   │   ├── shared/                     # Shared logic (serialization, parsing)
│   │   │   ├── mod.rs
│   │   │   ├── parser.rs               # Parse .cedar files, JSON policies
│   │   │   └── resolver.rs             # Policy resolution from raw text → PolicySet
│   │   │
│   │   ├── native/                     # Native-only backends
│   │   │   ├── mod.rs
│   │   │   ├── local_file.rs           # Read .cedar/.json from local directory
│   │   │   ├── r2_store.rs             # Policies from Cloudflare R2 (via foundation_db R2Store)
│   │   │   ├── d1_store.rs             # Policies from D1 SQLite (via foundation_db D1Store)
│   │   │   └── git_store.rs            # Policies from git repo (via gix crates)
│   │   │
│   │   └── wasm_bindgen/               # Wasm-only backends
│   │       ├── mod.rs
│   │       ├── d1_wasm.rs              # Policies from D1 via wasm-bindgen
│   │       ├── r2_wasm.rs              # Policies from R2 via wasm-bindgen
│   │       └── git_wasm.rs             # Policies from git via gix + browser fetch
│   │
│   └── middleware/                     # HTTP middleware (feature-gated)
│       ├── mod.rs
│       ├── tower_layer.rs              # tower::Layer for HTTP authorization
│       └── extractors.rs               # Request → Cedar principal/action/resource extractors
│
├── fundamentals/                       # Documentation directory
│   ├── cedar_basics.md                 # Cedar language primer: policies, entities, schemas
│   ├── cedar_patterns.md               # Common authorization patterns (RBAC, ABAC, ReBAC)
│   ├── cedar_wasm.md                   # Running Cedar in WASM / Cloudflare Workers
│   ├── cedar_partial_eval.md           # Partial evaluation guide
│   └── cedar_integration_guide.md      # How to hook into foundation_auth, foundation_http, etc.
│
└── tests/
    ├── policy_store_tests.rs           # PolicyStore backend tests
    ├── engine_tests.rs                 # CedarEngine evaluation tests
    └── integration.rs                  # Full integration: policy → engine → decision
```

---

## Core API

### CedarEngine

The central object — holds schema, policies, and entities, and evaluates authorization requests.

```rust
/// The Cedar authorization engine. Holds schema, policy set, and base entities.
/// Cheap to clone (internally Arc-backed by cedar-policy 4.x).
#[derive(Clone)]
pub struct CedarEngine {
    schema: Schema,
    policies: PolicySet,
    entities: Entities,
    authorizer: Authorizer,
}

impl CedarEngine {
    /// Build from schema text and concatenated Cedar policy text.
    /// Validates policies against schema at construction time.
    pub fn from_str(schema: &str, policies: &str) -> Result<Self, CedarError>;

    /// Build with a custom entity store (for static relations like admin UIDs).
    pub fn with_entities(
        schema: &str,
        policies: &str,
        entities: &Entities,
    ) -> Result<Self, CedarError>;

    /// Evaluate an authorization request.
    pub fn is_authorized(&self, request: &CedarRequest) -> CedarResponse;

    /// Evaluate with partial evaluation (when some entity data is unknown).
    #[cfg(feature = "partial-eval")]
    pub fn is_authorized_partial(&self, request: &CedarRequest) -> PartialCedarResponse;

    /// Get the schema (for validation, entity construction).
    pub fn schema(&self) -> &Schema;

    /// Get the current policy set.
    pub fn policies(&self) -> &PolicySet;
}
```

### CedarRequest

Builder for Cedar authorization requests.

```rust
/// An authorization request to evaluate.
pub struct CedarRequest {
    principal: EntityUid,
    action: EntityUid,
    resource: EntityUid,
    context: Context,
}

impl CedarRequest {
    pub fn builder() -> CedarRequestBuilder;
}

/// Builder for Cedar requests.
pub struct CedarRequestBuilder {
    principal: Option<EntityUid>,
    action: Option<EntityUid>,
    resource: Option<EntityUid>,
    context_pairs: Vec<(SmolStr, RestrictedExpression)>,
}

impl CedarRequestBuilder {
    pub fn principal(mut self, uid: EntityUid) -> Self;
    pub fn action(mut self, uid: EntityUid) -> Self;
    pub fn resource(mut self, uid: EntityUid) -> Self;
    pub fn context<K, V>(mut self, key: K, value: V) -> Self
        where K: Into<SmolStr>, V: Into<RestrictedExpression>;
    pub fn context_json(mut self, json: serde_json::Value) -> Self;
    pub fn build(self) -> Result<CedarRequest, CedarError>;
}
```

### CedarResponse

```rust
/// Result of an authorization evaluation.
#[derive(Debug, Clone)]
pub struct CedarResponse {
    decision: Decision,
    reasons: Vec<PolicyId>,
    errors: Vec<EvaluationError>,
}

impl CedarResponse {
    /// Whether the request was allowed.
    #[must_use]
    pub fn is_allowed(&self) -> bool;

    /// The Cedar decision (Allow or Deny).
    #[must_use]
    pub fn decision(&self) -> Decision;

    /// Policy IDs that contributed to the decision.
    #[must_use]
    pub fn reasons(&self) -> &[PolicyId];

    /// Evaluation errors (type errors, missing entities, etc.).
    #[must_use]
    pub fn errors(&self) -> &[EvaluationError];
}
```

---

## PolicyStore Traits

Following the async-first pattern: the async trait is primary, sync wraps async via valtron.

### Async Trait (PRIMARY IMPLEMENTATION)

All policy store backends implement this trait. The async methods contain the actual
I/O logic (file reads, HTTP fetches, git operations, DB queries):

```rust
/// Asynchronous policy store — reads policies from a source.
/// This is the PRIMARY trait — implementations live here.
#[async_trait::async_trait]
pub trait AsyncPolicyStore: Debug + Send + Sync {
    /// Load all policies from the source. Returns concatenated Cedar policy text.
    /// Multiple .cedar/.json files are concatenated.
    async fn load_policies_async(&self) -> Result<String, PolicyStoreError>;

    /// Load the schema from the source. Returns Cedar schema text.
    async fn load_schema_async(&self) -> Result<String, PolicyStoreError>;

    /// Load entities from the source (optional — many setups use empty entities).
    async fn load_entities_async(&self) -> Result<Option<String>, PolicyStoreError> {
        Ok(None)
    }

    /// Check if policies have changed since last load (for hot-reload).
    /// Returns a version identifier (hash, timestamp, commit SHA).
    async fn version_async(&self) -> Result<String, PolicyStoreError>;
}
```

### Sync Trait (WRAPPER — CALLS ASYNC VIA VALTRON)

```rust
/// Synchronous policy store — reads policies from a source.
/// This is a wrapper trait. Implementations bridge from AsyncPolicyStore via valtron.
pub trait PolicyStore: Debug + Send + Sync {
    fn load_policies(&self) -> Result<String, PolicyStoreError>;
    fn load_schema(&self) -> Result<String, PolicyStoreError>;
    fn load_entities(&self) -> Result<Option<String>, PolicyStoreError> { Ok(None) }
    fn version(&self) -> Result<String, PolicyStoreError>;
}
```

### Valtron Bridge: Async → Sync

The sync wrapper calls the async implementation via valtron. This works on **both**
native and wasm — valtron is the execution engine, not tokio:

```rust
use foundation_core::valtron::{from_future, execute, collect_one};

/// Bridges an AsyncPolicyStore to provide sync access via valtron.
/// Works on both native and wasm — valtron handles the execution.
pub struct SyncPolicyStoreBridge<S: AsyncPolicyStore> {
    inner: Arc<S>,
}

impl<S: AsyncPolicyStore> SyncPolicyStoreBridge<S> {
    pub fn new(store: S) -> Self {
        Self { inner: Arc::new(store) }
    }
}

impl<S: AsyncPolicyStore + 'static> PolicyStore for SyncPolicyStoreBridge<S> {
    fn load_policies(&self) -> Result<String, PolicyStoreError> {
        let store = Arc::clone(&self.inner);
        let task = from_future(async move {
            store.load_policies_async().await
        });
        let stream = execute(task, None)
            .map_err(|e| PolicyStoreError::Scheduling(e.to_string()))?;
        collect_one(stream)
            .ok_or_else(|| PolicyStoreError::NoResult)
            .and_then(|r| r)  // flatten Result<Result<T, E>, _>
    }
    // ... same pattern for load_schema, load_entities, version
}
```

**Caveat — when sync cannot wrap async:**
If a PolicyStore implementation requires `&mut self` for internal state mutation,
valtron bridging becomes difficult (requires interior mutability with `Arc<Mutex<T>>`).
In that case, implement both traits separately. Most policy stores are read-only
(`&self`), so the bridge pattern works.

---

## EntityProvider Traits

Entities represent users, resources, groups — the data Cedar evaluates against. The engine doesn't care where entities come from; providers supply them.

### Async Entity Provider (PRIMARY)

```rust
/// Async entity provider — supplies Cedar entities for authorization evaluation.
///
/// Implementations can pull from JWT claims, database queries (via AsyncQueryStore),
/// local files, or any other source. The engine is source-agnostic.
#[async_trait::async_trait]
pub trait AsyncEntityProvider: Debug + Send + Sync {
    /// Get entities relevant to a specific authorization request.
    ///
    /// The request contains principal, action, resource, and context — use
    /// these to fetch only the entities needed for evaluation (e.g., the
    /// user's groups, the resource's owner, etc.).
    async fn get_entities_async(&self, request: &CedarRequest) -> Result<Entities, EntityProviderError>;
}
```

### Sync Entity Provider (WRAPPER)

```rust
/// Sync entity provider (for non-async contexts).
/// Wraps AsyncEntityProvider via valtron.
pub trait SyncEntityProvider: Debug + Send + Sync {
    fn get_entities(&self, request: &CedarRequest) -> Result<Entities, EntityProviderError>;
}

/// Default bridge implementation.
pub struct SyncEntityProviderBridge<P: AsyncEntityProvider> {
    inner: Arc<P>,
}

impl<P: AsyncEntityProvider + 'static> SyncEntityProvider for SyncEntityProviderBridge<P> {
    fn get_entities(&self, request: &CedarRequest) -> Result<Entities, EntityProviderError> {
        let provider = Arc::clone(&self.inner);
        let request = request.clone(); // or Arc-share if Clone is expensive
        let task = from_future(async move {
            provider.get_entities_async(&request).await
        });
        let stream = execute(task, None)
            .map_err(|e| EntityProviderError::Scheduling(e.to_string()))?;
        collect_one(stream)
            .ok_or_else(|| EntityProviderError::NoResult)
            .and_then(|r| r)
    }
}
```

### Default Implementations

Both sync and async versions of each entity provider are provided. The async version
uses `AsyncQueryStore` for database lookups; the sync version wraps async via valtron.

#### JWT-based EntityProvider

Extracts entity information directly from JWT claims — no DB or remote lookups needed:

```rust
/// Entity provider that builds Cedar entities from JWT claims.
///
/// Example: A JWT with {"sub": "user123", "groups": ["admins", "users"]}
/// produces:
///   - User::"user123" entity
///   - Group::"admins" and Group::"users" as parent entities
pub struct JwtEntityProvider {
    claims_mapper: Box<dyn Fn(&serde_json::Value) -> Vec<Entity> + Send + Sync>,
}

impl JwtEntityProvider {
    /// Create from a claims mapping function.
    pub fn new<F>(mapper: F) -> Self
    where
        F: Fn(&serde_json::Value) -> Vec<Entity> + Send + Sync + 'static;

    /// Create with default mapping (extracts sub, email, groups from standard claims).
    pub fn default() -> Self;
}
```

#### Database-backed EntityProvider

Uses `foundation_db` `QueryStore` for entity lookups:

```rust
/// Entity provider backed by a foundation_db QueryStore.
pub struct DbEntityProvider {
    store: StorageProvider,
    query_builder: Box<dyn Fn(&CedarRequest) -> Vec<EntityQuery> + Send + Sync>,
}
```

#### Static EntityProvider

Hard-coded entities (useful for testing or fixed admin lists):

```rust
/// Static entity provider — returns a fixed set of entities.
pub struct StaticEntityProvider {
    entities: Entities,
}
```

---

## PolicyStore Backends

### 1. Local File Store (native)

Reads `.cedar` and `.json` policy files from a local directory.

```rust
/// Reads policies from a local directory.
///
/// Directory structure:
///   policies/
///     schema.cedar          # Schema (cedar syntax)
///     schema.json           # Or JSON schema
///     main.cedar            # Main policies
///     templates/            # Template policies
///       multi-tenant.cedar
///     entities.json         # Optional static entities
pub struct LocalFilePolicyStore {
    root_path: PathBuf,
    file_pattern: Option<String>,  // e.g., "*.cedar"
}
```

Features:
- Scans directory for `.cedar` and `.json` policy files
- Concatenates all policy files into a single PolicySet
- `version()` returns a hash of all file contents + timestamps
- Supports file watching for hot-reload (future enhancement)

### 2. R2 Blob Store (native + wasm)

Policies stored as objects in Cloudflare R2.

```rust
/// Reads policies from Cloudflare R2 bucket.
///
/// Object structure:
///   policies/schema.cedar
///   policies/main.cedar
///   policies/templates/multi-tenant.cedar
///   policies/entities.json
pub struct R2PolicyStore {
    bucket: String,
    prefix: String,  // e.g., "policies/"
    // Uses foundation_db R2Store (native) or R2WasmStorage (wasm)
}
```

Features:
- Uses existing `foundation_db` R2 backend (native) or R2WasmStorage (wasm)
- Lists objects under prefix, downloads and concatenates
- `version()` returns ETag or last-modified hash
- Supports ETag-based conditional fetches (skip download if unchanged)

### 3. D1 SQLite Store (native + wasm)

Policies stored in D1/Turso SQLite tables.

```rust
/// Reads policies from a SQLite database (D1, Turso, libsql).
///
/// Table structure:
///   CREATE TABLE cedar_policies (
///     id TEXT PRIMARY KEY,
///     name TEXT NOT NULL,
///     policy_text TEXT NOT NULL,
///     schema TEXT,          -- NULL for non-schema rows
///     updated_at INTEGER
///   );
pub struct D1PolicyStore {
    store: StorageProvider,  // Uses foundation_db QueryStore
    table: String,
}
```

Features:
- Uses existing `foundation_db` `QueryStore` / `AsyncQueryStore` traits
- Works with Turso (native), libsql (native), D1 (wasm via wasm-bindgen)
- `version()` returns MAX(updated_at) or hash of all policy rows
- Supports incremental updates via `updated_at` tracking

### 4. Git Repository Store (native + wasm)

Policies stored in a git repository. Git operations are provided by
`foundation_nativeapis` via the `PolicyFetcher` trait, which supports pluggable
transports (git protocol or HTTP raw-file fetch).

#### Pluggable Transport Design

The `PolicyFetcher` trait in `foundation_nativeapis` abstracts over transport:

```rust
/// Abstract trait for fetching policy content from a remote source.
/// Both git-based and HTTP-based fetchers implement this.
#[async_trait::async_trait]
pub trait PolicyFetcher: Debug + Send + Sync {
    /// Fetch a single file's content by path.
    async fn fetch_file(&self, path: &str) -> Result<Vec<u8>, PolicyStoreError>;

    /// Fetch all files under a directory prefix.
    async fn fetch_dir(&self, prefix: &str) -> Result<Vec<(String, Vec<u8>)>, PolicyStoreError>;

    /// Get the current version/commit identifier.
    async fn version(&self) -> Result<String, PolicyStoreError>;
}
```

Two implementations exist — the caller instantiates whichever fits the platform:

```rust
/// Git-based fetcher — uses gix for full git operations.
/// Available on both native and wasm (with appropriate gix feature flags).
pub struct GitPolicyFetcher {
    repo: Arc<Mutex<Repository>>,
    remote_url: String,
    branch: String,
}

/// HTTP-based fetcher — fetches raw files via HTTP.
/// Used when git protocol is unavailable (e.g., WASM without gix-protocol).
/// Requires the git hosting provider to support raw file URLs:
///   GitHub: https://raw.githubusercontent.com/{org}/{repo}/{branch}/{path}
///   GitLab: https://gitlab.com/{org}/{repo}/-/raw/{branch}/{path}
pub struct HttpPolicyFetcher {
    base_url: String,
    client: HttpClient,  // platform-specific HTTP client
}
```

**What goes in `foundation_nativeapis`:**
- `PolicyFetcher` trait (the abstract interface above)
- `GitPolicyFetcher` — native implementation using full gix
- `GitPolicyFetcherWasm` — WASM implementation using gix-protocol + custom HTTP transport
- `HttpPolicyFetcher` — fallback for both platforms when git protocol is unavailable
- Feature flags: `git-native` (full gix), `git-wasm` (gix subsets + HTTP fallback)

**What goes in `foundation_cedar`:**
- `GitPolicyStore` — accepts a `Box<dyn PolicyFetcher>`, doesn't care if git or HTTP underneath
- Knows about Cedar-specific paths (schema.cedar, policies/, entities/)
- Parses .cedar files from fetched blobs into PolicySet
- `version()` returns the git SHA from the fetcher

#### GitPolicyStore

```rust
/// Reads policies from a git repository via an injected PolicyFetcher.
///
/// Repo structure:
///   schema.cedar
///   policies/
///     main.cedar
///     rbac.cedar
///     abac.cedar
///   entities/
///     static.json
pub struct GitPolicyStore {
    fetcher: Box<dyn PolicyFetcher>,
    policy_paths: Vec<PathBuf>, // Paths to scan for policies
}

#[async_trait::async_trait]
impl AsyncPolicyStore for GitPolicyStore {
    async fn load_policies_async(&self) -> Result<String, PolicyStoreError> {
        let mut policies = String::new();
        for path in &self.policy_paths {
            let content = self.fetcher.fetch_file(path.to_str().unwrap()).await?;
            policies.push_str(&String::from_utf8_lossy(&content));
            policies.push_str("\n\n");
        }
        Ok(policies)
    }

    async fn version_async(&self) -> Result<String, PolicyStoreError> {
        self.fetcher.version().await
    }
}
```

#### Native Implementation

Uses `gix` crate for full git operations (clone, fetch, checkout):

```rust
// Native: full gix clone + fetch
use gix::{Repository, prepare_clone};

impl GitPolicyFetcher {
    /// Clone or open the repository.
    pub fn init(&self) -> Result<Repository, PolicyStoreError> {
        if self.repo_path.exists() {
            gix::discover(&self.repo_path)
                .map_err(PolicyStoreError::Git)
        } else {
            prepare_clone(&self.remote_url)
                .with_ref(&self.branch)
                .fetch_then_checkout(gix::progress::Discard, &gix::interrupt::IS_INTERRUPTED)
                .map_err(PolicyStoreError::Git)?
                .main_worktree(gix::progress::Discard, &gix::interrupt::IS_INTERRUPTED)
                .map(|(repo, _)| repo)
                .map_err(PolicyStoreError::Git)
        }
    }
}
```

#### WASM Implementation

Uses gix with WASM-compatible crates and HTTP transport:

```rust
// WASM: gix-protocol with custom transport via web-sys fetch API
// gix-protocol supports custom transports — use web-sys fetch API
use gix_protocol::fetch;
```

Key gix crates for WASM:
- `gix-odb` — object database reading in memory
- `gix-pack` — packfile decoding
- `gix-protocol` — Git protocol (custom transport via web-sys fetch)
- `gix` — high-level API (feature-gated for WASM)

If gix compilation is problematic in WASM, the `HttpPolicyFetcher` provides a
pragmatic fallback that fetches raw files via HTTP from the git hosting provider.

#### Policy Resolution from Git

```rust
impl PolicyStore for GitPolicyStore {
    fn load_policies(&self) -> Result<String, PolicyStoreError> {
        let repo = self.init()?;
        let head = repo.head_commit().map_err(PolicyStoreError::Git)?;
        let tree = head.tree().map_err(PolicyStoreError::Git)?;

        let mut policies = String::new();
        for path in &self.policy_paths {
            if let Some(blob) = tree.lookup_blob(path) {
                let data = blob.data().map_err(PolicyStoreError::Git)?;
                policies.push_str(&String::from_utf8_lossy(data));
                policies.push_str("\n\n");
            }
        }
        Ok(policies)
    }

    fn version(&self) -> Result<String, PolicyStoreError> {
        let repo = self.init()?;
        let head = repo.head_commit().map_err(PolicyStoreError::Git)?;
        Ok(head.id().to_string())  // Git SHA as version
    }
}
```

---

## Policy Resolution Pipeline

The `PolicyResolver` takes raw text from any `PolicyStore` and produces a `PolicySet`:

```rust
/// Resolves raw policy text into a validated PolicySet.
pub struct PolicyResolver;

impl PolicyResolver {
    /// Parse concatenated Cedar policy text into a PolicySet.
    pub fn parse_policies(text: &str) -> Result<PolicySet, CedarError>;

    /// Parse policy JSON (EST format) into a PolicySet.
    pub fn parse_policies_json(json: &str) -> Result<PolicySet, CedarError>;

    /// Parse schema text (Cedar or JSON format).
    pub fn parse_schema(text: &str, format: SchemaFormat) -> Result<Schema, CedarError>;

    /// Validate a PolicySet against a Schema.
    pub fn validate(policies: &PolicySet, schema: &Schema) -> Result<(), CedarError>;

    /// Load, parse, and validate from any PolicyStore.
    pub fn load_from_store(store: &impl PolicyStore) -> Result<CedarEngine, CedarError>;

    /// Async version.
    pub async fn load_from_async_store(store: &impl AsyncPolicyStore) -> Result<CedarEngine, CedarError>;
}
```

---

## Policy Hot-Reload

Policies can change at runtime. The `PolicyWatcher` periodically checks for updates and rebuilds the engine:

```rust
/// Watches for policy changes and rebuilds the CedarEngine.
pub struct PolicyWatcher<S> {
    store: Arc<S>,
    engine: RwLock<CedarEngine>,
    last_version: RwLock<String>,
    check_interval: Duration,
}

impl<S: AsyncPolicyStore> PolicyWatcher<S> {
    /// Start watching for policy changes.
    pub async fn start(self: Arc<Self>) -> JoinHandle<()>;

    /// Get the current engine (clones the Arc).
    pub fn engine(&self) -> CedarEngine;

    /// Force a reload check.
    pub async fn check(&self) -> Result<bool, PolicyStoreError>;  // true if reloaded
}
```

The watcher:
1. Calls `store.version()` periodically
2. If version changed, calls `PolicyResolver::load_from_async_store()`
3. Swaps the engine under `RwLock`
4. Zero-downtime policy updates — in-flight requests use the old engine

---

## HTTP Middleware Integration

Feature-gated behind `http-middleware`. Provides a `tower::Layer` that evaluates Cedar policies for each request.

```rust
/// Tower middleware layer that gates requests via Cedar policies.
pub struct CedarLayer<E, P> {
    engine: Arc<CedarEngine>,
    extractor: E,       // Extracts principal/action/resource from request
    entity_provider: Arc<P>,
}

impl<E, P> CedarLayer<E, P> {
    pub fn new(
        engine: CedarEngine,
        extractor: E,
        entity_provider: P,
    ) -> Self;
}

/// Extracts Cedar request components from an HTTP request.
pub trait RequestExtractor: Send + Sync {
    fn extract_principal(&self, request: &Request<Body>) -> Option<EntityUid>;
    fn extract_action(&self, request: &Request<Body>) -> Option<EntityUid>;
    fn extract_resource(&self, request: &Request<Body>) -> Option<EntityUid>;
    fn extract_context(&self, request: &Request<Body>) -> Context;
}
```

### Usage with foundation_http

```rust
use foundation_cedar::middleware::CedarLayer;
use foundation_cedar::engine::CedarEngine;

// Build the engine
let engine = CedarEngine::from_str(schema, policies)?;

// Create the middleware layer
let layer = CedarLayer::new(
    engine,
    JwtRequestExtractor::default(),  // Extracts from JWT token
    JwtEntityProvider::default(),     // Builds entities from JWT claims
);

// Apply to foundation_http app
let mut app = HttpApp::new_writer();
app.layer(layer);
app.route_writer(SimpleMethod::GET, "/api/protected");
```

---

## Integration with Spec 38 IdP Server

The IdP server (features 09-12) can optionally use Cedar for authorization gating:

### Example: Gating OIDC endpoints

```cedar
// Only admin users can manage OAuth clients
permit(
    principal in Group::"admins",
    action == Action::"create_client",
    resource == Client::"oidc"
);

// Users can only view their own userinfo
permit(
    principal == resource.owner,
    action == Action::"view_userinfo",
    resource == Userinfo
);

// Token introspection requires resource_server role
permit(
    principal has role && principal.role == "resource_server",
    action == Action::"introspect",
    resource == Token::"access"
);
```

### IdP Server + Cedar Integration Point

```rust
// In IdpServer::http_app(), optionally wrap handlers with Cedar middleware
#[cfg(feature = "cedar")]
pub fn http_app_with_cedar(
    &self,
    engine: CedarEngine,
    extractor: impl RequestExtractor,
    entity_provider: impl EntityProvider,
) -> HttpApp<Arc<dyn ServeWriter>> {
    let mut app = self.http_app();

    // Register Cedar-protected routes
    let cedar_layer = CedarLayer::new(engine, extractor, entity_provider);
    app.layer(cedar_layer);

    app
}
```

### Standalone Usage

```rust
// Completely independent of foundation_auth — any app can use foundation_cedar
use foundation_cedar::engine::CedarEngine;
use foundation_cedar::storage::LocalFilePolicyStore;
use foundation_cedar::entity::JwtEntityProvider;

// Load policies from local files
let store = LocalFilePolicyStore::new("/etc/myapp/policies");
let engine = foundation_cedar::load_from_store(&store)?;

// Evaluate a request
let request = CedarRequest::builder()
    .principal(EntityUid::from_str(r#"User::"alice""#)?)
    .action(EntityUid::from_str(r#"Action::"view""#)?)
    .resource(EntityUid::from_str(r#"Document::"report-2025""#)?)
    .context_json(json!({"department": "engineering"}))
    .build()?;

let response = engine.is_authorized(&request);
assert!(response.is_allowed());
```

---

## Fundamentals Directory

### `fundamentals/cedar_basics.md`

Cedar language primer covering:
- Policy syntax (`permit`, `forbid`, `when`, `unless`)
- Entity types and UIDs (`User::"alice"`, `Action::"view"`)
- Schemas (entity declarations, action declarations, context types)
- Templates (`?principal`, `?resource` slot variables)
- Operators (`in`, `==`, `has`, `like`, `is`)
- Extensions (`ip()`, `decimal()`, `datetime()`)
- Policy vs Template vs Link terminology
- JSON representation of policies (EST format)

### `fundamentals/cedar_patterns.md`

Common authorization patterns implemented in Cedar:
- **RBAC** — Role-based: `permit(principal in Group::"admins", ...)`
- **ABAC** — Attribute-based: `permit(...) when { principal.department == resource.owner }`
- **ReBAC** — Relationship-based: `permit(principal in Group::"team-1", action in Action::"read", resource in Team::"team-1")`
- **Multi-tenant** — Using context to scope: `permit(...) when { context.org == resource.org }`
- **Deny overrides** — `forbid` takes precedence over `permit`
- **Scoped actions** — Action groups: `action "read" in [ReadActions]`
- **Validation** — Using `Validator` to catch policy errors at load time

### `fundamentals/cedar_wasm.md`

Running Cedar in WASM environments:
- Cedar policy crate compiles to wasm32 (the `wasm` feature enables `tsify` + `wasm-bindgen`)
- Cloudflare Workers specifics: `SingleExecutorSingleton` for valtron pool, `.without_time()` for tracing
- `js-wasmbindgen` feature gate for CondVar compatibility
- Entity construction in WASM (no file I/O → use `Entities::from_json_str`)
- Policy loading in WASM (R2 via wasm-bindgen, D1 via wasm-bindgen, raw HTTP fetch)

### `fundamentals/cedar_partial_eval.md`

Partial evaluation guide:
- What is partial evaluation — evaluating with unknown entity data
- When to use — multi-step authorization where some entity lookups are expensive
- `is_authorized_partial` returns `PartialResponse` with residuals
- Concretizing partial responses when entity data becomes available
- Performance trade-offs

### `fundamentals/cedar_integration_guide.md`

How to integrate foundation_cedar into other crates:
- With `foundation_auth` — gating IdP server endpoints
- With `foundation_http` — tower middleware for HTTP routes
- With `connectrpc` — gRPC authorization middleware (see existing `connectrpc-cedar`)
- Standalone usage — minimal setup for any Rust application
- Migration guide — from ad-hoc authorization to Cedar policies

---

## D1 Schema for Policy Storage

When using the D1/Turso SQLite backend for policy storage:

```sql
-- Migration for cedar_policies table
CREATE TABLE IF NOT EXISTS cedar_policies (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    policy_text TEXT NOT NULL,
    policy_type TEXT NOT NULL DEFAULT 'policy',  -- 'policy', 'template', 'schema', 'entity'
    tags TEXT,  -- JSON array of tags for filtering
    updated_at INTEGER NOT NULL DEFAULT (strftime('%s', 'now') * 1000),
    created_at INTEGER NOT NULL DEFAULT (strftime('%s', 'now') * 1000)
);

CREATE INDEX IF NOT EXISTS idx_cedar_policies_type ON cedar_policies(policy_type);
CREATE INDEX IF NOT EXISTS idx_cedar_policies_updated ON cedar_policies(updated_at);
```

---

## Dependencies

### Core (always)
- `cedar-policy = "4.11"` — Cedar policy engine
- `serde`, `serde_json` — serialization
- `thiserror` — error handling
- `async-trait` — async trait support
- `foundation_core` (valtron) — async execution engine (native + wasm)

### Native backends
- `gix` (optional) — pure Rust git implementation (native clone + fetch)
- `gix-odb`, `gix-pack`, `gix-protocol` (optional, wasm) — lightweight git for WASM
- `foundation_db` (optional) — for R2/D1 storage backends
- `foundation_http` (optional) — for HTTP middleware

### Wasm backends
- `web-sys` (optional) — browser fetch API for git transport
- `foundation_db` with `wasm-bindgen-storage` — D1/R2 via wasm-bindgen

---

## Testing

- **Engine tests**: Parse policies, evaluate requests, verify Allow/Deny decisions
- **PolicyStore tests**: Each backend loads policies correctly, version detection works
- **EntityProvider tests**: JWT provider extracts claims, DB provider queries correctly
- **Integration tests**: Full pipeline — store → resolver → engine → decision
- **WASM tests**: Policy loading via wasm-bindgen backends (entity parsing, no HTTP)
- **Middleware tests**: Tower layer blocks unauthorized requests, passes authorized ones
- **Hot-reload tests**: Engine swaps when policy version changes

---

## Success Criteria

- [ ] `CedarEngine` evaluates `permit`/`forbid` policies correctly
- [ ] `CedarEngine` validates policies against schema at construction
- [ ] `LocalFilePolicyStore` reads `.cedar` files from directory
- [ ] `R2PolicyStore` fetches policies from Cloudflare R2 (native + wasm)
- [ ] `D1PolicyStore` reads policies from SQLite (native + wasm)
- [ ] `GitPolicyStore` clones/fetches and reads policies from git repo (native via gix)
- [ ] `GitPolicyStoreWasm` fetches policies via HTTP or in-memory gix (wasm)
- [ ] `JwtEntityProvider` builds entities from JWT claims
- [ ] `StaticEntityProvider` returns fixed entities
- [ ] `PolicyWatcher` detects version changes and rebuilds engine
- [ ] `CedarLayer` middleware gates HTTP requests
- [ ] `fundamentals/` directory documents Cedar language and patterns
- [ ] Works in both native and wasm32 targets
- [ ] Partial evaluation available behind feature flag
