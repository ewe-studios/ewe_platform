---
workspace_name: "ewe_platform"
spec_directory: "specifications/11-cloudflare-wrangler-deployment"
feature_directory: "specifications/11-cloudflare-wrangler-deployment/features/01-foundation-deployment-crate"
this_file: "specifications/11-cloudflare-wrangler-deployment/features/01-foundation-deployment-crate/feature.md"

status: pending
priority: high
created: 2026-03-26

depends_on: []

tasks:
  completed: 0
  uncompleted: 7
  total: 7
  completion_percentage: 0%
---


# Foundation Deployment Crate

## Overview

Create the `foundation_deployment` crate that serves as the core deployment utility library for Cloudflare and future deployment targets. This crate provides process execution, API client foundations, and deployment orchestration.

## Dependencies

This feature has no dependencies on other features in this specification.

This feature is required by:
- `wrangler-process-wrapper` - Uses process execution utilities
- `cloudflare-api-client` - Uses HTTP client foundations
- `deploy-planner` - Uses deployment orchestration types

## Requirements

### Crate Structure

Create `backends/foundation_deployment/` with the following structure:

```
backends/foundation_deployment/
├── Cargo.toml
└── src/
    ├── lib.rs
    ├── wrangler/
    │   └── mod.rs
    ├── cloudflare/
    │   └── mod.rs
    ├── process/
    │   └── mod.rs
    ├── project/
    │   └── mod.rs
    └── deploy/
        └── mod.rs
```

### Error Types

Define unified error handling using `derive_more`:

```rust
use derive_more::From;
use foundation_core::simple_http::HttpClientError;

#[derive(Debug, From)]
pub enum DeploymentError {
    #[from(ignore)]
    WranglerNotFound,

    #[from(ignore)]
    WranglerCommandFailed {
        exit_code: Option<i32>,
        stdout: String,
        stderr: String,
    },

    #[from(ignore)]
    CloudflareApiError {
        status: u16,
        message: String,
        error_code: Option<String>,
    },

    #[from(ignore)]
    ProcessExecutionFailed(String),

    #[from(ignore)]
    ConfigValidationFailed(String),

    #[from(ignore)]
    HttpClientError(HttpClientError),

    #[from(ignore)]
    IoError(std::io::Error),
}

impl std::error::Error for DeploymentError {}
impl core::fmt::Display for DeploymentError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::WranglerNotFound => write!(f, "wrangler CLI not found"),
            Self::WranglerCommandFailed { exit_code, stderr, .. } => {
                write!(f, "wrangler command failed (exit: {:?}): {}", exit_code, stderr)
            }
            Self::CloudflareApiError { status, message, .. } => {
                write!(f, "Cloudflare API error ({}): {}", status, message)
            }
            Self::ProcessExecutionFailed(msg) => write!(f, "Process execution failed: {}", msg),
            Self::ConfigValidationFailed(msg) => write!(f, "Config validation failed: {}", msg),
            Self::HttpClientError(err) => write!(f, "HTTP client error: {}", err),
            Self::IoError(err) => write!(f, "IO error: {}", err),
        }
    }
}
```

### Process Execution Module

```rust
// process/mod.rs

/// Process output capture
#[derive(Debug, Clone)]
pub struct ProcessOutput {
    pub exit_code: Option<i32>,
    pub stdout: String,
    pub stderr: String,
    pub success: bool,
}

/// Process executor with streaming output support
pub struct ProcessExecutor {
    command: std::process::Command,
    capture_output: bool,
    streaming: bool,
}

impl ProcessExecutor {
    pub fn new(command: &str) -> Self;

    pub fn arg<S: AsRef<OsStr>>(mut self, arg: S) -> Self;

    pub fn args<I, S>(mut self, args: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: AsRef<OsStr>;

    pub fn env<K, V>(mut self, key: K, val: V) -> Self
    where
        K: AsRef<OsStr>,
        V: AsRef<OsStr>;

    pub fn current_dir<P: AsRef<Path>>(mut self, dir: P) -> Self;

    /// Execute and capture all output
    pub fn execute(self) -> Result<ProcessOutput, DeploymentError>;

    /// Execute with streaming output (for long-running commands)
    pub fn execute_streaming<F>(self, on_line: F) -> Result<ProcessOutput, DeploymentError>
    where
        F: FnMut(&str);  // Called for each line of output
}
```

### Project Scanner Module

```rust
// project/mod.rs

/// Scanned project information
#[derive(Debug)]
pub struct ProjectInfo {
    pub name: String,
    pub version: String,
    pub root_dir: PathBuf,
    pub has_wasm_target: bool,
    pub wrangler_config: Option<WranglerConfig>,
    pub mise_config: Option<MiseConfig>,
}

/// Parsed wrangler.toml
#[derive(Debug, Deserialize)]
pub struct WranglerConfig {
    pub name: String,
    pub main: Option<String>,
    pub compatibility_date: Option<String>,
    pub workers_dev: Option<bool>,
    pub route: Option<String>,
    pub account_id: Option<String>,
    pub env: Option<HashMap<String, WranglerEnvironment>>,
}

/// Project scanner
pub struct ProjectScanner;

impl ProjectScanner {
    /// Scan a project directory
    pub fn scan(root: &Path) -> Result<ProjectInfo, DeploymentError>;

    /// Validate project configuration
    pub fn validate(info: &ProjectInfo) -> Result<(), DeploymentError>;
}
```

### Deployment Result Types

```rust
// deploy/mod.rs

/// Deployment result
#[derive(Debug)]
pub struct DeploymentResult {
    pub deployment_id: String,
    pub worker_name: String,
    pub environment: Option<String>,
    pub deployed_at: chrono::DateTime<chrono::Utc>,
    pub url: Option<String>,
}

/// Deployment configuration
#[derive(Debug, Clone)]
pub struct DeployConfig {
    pub worker_name: String,
    pub account_id: String,
    pub environment: Option<String>,
    pub dry_run: bool,
    pub verbose: bool,
}

/// Deployment progress events
#[derive(Debug)]
pub enum DeployProgress {
    Validating,
    Building,
    Uploading { bytes_sent: usize, total_bytes: usize },
    Deploying,
    Verifying,
    Complete(DeploymentResult),
    Failed(DeploymentError),
}
```

### Public API

```rust
// lib.rs

// Core types
pub use deploy::{DeployConfig, DeploymentResult, DeployProgress};
pub use error::DeploymentError;
pub use process::{ProcessExecutor, ProcessOutput};
pub use project::{ProjectInfo, ProjectScanner, WranglerConfig};

// Submodules for advanced usage
pub mod wrangler;
pub mod cloudflare;
pub mod process;
pub mod project;
pub mod deploy;
```

## Tasks

1. **Create crate structure**
   - [ ] Create `backends/foundation_deployment/Cargo.toml`
   - [ ] Create `backends/foundation_deployment/src/lib.rs`
   - [ ] Create module stubs for all submodules
   - [ ] Add crate to workspace members in root `Cargo.toml`

2. **Define error types**
   - [ ] Create `src/error.rs` with `DeploymentError` enum
   - [ ] Implement `Display` and `Error` traits
   - [ ] Add conversion from `HttpClientError` and `std::io::Error`
   - [ ] Write unit tests for error formatting

3. **Implement process executor**
   - [ ] Create `src/process/mod.rs`
   - [ ] Implement `ProcessOutput` struct
   - [ ] Implement `ProcessExecutor` with builder pattern
   - [ ] Add `execute()` method for captured output
   - [ ] Add `execute_streaming()` method for live output
   - [ ] Write unit tests for command building

4. **Implement project scanner**
   - [ ] Create `src/project/mod.rs`
   - [ ] Define `ProjectInfo`, `WranglerConfig`, `MiseConfig` structs
   - [ ] Implement `ProjectScanner::scan()`
   - [ ] Implement `ProjectScanner::validate()`
   - [ ] Add TOML parsing with `toml` crate
   - [ ] Write unit tests for config parsing

5. **Define deployment types**
   - [ ] Create `src/deploy/mod.rs`
   - [ ] Define `DeploymentResult` struct
   - [ ] Define `DeployConfig` struct
   - [ ] Define `DeployProgress` enum
   - [ ] Write unit tests for type constructors

6. **Add dependencies**
   - [ ] Add `foundation_core` workspace dependency
   - [ ] Add `tokio` with process feature
   - [ ] Add `chrono` for timestamps
   - [ ] Add `toml` for config parsing
   - [ ] Add `which` for command discovery

7. **Write integration test**
   - [ ] Create `backends/foundation_deployment/tests/deployment_tests.rs`
   - [ ] Test process execution with real commands
   - [ ] Test project scanning on example projects
   - [ ] Test config validation

## Implementation Notes

- Use `foundation_core::simple_http::client::SimpleHttpClient` as the HTTP client base
- Follow error handling patterns from `foundation_ai`
- Use `derive_more` for `From` implementations (already in workspace dependencies)
- Keep process execution async-compatible for valtron integration

## Success Criteria

- [ ] All 7 tasks completed
- [ ] `cargo check` passes with no warnings
- [ ] `cargo clippy -- -D warnings` passes
- [ ] Unit tests pass
- [ ] Process executor can run and capture `wrangler --version`
- [ ] Project scanner correctly parses `wrangler.toml` files

## Verification

```bash
# Build and check
cd backends/foundation_deployment
cargo check
cargo clippy -- -D warnings
cargo fmt -- --check

# Run tests
cargo test

# Test project scanner
cargo test project_scanner -- --nocapture
```

---

_Created: 2026-03-26_
