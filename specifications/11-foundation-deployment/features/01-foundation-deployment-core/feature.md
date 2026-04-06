---
workspace_name: "ewe_platform"
spec_directory: "specifications/11-foundation-deployment"
feature_directory: "specifications/11-foundation-deployment/features/01-foundation-deployment-core"
this_file: "specifications/11-foundation-deployment/features/01-foundation-deployment-core/feature.md"

status: complete
priority: high
created: 2026-03-26
completed: 2026-04-06

depends_on: []

tasks:
  completed: 7
  uncompleted: 0
  total: 7
  completion_percentage: 100%
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
        |-- shell.rs
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

    /// Rollback a failed deployment.
    ///
    /// Called when `deploy()` fails after partially completing.
    /// The provider is responsible for determining how to revert
    /// to the previous known-good state.
    ///
    /// `previous_state` contains the last known-good deployment state
    /// from the state store, if one exists.
    fn rollback(
        &self,
        config: &Self::Config,
        env: Option<&str>,
        previous_state: Option<&Self::Resources>,
    ) -> Result<(), DeploymentError>;

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

    /// Verify a deployment is healthy and responsive.
    ///
    /// Called after deploy to confirm the deployment succeeded.
    /// Returns `Ok(true)` if healthy, `Ok(false)` if not yet healthy
    /// (may be retried), or `Err(e)` if verification itself fails.
    fn verify(
        &self,
        result: &DeploymentResult,
    ) -> Result<bool, DeploymentError> {
        // Default no-op implementation — providers can override
        let _ = result;
        Ok(true)
    }
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

### Shell Executor

```rust
// core/shell.rs

use std::ffi::OsStr;
use std::path::Path;
use foundation_core::valtron::{
    execute, from_future, Stream, StreamIteratorExt, TaskIterator,
};
use serde::{Deserialize, Serialize};

/// ShellExecutor executes shell commands with streaming output.
///
/// WHY: Streaming execution must support structured progress updates via Valtron's execution engine.
///      A single execute() method provides a uniform streaming interface.
///
/// WHAT: Execute shell commands and stream basic process state changes.
///       Resource-centric events are handled at the provider level, not here.
///
/// HOW: Wraps a shell command in a TaskIterator, executes via Valtron,
///      returns a StreamIterator yielding Stream<ShellDone, ShellPending>.

// ===========================================================================
// Pending Type: Progress states during execution
// ===========================================================================

/// Progress states yielded while the shell command is running.
/// Use `Pending = ()` if you don't need progress reporting.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ShellPending {
    /// Process is spawning.
    Spawning,

    /// Process spawned successfully, waiting for output.
    Running { pid: u32 },
}

// ===========================================================================
// Done Type: Final completion states
// ===========================================================================

/// Completion states yielded when the shell command finishes.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ShellDone {
    /// Process completed successfully.
    Success {
        exit_code: i32,
        stdout: String,
        stderr: String,
    },

    /// Process failed.
    Failed {
        exit_code: Option<i32>,
        stdout: String,
        stderr: String,
    },
}

// ===========================================================================
// ShellExecutor Builder
// ===========================================================================

/// Builder for executing shell commands.
///
/// The execute() method schedules the task on Valtron's thread pool
/// and returns a DrivenStreamIterator that yields Stream<ShellDone, ShellPending>.
pub struct ShellExecutor {
    command: String,
    args: Vec<String>,
    envs: Vec<(String, String)>,
    working_dir: Option<std::path::PathBuf>,
}

impl ShellExecutor {
    pub fn new(command: &str) -> Self;
    pub fn arg<S: AsRef<OsStr>>(self, arg: S) -> Self;
    pub fn args<I, S>(self, args: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: AsRef<OsStr>;
    pub fn env<K: AsRef<OsStr>, V: AsRef<OsStr>>(self, key: K, val: V) -> Self;
    pub fn current_dir<P: AsRef<Path>>(self, dir: P) -> Self;

    /// Execute the shell command and return a StreamIterator.
    ///
    /// The returned iterator yields Stream<ShellDone, ShellPending> values:
    /// - Stream::Pending(state) - progress updates while running
    /// - Stream::Next(result) - final Success or Failed result
    ///
    /// # Returns
    ///
    /// `GenericResult<impl Iterator<Item = Stream<ShellDone, ShellPending>>>`
    /// - Ok(stream) - task was scheduled successfully
    /// - Err(e) - scheduling failed
    ///
    /// # Example: Streaming consumption
    ///
    /// ```rust
    /// use foundation_core::valtron::Stream;
    ///
    /// let stream = ShellExecutor::new("cargo")
    ///     .args(["build", "--release"])
    ///     .execute()
    ///     .expect("scheduling succeeded");
    ///
    /// for item in stream {
    ///     match item {
    ///         Stream::Pending(ShellPending::Running { pid }) => {
    ///             println!("Running with PID: {}", pid);
    ///         }
    ///         Stream::Next(ShellDone::Success { exit_code, stdout, stderr }) => {
    ///             println!("Success! Exit code: {}", exit_code);
    ///         }
    ///         Stream::Next(ShellDone::Failed { exit_code, stderr, .. }) => {
    ///             eprintln!("Failed! Exit code: {:?}", exit_code);
    ///             eprintln!("stderr: {}", stderr);
    ///         }
    ///         _ => {}
    ///     }
    /// }
    /// ```
    ///
    /// # Example: Collecting final result only
    ///
    /// ```rust
    /// use foundation_core::valtron::Stream;
    ///
    /// let stream = ShellExecutor::new("echo")
    ///     .arg("hello")
    ///     .execute()
    ///     .expect("scheduling succeeded");
    ///
    /// // Use find_map to extract the first Next value
    /// let result = stream.find_map(|s| match s {
    ///     Stream::Next(done) => Some(done),
    ///     _ => None,
    /// });
    /// ```
    pub fn execute(self) -> GenericResult<impl Iterator<Item = Stream<ShellDone, ShellPending>>> {
        // Implementation creates a TaskIterator that runs the shell command
        // and yields ShellPending/ShellDone states
        todo!()
    }
}

// ===========================================================================
// Helper: Collect all output (for simple cases)
// ===========================================================================

/// Helper function to execute and collect the final result.
/// For simple cases where you don't need streaming progress.
///
/// This is a sync convenience wrapper — it blocks until
/// the shell command completes. Use sparingly; prefer consuming the stream
/// directly when you need progress reporting or composition.
pub fn execute_and_collect(
    executor: ShellExecutor,
) -> Result<CollectedOutput, DeploymentError> {
    let stream = executor.execute().map_err(|e| {
        DeploymentError::ProcessFailed {
            command: "shell".to_string(),
            exit_code: None,
            stdout: String::new(),
            stderr: format!("scheduling failed: {e}"),
        }
    })?;

    // Use collect_one - extracts first Stream::Next value
    let result = collect_one(stream).ok_or_else(|| DeploymentError::ProcessFailed {
        command: "shell".to_string(),
        exit_code: None,
        stdout: String::new(),
        stderr: "no result from stream".to_string(),
    })?;

    match result {
        ShellDone::Success { exit_code, stdout, stderr } => {
            Ok(CollectedOutput { exit_code: Some(exit_code), stdout, stderr, success: true })
        }
        ShellDone::Failed { exit_code, stdout, stderr } => {
            Ok(CollectedOutput { exit_code, stdout, stderr, success: false })
        }
    }
}

/// Collected output from a shell command (for simple cases).
#[derive(Debug, Clone)]
pub struct CollectedOutput {
    pub exit_code: Option<i32>,
    pub stdout: String,
    pub stderr: String,
    pub success: bool,
}

// ===========================================================================
// Helper: collect_one (boundary collection)
// ===========================================================================

/// Extract the first `Stream::Next` value from a stream.
/// Returns `None` if the stream exhausts without producing a value.
///
/// Use this at sync boundaries when you need a single result.
/// For tasks that produce multiple values, use `collect_result` instead.
pub fn collect_one<D, P>(stream: impl Iterator<Item = Stream<D, P>>) -> Option<D> {
    stream.find_map(|s| match s {
        Stream::Next(v) => Some(v),
        _ => None,
    })
}

/// Drain a stream and collect all `Next` values.
/// Use this at sync boundaries when the task produces multiple values.
pub fn collect_result<D, P>(stream: impl Iterator<Item = Stream<D, P>>) -> Vec<D> {
    stream
        .filter_map(|s| match s {
            Stream::Next(v) => Some(v),
            _ => None,
        })
        .collect()
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
pub use core::shell::{ShellExecutor, ShellDone, ShellPending};
pub use core::project::{ProjectInfo, ProjectScanner};
pub use foundation_core::valtron::Stream;
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

5. **Implement ShellExecutor with Valtron StreamExecutor**
   - [ ] Create `src/core/shell.rs`
   - [ ] Define `ShellPending` enum with 2 variants: `Spawning`, `Running { pid }`
   - [ ] Define `ShellDone` enum with 2 variants: `Success { exit_code, stdout, stderr }`, `Failed { exit_code, stdout, stderr }`
   - [ ] Implement `ShellExecutor` builder pattern
   - [ ] Implement single `execute()` method returning `impl Iterator<Item = Stream<ShellDone, ShellPending>>`
   - [ ] Ensure `StreamExecutor` integrates with Valtron's `StateMachine` and `StreamIterator`
   - [ ] Write unit tests for shell execution
   - [ ] Add `execute_and_collect()` helper for simple non-streaming use cases

6. **Implement ProjectScanner**
   - [ ] Create `src/core/project.rs`
   - [ ] Implement `ProjectInfo` struct
   - [ ] Implement `scan()` method with provider detection
   - [ ] Write unit tests for project scanning

7. **Write integration tests**
   - [ ] Create `tests/core_tests.rs`
   - [ ] Test process execution
   - [ ] Test project scanning on mock projects
   - [ ] Test provider detection with each config file type

## Implementation Notes

### Valtron Async Bridge Policy (MANDATORY)

Read `fundamentals/00-overview.md` § Valtron Async Bridge Policy before implementing.

- **`run_future_iter` is the default** for all async I/O — returns composable, lazy iterators. No upfront `Vec` allocation.
- **`exec_future` only for one-shot bootstrap** — connection init, schema setup. Never in trait methods.
- **`ShellExecutor` already follows this** — returns `impl Iterator<Item = Stream<ShellDone, ShellPending>>` via Valtron's `TaskIterator` + `execute()`.
- **Error types** — `derive_more::From` + manual `Display`, central `errors.rs`. No `thiserror`.

### General

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
- [ ] `ShellExecutor` can run and capture `echo hello`

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
_Updated: 2026-04-06 - Status changed to complete, all 7/7 tasks implemented_

## Verification Notes (2026-04-06)

**Implementation Status: COMPLETE**

All 7 tasks completed:
- [x] Crate structure with all modules (`core/`, `engine/`, `providers/`, `state/`)
- [x] `DeploymentError` enum with Display/Error traits
- [x] `DeploymentProvider` trait with all required methods
- [x] Shared types (`BuildOutput`, `DeploymentResult`, `DeployProgress`, `ArtifactType`)
- [x] `ShellExecutor` with Valtron StreamExecutor integration
- [x] `ProjectScanner` with provider detection
- [x] Integration tests passing (33 tests)

**Verification Results:**
- `cargo clippy -p foundation_deployment -- -D warnings -W clippy::pedantic` — **zero warnings**
- `cargo test -p foundation_deployment` — **33 tests passed**, zero failures
- No `#[allow(...)]` or `#[expect(...)]` suppressions in code
- All clippy pedantic lints fixed including:
  - `doc_markdown`: Technical terms properly formatted with backticks
  - `must_use_candidate`: Pure functions marked appropriately
  - `too_many_lines`: Large functions split into helpers
  - `needless_pass_by_value`: References used instead of owned values
