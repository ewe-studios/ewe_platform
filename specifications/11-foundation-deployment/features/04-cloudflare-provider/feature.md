---
workspace_name: "ewe_platform"
spec_directory: "specifications/11-foundation-deployment"
feature_directory: "specifications/11-foundation-deployment/features/04-cloudflare-provider"
this_file: "specifications/11-foundation-deployment/features/04-cloudflare-provider/feature.md"

status: pending
priority: high
created: 2026-03-26

depends_on: ["01-foundation-deployment-core", "02-state-stores", "03-deployment-engine"]

tasks:
  completed: 0
  uncompleted: 7
  total: 7
  completion_percentage: 0%
---


# Cloudflare Provider

## Iron Law: Zero Warnings

> **All code must compile with zero warnings and pass all lints. No suppression. No exceptions.**
>
> - `cargo clippy -p foundation_deployment -- -D warnings -W clippy::pedantic` — zero warnings
> - `cargo doc -p foundation_deployment --no-deps` — zero rustdoc warnings
> - `cargo test -p foundation_deployment` — zero compilation warnings
> - **No `#[allow(...)]`, `#[expect(...)]`, or `#![allow(...)]` anywhere.** Fix the code, never suppress.

## Overview

Implement the Cloudflare Workers deployment provider. This provider is **API-first** — it deploys by calling the Cloudflare REST API directly via `SimpleHttpClient`, with no CLI tools required.

The provider:
- **Deploys via API** - uploads worker scripts, manages secrets, configures routes via `api.cloudflare.com/client/v4`
- **Captures state from API responses** - deployment IDs, URLs, version tags stored in state store
- **Generates `wrangler.toml` on demand** - for local dev with `wrangler dev`, not as deployment input
- **Falls back to CLI** - can optionally shell out to `wrangler` if the user prefers

## Dependencies

Depends on:
- `01-foundation-deployment-core` - `DeploymentProvider` trait, `ProcessExecutor`
- `02-state-stores` - `StateStore` for persistence
- `03-deployment-engine` - `DeploymentPlanner` for orchestration

Required by:
- `07-templates` - Cloudflare-specific template configs
- `09-examples-documentation` - Cloudflare examples

## Requirements

### wrangler.toml Config Parsing

```rust
// providers/cloudflare/config.rs

use serde::Deserialize;
use std::collections::HashMap;

/// Parsed wrangler.toml - the source of truth for Cloudflare deployments.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct WranglerConfig {
    pub name: String,
    pub main: Option<String>,
    pub compatibility_date: Option<String>,
    pub compatibility_flags: Option<Vec<String>>,
    pub workers_dev: Option<bool>,
    pub account_id: Option<String>,
    pub route: Option<String>,
    pub routes: Option<Vec<RouteConfig>>,

    pub build: Option<BuildConfig>,
    pub vars: Option<HashMap<String, String>>,
    pub env: Option<HashMap<String, WranglerEnv>>,

    pub kv_namespaces: Option<Vec<KvNamespaceConfig>>,
    pub d1_databases: Option<Vec<D1DatabaseConfig>>,
    pub r2_buckets: Option<Vec<R2BucketConfig>>,
    pub services: Option<Vec<ServiceBindingConfig>>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct BuildConfig {
    pub command: Option<String>,
    pub cwd: Option<String>,
    pub watch_dir: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct WranglerEnv {
    pub name: Option<String>,
    pub route: Option<String>,
    pub routes: Option<Vec<RouteConfig>>,
    pub vars: Option<HashMap<String, String>>,
    pub kv_namespaces: Option<Vec<KvNamespaceConfig>>,
    pub d1_databases: Option<Vec<D1DatabaseConfig>>,
    pub r2_buckets: Option<Vec<R2BucketConfig>>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct RouteConfig {
    pub pattern: String,
    pub zone_name: Option<String>,
    pub zone_id: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct KvNamespaceConfig {
    pub binding: String,
    pub id: String,
    pub preview_id: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct D1DatabaseConfig {
    pub binding: String,
    pub database_name: String,
    pub database_id: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct R2BucketConfig {
    pub binding: String,
    pub bucket_name: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ServiceBindingConfig {
    pub binding: String,
    pub service: String,
}

impl WranglerConfig {
    pub fn load(path: &Path) -> Result<Self, DeploymentError> {
        let content = std::fs::read_to_string(path)?;
        toml::from_str(&content).map_err(|e| DeploymentError::ConfigInvalid {
            file: path.display().to_string(),
            reason: e.to_string(),
        })
    }

    /// Get the effective worker name for a given environment.
    pub fn worker_name(&self, env: Option<&str>) -> String {
        match env {
            Some(env_name) => self.env.as_ref()
                .and_then(|envs| envs.get(env_name))
                .and_then(|e| e.name.clone())
                .unwrap_or_else(|| format!("{}-{}", self.name, env_name)),
            None => self.name.clone(),
        }
    }

    /// Get the effective account ID.
    pub fn effective_account_id(&self) -> Option<&str> {
        self.account_id.as_deref()
    }

    /// Validate config completeness.
    pub fn validate(&self) -> Result<(), DeploymentError> {
        if self.name.is_empty() {
            return Err(DeploymentError::ConfigInvalid {
                file: "wrangler.toml".into(),
                reason: "name is required".into(),
            });
        }
        Ok(())
    }
}
```

### CloudflareProvider Implementation

```rust
// providers/cloudflare/mod.rs

pub struct CloudflareProvider {
    /// Deployment mode: CLI (wrangler) or API (SimpleHttpClient).
    mode: CloudflareMode,
    working_dir: PathBuf,
}

pub enum CloudflareMode {
    /// Shell out to wrangler CLI.
    Cli,
    /// Use Cloudflare REST API directly.
    Api {
        api_token: String,
        account_id: String,
    },
}

impl CloudflareProvider {
    pub fn cli(working_dir: &Path) -> Self {
        Self {
            mode: CloudflareMode::Cli,
            working_dir: working_dir.to_path_buf(),
        }
    }

    pub fn api(working_dir: &Path, api_token: &str, account_id: &str) -> Self {
        Self {
            mode: CloudflareMode::Api {
                api_token: api_token.to_string(),
                account_id: account_id.to_string(),
            },
            working_dir: working_dir.to_path_buf(),
        }
    }

    /// Auto-detect mode: use API if credentials available, fall back to CLI.
    pub fn auto(working_dir: &Path) -> Self {
        match (
            std::env::var("CLOUDFLARE_API_TOKEN"),
            std::env::var("CLOUDFLARE_ACCOUNT_ID"),
        ) {
            (Ok(token), Ok(account)) => Self::api(working_dir, &token, &account),
            _ => Self::cli(working_dir),
        }
    }
}

impl DeploymentProvider for CloudflareProvider {
    type Config = WranglerConfig;
    type Resources = CloudflareResources;

    fn name(&self) -> &str { "cloudflare" }

    fn detect(project_dir: &Path) -> Option<WranglerConfig> {
        let config_path = project_dir.join("wrangler.toml");
        WranglerConfig::load(&config_path).ok()
    }

    fn validate(&self, config: &WranglerConfig) -> Result<(), DeploymentError> {
        config.validate()
    }

    fn build(&self, config: &WranglerConfig, env: Option<&str>) -> Result<BuildOutput, DeploymentError> {
        if let Some(build_config) = &config.build {
            if let Some(command) = &build_config.command {
                let output = ProcessExecutor::new("sh")
                    .args(["-c", command])
                    .current_dir(&self.working_dir)
                    .execute()?;
                if !output.success {
                    return Err(DeploymentError::BuildFailed(output.stderr));
                }
            }
        }
        Ok(BuildOutput {
            artifacts: vec![],
            duration_ms: 0,
        })
    }

    fn deploy(
        &self,
        config: &WranglerConfig,
        env: Option<&str>,
        dry_run: bool,
    ) -> Result<DeploymentResult, DeploymentError> {
        match &self.mode {
            CloudflareMode::Cli => self.deploy_cli(config, env, dry_run),
            CloudflareMode::Api { api_token, account_id } => {
                self.deploy_api(config, env, dry_run, api_token, account_id)
            }
        }
    }

    fn logs(&self, config: &WranglerConfig, env: Option<&str>) -> Result<(), DeploymentError> {
        let mut cmd = ProcessExecutor::new("wrangler").arg("tail");
        if let Some(env_name) = env {
            cmd = cmd.args(["--env", env_name]);
        }
        cmd.current_dir(&self.working_dir).execute_streaming(|line| {
            println!("{}", line);
        })?;
        Ok(())
    }

    fn destroy(&self, config: &WranglerConfig, env: Option<&str>) -> Result<(), DeploymentError> {
        match &self.mode {
            CloudflareMode::Cli => {
                let mut cmd = ProcessExecutor::new("wrangler").arg("delete");
                if let Some(env_name) = env {
                    cmd = cmd.args(["--env", env_name]);
                }
                cmd.current_dir(&self.working_dir).execute()?;
                Ok(())
            }
            CloudflareMode::Api { api_token, account_id } => {
                self.delete_worker_api(config, env, api_token, account_id)
            }
        }
    }

    fn status(&self, config: &WranglerConfig, env: Option<&str>) -> Result<CloudflareResources, DeploymentError> {
        // Query Cloudflare API for worker status, routes, bindings
        todo!()
    }
}
```

### Wrangler CLI Wrapper

```rust
// providers/cloudflare/wrangler.rs

impl CloudflareProvider {
    fn deploy_cli(
        &self,
        config: &WranglerConfig,
        env: Option<&str>,
        dry_run: bool,
    ) -> Result<DeploymentResult, DeploymentError> {
        let mut cmd = ProcessExecutor::new("wrangler").arg("deploy");
        if let Some(env_name) = env {
            cmd = cmd.args(["--env", env_name]);
        }
        if dry_run {
            cmd = cmd.arg("--dry-run");
        }
        let output = cmd.current_dir(&self.working_dir).execute()?;
        if !output.success {
            return Err(DeploymentError::ProcessFailed {
                command: "wrangler deploy".into(),
                exit_code: output.exit_code,
                stdout: output.stdout,
                stderr: output.stderr,
            });
        }
        parse_wrangler_deploy_output(&output.stdout, &output.stderr, config, env)
    }
}

/// Parse wrangler deploy output to extract deployment details.
fn parse_wrangler_deploy_output(
    stdout: &str,
    stderr: &str,
    config: &WranglerConfig,
    env: Option<&str>,
) -> Result<DeploymentResult, DeploymentError> {
    // Extract deployment ID, URL from wrangler output
    // Format varies by wrangler version; use regex matching
    let url = extract_worker_url(stdout);
    let deployment_id = extract_deployment_id(stdout)
        .unwrap_or_else(|| chrono::Utc::now().timestamp().to_string());

    Ok(DeploymentResult {
        deployment_id,
        provider: "cloudflare".to_string(),
        resource_name: config.worker_name(env),
        environment: env.map(String::from),
        url,
        deployed_at: chrono::Utc::now(),
    })
}
```

### Cloudflare REST API Client

```rust
// providers/cloudflare/api.rs

use foundation_core::simple_http::client::SimpleHttpClient;

const CF_API_BASE: &str = "https://api.cloudflare.com/client/v4";

impl CloudflareProvider {
    fn deploy_api(
        &self,
        config: &WranglerConfig,
        env: Option<&str>,
        dry_run: bool,
        api_token: &str,
        account_id: &str,
    ) -> Result<DeploymentResult, DeploymentError> {
        if dry_run {
            return Ok(DeploymentResult::dry_run("cloudflare", &config.worker_name(env)));
        }

        let worker_name = config.worker_name(env);
        let url = format!(
            "{}/accounts/{}/workers/scripts/{}",
            CF_API_BASE, account_id, worker_name
        );

        // Read built script from build output path
        let script_path = config.main.as_deref().unwrap_or("build/worker/shim.mjs");
        let script = std::fs::read(self.working_dir.join(script_path))?;

        // Upload worker script via PUT
        // Uses SimpleHttpClient with multipart/form-data for metadata + script
        // ...

        Ok(DeploymentResult {
            deployment_id: "api-deploy".to_string(),
            provider: "cloudflare".to_string(),
            resource_name: worker_name,
            environment: env.map(String::from),
            url: Some(format!("https://{}.workers.dev", config.name)),
            deployed_at: chrono::Utc::now(),
        })
    }

    fn delete_worker_api(
        &self,
        config: &WranglerConfig,
        env: Option<&str>,
        api_token: &str,
        account_id: &str,
    ) -> Result<(), DeploymentError> {
        let worker_name = config.worker_name(env);
        let url = format!(
            "{}/accounts/{}/workers/scripts/{}",
            CF_API_BASE, account_id, worker_name
        );
        // DELETE request via SimpleHttpClient
        Ok(())
    }
}

/// Cloudflare API response wrapper.
#[derive(Debug, Deserialize)]
pub struct CfApiResponse<T> {
    pub success: bool,
    pub errors: Vec<CfApiError>,
    pub messages: Vec<String>,
    pub result: Option<T>,
}

#[derive(Debug, Deserialize)]
pub struct CfApiError {
    pub code: u32,
    pub message: String,
}

#[derive(Debug)]
pub struct CloudflareResources {
    pub worker_name: String,
    pub routes: Vec<String>,
    pub kv_namespaces: Vec<String>,
    pub d1_databases: Vec<String>,
    pub r2_buckets: Vec<String>,
    pub secrets: Vec<String>,
}
```

## Tasks

1. **Create module structure**
   - [ ] Create `src/providers/cloudflare/mod.rs`, `config.rs`, `wrangler.rs`, `api.rs`
   - [ ] Register in `src/providers/mod.rs`
   - [ ] Export from `src/lib.rs`

2. **Implement wrangler.toml parsing**
   - [ ] Define all config structs
   - [ ] Implement `WranglerConfig::load()`
   - [ ] Implement `worker_name()`, `effective_account_id()`
   - [ ] Implement `validate()`
   - [ ] Write unit tests with sample wrangler.toml files

3. **Implement CloudflareProvider trait**
   - [ ] Implement `detect()` - find and parse wrangler.toml
   - [ ] Implement `validate()` - check config completeness
   - [ ] Implement `build()` - run build command from config
   - [ ] Implement `deploy()` - dispatch to CLI or API mode
   - [ ] Implement `logs()`, `destroy()`, `status()`

4. **Implement CLI mode (wrangler wrapper)**
   - [ ] Implement `deploy_cli()` with output parsing
   - [ ] Implement wrangler deploy output parser (regex)
   - [ ] Handle `--env`, `--dry-run` flags
   - [ ] Write unit tests with mock output

5. **Implement API mode**
   - [ ] Implement `deploy_api()` using SimpleHttpClient
   - [ ] Handle multipart upload for worker scripts
   - [ ] Implement `delete_worker_api()`
   - [ ] Parse `CfApiResponse` wrapper
   - [ ] Write tests with mock HTTP server

6. **Secret management**
   - [ ] Implement `put_secret()` (CLI: `wrangler secret put`, API: PUT secrets endpoint)
   - [ ] Implement `list_secrets()`
   - [ ] Implement `delete_secret()`

7. **Write integration tests**
   - [ ] Test wrangler.toml parsing with real config files
   - [ ] Test CLI deploy (requires wrangler installed, mark `#[ignore]`)
   - [ ] Test API deploy (requires CF credentials, mark `#[ignore]`)

## Cloudflare API Endpoints Used

| Method | Endpoint | Purpose |
|--------|----------|---------|
| `PUT` | `/accounts/{id}/workers/scripts/{name}` | Upload worker |
| `DELETE` | `/accounts/{id}/workers/scripts/{name}` | Delete worker |
| `GET` | `/accounts/{id}/workers/scripts` | List workers |
| `PUT` | `/accounts/{id}/workers/scripts/{name}/secrets` | Put secret |
| `GET` | `/accounts/{id}/workers/scripts/{name}/secrets` | List secrets |

## Success Criteria

- [ ] All 7 tasks completed
- [ ] `cargo clippy -p foundation_deployment -- -D warnings -W clippy::pedantic` — zero warnings, zero suppression
- [ ] `cargo doc -p foundation_deployment --no-deps` — zero rustdoc warnings
- [ ] No `#[allow(...)]` or `#[expect(...)]` anywhere in the code
- [ ] wrangler.toml parsing handles all common configurations
- [ ] CLI mode deploys successfully
- [ ] API mode deploys successfully
- [ ] Environment support works (staging, production)

## Verification

```bash
cd backends/foundation_deployment
cargo test cloudflare -- --nocapture

# Integration (requires wrangler + CF account)
cargo test cloudflare_integration -- --ignored --nocapture
```

---

_Created: 2026-03-26_
