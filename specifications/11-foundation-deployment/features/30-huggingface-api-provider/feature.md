---
workspace_name: "ewe_platform"
spec_directory: "specifications/11-foundation-deployment"
feature_directory: "specifications/11-foundation-deployment/features/30-huggingface-api-provider"
this_file: "specifications/11-foundation-deployment/features/30-huggingface-api-provider/feature.md"

status: completed
priority: high
created: 2026-04-06
completed: 2026-04-08

depends_on: ["01-foundation-deployment-core", "26-gen-provider-clients", "27-provider-api-feature-flags"]

tasks:
  completed: 8
  uncompleted: 0
  total: 8
  completion_percentage: 100%
---


# Hugging Face Hub API Provider

## Required Skills

**Before implementing, read the following skills:**

```bash
/read-skill rust-clean-code
/read-skill valtron
/read-skill simple_http
```

These skills cover:
- **rust-clean-code**: Code style, patterns, and quality standards for this project
- **valtron**: Thread pool execution, `StreamIterator`, and `from_future` patterns
- **simple_http**: HTTP client usage, request/response handling, multipart forms

## Iron Law: Zero Warnings

> **All code must compile with zero warnings and pass all lints. No suppression. No exceptions.**
>
> - `cargo clippy -p foundation_deployment -- -D warnings -W clippy::pedantic` — zero warnings
> - `cargo doc -p foundation_deployment --no-deps` — zero rustdoc warnings
> - `cargo test -p foundation_deployment` — zero compilation warnings
> - **No `#[allow(...)]`, `#[expect(...)]`, or `#![allow(...)]` anywhere.** Fix the code, never suppress.

## Overview

Implement a **Hugging Face Hub API provider** that reimplements the functionality of the `huggingface_hub_rust` crate using **only `valtron` and `simple_http`** — no `tokio`, no `reqwest`, no async unless absolutely unavoidable.

Built from: github.com/huggingface/huggingface_hub_rust

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

## Dependencies

Add `foundation_macros` and `derive_more` to `backends/foundation_deployment/Cargo.toml` (no feature flag needed for foundation_macros):

```toml
[dependencies]
foundation_macros = { path = "../foundation_macros" }  # For JsonHash derive
derive_more = { workspace = true }  # For From derive on error types
```

Add feature flag for the huggingface provider:

```toml
[features]
huggingface = []  # Enable huggingface provider module
```

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
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonHash)]
#[serde(rename_all = "lowercase")]
pub enum RepoType {
    Model,
    Dataset,
    Space,
    Kernel,
}

/// Model information returned by GET /api/models/{id}
#[derive(Debug, Clone, Serialize, Deserialize, JsonHash)]
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
#[derive(Debug, Clone, Serialize, Deserialize, JsonHash)]
#[serde(rename_all = "camelCase")]
pub struct DatasetInfo { /* similar structure */ }

/// Space information
#[derive(Debug, Clone, Serialize, Deserialize, JsonHash)]
#[serde(rename_all = "camelCase")]
pub struct SpaceInfo {
    pub id: String,
    pub sdk: Option<String>,
    pub host: Option<String>,
    pub subdomain: Option<String>,
    pub runtime: Option<serde_json::Value>,
    // ... more fields
}

/// Union type for repository info responses
#[derive(Debug, Clone, JsonHash)]
pub enum RepoInfo {
    Model(ModelInfo),
    Dataset(DatasetInfo),
    Space(SpaceInfo),
}
```

### User Types

```rust
#[derive(Debug, Clone, Deserialize, JsonHash)]
pub struct User {
    pub username: String,
    pub fullname: Option<String>,
    pub avatar_url: Option<String>,
    pub user_type: Option<String>,
    pub is_pro: Option<bool>,
    pub email: Option<String>,
    pub orgs: Option<Vec<OrgMembership>>,
}

#[derive(Debug, Clone, Deserialize, JsonHash)]
pub struct Organization {
    pub name: String,
    pub fullname: Option<String>,
    pub avatar_url: Option<String>,
    pub org_type: Option<String>,
}
```

### File/Tree Types

```rust
#[derive(Debug, Clone, Deserialize, JsonHash)]
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

#[derive(Debug, Clone, Serialize, Deserialize, JsonHash)]
pub struct RepoSibling {
    pub rfilename: String,
    pub size: Option<u64>,
    pub lfs: Option<BlobLfsInfo>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonHash)]
pub struct BlobLfsInfo {
    pub size: Option<u64>,
    pub sha256: Option<String>,
    pub pointer_size: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonHash)]
#[serde(rename_all = "camelCase")]
pub struct LastCommitInfo {
    pub id: Option<String>,
    pub title: Option<String>,
    pub date: Option<String>,
}
```

### Commit Types

```rust
#[derive(Debug, Clone, Deserialize, JsonHash)]
pub struct CommitAuthor {
    pub user: Option<String>,
    pub name: Option<String>,
    pub email: Option<String>,
}

#[derive(Debug, Clone, Deserialize, JsonHash)]
pub struct GitCommitInfo {
    pub id: String,
    pub authors: Vec<CommitAuthor>,
    pub date: Option<String>,
    pub title: String,
    pub message: String,
}

#[derive(Debug, Clone, Deserialize, JsonHash)]
#[serde(rename_all = "camelCase")]
pub struct CommitInfo {
    pub commit_url: Option<String>,
    pub commit_message: Option<String>,
    pub commit_description: Option<String>,
    pub commit_oid: Option<String>,
    pub pr_url: Option<String>,
    pub pr_num: Option<u64>,
}

#[derive(Debug, Clone, JsonHash)]
pub enum CommitOperation {
    Add { path_in_repo: String, source: AddSource },
    Delete { path_in_repo: String },
}

#[derive(Clone, JsonHash)]
pub enum AddSource {
    File(PathBuf),
    Bytes(Vec<u8>),
}
```

### Response Types

```rust
#[derive(Debug, Clone, Deserialize, JsonHash)]
pub struct RepoUrl {
    pub url: String,
}
```

## Error Handling

```rust
// backends/foundation_deployment/src/providers/huggingface/error.rs

use derive_more::From;
use std::fmt;

/// Hugging Face Hub API error types.
/// 
/// Error conventions (from state-stores/LEARNINGS.md):
/// - Use `derive_more::From` for automatic From<> impls
/// - Manual `Display` implementation (no `thiserror`)
/// - All variants are `pub`
#[derive(Debug, From)]
pub enum HuggingFaceError {
    /// HTTP error with status, URL, and response body
    Http {
        status: u16,
        url: String,
        body: String,
    },

    /// Authentication required (401)
    AuthRequired,

    /// Repository not found (404 on repo endpoint)
    RepoNotFound { repo_id: String },

    /// Revision not found (404 on revision endpoint)
    RevisionNotFound { repo_id: String, revision: String },

    /// File not found (404 on file endpoint)
    FileNotFound { path: String, repo_id: String },

    /// Invalid repository type
    InvalidRepoType {
        expected: RepoType,
        actual: RepoType,
    },

    /// Invalid parameter
    InvalidParameter(String),

    /// Generic backend error (wraps other errors)
    #[from(ignore)]
    Backend(String),

    /// Valtron execution error
    #[from(ignore)]
    Valtron(String),

    /// I/O error
    Io(std::io::Error),

    /// JSON parse error
    Json(serde_json::Error),

    /// HTTP parse error
    HttpParse(http::Error),
}

impl fmt::Display for HuggingFaceError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            HuggingFaceError::Http { status, url, body } => {
                write!(f, "HTTP error: {} {} - {}", status, url, body)
            }
            HuggingFaceError::AuthRequired => write!(f, "Authentication required"),
            HuggingFaceError::RepoNotFound { repo_id } => {
                write!(f, "Repository not found: {}", repo_id)
            }
            HuggingFaceError::RevisionNotFound { repo_id, revision } => {
                write!(f, "Revision not found: {} in {}", revision, repo_id)
            }
            HuggingFaceError::FileNotFound { path, repo_id } => {
                write!(f, "File not found: {} in {}", path, repo_id)
            }
            HuggingFaceError::InvalidRepoType { expected, actual } => {
                write!(f, "Invalid repository type: expected {}, got {}", expected, actual)
            }
            HuggingFaceError::InvalidParameter(msg) => {
                write!(f, "Invalid parameter: {}", msg)
            }
            HuggingFaceError::Backend(msg) => write!(f, "Backend error: {}", msg),
            HuggingFaceError::Valtron(msg) => write!(f, "Valtron error: {}", msg),
            HuggingFaceError::Io(e) => write!(f, "I/O error: {}", e),
            HuggingFaceError::Json(e) => write!(f, "JSON error: {}", e),
            HuggingFaceError::HttpParse(e) => write!(f, "HTTP parse error: {}", e),
        }
    }
}

impl std::error::Error for HuggingFaceError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            HuggingFaceError::Io(e) => Some(e),
            HuggingFaceError::Json(e) => Some(e),
            HuggingFaceError::HttpParse(e) => Some(e),
            _ => None,
        }
    }
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
    
    /// Get authenticated user info (blocking, uses valtron internally)
    pub fn whoami(&self) -> Result<User>;
    
    /// Check if token is valid (blocking)
    pub fn auth_check(&self) -> Result<()>;
    
    /// List models with pagination (returns valtron StreamIterator)
    pub fn list_models(&self, params: &ListModelsParams) -> Result<impl StreamIterator<D = Result<ModelInfo>, P = ()> + Send>;
    
    /// List datasets with pagination (returns valtron StreamIterator)
    pub fn list_datasets(&self, params: &ListDatasetsParams) -> Result<impl StreamIterator<D = Result<DatasetInfo>, P = ()> + Send>;
    
    /// List spaces with pagination (returns valtron StreamIterator)
    pub fn list_spaces(&self, params: &ListSpacesParams) -> Result<impl StreamIterator<D = Result<SpaceInfo>, P = ()> + Send>;
    
    /// Create a repository (blocking, uses valtron internally)
    pub fn create_repo(&self, params: &CreateRepoParams) -> Result<RepoUrl>;
    
    /// Delete a repository (blocking, uses valtron internally)
    pub fn delete_repo(&self, params: &DeleteRepoParams) -> Result<()>;
    
    /// Move/rename a repository (blocking, uses valtron internally)
    pub fn move_repo(&self, params: &MoveRepoParams) -> Result<RepoUrl>;
    
    /// Get repository handle for model operations
    pub fn model(&self, owner: impl Into<String>, name: impl Into<String>) -> HFRepository;
    
    /// Get repository handle for dataset operations
    pub fn dataset(&self, owner: impl Into<String>, name: impl Into<String>) -> HFRepository;
    
    /// Get repository handle for space operations
    pub fn space(&self, owner: impl Into<String>, name: impl Into<String>) -> HFRepository;
    
    // Internal helpers (still public per always-public policy)
    pub fn auth_headers(&self) -> HeaderMap;
    pub fn api_url(&self, repo_type: Option<RepoType>, repo_id: &str) -> String;
    pub fn download_url(&self, repo_type: Option<RepoType>, repo_id: &str, revision: &str, filename: &str) -> String;
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
    /// Get repository info (blocking, uses valtron internally)
    pub fn info(&self, params: &RepoInfoParams) -> Result<RepoInfo>;
    
    /// Check if repository exists (blocking)
    pub fn exists(&self) -> Result<bool>;
    
    /// Check if revision exists (blocking)
    pub fn revision_exists(&self, params: &RepoRevisionExistsParams) -> Result<bool>;
    
    /// Check if file exists (blocking)
    pub fn file_exists(&self, params: &RepoFileExistsParams) -> Result<bool>;
    
    /// List files in repository (blocking)
    pub fn list_files(&self, params: &RepoListFilesParams) -> Result<Vec<String>>;
    
    /// List tree entries (returns valtron StreamIterator)
    pub fn list_tree(&self, params: &RepoListTreeParams) -> Result<impl StreamIterator<D = Result<RepoTreeEntry>, P = ()> + Send>;
    
    /// Get info for specific paths (blocking)
    pub fn get_paths_info(&self, params: &RepoGetPathsInfoParams) -> Result<Vec<RepoTreeEntry>>;
    
    /// Download a file (blocking, uses valtron internally)
    pub fn download_file(&self, params: &RepoDownloadFileParams) -> Result<PathBuf>;
    
    /// Upload a file (blocking, uses valtron internally)
    pub fn upload_file(&self, params: &RepoUploadFileParams) -> Result<CommitInfo>;
    
    /// Delete a file (blocking, uses valtron internally)
    pub fn delete_file(&self, params: &RepoDeleteFileParams) -> Result<CommitInfo>;
    
    /// Create a commit with multiple operations (blocking, uses valtron internally)
    pub fn create_commit(&self, params: &RepoCreateCommitParams) -> Result<CommitInfo>;
    
    /// Update repository settings (blocking)
    pub fn update_settings(&self, params: &RepoUpdateSettingsParams) -> Result<()>;
    
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

## Key Learnings Applied (from project LEARNINGS.md)

This specification incorporates lessons from previous implementations:

### From state-stores/LEARNINGS.md

| Lesson | Applied To |
|--------|------------|
| `SimpleHttpClient` is the only HTTP client | All HTTP calls use `foundation_core::simple_http` |
| Error types use `derive_more::From` + manual `Display` | `HuggingFaceError` implementation |
| Always public methods (`pub`, never `pub(crate)`) | All public API methods |
| Valtron `schedule_future` for single-value ops | `whoami()`, `create_repo()`, etc. |
| Stream expansion for multi-value ops | `list_models()`, `list_datasets()` |

### From foundation-db/LEARNINGS.md

| Lesson | Applied To |
|--------|------------|
| Methods return streams, callers block when needed | All methods return `StreamIterator` or `HuggingFaceStream` |
| `Send + 'static` requirement for async blocks | Clone `Arc`, own `String` data before async block |
| `!Send` types consumed inside async block | Collect iterators before returning |
| Turbo-fish `Ok::<_, E>` for explicit error types | All async blocks |
| Three-level error handling | Valtron failure, empty stream, backend error |

### From rust-clean-code skill

| Lesson | Applied To |
|--------|------------|
| No `unwrap()` - use `?` or proper error handling | All error handling |
| Full rustdoc on public items | All `pub` items documented |
| `JsonHash` derive on serializable types | All data types |
| Builder pattern for complex configurations | `HFClientBuilder`, parameter types |

## Implementation Notes

### Valtron Patterns (from `read-skill valtron` + project LEARNINGS.md)

**The fundamental rule:** Methods return streams for multi-value operations. For single-value operations where the result is needed immediately (auth, user info for subsequent requests), blocking internally is acceptable.

#### SimpleHttpClient Integration Pattern

All HTTP operations follow the pattern from `backends/foundation_deployment/src/providers/prisma-postgres/clients/mod.rs`:

1. Start with `ClientRequestBuilder` from `client.get(url)?` or `client.post(url)?`
2. Call `.build_send_request()` to get `SendRequestTask`
3. Transform with `.map_ready()` handling `RequestIntro` enum
4. Use `body_reader::collect_string(stream)` to read response body
5. Apply `.map_pending()` for progress state (usually `map_pending(|_| ())`)
6. Call `execute(task, None)` to get `StreamIterator`

#### Single-Value Operations (Blocking Acceptable)

For operations that return exactly one result and where blocking is acceptable (auth, user info):

```rust
use foundation_core::valtron::{execute, Stream};
use foundation_core::simple_http::{RequestIntro, body_reader};

/// Get authenticated user info.
/// Blocks internally — acceptable for single-value ops where result is needed immediately.
pub fn whoami(&self) -> Result<User> {
    let client = self.inner.client.clone();
    let url = format!("{}/api/whoami-v2", self.inner.endpoint);
    let headers = self.auth_headers();
    
    // Start with ClientRequestBuilder
    let builder = client.get(&url)?
        .headers(headers);
    
    // Build SendRequestTask and transform
    let task = builder
        .build_send_request()
        .map_err(|e| HuggingFaceError::HttpParse(e.into()))?
        .map_ready(|intro| match intro {
            RequestIntro::Success { stream, status } => {
                let headers = status.headers().clone();
                if !status.is_success() {
                    return Err(HuggingFaceError::Http {
                        status: status.as_u16(),
                        url: url.clone(),
                        body: format!("HTTP {}", status.as_u16()),
                    });
                }
                // Read body using body_reader helper
                let body = body_reader::collect_string(stream);
                // Parse JSON inside map_ready
                let user: User = serde_json::from_str(&body)
                    .map_err(HuggingFaceError::Json)?;
                Ok(user)
            }
            RequestIntro::Failed(e) => Err(HuggingFaceError::Backend(
                format!("Request failed: {}", e)
            )),
        })
        .map_pending(|_| ());  // Discard progress info
    
    // Execute and collect single result
    let stream = execute(task, None)
        .map_err(|e| HuggingFaceError::Valtron(e.to_string()))?;
    
    // Use standard Iterator::find_map to extract first Next value
    stream.find_map(|s| match s {
        Stream::Next(result) => Some(result),
        _ => None,
    }).ok_or_else(|| HuggingFaceError::Backend("No result from whoami".into()))
}

// Caller uses directly:
let user = hf_client.whoami()?;  // Returns Result<User>
```

**When to block internally:**
- `whoami()` — need user info for subsequent operations
- `auth_check()` — must know before proceeding
- `create_repo()` — need the repo URL immediately
- Single-item lookups where the caller needs the value

#### Multi-Value Operations (Return Streams)

For listing operations with pagination, return streams:

```rust
use foundation_core::valtron::{execute, Stream};
use foundation_core::simple_http::{RequestIntro, body_reader};

pub type HuggingFaceStream<T> = Box<dyn Iterator<Item = Stream<Result<T, HuggingFaceError>, ()>> + Send>;

/// List models with pagination.
/// Returns stream — caller composes and collects at boundary.
pub fn list_models(&self, params: &ListModelsParams) -> Result<HuggingFaceStream<ModelInfo>> {
    let client = self.inner.client.clone();
    let url = build_list_url(&self.inner.endpoint, "models", params);
    
    let builder = client.get(&url)?;
    
    let task = builder
        .build_send_request()
        .map_err(|e| HuggingFaceError::HttpParse(e.into()))?
        .map_ready(|intro| match intro {
            RequestIntro::Success { stream, status } => {
                let headers = status.headers().clone();
                if !status.is_success() {
                    return Err(HuggingFaceError::Http {
                        status: status.as_u16(),
                        url: url.clone(),
                        body: format!("HTTP {}", status.as_u16()),
                    });
                }
                let body = body_reader::collect_string(stream);
                
                // Parse response — collect all items inside async block (Send requirement)
                let models: Vec<ModelInfo> = serde_json::from_str(&body)
                    .map_err(HuggingFaceError::Json)?;
                Ok(models)
            }
            RequestIntro::Failed(e) => Err(HuggingFaceError::Backend(
                format!("Request failed: {}", e)
            )),
        })
        .map_pending(|_| ());
    
    let stream = execute(task, None)
        .map_err(|e| HuggingFaceError::Valtron(e.to_string()))?;
    
    // Expand Vec into individual stream items using flat_map_next
    Ok(Box::new(stream.flat_map_next(|result| {
        match result {
            Ok(models) => models.into_iter().map(|m| Stream::Next(Ok(m))).collect::<Vec<_>>().into_iter(),
            Err(e) => vec![Stream::Next(Err(e))].into_iter(),
        }
    })))
}

// Caller consumes stream at boundary:
let models: Vec<ModelInfo> = hf_client
    .list_models(&params)?
    .filter_map(|s| match s {
        Stream::Next(Ok(m)) => Some(m),
        _ => None,
    })
    .collect();
```

### Error Handling with map_circuit

Use `map_circuit` to short-circuit on error while preserving the error value:

```rust
use foundation_core::valtron::map_circuit;

let stream = raw_stream.map_circuit(|item| match item {
    Stream::Next(Err(e)) => ShortCircuit::ReturnAndStop(Stream::Next(Err(e))),
    Stream::Next(Ok(v)) => ShortCircuit::Continue(Stream::Next(Ok(v))),
    _ => ShortCircuit::Continue(item),
});
```

### Key Valtron Patterns from Learnings

| Pattern | Used For | Example |
|---------|----------|---------|
| `ClientRequestBuilder` → `build_send_request()` → `execute` | All HTTP operations | `whoami()`, `list_models()` |
| `RequestIntro::Success { stream, status }` handling | HTTP response processing | Check status, read body with `body_reader::collect_string()` |
| `execute` + `find_map` | Single-value HTTP ops (blocking OK) | `whoami()`, `auth_check()` |
| `execute` + `flat_map_next` | Multi-value ops (return stream) | `list_models()`, `list_datasets()` |
| `Send + 'static` | All captured variables in closures | Clone `Arc`, own `String` data |
| Turbo-fish `Ok::<_, E>` | Explicit error types | `Ok::<_, HuggingFaceError>(value)` |
| `map_pending(|_| ())` | Discard progress info | When `Pending = ()` is sufficient |

### Three-Level Error Handling

Every `execute` call handles errors at three levels:

1. **Valtron execution failure** — `execute()` itself fails (runtime/pool issue)
2. **Empty stream** — The future ran but produced no `Stream::Next` item
3. **Backend error** — The future's `Result` was `Err` (HTTP error, JSON parse error)

All three are mapped to `HuggingFaceError` variants, preserving the original error message.

### When to Use run_future_iter

For genuinely streaming large result sets where collecting to `Vec` would cause OOM:

```rust
use foundation_core::valtron::{run_future_iter, ThreadedValue};

// Only needed for !Send row iterators from databases
// HTTP responses are typically Send, so from_future is sufficient
let iter = run_future_iter(
    move || async move { /* ... */ },
    None,
    None,
)?;
let stream = iter.map(|tv| match tv {
    ThreadedValue::Value(result) => Stream::Next(result),
});
```

For HTTP API responses, the `ClientRequestBuilder` → `build_send_request()` → `execute()` pattern is sufficient since responses are bounded and `Send`.

### Rust Clean Code Standards (from `read-skill rust-clean-code` + LEARNINGS.md)

- **Error types**: Use `derive_more::From` + manual `Display` (no `thiserror`)
- **Documentation**: Full rustdoc on all public items
- **No `unwrap()`**: Use `?` or proper error handling
- **Type state**: Use builder pattern for complex configurations
- **Feature flags**: Guard optional functionality with `#[cfg(feature = "...")]`
- **JsonHash**: Derive `JsonHash` on all serializable types for state hashing (from `foundation_macros`)
- **Always public**: All items use `pub`, never `pub(crate)` or `pub(super)`
- **Stream-returning methods**: Methods return streams, callers block when needed
- **SimpleHttpClient pattern**: Use `ClientRequestBuilder` → `build_send_request()` → `RequestIntro` handling → `body_reader::collect_string()`

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
- **Phase 1**: No caching, direct API calls only (all calls via valtron thread pool)
- **Phase 2**: Add optional file caching (etag-based, symlink snapshots) using valtron for async file I/O

## Tasks

**Before starting implementation, ensure you have read:**
- `rust-clean-code` — Code standards and patterns
- `valtron` — Thread pool execution, StreamIterator
- `simple_http` — HTTP client usage, multipart forms

### 1. Core Module Structure (completed)

- [x] Create `backends/foundation_deployment/src/providers/huggingface/mod.rs`
- [x] Create `constants.rs` with endpoint URLs, env var names, defaults
- [x] Create `error.rs` with `HuggingFaceError` types using `derive_more::From` + manual `Display`
- [x] Create `types.rs` with all data types (RepoType, ModelInfo, etc.) - all with `JsonHash` derive
- [x] Add `huggingface` feature flag to `Cargo.toml`
- [x] Ensure `foundation_macros` and `derive_more` are in dependencies

### 2. HTTP Client (completed)

- [x] Create `client.rs` with `HFClient` and `HFClientBuilder`
- [x] Implement token resolution (env → file → cache)
- [x] Implement `auth_headers()` helper
- [x] Implement URL builders (`api_url()`, `download_url()`)
- [x] Implement `whoami()` and `auth_check()`

### 3. Repository Handle (completed)

- [x] Create `repository.rs` with `HFRepository` struct
- [x] Implement `info()` for model/dataset/space
- [x] Implement `exists()`, `revision_exists()`, `file_exists()`
- [x] Implement `update_settings()`
- [x] Implement `list_files()`, `list_tree()`, `get_paths_info()`

### 4. Listing Operations (completed)

- [x] Implement `list_models()` with pagination
- [x] Implement `list_datasets()` with pagination
- [x] Implement `list_spaces()` with pagination
- [x] Implement user/org endpoints (`get_user_overview()`, etc.)

### 5. Repository CRUD (completed)

- [x] Implement `create_repo()`
- [x] Implement `delete_repo()`
- [x] Implement `move_repo()`

### 6. File Download (completed)

- [x] Implement `download_file()` (direct download, no cache for Phase 1)
- [x] Handle LFS files (detect `lfs` field in tree entries)

### 7. File Upload & Commits (completed)

- [x] Implement `upload_file()` via `create_commit()`
- [x] Implement `create_commit()` with multipart form data
- [x] Implement `delete_file()`, `delete_folder()`

### 8. Tests & Documentation (completed)

- [x] Write unit tests for type serialization
- [x] Write integration tests (require `HF_TOKEN`)
- [x] Add rustdoc documentation
- [x] Add usage examples

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

**Completed 2026-04-08:**

```bash
# Enable feature and compile - PASSED
cargo check -p foundation_deployment --features huggingface

# Run tests - PASSED (46 tests in foundation_deployment, 27 in foundation_testing)
cargo test -p foundation_deployment --features huggingface
cargo test -p foundation_testing --features huggingface

# Generate docs - PASSED
cargo doc -p foundation_deployment --features huggingface --no-deps
```

**Integration tests:** 5 tests defined in `tests/huggingface_integration.rs` (ignored by default, require `HF_TOKEN` environment variable)

**Implementation statistics:**
- ~2000 lines of code across 6 files
- 50+ types with `JsonHash` derive
- Full rustdoc coverage
- Zero compilation warnings

## References

- Reference implementation: `/home/darkvoid/Boxxed/@dev/sources/huggingface_hub_rust`
- Hugging Face API docs: https://huggingface.co/docs/hub/api
- Python client: https://github.com/huggingface/huggingface_hub

---

_Created: 2026-04-06_
