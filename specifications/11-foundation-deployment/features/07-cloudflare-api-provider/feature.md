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

Required by:
- `07-templates` - Cloudflare-specific template configs
- `09-examples-documentation` - Cloudflare examples

## Requirements

### Authentication

```rust
// providers/cloudflare_api/auth.rs

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

### API Client Structure

```rust
// providers/cloudflare_api/client.rs

use foundation_core::simple_http::client::SimpleHttpClient;
use std::collections::HashMap;

const CLOUDFLARE_API_BASE: &str = "https://api.cloudflare.com/client/v4";

/// Cloudflare REST API client using SimpleHttpClient.
/// All methods are API-only — no CLI fallback.
pub struct CloudflareApiClient {
    client: SimpleHttpClient,
    auth: CloudflareAuth,
}

impl CloudflareApiClient {
    pub fn new(auth: CloudflareAuth) -> Result<Self, CloudflareApiError> {
        let mut client = SimpleHttpClient::new();
        auth.configure_client(&mut client);
        Ok(Self { client, auth })
    }

    // =======================================================================
    // Worker Scripts API
    // =======================================================================

    /// PUT /accounts/{account_id}/workers/scripts/{script_name}
    ///
    /// Upload a worker script with optional bindings.
    /// Uses multipart/form-data for script + metadata.
    pub async fn put_worker_script(
        &self,
        script_name: &str,
        script_content: &[u8],
        bindings: &[WorkerBinding],
        compatibility_date: Option<&str>,
        compatibility_flags: &[String],
    ) -> Result<WorkerScript, CloudflareApiError> {
        let url = format!(
            "{}/accounts/{}/workers/scripts/{}",
            CLOUDFLARE_API_BASE, self.auth.account_id, script_name
        );

        // Build multipart/form-data body
        let boundary = self.generate_boundary();
        let body = self.build_multipart_script(
            script_content,
            bindings,
            compatibility_date,
            compatibility_flags,
            &boundary,
        );

        let mut headers = vec![
            ("Authorization".to_string(), format!("Bearer {}", self.auth.api_token)),
            ("Content-Type".to_string(), format!("multipart/form-data; boundary={}", boundary)),
        ];

        let response = self.client.put(&url, body.as_bytes(), headers).await?;
        self.parse_response::<PutWorkerScriptResponse>(response)
            .and_then(|r| r.result.ok_or(CloudflareApiError::UnexpectedResponse))
    }

    /// GET /accounts/{account_id}/workers/scripts/{script_name}
    pub async fn get_worker_script(
        &self,
        script_name: &str,
    ) -> Result<WorkerScript, CloudflareApiError> {
        let url = format!(
            "{}/accounts/{}/workers/scripts/{}",
            CLOUDFLARE_API_BASE, self.auth.account_id, script_name
        );

        let response = self.client.get(&url).await?;
        self.parse_response::<GetWorkerScriptResponse>(response)
            .and_then(|r| r.result.ok_or(CloudflareApiError::UnexpectedResponse))
    }

    /// DELETE /accounts/{account_id}/workers/scripts/{script_name}
    pub async fn delete_worker_script(
        &self,
        script_name: &str,
    ) -> Result<(), CloudflareApiError> {
        let url = format!(
            "{}/accounts/{}/workers/scripts/{}",
            CLOUDFLARE_API_BASE, self.auth.account_id, script_name
        );

        let response = self.client.delete(&url).await?;
        let result = self.parse_response::<DeleteWorkerScriptResponse>(response)?;
        if result.success {
            Ok(())
        } else {
            Err(CloudflareApiError::ApiError(result.errors))
        }
    }

    // =======================================================================
    // Secrets Management API
    // =======================================================================

    /// PUT /accounts/{account_id}/workers/scripts/{script_name}/secrets
    pub async fn put_secret(
        &self,
        script_name: &str,
        secret_name: &str,
        secret_text: &str,
    ) -> Result<(), CloudflareApiError> {
        let url = format!(
            "{}/accounts/{}/workers/scripts/{}/secrets",
            CLOUDFLARE_API_BASE, self.auth.account_id, script_name
        );

        let body = serde_json::json!({
            "name": secret_name,
            "text": secret_text,
            "type": "secret_text"
        });

        let response = self.client.put(&url, body.to_string().as_bytes()).await?;
        let result = self.parse_response::<PutSecretResponse>(response)?;
        if result.success {
            Ok(())
        } else {
            Err(CloudflareApiError::ApiError(result.errors))
        }
    }

    /// GET /accounts/{account_id}/workers/scripts/{script_name}/secrets
    pub async fn list_secrets(
        &self,
        script_name: &str,
    ) -> Result<Vec<SecretInfo>, CloudflareApiError> {
        let url = format!(
            "{}/accounts/{}/workers/scripts/{}/secrets",
            CLOUDFLARE_API_BASE, self.auth.account_id, script_name
        );

        let response = self.client.get(&url).await?;
        let result = self.parse_response::<ListSecretsResponse>(response)?;
        Ok(result.result.unwrap_or_default())
    }

    /// DELETE /accounts/{account_id}/workers/scripts/{script_name}/secrets/{secret_name}
    pub async fn delete_secret(
        &self,
        script_name: &str,
        secret_name: &str,
    ) -> Result<(), CloudflareApiError> {
        let url = format!(
            "{}/accounts/{}/workers/scripts/{}/secrets/{}",
            CLOUDFLARE_API_BASE, self.auth.account_id, script_name, secret_name
        );

        let response = self.client.delete(&url).await?;
        let result = self.parse_response::<DeleteSecretResponse>(response)?;
        if result.success {
            Ok(())
        } else {
            Err(CloudflareApiError::ApiError(result.errors))
        }
    }

    // =======================================================================
    // KV Namespaces API
    // =======================================================================

    /// POST /accounts/{account_id}/storage/kv/namespaces
    pub async fn create_kv_namespace(
        &self,
        title: &str,
    ) -> Result<KvNamespace, CloudflareApiError> {
        let url = format!(
            "{}/accounts/{}/storage/kv/namespaces",
            CLOUDFLARE_API_BASE, self.auth.account_id
        );

        let body = serde_json::json!({ "title": title });
        let response = self.client.post(&url, body.to_string().as_bytes()).await?;
        self.parse_response::<CreateKvNamespaceResponse>(response)
            .and_then(|r| r.result.ok_or(CloudflareApiError::UnexpectedResponse))
    }

    /// PUT /accounts/{account_id}/storage/kv/namespaces/{namespace_id}/bulk
    pub async fn bulk_write_kv(
        &self,
        namespace_id: &str,
        entries: &[KvEntry],
    ) -> Result<(), CloudflareApiError> {
        let url = format!(
            "{}/accounts/{}/storage/kv/namespaces/{}/bulk",
            CLOUDFLARE_API_BASE, self.auth.account_id, namespace_id
        );

        let body = serde_json::to_vec(entries)?;
        let response = self.client.put(&url, &body).await?;
        let result = self.parse_response::<BulkKvResponse>(response)?;
        if result.success {
            Ok(())
        } else {
            Err(CloudflareApiError::ApiError(result.errors))
        }
    }

    // =======================================================================
    // D1 Databases API
    // =======================================================================

    /// POST /accounts/{account_id}/d1/database
    pub async fn create_d1_database(
        &self,
        name: &str,
    ) -> Result<D1Database, CloudflareApiError> {
        let url = format!(
            "{}/accounts/{}/d1/database",
            CLOUDFLARE_API_BASE, self.auth.account_id
        );

        let body = serde_json::json!({ "name": name });
        let response = self.client.post(&url, body.to_string().as_bytes()).await?;
        self.parse_response::<CreateD1DatabaseResponse>(response)
            .and_then(|r| r.result.ok_or(CloudflareApiError::UnexpectedResponse))
    }

    // =======================================================================
    // R2 Buckets API
    // =======================================================================

    /// PUT /accounts/{account_id}/r2/buckets/{bucket_name}
    pub async fn create_r2_bucket(
        &self,
        bucket_name: &str,
    ) -> Result<R2Bucket, CloudflareApiError> {
        let url = format!(
            "{}/accounts/{}/r2/buckets/{}",
            CLOUDFLARE_API_BASE, self.auth.account_id, bucket_name
        );

        let response = self.client.put(&url, &[]).await?;
        self.parse_response::<CreateR2BucketResponse>(response)
            .and_then(|r| r.result.ok_or(CloudflareApiError::UnexpectedResponse))
    }

    // =======================================================================
    // Queues API
    // =======================================================================

    /// POST /accounts/{account_id}/queues
    pub async fn create_queue(
        &self,
        queue_name: &str,
    ) -> Result<Queue, CloudflareApiError> {
        let url = format!(
            "{}/accounts/{}/queues",
            CLOUDFLARE_API_BASE, self.auth.account_id
        );

        let body = serde_json::json!({ "name": queue_name });
        let response = self.client.post(&url, body.to_string().as_bytes()).await?;
        self.parse_response::<CreateQueueResponse>(response)
            .and_then(|r| r.result.ok_or(CloudflareApiError::UnexpectedResponse))
    }

    // =======================================================================
    // Routes API
    // =======================================================================

    /// POST /zones/{zone_id}/workers/routes
    pub async fn create_route(
        &self,
        zone_id: &str,
        pattern: &str,
        script_name: &str,
    ) -> Result<Route, CloudflareApiError> {
        let url = format!(
            "{}/zones/{}/workers/routes",
            CLOUDFLARE_API_BASE, zone_id
        );

        let body = serde_json::json!({
            "pattern": pattern,
            "script": script_name
        });
        let response = self.client.post(&url, body.to_string().as_bytes()).await?;
        self.parse_response::<CreateRouteResponse>(response)
            .and_then(|r| r.result.ok_or(CloudflareApiError::UnexpectedResponse))
    }

    // =======================================================================
    // Helper Methods
    // =======================================================================

    fn parse_response<T: serde::de::DeserializeOwned>(
        &self,
        response: foundation_core::simple_http::Response,
    ) -> Result<T, CloudflareApiError> {
        let body = response.body();
        serde_json::from_slice(body).map_err(|e| {
            CloudflareApiError::ParseError(format!("Failed to parse response: {}", e))
        })
    }

    fn generate_boundary(&self) -> String {
        use std::time::{SystemTime, UNIX_EPOCH};
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        format!("----RustCloudflareBoundary{}", timestamp)
    }

    fn build_multipart_script(
        &self,
        script_content: &[u8],
        bindings: &[WorkerBinding],
        compatibility_date: Option<&str>,
        compatibility_flags: &[String],
        boundary: &str,
    ) -> Vec<u8> {
        let mut body = Vec::new();

        // Script part
        body.extend_from_slice(format!("--{}\r\n", boundary).as_bytes());
        body.extend_from_slice(b"Content-Disposition: form-data; name=\"index.js\"; filename=\"index.js\"\r\n");
        body.extend_from_slice(b"Content-Type: application/javascript\r\n\r\n");
        body.extend_from_slice(script_content);
        body.extend_from_slice(b"\r\n");

        // Metadata part
        body.extend_from_slice(format!("--{}\r\n", boundary).as_bytes());
        body.extend_from_slice(b"Content-Disposition: form-data; name=\"metadata\"\r\n");
        body.extend_from_slice(b"Content-Type: application/json\r\n\r\n");

        let metadata = serde_json::json!({
            "bindings": bindings,
            "compatibility_date": compatibility_date,
            "compatibility_flags": compatibility_flags,
        });
        body.extend_from_slice(metadata.to_string().as_bytes());
        body.extend_from_slice(b"\r\n");

        // Closing boundary
        body.extend_from_slice(format!("--{}--\r\n", boundary).as_bytes());

        body
    }
}
```

### Error Types

```rust
// providers/cloudflare_api/error.rs

use foundation_core::simple_http::client::HttpClientError;

/// Cloudflare API-specific error types.
#[derive(Debug)]
pub enum CloudflareApiError {
    /// Invalid API token format or missing.
    InvalidToken,

    /// Invalid account ID format (expected 32-char hex).
    InvalidAccountId,

    /// API returned an error response.
    ApiError(Vec<CfApiError>),

    /// HTTP client error.
    HttpError(HttpClientError),

    /// Unexpected response structure.
    UnexpectedResponse,

    /// JSON parse error.
    ParseError(String),

    /// Serialization error.
    SerializeError(String),

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
            Self::ApiError(errors) => {
                for err in errors {
                    writeln!(f, "Cloudflare API error [{}]: {}", err.code, err.message)?;
                }
                Ok(())
            }
            Self::HttpError(e) => write!(f, "HTTP error: {}", e),
            Self::UnexpectedResponse => write!(f, "Unexpected response from Cloudflare API"),
            Self::ParseError(msg) => write!(f, "Parse error: {}", msg),
            Self::SerializeError(msg) => write!(f, "Serialization error: {}", msg),
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

impl From<HttpClientError> for CloudflareApiError {
    fn from(e: HttpClientError) -> Self {
        Self::HttpError(e)
    }
}

impl From<serde_json::Error> for CloudflareApiError {
    fn from(e: serde_json::Error) -> Self {
        Self::SerializeError(e.to_string())
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct CfApiError {
    pub code: u32,
    pub message: String,
}
```

### CloudflareApiProvider Implementation

```rust
// providers/cloudflare_api/mod.rs

use std::path::{Path, PathBuf};
use crate::core::traits::DeploymentProvider;
use crate::core::types::{BuildOutput, DeploymentResult};
use crate::error::DeploymentError;
use super::cloudflare::WranglerConfig; // Reuse config from CLI provider

pub struct CloudflareApiProvider {
    working_dir: PathBuf,
    auth: CloudflareAuth,
    client: CloudflareApiClient,
}

impl CloudflareApiProvider {
    /// Create new API provider from explicit credentials.
    pub fn new(working_dir: &Path, api_token: &str, account_id: &str) -> Result<Self, CloudflareApiError> {
        let auth = CloudflareAuth::new(api_token, account_id);
        let client = CloudflareApiClient::new(auth.clone())?;
        Ok(Self {
            working_dir: working_dir.to_path_buf(),
            auth,
            client,
        })
    }

    /// Create from environment variables.
    pub fn from_env(working_dir: &Path) -> Option<Self> {
        let auth = CloudflareAuth::from_env()?;
        let client = CloudflareApiClient::new(auth.clone()).ok()?;
        Some(Self {
            working_dir: working_dir.to_path_buf(),
            auth,
            client,
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

        // Build bindings from config
        let bindings = self.build_bindings(config, env);

        // Deploy via API
        use futures::executor::block_on;
        let result = block_on(self.client.put_worker_script(
            &worker_name,
            &script_content,
            &bindings,
            config.compatibility_date.as_deref(),
            config.compatibility_flags.as_deref().unwrap_or(&[]),
        ))?;

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
                // Redeploy previous version by uploading previous script
                // Note: Cloudflare API doesn't support version rollback directly
                // Must re-upload the previous script content
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
        use futures::executor::block_on;
        block_on(self.client.delete_worker_script(&worker_name))?;
        Ok(())
    }

    fn status(&self, config: &Self::Config, env: Option<&str>) -> Result<Self::Resources, DeploymentError> {
        let worker_name = config.worker_name(env);
        use futures::executor::block_on;
        let script = block_on(self.client.get_worker_script(&worker_name))?;

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

1. **Create module structure**
   - [ ] Create `src/providers/cloudflare_api/mod.rs`, `client.rs`, `auth.rs`, `error.rs`, `resources.rs`
   - [ ] Register in `src/providers/mod.rs`
   - [ ] Add to feature flags (separate from CLI provider)

2. **Implement authentication**
   - [ ] Implement `CloudflareAuth` struct
   - [ ] Implement `from_env()` and `new()` constructors
   - [ ] Implement `configure_client()` for Bearer token setup
   - [ ] Implement `validate()` for credential validation

3. **Implement API client**
   - [ ] Implement `CloudflareApiClient` with SimpleHttpClient
   - [ ] Implement worker script endpoints (PUT, GET, DELETE)
   - [ ] Implement secrets management endpoints
   - [ ] Implement KV namespace endpoints
   - [ ] Implement D1 database endpoints
   - [ ] Implement R2 bucket endpoints
   - [ ] Implement Queue endpoints
   - [ ] Implement Routes endpoints
   - [ ] Implement multipart/form-data builder for script uploads

4. **Implement error types**
   - [ ] Define `CloudflareApiError` enum
   - [ ] Implement `Display` and `Error` traits
   - [ ] Implement `From` conversions for HttpClientError and serde_json::Error
   - [ ] Handle rate limiting with retry-after

5. **Implement CloudflareApiProvider trait**
   - [ ] Implement `detect()` - find wrangler.toml
   - [ ] Implement `validate()` - validate config + credentials
   - [ ] Implement `build()` - run build command
   - [ ] Implement `deploy()` - upload via API with multipart
   - [ ] Implement `destroy()` - DELETE worker script
   - [ ] Implement `status()` - GET worker info
   - [ ] Implement `rollback()` - re-upload previous version
   - [ ] Implement `verify()` - HTTP health check

6. **Implement binding management**
   - [ ] Parse bindings from `WranglerConfig`
   - [ ] Convert to API binding types
   - [ ] Handle KV, D1, R2, Queue, Service bindings
   - [ ] Handle secrets vs plain text variables

7. **Implement secrets management**
   - [ ] Implement `put_secret()` via API
   - [ ] Implement `list_secrets()` via API
   - [ ] Implement `delete_secret()` via API
   - [ ] Write tests for secret CRUD

8. **Write unit tests**
   - [ ] Test Bearer token authentication header
   - [ ] Test multipart form-data construction
   - [ ] Test API response parsing
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
