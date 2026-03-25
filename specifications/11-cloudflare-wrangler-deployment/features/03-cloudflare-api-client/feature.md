---
workspace_name: "ewe_platform"
spec_directory: "specifications/11-cloudflare-wrangler-deployment"
feature_directory: "specifications/11-cloudflare-wrangler-deployment/features/03-cloudflare-api-client"
this_file: "specifications/11-cloudflare-wrangler-deployment/features/03-cloudflare-api-client/feature.md"

status: pending
priority: high
created: 2026-03-26

depends_on: ["01-foundation-deployment-crate"]

tasks:
  completed: 0
  uncompleted: 8
  total: 8
  completion_percentage: 0%
---


# Cloudflare API Client

## Overview

Build a comprehensive Cloudflare REST API client using `foundation_core::simple_http` and valtron async patterns, enabling programmatic management of Workers, KV, D1, R2, and secrets without relying on wrangler CLI.

## Dependencies

This feature depends on:
- `01-foundation-deployment-crate` - Uses `DeploymentError` and foundational types

This feature is required by:
- `deploy-planner` - Uses API client for direct deployments
- `08-examples-documentation` - Examples may use API directly

## Requirements

### Authentication

```rust
// cloudflare/auth.rs

/// Cloudflare API authentication
#[derive(Debug, Clone)]
pub struct CloudflareAuth {
    api_token: String,  // Consider zeroize for production
    account_id: String,
}

impl CloudflareAuth {
    /// Create from API token and account ID
    pub fn new(api_token: &str, account_id: &str) -> Self;

    /// Create from environment variables
    /// Reads CLOUDFLARE_API_TOKEN and CLOUDFLARE_ACCOUNT_ID
    pub fn from_env() -> Result<Self, DeploymentError>;

    /// Get authorization header
    pub fn auth_header(&self) -> String;  // "Bearer {token}"

    /// Get account ID
    pub fn account_id(&self) -> &str;
}
```

### API Client

```rust
// cloudflare/client.rs

/// Cloudflare API client
pub struct CloudflareClient {
    http_client: SimpleHttpClient,
    auth: CloudflareAuth,
    base_url: String,  // https://api.cloudflare.com/client/v4
}

impl CloudflareClient {
    /// Create new client
    pub fn new(auth: CloudflareAuth) -> Self;

    /// Create with custom base URL (for testing)
    pub fn with_base_url(auth: CloudflareAuth, base_url: &str) -> Self;

    /// Build API URL
    fn api_url(&self, path: &str) -> String;

    /// Make authenticated request
    async fn request<T>(
        &self,
        method: &str,
        path: &str,
        body: Option<&impl Serialize>,
    ) -> Result<T, DeploymentError>
    where
        T: DeserializeOwned;
}
```

### Workers API

```rust
// cloudflare/workers.rs

/// Worker script information
#[derive(Debug, Deserialize)]
pub struct WorkerScript {
    pub id: String,
    pub tag: String,
    pub tag_modified: String,
    pub created_on: String,
    pub modified_on: String,
    pub usage_model: UsageModel,
    pub environments: Vec<WorkerEnvironment>,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum UsageModel {
    Bundled,
    Unbound,
}

#[derive(Debug, Deserialize)]
pub struct WorkerEnvironment {
    pub environment: String,
    pub route: Option<String>,
    pub routes: Option<Vec<String>>,
}

/// Worker upload metadata
#[derive(Debug, Serialize)]
pub struct WorkerUploadMetadata {
    pub main_module: String,
    pub bindings: Vec<Binding>,
    pub compatibility_date: Option<String>,
    pub compatibility_flags: Option<Vec<String>>,
    pub usage_model: Option<UsageModel>,
}

#[derive(Debug, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Binding {
    PlainText { name: String, text: String },
    SecretText { name: String, text: String },
    KvNamespace { name: String, namespace_id: String },
    D1Database { name: String, id: String },
    R2Bucket { name: String, bucket_name: String },
}

impl WorkersApi for CloudflareClient {
    /// Upload worker script
    async fn upload_worker(
        &self,
        name: &str,
        script: &[u8],
        metadata: WorkerUploadMetadata,
    ) -> Result<WorkerScript, DeploymentError>;

    /// Get worker script
    async fn get_worker(&self, name: &str) -> Result<WorkerScript, DeploymentError>;

    /// Delete worker
    async fn delete_worker(&self, name: &str) -> Result<(), DeploymentError>;

    /// List all workers
    async fn list_workers(&self) -> Result<Vec<WorkerScript>, DeploymentError>;

    /// Deploy to workers.dev subdomain
    async fn enable_workers_dev(&self, name: &str) -> Result<(), DeploymentError>;

    /// Add route to worker
    async fn add_route(&self, name: &str, zone_id: &str, pattern: &str) -> Result<(), DeploymentError>;
}
```

### Secrets API

```rust
// cloudflare/secrets.rs

/// Secret information
#[derive(Debug, Deserialize)]
pub struct SecretInfo {
    pub name: String,
    pub r#type: String,  // "secret_text"
}

impl SecretsApi for CloudflareClient {
    /// Create or update secret
    async fn put_secret(&self, worker: &str, name: &str, value: &str) -> Result<(), DeploymentError>;

    /// List secrets
    async fn list_secrets(&self, worker: &str) -> Result<Vec<SecretInfo>, DeploymentError>;

    /// Delete secret
    async fn delete_secret(&self, worker: &str, name: &str) -> Result<(), DeploymentError>;
}
```

### KV API

```rust
// cloudflare/kv.rs

/// KV namespace information
#[derive(Debug, Deserialize)]
pub struct KvNamespace {
    pub id: String,
    pub title: String,
    pub supports_url_encoding: bool,
}

/// KV metadata with list info
#[derive(Debug, Deserialize)]
pub struct KvListResult {
    pub keys: Vec<KvKey>,
    pub list_complete: bool,
    pub cursor: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct KvKey {
    pub name: String,
    pub expiration: Option<u64>,
    pub metadata: Option<serde_json::Value>,
}

impl KvApi for CloudflareClient {
    /// List KV namespaces
    async fn list_namespaces(&self) -> Result<Vec<KvNamespace>, DeploymentError>;

    /// Create KV namespace
    async fn create_namespace(&self, title: &str) -> Result<KvNamespace, DeploymentError>;

    /// Put KV value
    async fn put_value(
        &self,
        namespace_id: &str,
        key: &str,
        value: &[u8],
        expiration_ttl: Option<u64>,
        metadata: Option<serde_json::Value>,
    ) -> Result<(), DeploymentError>;

    /// Get KV value
    async fn get_value(&self, namespace_id: &str, key: &str) -> Result<Option<Vec<u8>>, DeploymentError>;

    /// Delete KV value
    async fn delete_value(&self, namespace_id: &str, key: &str) -> Result<(), DeploymentError>;

    /// List keys in namespace
    async fn list_keys(&self, namespace_id: &str, prefix: Option<&str>) -> Result<KvListResult, DeploymentError>;
}
```

### D1 API

```rust
// cloudflare/d1.rs

/// D1 database information
#[derive(Debug, Deserialize)]
pub struct D1Database {
    pub uuid: String,
    pub name: String,
    pub version: String,
    pub created_at: String,
}

/// D1 query result
#[derive(Debug, Deserialize)]
pub struct D1QueryResult {
    pub success: bool,
    pub meta: D1Meta,
    pub results: Vec<serde_json::Value>,
}

#[derive(Debug, Deserialize)]
pub struct D1Meta {
    pub changed_db: bool,
    pub size_after: u64,
    pub duration: f64,
}

impl D1Api for CloudflareClient {
    /// Create D1 database
    async fn create_database(&self, name: &str) -> Result<D1Database, DeploymentError>;

    /// List D1 databases
    async fn list_databases(&self) -> Result<Vec<D1Database>, DeploymentError>;

    /// Delete D1 database
    async fn delete_database(&self, database_id: &str) -> Result<(), DeploymentError>;

    /// Execute SQL query
    async fn execute_query(&self, database_id: &str, sql: &str) -> Result<D1QueryResult, DeploymentError>;
}
```

### R2 API

```rust
// cloudflare/r2.rs

/// R2 bucket information
#[derive(Debug, Deserialize)]
pub struct R2Bucket {
    pub name: String,
    pub creation_date: String,
}

impl R2Api for CloudflareClient {
    /// Create R2 bucket
    async fn create_bucket(&self, name: &str) -> Result<R2Bucket, DeploymentError>;

    /// List R2 buckets
    async fn list_buckets(&self) -> Result<Vec<R2Bucket>, DeploymentError>;

    /// Delete R2 bucket
    async fn delete_bucket(&self, name: &str) -> Result<(), DeploymentError>;
}
```

### Account API

```rust
// cloudflare/accounts.rs

/// Account information
#[derive(Debug, Deserialize)]
pub struct AccountInfo {
    pub id: String,
    pub name: String,
    pub r#type: String,
}

impl AccountsApi for CloudflareClient {
    /// Get account details
    async fn get_account(&self, account_id: &str) -> Result<AccountInfo, DeploymentError>;

    /// List accounts (for users with multiple)
    async fn list_accounts(&self) -> Result<Vec<AccountInfo>, DeploymentError>;
}
```

## Tasks

1. **Create cloudflare module structure**
   - [ ] Create `src/cloudflare/mod.rs`
   - [ ] Create `src/cloudflare/auth.rs`
   - [ ] Create `src/cloudflare/client.rs`
   - [ ] Export from `src/lib.rs`

2. **Implement authentication**
   - [ ] Define `CloudflareAuth` struct
   - [ ] Implement `from_env()` method
   - [ ] Implement auth header generation
   - [ ] Write unit tests for auth creation

3. **Implement base client**
   - [ ] Define `CloudflareClient` struct
   - [ ] Implement HTTP request methods using `SimpleHttpClient`
   - [ ] Add error handling and response parsing
   - [ ] Write unit tests with mock server

4. **Implement Workers API**
   - [ ] Create `src/cloudflare/workers.rs`
   - [ ] Define all Worker types
   - [ ] Implement upload, get, delete, list methods
   - [ ] Write integration tests (requires API token)

5. **Implement Secrets API**
   - [ ] Create `src/cloudflare/secrets.rs`
   - [ ] Define `SecretInfo` type
   - [ ] Implement put, list, delete methods
   - [ ] Write integration tests

6. **Implement KV API**
   - [ ] Create `src/cloudflare/kv.rs`
   - [ ] Define KV types
   - [ ] Implement namespace and key operations
   - [ ] Write integration tests

7. **Implement D1 and R2 APIs**
   - [ ] Create `src/cloudflare/d1.rs`
   - [ ] Create `src/cloudflare/r2.rs`
   - [ ] Define D1 and R2 types
   - [ ] Implement CRUD operations
   - [ ] Write integration tests

8. **Write comprehensive tests**
   - [ ] Unit tests for type serialization
   - [ ] Integration tests with mock API
   - [ ] Live API tests (marked with `#[ignore]`)

## Implementation Notes

- Use `foundation_core::simple_http::client::SimpleHttpClient` for HTTP requests
- Follow valtron async patterns for any long-running operations
- Cloudflare API returns wrapped responses: `{ success: bool, result: T, errors: [] }`
- Rate limiting: Cloudflare allows ~1200 requests per 5 minutes
- Use `serde` for JSON serialization/deserialization

## Cloudflare API Response Format

```json
{
  "success": true,
  "errors": [],
  "messages": [],
  "result": { ... }
}
```

## Success Criteria

- [ ] All 8 tasks completed
- [ ] `cargo clippy -- -D warnings` passes
- [ ] All unit tests pass
- [ ] Can upload worker via API
- [ ] Can manage secrets via API
- [ ] Can manage KV namespaces and keys via API

## Verification

```bash
# Build and check
cd backends/foundation_deployment
cargo check
cargo clippy -- -D warnings

# Run unit tests
cargo test cloudflare

# Run integration tests (requires API token)
export CLOUDFLARE_API_TOKEN=your_token
export CLOUDFLARE_ACCOUNT_ID=your_account
cargo test cloudflare_api -- --ignored --nocapture
```

---

_Created: 2026-03-26_
