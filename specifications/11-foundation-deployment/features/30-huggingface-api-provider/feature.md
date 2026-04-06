---
workspace_name: "ewe_platform"
spec_directory: "specifications/11-foundation-deployment"
feature_directory: "specifications/11-foundation-deployment/features/30-huggingface-api-provider"
this_file: "specifications/11-foundation-deployment/features/30-huggingface-api-provider/feature.md"

status: pending
priority: high
created: 2026-04-06

depends_on: ["01-foundation-deployment-core", "26-gen-provider-clients", "27-provider-api-feature-flags"]

tasks:
  completed: 0
  uncompleted: 8
  total: 8
  completion_percentage: 0%
---


# Hugging Face Hub API Provider

## Iron Law: Zero Warnings

> **All code must compile with zero warnings and pass all lints. No suppression. No exceptions.**
>
> - `cargo clippy -p foundation_deployment -- -D warnings -W clippy::pedantic` — zero warnings
> - `cargo doc -p foundation_deployment --no-deps` — zero rustdoc warnings
> - `cargo test -p foundation_deployment` — zero compilation warnings
> - **No `#[allow(...)]`, `#[expect(...)]`, or `#![allow(...)]` anywhere.** Fix the code, never suppress.

## Overview

Implement a **Hugging Face Hub API provider** that reimplements the functionality of the `huggingface_hub_rust` crate using **only `valtron` and `simple_http`** — no `tokio`, no `reqwest`, no async unless absolutely unavoidable.

This provider enables:
- **Repository management** — create, delete, move, and update repositories (models, datasets, spaces)
- **File operations** — list, download, upload, and delete files in repositories
- **Commit operations** — create commits with multiple file operations
- **User/Org operations** — get user info, organization details, followers, members
- **Listing operations** — list models, datasets, spaces with filtering and pagination

### Key Constraints

| Constraint | Requirement |
|------------|-------------|
| HTTP Client | `foundation_core::simple_http::client::SimpleHttpClient` only |
| Runtime | `valtron` thread pool for blocking operations |
| Async | Avoid unless absolutely required (no tokio) |
| Dependencies | NO `reqwest`, NO `tokio`, NO `reqwest-middleware` |

### Reference Implementation

The existing `huggingface_hub_rust` crate (`/home/darkvoid/Boxxed/@dev/sources/huggingface_hub_rust`) provides the reference API design. This implementation replicates its functionality using our stack.

## Architecture

All provider implementations live in `backends/foundation_deployment/src/providers/huggingface/`:

```
backends/foundation_deployment/src/providers/huggingface/
├── mod.rs             # Module declaration
├── provider.rs        # DeploymentProvider trait implementation (future)
├── fetch.rs           # OpenAPI spec fetcher (if available)
├── client.rs          # Hugging Face API client (HFClient equivalent)
├── repository.rs      # Repository handle (HFRepository equivalent)
├── types.rs           # Type definitions (RepoType, ModelInfo, etc.)
├── error.rs           # Error types (HFError equivalent)
├── constants.rs       # Constants (endpoints, env vars, cache paths)
└── resources/
    └── mod.rs         # Auto-generated resource types from OpenAPI
```

Fetched raw specs (if OpenAPI becomes available):
```
artefacts/cloud_providers/huggingface/
├── openapi.json       # Consolidated spec
└── _manifest.json     # Fetch metadata
```

## Hugging Face API Endpoints

The Hugging Face Hub API does **not** have an official OpenAPI/Swagger specification. All endpoints are documented via the Python `huggingface_hub` library and the `huggingface_hub_rust` reference implementation.

### Base URL

```
https://huggingface.co/api
```

Custom endpoint can be set via `HF_ENDPOINT` environment variable.

### Repository Types

| Repo Type | URL Prefix | API Segment |
|-----------|------------|-------------|
| Model (default) | (none) | `models` |
| Dataset | `datasets/` | `datasets` |
| Space | `spaces/` | `spaces` |
| Kernel | `kernels/` | `kernels` |

### Authentication

| Method | Details |
|--------|---------|
| Token Location | `HF_TOKEN` env var, `HF_TOKEN_PATH` file, or `$HF_HOME/token` |
| Token Format | Bearer token in `Authorization` header |
| Implicit Token | Enabled by default; disable via `HF_HUB_DISABLE_IMPLICIT_TOKEN` |

### Core Endpoints

#### User Authentication

| Method | Endpoint | Description |
|--------|----------|-------------|
| GET | `/api/whoami-v2` | Get authenticated user info |
| GET | `/api/users/{username}/overview` | Get public user overview |
| GET | `/api/organizations/{org}/overview` | Get organization overview |
| GET | `/api/users/{username}/followers` | List user followers (paginated) |
| GET | `/api/users/{username}/following` | List users being followed |
| GET | `/api/organizations/{org}/members` | List organization members |

#### Repository CRUD

| Method | Endpoint | Description |
|--------|----------|-------------|
| GET | `/api/models/{repo_id}` | Get model info |
| GET | `/api/datasets/{repo_id}` | Get dataset info |
| GET | `/api/spaces/{repo_id}` | Get space info |
| GET | `/api/models/{repo_id}/revision/{revision}` | Get model at specific revision |
| POST | `/api/repos/create` | Create a new repository |
| DELETE | `/api/repos/delete` | Delete a repository |
| POST | `/api/repos/move` | Move/rename a repository |
| PUT | `/api/models/{repo_id}/settings` | Update repository settings |

#### Repository Existence Checks

| Method | Endpoint | Description |
|--------|----------|-------------|
| HEAD | `/api/models/{repo_id}` | Check if model exists |
| HEAD | `/api/models/{repo_id}/revision/{revision}` | Check if revision exists |
| HEAD | `/{prefix}{repo_id}/resolve/{revision}/{filename}` | Check if file exists |

#### Listing Operations (Paginated)

| Method | Endpoint | Description |
|--------|----------|-------------|
| GET | `/api/models` | List models with filters |
| GET | `/api/datasets` | List datasets with filters |
| GET | `/api/spaces` | List spaces with filters |

Query parameters: `search`, `author`, `filter`, `sort`, `pipeline_tag`, `full`, `cardData`, `config`, `limit`

#### File Operations

| Method | Endpoint | Description |
|--------|----------|-------------|
| GET | `/api/{type}s/{repo_id}/tree/{revision}` | List tree entries (paginated) |
| POST | `/api/{type}s/{repo_id}/paths-info/{revision}` | Get info for specific paths |
| GET | `/{prefix}{repo_id}/resolve/{revision}/{filename}` | Download file |
| POST | `/api/{type}s/{repo_id}/commit/{revision}` | Create commit with operations |

#### Commit Operations

| Operation | Description |
|-----------|-------------|
| `Add` (file) | Upload a file from path or bytes |
| `Delete` (file) | Delete a file |
| `Delete` (folder) | Delete a directory |

### Response Headers

| Header | Description |
|--------|-------------|
| `x-repo-commit` | Commit SHA of the repository |
| `x-linked-etag` | ETag for the resource |
| `x-xet-hash` | Xet hash for Xet-enabled files |

## Data Types

### Repository Info Types

```rust
/// Type of repository on the Hub
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum RepoType {
    Model,
    Dataset,
    Space,
    Kernel,
}

/// Model information returned by GET /api/models/{id}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelInfo {
    pub id: String,
    pub model_id: Option<String>,
    pub author: Option<String>,
    pub sha: Option<String>,
    pub private: Option<bool>,
    pub gated: Option<serde_json::Value>,
    pub disabled: Option<bool>,
    pub downloads: Option<u64>,
    pub likes: Option<u64>,
    pub tags: Option<Vec<String>>,
    pub pipeline_tag: Option<String>,
    pub library_name: Option<String>,
    pub created_at: Option<String>,
    pub last_modified: Option<String>,
    pub siblings: Option<Vec<RepoSibling>>,
    pub card_data: Option<serde_json::Value>,
    // ... more fields
}

/// Dataset information
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DatasetInfo { /* similar structure */ }

/// Space information
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SpaceInfo {
    pub id: String,
    pub sdk: Option<String>,
    pub host: Option<String>,
    pub subdomain: Option<String>,
    pub runtime: Option<serde_json::Value>,
    // ... more fields
}
```

### User Types

```rust
#[derive(Debug, Clone, Deserialize)]
pub struct User {
    pub username: String,
    pub fullname: Option<String>,
    pub avatar_url: Option<String>,
    pub user_type: Option<String>,
    pub is_pro: Option<bool>,
    pub email: Option<String>,
    pub orgs: Option<Vec<OrgMembership>>,
}
```

### File/Tree Types

```rust
#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum RepoTreeEntry {
    File {
        oid: String,
        size: u64,
        path: String,
        lfs: Option<BlobLfsInfo>,
        last_commit: Option<LastCommitInfo>,
    },
    Directory {
        oid: String,
        path: String,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RepoSibling {
    pub rfilename: String,
    pub size: Option<u64>,
    pub lfs: Option<BlobLfsInfo>,
}
```

## Error Handling

```rust
// backends/foundation_deployment/src/providers/huggingface/error.rs

use thiserror::Error;

#[derive(Error, Debug)]
pub enum HuggingFaceError {
    #[error("HTTP error: {status} {url}")]
    Http {
        status: u16,
        url: String,
        body: String,
    },

    #[error("Authentication required")]
    AuthRequired,

    #[error("Repository not found: {repo_id}")]
    RepoNotFound { repo_id: String },

    #[error("Revision not found: {revision} in {repo_id}")]
    RevisionNotFound { repo_id: String, revision: String },

    #[error("File not found: {path} in {repo_id}")]
    FileNotFound { path: String, repo_id: String },

    #[error("Invalid repository type: expected {expected}, got {actual}")]
    InvalidRepoType {
        expected: RepoType,
        actual: RepoType,
    },

    #[error("Invalid parameter: {0}")]
    InvalidParameter(String),

    #[error(transparent)]
    Io(#[from] std::io::Error),

    #[error(transparent)]
    Json(#[from] serde_json::Error),

    #[error(transparent)]
    HttpParse(#[from] http::Error),

    #[error("{0}")]
    Other(String),
}

pub type Result<T> = std::result::Result<T, HuggingFaceError>;
```

### Error Mapping

| HTTP Status | Error Variant |
|-------------|---------------|
| 401 | `AuthRequired` |
| 404 (repo endpoint) | `RepoNotFound` |
| 404 (revision endpoint) | `RevisionNotFound` |
| 404 (file endpoint) | `FileNotFound` |
| 404 (generic) | `Http` |
| 5xx | `Http` (transient) |

## Requirements

### Module Structure

```rust
// backends/foundation_deployment/src/providers/huggingface/mod.rs

pub mod client;
pub mod constants;
pub mod error;
pub mod repository;
pub mod types;

// Future additions
// pub mod fetch;      // If OpenAPI spec becomes available
// pub mod resources;  // Auto-generated types
// pub mod provider;   // DeploymentProvider implementation
```

### Client Implementation

```rust
// backends/foundation_deployment/src/providers/huggingface/client.rs

use foundation_core::simple_http::client::SimpleHttpClient;
use std::sync::Arc;

/// Hugging Face Hub API client.
///
/// Cheap to clone — uses Arc internally.
#[derive(Clone)]
pub struct HFClient {
    inner: Arc<HFClientInner>,
}

struct HFClientInner {
    client: SimpleHttpClient,
    endpoint: String,
    token: Option<String>,
    cache_dir: PathBuf,
    cache_enabled: bool,
}

/// Builder for HFClient
pub struct HFClientBuilder {
    endpoint: Option<String>,
    token: Option<String>,
    cache_dir: Option<PathBuf>,
    cache_enabled: Option<bool>,
}

impl HFClientBuilder {
    pub fn new() -> Self;
    pub fn endpoint(self, endpoint: impl Into<String>) -> Self;
    pub fn token(self, token: impl Into<String>) -> Self;
    pub fn cache_dir(self, path: impl Into<PathBuf>) -> Self;
    pub fn cache_enabled(self, enabled: bool) -> Self;
    pub fn build(self) -> Result<HFClient>;
}

impl HFClient {
    /// Create with defaults (reads HF_TOKEN, HF_ENDPOINT from env)
    pub fn new() -> Result<Self>;
    
    /// Get builder for fine-grained configuration
    pub fn builder() -> HFClientBuilder;
    
    /// Get authenticated user info
    pub async fn whoami(&self) -> Result<User>;
    
    /// Check if token is valid
    pub async fn auth_check(&self) -> Result<()>;
    
    /// List models with pagination
    pub fn list_models(&self, params: &ListModelsParams) -> Result<impl Iterator<Item = Result<ModelInfo>>>;
    
    /// List datasets with pagination
    pub fn list_datasets(&self, params: &ListDatasetsParams) -> Result<impl Iterator<Item = Result<DatasetInfo>>>;
    
    /// List spaces with pagination
    pub fn list_spaces(&self, params: &ListSpacesParams) -> Result<impl Iterator<Item = Result<SpaceInfo>>>;
    
    /// Create a repository
    pub async fn create_repo(&self, params: &CreateRepoParams) -> Result<RepoUrl>;
    
    /// Delete a repository
    pub async fn delete_repo(&self, params: &DeleteRepoParams) -> Result<()>;
    
    /// Move/rename a repository
    pub async fn move_repo(&self, params: &MoveRepoParams) -> Result<RepoUrl>;
    
    /// Get repository handle for model operations
    pub fn model(&self, owner: impl Into<String>, name: impl Into<String>) -> HFRepository;
    
    /// Get repository handle for dataset operations
    pub fn dataset(&self, owner: impl Into<String>, name: impl Into<String>) -> HFRepository;
    
    /// Get repository handle for space operations
    pub fn space(&self, owner: impl Into<String>, name: impl Into<String>) -> HFRepository;
    
    // Internal helpers
    pub(crate) fn auth_headers(&self) -> HeaderMap;
    pub(crate) fn api_url(&self, repo_type: Option<RepoType>, repo_id: &str) -> String;
    pub(crate) fn download_url(&self, repo_type: Option<RepoType>, repo_id: &str, revision: &str, filename: &str) -> String;
}
```

### Repository Handle

```rust
// backends/foundation_deployment/src/providers/huggingface/repository.rs

/// Handle for a single repository.
#[derive(Clone)]
pub struct HFRepository {
    client: HFClient,
    owner: String,
    name: String,
    repo_type: RepoType,
    default_revision: Option<String>,
}

impl HFRepository {
    /// Get repository info
    pub async fn info(&self, params: &RepoInfoParams) -> Result<RepoInfo>;
    
    /// Check if repository exists
    pub async fn exists(&self) -> Result<bool>;
    
    /// Check if revision exists
    pub async fn revision_exists(&self, params: &RepoRevisionExistsParams) -> Result<bool>;
    
    /// Check if file exists
    pub async fn file_exists(&self, params: &RepoFileExistsParams) -> Result<bool>;
    
    /// List files in repository
    pub async fn list_files(&self, params: &RepoListFilesParams) -> Result<Vec<String>>;
    
    /// List tree entries
    pub fn list_tree(&self, params: &RepoListTreeParams) -> Result<impl Iterator<Item = Result<RepoTreeEntry>>>;
    
    /// Get info for specific paths
    pub async fn get_paths_info(&self, params: &RepoGetPathsInfoParams) -> Result<Vec<RepoTreeEntry>>;
    
    /// Download a file
    pub async fn download_file(&self, params: &RepoDownloadFileParams) -> Result<PathBuf>;
    
    /// Upload a file
    pub async fn upload_file(&self, params: &RepoUploadFileParams) -> Result<CommitInfo>;
    
    /// Delete a file
    pub async fn delete_file(&self, params: &RepoDeleteFileParams) -> Result<CommitInfo>;
    
    /// Create a commit with multiple operations
    pub async fn create_commit(&self, params: &RepoCreateCommitParams) -> Result<CommitInfo>;
    
    /// Update repository settings
    pub async fn update_settings(&self, params: &RepoUpdateSettingsParams) -> Result<()>;
    
    // Helpers
    pub fn repo_path(&self) -> String;  // "{owner}/{name}"
    pub fn repo_type(&self) -> RepoType;
}
```

### Parameter Types

```rust
// backends/foundation_deployment/src/providers/huggingface/types.rs

#[derive(Default)]
pub struct ListModelsParams {
    pub search: Option<String>,
    pub author: Option<String>,
    pub filter: Option<String>,
    pub sort: Option<String>,
    pub pipeline_tag: Option<String>,
    pub full: Option<bool>,
    pub card_data: Option<bool>,
    pub fetch_config: Option<bool>,
    pub limit: Option<usize>,
}

#[derive(Default)]
pub struct ListDatasetsParams {
    pub search: Option<String>,
    pub author: Option<String>,
    pub filter: Option<String>,
    pub sort: Option<String>,
    pub full: Option<bool>,
    pub limit: Option<usize>,
}

#[derive(Default)]
pub struct ListSpacesParams {
    pub search: Option<String>,
    pub author: Option<String>,
    pub filter: Option<String>,
    pub sort: Option<String>,
    pub full: Option<bool>,
    pub limit: Option<usize>,
}

#[derive(Default)]
pub struct RepoInfoParams {
    pub revision: Option<String>,
}

#[derive(Default)]
pub struct CreateRepoParams {
    pub repo_id: String,
    pub repo_type: Option<RepoType>,
    pub private: Option<bool>,
    pub exist_ok: bool,
    pub space_sdk: Option<String>,
}

#[derive(Default)]
pub struct DeleteRepoParams {
    pub repo_id: String,
    pub repo_type: Option<RepoType>,
    pub missing_ok: bool,
}

#[derive(Default)]
pub struct MoveRepoParams {
    pub from_id: String,
    pub to_id: String,
    pub repo_type: Option<RepoType>,
}

#[derive(Default)]
pub struct RepoUploadFileParams {
    pub source: AddSource,
    pub path_in_repo: String,
    pub revision: Option<String>,
    pub commit_message: Option<String>,
}

#[derive(Default)]
pub struct RepoCreateCommitParams {
    pub operations: Vec<CommitOperation>,
    pub commit_message: String,
    pub commit_description: Option<String>,
    pub revision: Option<String>,
    pub create_pr: Option<bool>,
}

#[derive(Clone)]
pub enum AddSource {
    File(PathBuf),
    Bytes(Vec<u8>),
}

#[derive(Clone)]
pub enum CommitOperation {
    Add { path_in_repo: String, source: AddSource },
    Delete { path_in_repo: String },
}
```

## Implementation Notes

### No Async Runtime

Since we cannot use `tokio`, all "async" operations will be implemented using:
1. **Blocking HTTP calls** via `SimpleHttpClient` (which is synchronous)
2. **Valtron thread pool** for any blocking I/O (file operations, etc.)
3. **Iterators instead of Streams** for pagination (no async streams)

### Pagination Without Streams

The reference implementation uses `futures::Stream` for pagination. We'll use synchronous iterators:

```rust
pub fn list_models(&self, params: &ListModelsParams) -> Result<impl Iterator<Item = Result<ModelInfo>>> {
    // Use valtron's StreamIterator adapted for sync iteration
    // Or implement a simple paging iterator
}
```

### File Uploads

File uploads require `multipart/form-data` encoding. The `simple_http` crate should support this, but if not:

```rust
// Manual multipart construction for commit operations
fn build_multipart_body(operations: &[CommitOperation]) -> Result<(Vec<u8>, String)> {
    // Construct multipart body manually
    // See: https://huggingface.co/docs/hub/commit-api
}
```

### Caching

The reference implementation has a file cache. For Phase 1, we'll skip caching:
- **Phase 1**: No caching, direct API calls only
- **Phase 2**: Add optional file caching (etag-based, symlink snapshots)

## Tasks

### 1. Core Module Structure (pending)

- [ ] Create `backends/foundation_deployment/src/providers/huggingface/mod.rs`
- [ ] Create `constants.rs` with endpoint URLs, env var names, defaults
- [ ] Create `error.rs` with `HuggingFaceError` types
- [ ] Create `types.rs` with all data types (RepoType, ModelInfo, etc.)
- [ ] Add `huggingface` feature flag to `Cargo.toml`

### 2. HTTP Client (pending)

- [ ] Create `client.rs` with `HFClient` and `HFClientBuilder`
- [ ] Implement token resolution (env → file → cache)
- [ ] Implement `auth_headers()` helper
- [ ] Implement URL builders (`api_url()`, `download_url()`)
- [ ] Implement `whoami()` and `auth_check()`

### 3. Repository Handle (pending)

- [ ] Create `repository.rs` with `HFRepository` struct
- [ ] Implement `info()` for model/dataset/space
- [ ] Implement `exists()`, `revision_exists()`, `file_exists()`
- [ ] Implement `update_settings()`
- [ ] Implement `list_files()`, `list_tree()`, `get_paths_info()`

### 4. Listing Operations (pending)

- [ ] Implement `list_models()` with pagination
- [ ] Implement `list_datasets()` with pagination
- [ ] Implement `list_spaces()` with pagination
- [ ] Implement user/org endpoints (`get_user_overview()`, etc.)

### 5. Repository CRUD (pending)

- [ ] Implement `create_repo()`
- [ ] Implement `delete_repo()`
- [ ] Implement `move_repo()`

### 6. File Download (pending)

- [ ] Implement `download_file()` (direct download, no cache for Phase 1)
- [ ] Handle LFS files (detect `lfs` field in tree entries)

### 7. File Upload & Commits (pending)

- [ ] Implement `upload_file()` via `create_commit()`
- [ ] Implement `create_commit()` with multipart form data
- [ ] Implement `delete_file()`, `delete_folder()`

### 8. Tests & Documentation (pending)

- [ ] Write unit tests for type serialization
- [ ] Write integration tests (require `HF_TOKEN`)
- [ ] Add rustdoc documentation
- [ ] Add usage examples

## Success Criteria

- [ ] All core CRUD operations implemented
- [ ] Zero warnings with `-D warnings -W clippy::pedantic`
- [ ] Full rustdoc coverage
- [ ] Integration tests pass with valid `HF_TOKEN`
- [ ] API parity with `huggingface_hub_rust` for:
  - Repository info/retrieval
  - Repository CRUD (create/delete/move)
  - File listing and downloads
  - File uploads via commits
  - Model/dataset/space listing

## Out of Scope (Phase 1)

- **Xet integration** — Xet is a specialized storage backend; skip for now
- **Spaces management** — Hardware provisioning, secrets, variables (requires special API)
- **Commit branches** — Creating branches, tags, PRs (nice-to-have)
- **File caching** — Local cache with etag/symlink logic
- **Blocking wrapper** — Separate sync/async APIs (we're sync-only)

## Verification

```bash
# Enable feature and compile
cargo check -p foundation_deployment --features huggingface

# Run tests
cargo test -p foundation_deployment --features huggingface

# Run clippy
cargo clippy -p foundation_deployment --features huggingface -- -D warnings -W clippy::pedantic

# Generate docs
cargo doc -p foundation_deployment --features huggingface --no-deps
```

## References

- Reference implementation: `/home/darkvoid/Boxxed/@dev/sources/huggingface_hub_rust`
- Hugging Face API docs: https://huggingface.co/docs/hub/api
- Python client: https://github.com/huggingface/huggingface_hub

---

_Created: 2026-04-06_
