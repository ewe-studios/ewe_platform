---
workspace_name: "ewe_platform"
spec_directory: "specifications/11-foundation-deployment"
feature_directory: "specifications/11-foundation-deployment/features/07-cloudflare-api-provider"
this_file: "specifications/11-foundation-deployment/features/07-cloudflare-api-provider/feature.md"

status: pending
priority: high
created: 2026-04-06

depends_on: ["01-foundation-deployment-core", "02-state-stores", "03-deployment-engine", "04-cloudflare-cli-provider"]

tasks:
  completed: 0
  uncompleted: 10
  total: 10
  completion_percentage: 0%
---


# Cloudflare API Provider (API-First)

## Iron Law: Zero Warnings

> **All code must compile with zero warnings and pass all lints. No suppression. No exceptions.**
>
> - `cargo clippy -p foundation_deployment -- -D warnings -W clippy::pedantic` — zero warnings
> - `cargo doc -p foundation_deployment --no-deps` — zero rustdoc warnings
> - `cargo test -p foundation_deployment` — zero compilation warnings
> - **No `#[allow(...)]`, `#[expect(...)]`, or `#![allow(...)]` anywhere.** Fix the code, never suppress.

## Overview

Implement the **API-first** Cloudflare Workers deployment provider. This provider calls the Cloudflare REST API directly via `SimpleHttpClient` with Bearer token authentication — **no wrangler CLI dependency**.

This is distinct from `04-cloudflare-provider` which is CLI-based (wrangler wrapper). The API provider:

- **Deploys via REST API only** - uploads worker scripts via `PUT /accounts/{id}/workers/scripts/{name}`
- **Uses multipart/form-data** - for script uploads with bindings metadata
- **Bearer token auth** - from `CLOUDFLARE_API_TOKEN` environment variable
- **Account ID required** - from `CLOUDFLARE_ACCOUNT_ID` environment variable
- **No CLI fallback** - this provider is API-only; use `04-cloudflare-provider` for CLI mode
- **Full binding support** - KV namespaces, D1 databases, R2 buckets, Queues, Service bindings, Secrets

### Relationship to CLI Provider

| Feature | `04-cloudflare-provider` (CLI) | `04-cloudflare-api-provider` (API-First) |
|---------|--------------------------------|------------------------------------------|
| Dependency | Requires `wrangler` CLI installed | No external dependencies |
| Auth | Wrangler handles auth | Bearer token + Account ID |
| Upload | `wrangler deploy` command | Direct REST API PUT |
| Bindings | Wrangler parses wrangler.toml | Manual multipart construction |
| Use Case | Local dev, quick iteration | CI/CD, production automation |

## Dependencies

Depends on:
- `01-foundation-deployment-core` - `DeploymentProvider` trait, `ShellExecutor`
- `02-state-stores` - `StateStore` for persistence
- `03-deployment-engine` - `DeploymentPlanner` for orchestration
- `04-cloudflare-provider` - Reference for config parsing (`WranglerConfig`)
- **Existing `providers/cloudflare/clients/`** - Reuse generated API client types and HTTP client wrappers
- **Existing `providers/cloudflare/resources/`** - Reuse generated resource types from OpenAPI spec

Required by:
- `07-templates` - Cloudflare-specific template configs
- `09-examples-documentation` - Cloudflare examples

## Architecture

The API provider implementation should:

1. **Reuse existing generated clients** in `backends/foundation_deployment/src/providers/cloudflare/clients/`:
   - `types.rs` - `ApiErrorDetails`, `ApiErrorBody`, `ApiResponse<T>`, `ApiError`
   - Auto-generated client functions for each Cloudflare API endpoint

2. **Reuse existing generated resources** in `backends/foundation_deployment/src/providers/cloudflare/resources/`:
   - All generated resource types from the Cloudflare OpenAPI spec
   - Types already have `Serialize`, `Deserialize`, `Debug`, `Clone` derives

3. **Implement the API provider** in `backends/foundation_deployment/src/providers/cloudflare/api/`:
   - `mod.rs` - Module declaration and `CloudflareApiProvider` struct
   - `auth.rs` - `CloudflareAuth` with Bearer token handling
   - `error.rs` - `CloudflareApiError` wrapping the generated `ApiError`
   - `provider.rs` - `DeploymentProvider` trait implementation

## Requirements

### Module Structure

```rust
// providers/cloudflare/api/mod.rs

pub mod auth;
pub mod error;
pub mod provider;

pub use auth::CloudflareAuth;
pub use error::CloudflareApiError;
pub use provider::CloudflareApiProvider;
```

**Note:** The implementation should reuse:
- `super::clients::types` - Generated API types (`ApiError`, `ApiResponse`, etc.)
- `super::clients::*` - Generated client functions for API calls
- `super::resources::*` - Generated resource types

### Authentication

```rust
// providers/cloudflare/api/auth.rs

use foundation_core::simple_http::client::SimpleHttpClient;

/// Cloudflare API authentication using Bearer token.
///
/// Required environment variables:
/// - `CLOUDFLARE_API_TOKEN` - API token with Worker permissions
/// - `CLOUDFLARE_ACCOUNT_ID` - Account ID (32-char hex string)
///
/// Token permissions required:
/// - `Workers:Write` - Deploy worker scripts
/// - `Workers KV:Write` - Manage KV namespaces
/// - `D1:Write` - Manage D1 databases
/// - `R2:Write` - Manage R2 buckets
/// - `Queues:Write` - Manage queues
#[derive(Debug, Clone)]
pub struct CloudflareAuth {
    pub api_token: String,
    pub account_id: String,
}

impl CloudflareAuth {
    /// Create from environment variables.
    /// Returns `None` if either variable is missing.
    pub fn from_env() -> Option<Self> {
        let api_token = std::env::var("CLOUDFLARE_API_TOKEN").ok()?;
        let account_id = std::env::var("CLOUDFLARE_ACCOUNT_ID").ok()?;
        Some(Self { api_token, account_id })
    }

    /// Create from explicit values.
    pub fn new(api_token: &str, account_id: &str) -> Self {
        Self {
            api_token: api_token.to_string(),
            account_id: account_id.to_string(),
        }
    }

    /// Configure HTTP client with Bearer token auth.
    pub fn configure_client(&self, client: &mut SimpleHttpClient) {
        client.add_header(
            "Authorization".to_string(),
            format!("Bearer {}", self.api_token),
        );
        client.add_header(
            "Content-Type".to_string(),
            "application/json".to_string(),
        );
    }

    /// Validate token format (basic check).
    pub fn validate(&self) -> Result<(), CloudflareApiError> {
        if self.api_token.is_empty() {
            return Err(CloudflareApiError::InvalidToken);
        }
        if self.account_id.is_empty() || self.account_id.len() != 32 {
            return Err(CloudflareApiError::InvalidAccountId);
        }
        Ok(())
    }
}
```

### API Client Integration

**Use existing generated clients** in `providers/cloudflare/clients/`:

The code generator (`gen_provider_clients`) has already generated client functions for Cloudflare API endpoints. The API provider should:

1. **Import generated types and functions:**
```rust
use crate::providers::cloudflare::clients::types::{ApiError, ApiResponse};
// Generated client functions for each endpoint
use crate::providers::cloudflare::clients::workers::{
    put_worker_script,
    get_worker_script,
    delete_worker_script,
    // ... more functions
};
```

2. **Use generated resource types:**
```rust
use crate::providers::cloudflare::resources::{
    WorkerScript, KvNamespace, D1Database, R2Bucket, Queue,
    // ... more resource types
};
```

3. **Wrap with provider-specific auth and error handling:**
```rust
// providers/cloudflare/api/provider.rs

use super::auth::CloudflareAuth;
use super::error::CloudflareApiError;
use crate::providers::cloudflare::clients::types::{ApiError, ApiResponse};
use crate::providers::cloudflare::resources::*;
use foundation_core::simple_http::client::SimpleHttpClient;

/// Cloudflare REST API provider using SimpleHttpClient.
/// All methods are API-only — no CLI fallback.
pub struct CloudflareApiProvider {
    client: SimpleHttpClient,
    auth: CloudflareAuth,
    account_id: String,
}

impl CloudflareApiProvider {
    pub fn new(auth: CloudflareAuth) -> Result<Self, CloudflareApiError> {
        let mut client = SimpleHttpClient::new();
        auth.configure_client(&mut client);
        Ok(Self {
            client,
            account_id: auth.account_id.clone(),
            auth,
        })
    }
    
    // Delegate to generated client functions with auth/account_id
}
```

### Error Types

**Reuse generated `ApiError`** from `providers/cloudflare/clients/types.rs` and add provider-specific wrapping:

```rust
// providers/cloudflare/api/error.rs

use crate::providers::cloudflare::clients::types::ApiError;

/// Cloudflare API-specific error wrapper.
#[derive(Debug)]
pub enum CloudflareApiError {
    /// Invalid API token format or missing.
    InvalidToken,

    /// Invalid account ID format (expected 32-char hex).
    InvalidAccountId,

    /// API error from generated client.
    ApiError(ApiError),

    /// Resource not found.
    NotFound(String),

    /// Permission denied (insufficient token scopes).
    PermissionDenied(String),

    /// Rate limit exceeded.
    RateLimited { retry_after: Option<u64> },
}

impl std::error::Error for CloudflareApiError {}

impl std::fmt::Display for CloudflareApiError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidToken => write!(f, "Invalid Cloudflare API token"),
            Self::InvalidAccountId => write!(f, "Invalid Cloudflare Account ID (expected 32-char hex)"),
            Self::ApiError(e) => write!(f, "Cloudflare API error: {}", e),
            Self::NotFound(resource) => write!(f, "Resource not found: {}", resource),
            Self::PermissionDenied(reason) => write!(f, "Permission denied: {}", reason),
            Self::RateLimited { retry_after } => {
                write!(f, "Rate limit exceeded")?;
                if let Some(secs) = retry_after {
                    write!(f, ", retry after {}s", secs)?;
                }
                Ok(())
            }
        }
    }
}

impl From<ApiError> for CloudflareApiError {
    fn from(e: ApiError) -> Self {
        Self::ApiError(e)
    }
}

impl From<serde_json::Error> for CloudflareApiError {
    fn from(e: serde_json::Error) -> Self {
        Self::ApiError(ApiError::ParseFailed(e.to_string()))
    }
}
```

### CloudflareApiProvider Implementation

```rust
// providers/cloudflare/api/provider.rs

use std::path::{Path, PathBuf};
use crate::core::traits::DeploymentProvider;
use crate::core::types::{BuildOutput, DeploymentResult};
use crate::error::DeploymentError;
use super::auth::CloudflareAuth;
use super::error::CloudflareApiError;
use crate::providers::cloudflare::WranglerConfig; // Reuse config from CLI provider

pub struct CloudflareApiProvider {
    working_dir: PathBuf,
    auth: CloudflareAuth,
}

impl CloudflareApiProvider {
    /// Create new API provider from explicit credentials.
    pub fn new(working_dir: &Path, api_token: &str, account_id: &str) -> Result<Self, CloudflareApiError> {
        let auth = CloudflareAuth::new(api_token, account_id);
        Ok(Self {
            working_dir: working_dir.to_path_buf(),
            auth,
        })
    }

    /// Create from environment variables.
    pub fn from_env(working_dir: &Path) -> Option<Self> {
        let auth = CloudflareAuth::from_env()?;
        Some(Self {
            working_dir: working_dir.to_path_buf(),
            auth,
        })
    }

    /// Detect Cloudflare project by finding wrangler.toml.
    pub fn detect(project_dir: &Path) -> Option<WranglerConfig> {
        let config_path = project_dir.join("wrangler.toml");
        WranglerConfig::load(&config_path).ok()
    }
}

impl DeploymentProvider for CloudflareApiProvider {
    type Config = WranglerConfig;
    type Resources = CloudflareApiResources;

    fn name(&self) -> &str {
        "cloudflare-api"
    }

    fn detect(project_dir: &Path) -> Option<Self::Config> {
        Self::detect(project_dir)
    }

    fn validate(&self, config: &Self::Config) -> Result<(), DeploymentError> {
        config.validate()?;
        self.auth.validate().map_err(|e| {
            DeploymentError::Cloudflare {
                status: 0,
                message: e.to_string(),
                error_code: None,
            }
        })
    }

    fn build(&self, config: &Self::Config, _env: Option<&str>) -> Result<BuildOutput, DeploymentError> {
        // Execute build command from wrangler.toml if present
        if let Some(build_config) = &config.build {
            if let Some(command) = &build_config.command {
                use crate::core::shell::{execute_and_collect, ShellExecutor};
                let output = execute_and_collect(
                    ShellExecutor::new("sh")
                        .args(["-c", command])
                        .current_dir(&self.working_dir)
                )?;

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
        config: &Self::Config,
        env: Option<&str>,
        dry_run: bool,
    ) -> Result<DeploymentResult, DeploymentError> {
        if dry_run {
            return Ok(DeploymentResult::dry_run("cloudflare-api", &config.worker_name(env)));
        }

        let worker_name = config.worker_name(env);
        let script_path = config.main.as_deref().unwrap_or("src/index.ts");
        let script_content = std::fs::read(self.working_dir.join(script_path))
            .map_err(|e| DeploymentError::IoError(e))?;

        // Use generated client function to deploy
        // let result = clients::workers::put_worker_script(...)?;

        Ok(DeploymentResult {
            deployment_id: result.version.unwrap_or_default(),
            provider: "cloudflare-api".to_string(),
            resource_name: worker_name,
            environment: env.map(String::from),
            url: Some(format!("https://{}.workers.dev", worker_name)),
            deployed_at: chrono::Utc::now(),
        })
    }

    fn rollback(
        &self,
        config: &Self::Config,
        env: Option<&str>,
        previous_state: Option<&Self::Resources>,
    ) -> Result<(), DeploymentError> {
        match previous_state {
            Some(prev) => {
                // Redeploy previous version by uploading previous script content
                todo!("Implement version rollback")
            }
            None => self.destroy(config, env),
        }
    }

    fn logs(&self, config: &Self::Config, env: Option<&str>) -> Result<(), DeploymentError> {
        // Cloudflare doesn't have a streaming logs API
        // Use Cloudflare Logpush or Workers Analytics Engine instead
        Err(DeploymentError::ConfigInvalid {
            file: "logs".into(),
            reason: "Cloudflare API provider does not support streaming logs. Use Cloudflare Dashboard or Logpush instead.".into(),
        })
    }

    fn destroy(&self, config: &Self::Config, env: Option<&str>) -> Result<(), DeploymentError> {
        let worker_name = config.worker_name(env);
        // Use generated client function: clients::workers::delete_worker_script(...)
        Ok(())
    }

    fn status(&self, config: &Self::Config, env: Option<&str>) -> Result<Self::Resources, DeploymentError> {
        let worker_name = config.worker_name(env);
        // Use generated client function: clients::workers::get_worker_script(...)
        Ok(CloudflareApiResources {
            worker_name,
            version: script.version,
            bindings: self.extract_binding_names(&script.bindings),
            routes: vec![],
        })
    }

    fn verify(&self, result: &DeploymentResult) -> Result<bool, DeploymentError> {
        if let Some(url) = &result.url {
            use foundation_core::simple_http::client::SimpleHttpClient;
            let client = SimpleHttpClient::new();
            match client.get(url) {
                Ok(resp) if resp.status().is_success() => Ok(true),
                Ok(resp) if resp.status() == 404 => Ok(false),
                Ok(resp) => Err(DeploymentError::HttpError(
                    foundation_core::simple_http::client::HttpClientError::UnexpectedStatus(resp.status())
                )),
                Err(e) => Err(DeploymentError::HttpError(e)),
            }
        } else {
            Ok(true)
        }
    }
}
```

## Tasks

1. **Review existing generated code**
   - [ ] Review `providers/cloudflare/clients/types.rs` - generated API types
   - [ ] Review `providers/cloudflare/clients/*.rs` - generated client functions
   - [ ] Review `providers/cloudflare/resources/*.rs` - generated resource types
   - [ ] Identify gaps: what endpoints need to be generated for full provider support

2. **Create module structure**
   - [ ] Create `src/providers/cloudflare/api/mod.rs`, `auth.rs`, `error.rs`, `provider.rs`
   - [ ] Register in `src/providers/cloudflare/mod.rs`
   - [ ] Add to feature flags (separate from CLI provider)

3. **Implement authentication**
   - [ ] Implement `CloudflareAuth` struct
   - [ ] Implement `from_env()` and `new()` constructors
   - [ ] Implement `configure_client()` for Bearer token setup
   - [ ] Implement `validate()` for credential validation

4. **Implement error types**
   - [ ] Define `CloudflareApiError` wrapping generated `ApiError`
   - [ ] Implement `Display` and `Error` traits
   - [ ] Implement `From` conversions for `ApiError` and `serde_json::Error`
   - [ ] Handle rate limiting with retry-after

5. **Implement CloudflareApiProvider trait**
   - [ ] Implement `detect()` - find wrangler.toml
   - [ ] Implement `validate()` - validate config + credentials
   - [ ] Implement `build()` - run build command
   - [ ] Implement `deploy()` - upload via API using generated client functions
   - [ ] Implement `destroy()` - DELETE worker script
   - [ ] Implement `status()` - GET worker info
   - [ ] Implement `rollback()` - re-upload previous version
   - [ ] Implement `verify()` - HTTP health check

6. **Implement binding management**
   - [ ] Parse bindings from `WranglerConfig`
   - [ ] Convert to API binding types (use generated types)
   - [ ] Handle KV, D1, R2, Queue, Service bindings
   - [ ] Handle secrets vs plain text variables

7. **Implement secrets management**
   - [ ] Implement `put_secret()` via generated client
   - [ ] Implement `list_secrets()` via generated client
   - [ ] Implement `delete_secret()` via generated client
   - [ ] Write tests for secret CRUD

8. **Write unit tests**
   - [ ] Test Bearer token authentication header
   - [ ] Test API response parsing with generated types
   - [ ] Test error type conversions
   - [ ] Test binding conversion from config

9. **Write integration tests**
   - [ ] Test worker script upload (requires credentials, mark `#[ignore]`)
   - [ ] Test secrets management (requires credentials, mark `#[ignore]`)
   - [ ] Test KV namespace operations (requires credentials, mark `#[ignore]`)
   - [ ] Test full deploy-verify-destroy cycle

10. **Documentation**
    - [ ] Document all public API methods
    - [ ] Add usage examples for API provider
    - [ ] Document required environment variables
    - [ ] Document token permissions required

## Cloudflare API Endpoints Used

| Method | Endpoint | Purpose |
|--------|----------|---------|
| `PUT` | `/client/v4/accounts/{id}/workers/scripts/{name}` | Upload worker script |
| `GET` | `/client/v4/accounts/{id}/workers/scripts/{name}` | Get worker script info |
| `DELETE` | `/client/v4/accounts/{id}/workers/scripts/{name}` | Delete worker script |
| `PUT` | `/client/v4/accounts/{id}/workers/scripts/{name}/secrets` | Create/update secret |
| `GET` | `/client/v4/accounts/{id}/workers/scripts/{name}/secrets` | List secrets |
| `DELETE` | `/client/v4/accounts/{id}/workers/scripts/{name}/secrets/{secret}` | Delete secret |
| `POST` | `/client/v4/accounts/{id}/storage/kv/namespaces` | Create KV namespace |
| `PUT` | `/client/v4/accounts/{id}/storage/kv/namespaces/{id}/bulk` | Bulk write KV entries |
| `POST` | `/client/v4/accounts/{id}/d1/database` | Create D1 database |
| `PUT` | `/client/v4/accounts/{id}/r2/buckets/{name}` | Create R2 bucket |
| `POST` | `/client/v4/accounts/{id}/queues` | Create queue |
| `POST` | `/client/v4/zones/{zone_id}/workers/routes` | Create route |

## Implementation Notes

### Multipart/form-data Construction

The Cloudflare Workers API requires multipart/form-data for script uploads:

```
------RustCloudflareBoundary12345
Content-Disposition: form-data; name="index.js"; filename="index.js"
Content-Type: application/javascript

<worker script content here>
------RustCloudflareBoundary12345
Content-Disposition: form-data; name="metadata"
Content-Type: application/json

{
  "bindings": [...],
  "compatibility_date": "2024-01-01",
  "compatibility_flags": ["nodejs_compat"]
}
------RustCloudflareBoundary12345--
```

### Token Permissions

The API token must have these permissions:
- `Workers:Write` - Deploy and manage worker scripts
- `Workers KV:Write` - Manage KV namespaces
- `D1:Write` - Manage D1 databases
- `R2:Write` - Manage R2 buckets
- `Queues:Write` - Manage queues

### No CLI Fallback

This provider is API-only. If users need CLI fallback (wrangler), they should use `04-cloudflare-provider` instead.

### Version Rollback Limitations

Cloudflare API doesn't support direct version rollback. To rollback, you must re-upload the previous script content. The state store should track previous script content for this purpose.

### Logs API

Cloudflare doesn't provide a streaming logs API for Workers. Users should use:
- Cloudflare Dashboard for manual log viewing
- Logpush for automated log delivery
- Workers Analytics Engine for metrics

## Success Criteria

- [ ] All 10 tasks completed
- [ ] `cargo clippy -p foundation_deployment -- -D warnings -W clippy::pedantic` — zero warnings
- [ ] `cargo doc -p foundation_deployment --no-deps` — zero rustdoc warnings
- [ ] No `#[allow(...)]` or `#[expect(...)]` anywhere
- [ ] Bearer token auth works correctly
- [ ] Multipart upload constructs valid form-data
- [ ] Worker script upload succeeds via API
- [ ] Secrets CRUD operations work
- [ ] KV, D1, R2, Queue resource creation works
- [ ] Error handling provides clear messages

## Verification

```bash
cd backends/foundation_deployment
cargo test cloudflare_api -- --nocapture

# Integration (requires Cloudflare credentials)
export CLOUDFLARE_API_TOKEN="your_token"
export CLOUDFLARE_ACCOUNT_ID="your_account_id"
cargo test cloudflare_api_integration -- --ignored --nocapture
```

---

_Created: 2026-04-06_
_Status: pending — API-first Cloudflare provider implementation_
