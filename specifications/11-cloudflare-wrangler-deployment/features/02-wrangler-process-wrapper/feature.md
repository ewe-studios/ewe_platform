---
workspace_name: "ewe_platform"
spec_directory: "specifications/11-cloudflare-wrangler-deployment"
feature_directory: "specifications/11-cloudflare-wrangler-deployment/features/02-wrangler-process-wrapper"
this_file: "specifications/11-cloudflare-wrangler-deployment/features/02-wrangler-process-wrapper/feature.md"

status: pending
priority: high
created: 2026-03-26

depends_on: ["01-foundation-deployment-crate"]

tasks:
  completed: 0
  uncompleted: 6
  total: 6
  completion_percentage: 0%
---


# Wrangler Process Wrapper

## Overview

Build a comprehensive wrapper around the wrangler CLI tool, providing type-safe command builders, output parsing, and error handling for all common wrangler operations.

## Dependencies

This feature depends on:
- `01-foundation-deployment-crate` - Uses `ProcessExecutor` and `DeploymentError`

This feature is required by:
- `07-mise-integration` - Uses wrangler commands in mise tasks
- `08-examples-documentation` - Examples use wrangler wrapper

## Requirements

### Wrangler Command Builder

```rust
// wrangler/commands.rs

/// Wrangler subcommands
#[derive(Debug, Clone)]
pub enum WranglerCommand {
    /// wrangler deploy
    Deploy {
        env: Option<String>,
        dry_run: bool,
        verbose: bool,
    },

    /// wrangler dev
    Dev {
        local: bool,
        port: Option<u16>,
        ip: Option<String>,
    },

    /// wrangler tail
    Tail {
        env: Option<String>,
        status: Option<String>,
        header: Option<String>,
    },

    /// wrangler secret put <key>
    SecretPut {
        key: String,
        env: Option<String>,
    },

    /// wrangler secret list
    SecretList {
        env: Option<String>,
    },

    /// wrangler kv:key put <key> <value>
    KvPut {
        namespace_id: String,
        key: String,
        value: String,
        expiration_ttl: Option<u64>,
    },

    /// wrangler kv:key get <key>
    KvGet {
        namespace_id: String,
        key: String,
    },

    /// wrangler whoami
    Whoami,

    /// Custom command
    Custom(Vec<String>),
}

impl WranglerCommand {
    /// Convert command to wrangler CLI arguments
    pub fn to_args(&self) -> Vec<String>;

    /// Execute command and return output
    pub fn execute(&self, working_dir: &Path) -> Result<ProcessOutput, DeploymentError>;

    /// Execute with streaming output
    pub fn execute_streaming<F>(
        &self,
        working_dir: &Path,
        on_line: F,
    ) -> Result<ProcessOutput, DeploymentError>
    where
        F: FnMut(&str);
}
```

### Wrangler Runner

```rust
// wrangler/mod.rs

/// Wrangler version information
#[derive(Debug, Clone)]
pub struct WranglerVersion {
    pub major: u32,
    pub minor: u32,
    pub patch: u32,
}

/// Wrangler runner - high-level API for wrangler operations
pub struct WranglerRunner {
    working_dir: PathBuf,
    version: Option<WranglerVersion>,
}

impl WranglerRunner {
    /// Create new runner with working directory
    pub fn new(working_dir: &Path) -> Self;

    /// Check if wrangler is installed
    pub fn is_installed() -> bool;

    /// Get wrangler version
    pub fn version(&mut self) -> Result<WranglerVersion, DeploymentError>;

    /// Deploy worker
    pub fn deploy(&self, env: Option<&str>, dry_run: bool) -> Result<DeployOutput, DeploymentError>;

    /// Start dev server
    pub fn dev(&self, local: bool, port: u16) -> Result<(), DeploymentError>;

    /// Tail worker logs
    pub fn tail(&self, env: Option<&str>) -> Result<(), DeploymentError>;

    /// Put secret
    pub fn secret_put(&self, key: &str, value: &str, env: Option<&str>) -> Result<(), DeploymentError>;

    /// List secrets
    pub fn secret_list(&self, env: Option<&str>) -> Result<Vec<SecretInfo>, DeploymentError>;

    /// Put KV value
    pub fn kv_put(&self, namespace: &str, key: &str, value: &str) -> Result<(), DeploymentError>;

    /// Get KV value
    pub fn kv_get(&self, namespace: &str, key: &str) -> Result<Option<String>, DeploymentError>;

    /// Get account info
    pub fn whoami(&self) -> Result<AccountInfo, DeploymentError>;
}
```

### Deploy Output Parsing

```rust
/// Output from wrangler deploy
#[derive(Debug)]
pub struct DeployOutput {
    pub deployment_id: String,
    pub worker_name: String,
    pub url: Option<String>,
    pub uploaded_at: chrono::DateTime<chrono::Utc>,
    pub resources: DeployedResources,
}

#[derive(Debug)]
pub struct DeployedResources {
    pub bindings: Vec<Binding>,
    pub routes: Vec<String>,
    pub triggers: Vec<Trigger>,
}

/// Parse wrangler deploy output
pub fn parse_deploy_output(stdout: &str, stderr: &str) -> Result<DeployOutput, DeploymentError> {
    // Example wrangler output:
    // "Deploying worker: my-worker"
    // "Upload complete"
    // "Deployed my-worker triggered by http://*.example.com"
    // "Deployment ID: abc123..."

    // Extract deployment ID, worker name, URL from output
}
```

### Wrangler Configuration

```rust
// wrangler/config.rs

/// Parsed wrangler.toml with additional helpers
#[derive(Debug, Clone, Deserialize)]
pub struct WranglerToml {
    pub name: String,
    pub main: Option<String>,
    pub compatibility_date: Option<String>,
    pub compatibility_flags: Option<Vec<String>>,
    pub workers_dev: Option<bool>,
    pub route: Option<String>,
    pub routes: Option<Vec<RouteConfig>>,
    pub account_id: Option<String>,
    pub zone_id: Option<String>,
    pub env: Option<HashMap<String, WranglerEnv>>,
    pub vars: Option<HashMap<String, String>>,
    pub kv_namespaces: Option<Vec<KvNamespaceConfig>>,
    pub d1_databases: Option<Vec<D1DatabaseConfig>>,
    pub r2_buckets: Option<Vec<R2BucketConfig>>,
}

impl WranglerToml {
    /// Load from file
    pub fn load(path: &Path) -> Result<Self, DeploymentError>;

    /// Get effective account ID (from env or root)
    pub fn account_id(&self, env: Option<&str>) -> Option<String>;

    /// Get effective worker name (from env or root)
    pub fn worker_name(&self, env: Option<&str>) -> String;

    /// Validate configuration
    pub fn validate(&self) -> Result<(), DeploymentError>;
}
```

### Version Detection

```rust
impl WranglerRunner {
    /// Parse wrangler version string
    fn parse_version(output: &str) -> Result<WranglerVersion, DeploymentError> {
        // Output format: "wrangler/3.0.0" or "3.0.0"
        // Extract version numbers
    }

    /// Check minimum required version
    pub fn check_min_version(required: WranglerVersion) -> Result<bool, DeploymentError> {
        let current = Self::version()?;
        Ok(current >= required)
    }
}
```

## Tasks

1. **Create wrangler module structure**
   - [ ] Create `src/wrangler/mod.rs`
   - [ ] Create `src/wrangler/commands.rs`
   - [ ] Create `src/wrangler/config.rs`
   - [ ] Export from `src/lib.rs`

2. **Implement command builder**
   - [ ] Define `WranglerCommand` enum with all variants
   - [ ] Implement `to_args()` for each command
   - [ ] Implement `execute()` using `ProcessExecutor`
   - [ ] Write unit tests for argument generation

3. **Implement wrangler runner**
   - [ ] Define `WranglerRunner` struct
   - [ ] Implement `is_installed()` using `which` crate
   - [ ] Implement `version()` with parsing
   - [ ] Implement all command methods (deploy, dev, tail, etc.)
   - [ ] Write integration tests (requires wrangler installed)

4. **Implement output parsing**
   - [ ] Define `DeployOutput`, `DeployedResources` structs
   - [ ] Implement `parse_deploy_output()` function
   - [ ] Handle various wrangler output formats
   - [ ] Write unit tests with sample outputs

5. **Implement wrangler.toml parsing**
   - [ ] Define all config structs (`WranglerToml`, `RouteConfig`, etc.)
   - [ ] Implement `load()` method
   - [ ] Implement helper methods (account_id, worker_name)
   - [ ] Write unit tests with sample config files

6. **Write end-to-end tests**
   - [ ] Test full deploy flow (requires Cloudflare account)
   - [ ] Test secret management
   - [ ] Test KV operations
   - [ ] Mark integration tests with `#[ignore]`

## Implementation Notes

- Use regex for parsing wrangler output
- `which` crate for finding wrangler in PATH
- Handle both wrangler v2 and v3 output formats
- Consider using `semver` crate for version comparison

## Success Criteria

- [ ] All 6 tasks completed
- [ ] `cargo clippy -- -D warnings` passes
- [ ] All unit tests pass
- [ ] Can deploy a worker using `WranglerRunner::deploy()`
- [ ] Can parse wrangler.toml correctly
- [ ] Version detection works for installed wrangler

## Verification

```bash
# Check wrangler is installed
which wrangler
wrangler --version

# Run tests
cd backends/foundation_deployment
cargo test wrangler -- --nocapture

# Test command builder
cargo test command_args -- --nocapture
```

---

_Created: 2026-03-26_
