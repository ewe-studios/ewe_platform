---
workspace_name: "ewe_platform"
spec_directory: "specifications/11-foundation-deployment"
feature_directory: "specifications/11-foundation-deployment/features/01-foundation-deployment-core"
this_file: "specifications/11-foundation-deployment/features/01-foundation-deployment-core/feature.md"

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


# Foundation Deployment Core

## Iron Law: Zero Warnings

> **All code must compile with zero warnings and pass all lints. No suppression. No exceptions.**
>
> - `cargo clippy -p foundation_deployment -- -D warnings -W clippy::pedantic` — zero warnings
> - `cargo doc -p foundation_deployment --no-deps` — zero rustdoc warnings
> - `cargo test -p foundation_deployment` — zero compilation warnings
> - **No `#[allow(...)]`, `#[expect(...)]`, or `#![allow(...)]` anywhere.** Fix the code, never suppress.

## Overview

Create the `foundation_deployment` crate with provider-agnostic primitives: the `DeploymentProvider` trait, unified error types, process execution, project scanning, and shared deployment types. Everything in this feature is cloud-agnostic.

## Dependencies

This feature has no dependencies on other features in this specification.

Required by:
- All other features in this specification

## Requirements

### Crate Structure

```
backends/foundation_deployment/
|-- Cargo.toml
+-- src/
    |-- lib.rs
    |-- error.rs
    |-- config.rs
    +-- core/
        |-- mod.rs
        |-- traits.rs
        |-- types.rs
        |-- process.rs
        +-- project.rs
```

### DeploymentProvider Trait

```rust
// core/traits.rs

use std::path::Path;
use serde::{Serialize, de::DeserializeOwned};

/// Every cloud deployment target implements this trait.
/// The engine calls these methods without knowing which cloud it's talking to.
pub trait DeploymentProvider {
    /// Provider-specific configuration type (parsed from native config file).
    type Config: DeserializeOwned + Serialize + Clone + std::fmt::Debug;

    /// Provider-specific resource information returned by `status()`.
    type Resources: std::fmt::Debug;

    /// Human-readable provider name ("cloudflare", "gcp", "aws").
    fn name(&self) -> &str;

    /// Try to detect this provider's config file in `project_dir`.
    /// Returns `Some(config)` if found and parseable, `None` otherwise.
    fn detect(project_dir: &Path) -> Option<Self::Config>
    where
        Self: Sized;

    /// Validate the configuration without deploying.
    fn validate(&self, config: &Self::Config) -> Result<(), DeploymentError>;

    /// Build the project artifacts.
    /// `env` selects a named environment (staging, production, etc.).
    fn build(
        &self,
        config: &Self::Config,
        env: Option<&str>,
    ) -> Result<BuildOutput, DeploymentError>;

    /// Deploy to the target.
    /// If `dry_run` is true, validate and build but don't actually push.
    fn deploy(
        &self,
        config: &Self::Config,
        env: Option<&str>,
        dry_run: bool,
    ) -> Result<DeploymentResult, DeploymentError>;

    /// Tail logs from the deployed service.
    fn logs(
        &self,
        config: &Self::Config,
        env: Option<&str>,
    ) -> Result<(), DeploymentError>;

    /// Tear down deployed resources.
    fn destroy(
        &self,
        config: &Self::Config,
        env: Option<&str>,
    ) -> Result<(), DeploymentError>;

    /// Query current status of deployed resources.
    fn status(
        &self,
        config: &Self::Config,
        env: Option<&str>,
    ) -> Result<Self::Resources, DeploymentError>;
}
```

### DeploymentTarget Enum

```rust
// config.rs

use std::path::Path;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeploymentTarget {
    Cloudflare,
    GcpCloudRun,
    AwsLambda,
}

impl DeploymentTarget {
    /// Detect provider from project directory by checking for native config files.
    pub fn detect(project_dir: &Path) -> Option<Self> {
        if project_dir.join("wrangler.toml").exists() {
            Some(Self::Cloudflare)
        } else if project_dir.join("service.yaml").exists() {
            Some(Self::GcpCloudRun)
        } else if project_dir.join("template.yaml").exists() {
            Some(Self::AwsLambda)
        } else {
            None
        }
    }

    pub fn config_file(&self) -> &str {
        match self {
            Self::Cloudflare => "wrangler.toml",
            Self::GcpCloudRun => "service.yaml",
            Self::AwsLambda => "template.yaml",
        }
    }
}
```

### Error Types

```rust
// error.rs

use derive_more::From;

/// Unified error type. Provider-agnostic errors at the top level,
/// provider-specific errors nested inside variants.
#[derive(Debug)]
pub enum DeploymentError {
    // --- Generic errors ---
    /// A shelled-out process failed.
    ProcessFailed {
        command: String,
        exit_code: Option<i32>,
        stdout: String,
        stderr: String,
    },

    /// Config file is invalid or missing.
    ConfigInvalid {
        file: String,
        reason: String,
    },

    /// No provider detected in project directory.
    NoProviderDetected {
        project_dir: String,
    },

    /// Build step failed.
    BuildFailed(String),

    /// Deployment was rejected (e.g. quota, permissions).
    DeployRejected {
        reason: String,
    },

    /// State store operation failed.
    StateFailed(String),

    /// HTTP request to provider API failed.
    HttpError(foundation_core::simple_http::HttpClientError),

    /// IO error.
    IoError(std::io::Error),

    /// SQLite / Turso error.
    SqliteError(String),

    // --- Provider-specific wrappers ---
    /// Cloudflare-specific error details.
    Cloudflare {
        status: u16,
        message: String,
        error_code: Option<String>,
    },

    /// GCP-specific error details.
    Gcp {
        status: u16,
        message: String,
    },

    /// AWS-specific error details.
    Aws {
        status: u16,
        message: String,
        request_id: Option<String>,
    },
}

impl std::error::Error for DeploymentError {}

impl core::fmt::Display for DeploymentError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ProcessFailed { command, exit_code, stderr, .. } => {
                write!(f, "process '{}' failed (exit {:?}): {}", command, exit_code, stderr)
            }
            Self::ConfigInvalid { file, reason } => {
                write!(f, "invalid config '{}': {}", file, reason)
            }
            Self::NoProviderDetected { project_dir } => {
                write!(f, "no deployment provider detected in '{}'", project_dir)
            }
            Self::BuildFailed(msg) => write!(f, "build failed: {}", msg),
            Self::DeployRejected { reason } => write!(f, "deploy rejected: {}", reason),
            Self::StateFailed(msg) => write!(f, "state error: {}", msg),
            Self::HttpError(err) => write!(f, "HTTP error: {}", err),
            Self::IoError(err) => write!(f, "IO error: {}", err),
            Self::SqliteError(msg) => write!(f, "SQLite error: {}", msg),
            Self::Cloudflare { status, message, .. } => {
                write!(f, "Cloudflare API error ({}): {}", status, message)
            }
            Self::Gcp { status, message } => {
                write!(f, "GCP API error ({}): {}", status, message)
            }
            Self::Aws { status, message, .. } => {
                write!(f, "AWS API error ({}): {}", status, message)
            }
        }
    }
}
```

### Shared Types

```rust
// core/types.rs

use chrono::{DateTime, Utc};

/// Output from a build step.
#[derive(Debug, Clone)]
pub struct BuildOutput {
    pub artifacts: Vec<BuildArtifact>,
    pub duration_ms: u64,
}

#[derive(Debug, Clone)]
pub struct BuildArtifact {
    pub path: std::path::PathBuf,
    pub size_bytes: u64,
    pub artifact_type: ArtifactType,
}

#[derive(Debug, Clone)]
pub enum ArtifactType {
    WasmModule,
    JsBundle,
    ContainerImage { tag: String },
    ZipArchive,
    Binary,
}

/// Output from a deployment.
#[derive(Debug, Clone)]
pub struct DeploymentResult {
    pub deployment_id: String,
    pub provider: String,
    pub resource_name: String,
    pub environment: Option<String>,
    pub url: Option<String>,
    pub deployed_at: DateTime<Utc>,
}

/// Progress events emitted during deployment.
#[derive(Debug, Clone)]
pub enum DeployProgress {
    Detecting,
    Validating,
    Building { step: String },
    Packaging,
    Uploading { bytes_sent: u64, total_bytes: u64 },
    Deploying,
    Verifying,
    Complete(DeploymentResult),
    Failed(String),
    RollingBack,
}
```

### Process Executor

```rust
// core/process.rs

use std::ffi::OsStr;
use std::path::Path;
use foundation_core::valtron::{StreamExecutor, StreamIteratorExt, StateMachine};
use serde::{Deserialize, Serialize};

/// ProcessExecutor has a single execute() method returning StreamExecutor.
/// The associated type `Output` defines what the StreamIterator's Stream yields for Next/Pending.
///
/// WHY: Streaming execution must support structured progress updates via Valtron's StateMachine.
///      A single execute() method returning StreamExecutor provides a uniform async streaming interface.
///
/// WHAT: Execute external processes (wrangler, gcloud, aws, docker, etc.) and stream
///       resource-centric events via the associated Output type.
///
/// HOW: Uses foundation_core::valtron::StreamExecutor with associated type for Output.

// ===========================================================================
// Associated Type: ProcessOutput (defines Stream Executor's Output type)
// ===========================================================================

/// The Output type for ProcessExecutor's StreamExecutor.
/// Defines what the StreamIterator yields during process execution.
///
/// This is the associated type that tells the Stream what Next and Pending are.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ProcessOutput {
    // === Pending States (work in progress) ===

    /// Process is spawning.
    Spawning,

    /// Process spawned successfully, waiting for output.
    Running { pid: u32 },

    /// Building step in progress.
    BuildRunning { step: String, elapsed_ms: u64 },

    /// Resource creation in progress.
    ResourceCreating {
        resource_type: String,
        resource_name: String,
    },

    /// Resource update in progress.
    ResourceUpdating {
        resource_type: String,
        resource_name: String,
    },

    /// Upload in progress.
    Uploading {
        bytes_sent: u64,
        total_bytes: Option<u64>,
    },

    /// Verification in progress.
    Verifying {
        check: String,
    },

    // === Next States (completed items yielded by stream) ===

    /// Build step completed.
    BuildCompleted {
        step: String,
        duration_ms: u64,
        success: bool,
    },

    /// Resource was created.
    ResourceCreated {
        resource_type: String,
        resource_name: String,
        resource_id: String,
        metadata: serde_json::Value,
    },

    /// Resource was updated.
    ResourceUpdated {
        resource_type: String,
        resource_name: String,
        revision: String,
    },

    /// Resource was deleted.
    ResourceDeleted {
        resource_type: String,
        resource_name: String,
    },

    /// Output line from stdout.
    StdoutLine { line: String },

    /// Output line from stderr.
    StderrLine { line: String },

    /// Process completed.
    ProcessExited {
        exit_code: Option<i32>,
        success: bool,
        stdout: String,
        stderr: String,
    },

    /// Error occurred.
    Error {
        message: String,
        source: Option<String>,
    },
}

// ===========================================================================
// ProcessExecutor Builder
// ===========================================================================

/// Builder for executing external processes (wrangler, gcloud, aws, docker, etc.).
///
/// The execute() method returns StreamExecutor<ProcessOutput> where:
/// - ProcessOutput is the associated type defining stream output
/// - Pending states: Spawning, Running, BuildRunning, ResourceCreating, etc.
/// - Next states: BuildCompleted, ResourceCreated, StdoutLine, ProcessExited, etc.
pub struct ProcessExecutor {
    command: String,
    args: Vec<String>,
    envs: Vec<(String, String)>,
    working_dir: Option<std::path::PathBuf>,
}

/// The StreamExecutor type returned by execute().
/// ProcessOutput is the associated type - it defines what the Stream yields.
pub type ProcessStreamExecutor = StreamExecutor<ProcessOutput>;

impl ProcessExecutor {
    pub fn new(command: &str) -> Self;
    pub fn arg<S: AsRef<OsStr>>(self, arg: S) -> Self;
    pub fn args<I, S>(self, args: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: AsRef<OsStr>;
    pub fn env<K: AsRef<OsStr>, V: AsRef<OsStr>>(self, key: K, val: V) -> Self;
    pub fn current_dir<P: AsRef<Path>>(self, dir: P) -> Self;

    /// Execute the process and return a StreamExecutor.
    ///
    /// The StreamExecutor implements foundation_core::valtron::StateMachine
    /// and can be consumed via StreamIterator for async streaming.
    ///
    /// # Associated Type
    ///
    /// ProcessOutput defines what the StreamIterator's Stream yields:
    /// - Pending: Spawning, Running, BuildRunning, ResourceCreating, Uploading, Verifying
    /// - Next: BuildCompleted, ResourceCreated, StdoutLine, StderrLine, ProcessExited, Error
    ///
    /// # Example: Streaming consumption
    ///
    /// ```rust
    /// use foundation_core::valtron::StreamIteratorExt;
    ///
    /// let executor = ProcessExecutor::new("wrangler")
    ///     .args(["deploy", "--env", "staging"])
    ///     .execute();
    ///
    /// // Consume via Valtron's StreamIterator
    /// let results = executor
    ///     .into_stream_iter()
    ///     .map_with_progress(|output| async move {
    ///         match output {
    ///             ProcessOutput::ResourceCreated { name, id, .. } => {
    ///                 println!("Created: {} ({})", name, id);
    ///             }
    ///             ProcessOutput::Uploading { bytes_sent, total_bytes } => {
    ///                 let pct = (bytes_sent as f64 / total_bytes.unwrap_or(bytes_sent) as f64) * 100.0;
    ///                 println!("Upload: {:.1}%", pct);
    ///             }
    ///             ProcessOutput::StdoutLine { line } => {
    ///                 println!("> {}", line);
    ///             }
    ///             ProcessOutput::ProcessExited { exit_code, success, .. } => {
    ///                 println!("Process finished: success={}", success);
    ///             }
    ///             _ => {}
    ///         }
    ///         output
    ///     })
    ///     .buffered(1)
    ///     .collect::<Vec<_>>()
    ///     .await;
    /// ```
    ///
    /// # Example: Collecting final result only
    ///
    /// ```rust
    /// let executor = ProcessExecutor::new("docker")
    ///     .args(["build", "-t", "myapp", "."])
    ///     .execute();
    ///
    /// // Just wait for completion, ignore intermediate events
    /// let final_result = executor
    ///     .into_stream_iter()
    ///     .filter(|o| matches!(o, ProcessOutput::ProcessExited { .. }))
    ///     .next()
    ///     .await;
    /// ```
    pub fn execute(self) -> ProcessStreamExecutor;
}

// ===========================================================================
// Helper: Collect all output (for simple cases)
// ===========================================================================

/// Helper function to execute and collect all output into a single result.
/// For simple cases where you don't need streaming.
pub async fn execute_and_collect(executor: ProcessExecutor) -> Result<CollectedOutput, DeploymentError> {
    let stream = executor.execute().into_stream_iter();

    let mut stdout = String::new();
    let mut stderr = String::new();
    let mut exit_code: Option<i32> = None;
    let mut success = false;

    for await output in stream {
        match output {
            ProcessOutput::StdoutLine { line } => stdout.push_str(&line),
            ProcessOutput::StderrLine { line } => stderr.push_str(&line),
            ProcessOutput::ProcessExited { exit_code: code, success: s, .. } => {
                exit_code = code;
                success = s;
                break;
            }
            ProcessOutput::Error { message, .. } => {
                return Err(DeploymentError::ProcessFailed {
                    command: "unknown".to_string(),
                    exit_code,
                    stdout,
                    stderr,
                });
            }
            _ => {}
        }
    }

    Ok(CollectedOutput { exit_code, success, stdout, stderr })
}

/// Collected output from a process (for simple cases).
#[derive(Debug, Clone)]
pub struct CollectedOutput {
    pub exit_code: Option<i32>,
    pub stdout: String,
    pub stderr: String,
    pub success: bool,
}
```

### Project Scanner

```rust
// core/project.rs

use std::path::{Path, PathBuf};

/// Scanned project information (provider-agnostic).
#[derive(Debug)]
pub struct ProjectInfo {
    pub name: String,
    pub root_dir: PathBuf,
    pub target: DeploymentTarget,
    pub has_cargo_toml: bool,
    pub has_dockerfile: bool,
    pub has_mise_toml: bool,
    pub config_file: PathBuf,
}

pub struct ProjectScanner;

impl ProjectScanner {
    /// Scan a project directory, detect provider, gather metadata.
    pub fn scan(root: &Path) -> Result<ProjectInfo, DeploymentError>;
}
```

### Public API (lib.rs)

```rust
// lib.rs

pub mod core;
pub mod state;
pub mod engine;
pub mod providers;
pub mod template;

mod error;
mod config;

// Re-exports
pub use error::DeploymentError;
pub use config::DeploymentTarget;
pub use core::traits::DeploymentProvider;
pub use core::types::{BuildOutput, DeploymentResult, DeployProgress};
pub use core::process::{ProcessExecutor, ProcessOutput};
pub use core::project::{ProjectInfo, ProjectScanner};
pub use state::traits::StateStore;
```

## Tasks

1. **Create crate structure**
   - [ ] Create `backends/foundation_deployment/Cargo.toml`
   - [ ] Create all module files with stubs
   - [ ] Add crate to workspace members in root `Cargo.toml`
   - [ ] Add workspace dependencies (`foundation_core`, `derive_more`, `chrono`, `toml`, `serde`, `serde_json`, `serde_yaml`)

2. **Define error types**
   - [ ] Implement `DeploymentError` enum in `src/error.rs`
   - [ ] Implement `Display` and `Error` traits
   - [ ] Add `From` conversions for `std::io::Error` and `HttpClientError`
   - [ ] Write unit tests for error formatting

3. **Define DeploymentProvider trait**
   - [ ] Create `src/core/traits.rs` with trait definition
   - [ ] Define associated types and all methods
   - [ ] Document each method with usage examples

4. **Define shared types**
   - [ ] Create `src/core/types.rs` with `BuildOutput`, `DeploymentResult`, `DeployProgress`
   - [ ] Define `ArtifactType` enum
   - [ ] Write unit tests for type conversions

5. **Implement ProcessExecutor with Valtron StreamExecutor**
   - [ ] Create `src/core/process.rs`
   - [ ] Define `ProcessOutput` enum as the associated type for StreamExecutor
   - [ ] Define Pending states: `Spawning`, `Running`, `BuildRunning`, `ResourceCreating`, `Uploading`, `Verifying`
   - [ ] Define Next states: `BuildCompleted`, `ResourceCreated`, `StdoutLine`, `StderrLine`, `ProcessExited`, `Error`
   - [ ] Implement `ProcessExecutor` builder pattern
   - [ ] Implement single `execute()` method returning `StreamExecutor<ProcessOutput>`
   - [ ] Ensure `StreamExecutor` integrates with Valtron's `StateMachine` and `StreamIterator`
   - [ ] Write unit tests for process execution
   - [ ] Add `execute_and_collect()` helper for simple non-streaming use cases

6. **Implement ProjectScanner**
   - [ ] Create `src/core/project.rs`
   - [ ] Implement `ProjectInfo` struct
   - [ ] Implement `scan()` method with provider detection
   - [ ] Write unit tests for project scanning

4. **Define shared types**
   - [ ] Create `src/core/types.rs` with `BuildOutput`, `DeploymentResult`, `DeployProgress`
   - [ ] Create `src/config.rs` with `DeploymentTarget` enum and detection logic
   - [ ] Write unit tests for target detection

5. **Implement ProcessExecutor**
   - [ ] Create `src/core/process.rs`
   - [ ] Implement builder pattern
   - [ ] Implement `execute()` with full output capture
   - [ ] Implement `execute_streaming()` with line callbacks
   - [ ] Write unit tests with real commands (`echo`, `ls`)

6. **Implement ProjectScanner**
   - [ ] Create `src/core/project.rs`
   - [ ] Implement `scan()` with config file detection
   - [ ] Write unit tests with temp directories

7. **Write integration tests**
   - [ ] Create `tests/core_tests.rs`
   - [ ] Test process execution
   - [ ] Test project scanning on mock projects
   - [ ] Test provider detection with each config file type

## Implementation Notes

- Use `foundation_core::simple_http::client::SimpleHttpClient` as the HTTP base for API providers
- Follow error patterns from `foundation_ai`
- Use `derive_more` for `From` implementations
- `serde_yaml` needed for GCP `service.yaml` and AWS `template.yaml` parsing

## Success Criteria

- [ ] `cargo clippy -p foundation_deployment -- -D warnings -W clippy::pedantic` — zero warnings, zero suppression
- [ ] `cargo doc -p foundation_deployment --no-deps` — zero rustdoc warnings
- [ ] `cargo test -p foundation_deployment` — zero compilation warnings
- [ ] No `#[allow(...)]` or `#[expect(...)]` anywhere in the code
- [ ] All unit tests pass
- [ ] `DeploymentTarget::detect()` correctly identifies all 3 config file types
- [ ] `ProcessExecutor` can run and capture `echo hello`

## Verification

```bash
cd backends/foundation_deployment
cargo check
cargo clippy -- -D warnings
cargo fmt -- --check
cargo test core -- --nocapture
```

---

_Created: 2026-03-26_
