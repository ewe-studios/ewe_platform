---
workspace_name: "ewe_platform"
spec_directory: "specifications/11-foundation-deployment"
feature_directory: "specifications/11-foundation-deployment/features/08-gcp-cloud-run-api-provider"
this_file: "specifications/11-foundation-deployment/features/08-gcp-cloud-run-api-provider/feature.md"

status: pending
priority: high
created: 2026-04-06

depends_on: ["01-foundation-deployment-core", "02-state-stores", "03-deployment-engine", "05-gcp-cloud-run-cli-provider"]

tasks:
  completed: 0
  uncompleted: 10
  total: 10
  completion_percentage: 0%
---


# GCP Cloud Run API Provider (API-First)

## Iron Law: Zero Warnings

> **All code must compile with zero warnings and pass all lints. No suppression. No exceptions.**
>
> - `cargo clippy -p foundation_deployment -- -D warnings -W clippy::pedantic` — zero warnings
> - `cargo doc -p foundation_deployment --no-deps` — zero rustdoc warnings
> - `cargo test -p foundation_deployment` — zero compilation warnings
> - **No `#[allow(...)]`, `#[expect(...)]`, or `#![allow(...)]` anywhere.** Fix the code, never suppress.

## Overview

Implement the **API-first** GCP Cloud Run deployment provider. This provider calls the Cloud Run Admin API v2 directly via `SimpleHttpClient` with OAuth2/service account JWT authentication — **no gcloud CLI dependency**.

This is distinct from `05-gcp-cloud-run-provider` which is CLI-based (gcloud wrapper). The API provider:

- **Deploys via REST API only** - creates/updates services via `run.googleapis.com/v2`
- **OAuth2 service account auth** - from `GOOGLE_APPLICATION_CREDENTIALS` or explicit JWT
- **JWT signing** - implement in-house JWT creation and signing (no gcloud token fetching)
- **No CLI fallback** - this provider is API-only; use `05-gcp-cloud-run-provider` for CLI mode
- **Supports Services and Jobs** - both long-running HTTP services and batch/scheduled jobs
- **Container image build** - via Docker API or Cloud Build API (optional)

### Relationship to CLI Provider

| Feature | `05-gcp-cloud-run-provider` (CLI) | `05-gcp-cloud-run-api-provider` (API-First) |
|---------|----------------------------------|---------------------------------------------|
| Dependency | Requires `gcloud` CLI installed | No external dependencies |
| Auth | gcloud handles OAuth2 | Service account JWT signing |
| Deploy | `gcloud run deploy` command | Direct REST API POST/PATCH |
| Image Build | `docker build` or `gcloud builds` | Docker API or Cloud Build API |
| Use Case | Local dev, quick iteration | CI/CD, production automation |

## Dependencies

Depends on:
- `01-foundation-deployment-core` - `DeploymentProvider` trait, `ShellExecutor`
- `02-state-stores` - `StateStore` for persistence
- `03-deployment-engine` - `DeploymentPlanner` for orchestration
- `05-gcp-cloud-run-provider` - Reference for config parsing (`CloudRunServiceConfig`)
- **Existing `providers/gcp/resources/`** - Reuse generated resource types from GCP OpenAPI spec
- **Existing `providers/gcp/clients/`** (if available) - Reuse generated API client functions

Required by:
- `07-templates` - GCP-specific template configs
- `09-examples-documentation` - GCP examples

## Architecture

The API provider implementation should:

1. **Reuse existing generated resources** in `backends/foundation_deployment/src/providers/gcp/resources/`:
   - All generated resource types from the GCP OpenAPI spec
   - Types already have `Serialize`, `Deserialize`, `Debug`, `Clone` derives
   - Includes Cloud Run v2 API types: `Service`, `Job`, `Revision`, `Execution`

2. **Implement the API provider** in `backends/foundation_deployment/src/providers/gcp/api/`:
   - `mod.rs` - Module declaration and `GcpApiProvider` struct
   - `auth.rs` - `GcpAuth` with service account JWT handling
   - `error.rs` - `GcpApiError` wrapping the generated `ApiError`
   - `provider.rs` - `DeploymentProvider` trait implementation
   - `jwt.rs` - JWT creation and signing for OAuth2

**Note:** GCP has extensive generated resources under `providers/gcp/resources/`. The implementation should use these types directly rather than duplicating them.

## Requirements

### Authentication

```rust
// providers/gcp_api/auth.rs

use foundation_core::simple_http::client::SimpleHttpClient;

/// GCP OAuth2 authentication using service account JWT.
///
/// Required environment variables (one of):
/// - `GOOGLE_APPLICATION_CREDENTIALS` - Path to service account JSON
/// - Or explicit: `GCP_SA_EMAIL`, `GCP_SA_PRIVATE_KEY`, `GCP_PROJECT_ID`
///
/// Service account permissions required:
/// - `run.services.create`, `run.services.update`, `run.services.delete`
/// - `run.jobs.create`, `run.jobs.update`, `run.jobs.delete`
/// - `artifactregistry.repositories.uploadArtifacts` (for image push)
#[derive(Debug, Clone)]
pub struct GcpAuth {
    pub service_account_email: String,
    pub private_key: String,  // PEM-encoded RSA private key
    pub project_id: String,
    pub region: String,
}

impl GcpAuth {
    /// Load from GOOGLE_APPLICATION_CREDENTIALS environment variable.
    /// Expects path to service account JSON file.
    pub fn from_env() -> Option<Self> {
        let creds_path = std::env::var("GOOGLE_APPLICATION_CREDENTIALS").ok()?;
        let creds_content = std::fs::read_to_string(&creds_path).ok()?;
        let creds: GcpServiceAccountCreds = serde_json::from_str(&creds_content).ok()?;
        
        Some(Self {
            service_account_email: creds.client_email,
            private_key: creds.private_key,
            project_id: creds.project_id,
            region: std::env::var("GOOGLE_CLOUD_REGION").ok()?,
        })
    }

    /// Create from explicit values.
    pub fn new(
        service_account_email: &str,
        private_key: &str,
        project_id: &str,
        region: &str,
    ) -> Self {
        Self {
            service_account_email: service_account_email.to_string(),
            private_key: private_key.to_string(),
            project_id: project_id.to_string(),
            region: region.to_string(),
        }
    }

    /// Generate OAuth2 access token via JWT bearer assertion.
    /// Returns token valid for 1 hour.
    pub fn generate_access_token(&self) -> Result<String, GcpAuthError> {
        use chrono::{Utc, Duration};
        
        let now = Utc::now();
        let expiry = now + Duration::hours(1);
        
        // Create JWT claim set
        let claims = GcpJwtClaims {
            iss: self.service_account_email.clone(),
            scope: "https://www.googleapis.com/auth/cloud-platform".to_string(),
            aud: "https://oauth2.googleapis.com/token".to_string(),
            exp: expiry.timestamp(),
            iat: now.timestamp(),
        };
        
        // Sign JWT with service account private key
        let jwt = self.sign_jwt(&claims)?;
        
        // Exchange JWT for access token
        self.exchange_jwt_for_token(&jwt)
    }

    /// Configure HTTP client with Bearer token.
    pub fn configure_client(&self, client: &mut SimpleHttpClient) -> Result<(), GcpAuthError> {
        let token = self.generate_access_token()?;
        client.add_header(
            "Authorization".to_string(),
            format!("Bearer {}", token),
        );
        client.add_header(
            "Content-Type".to_string(),
            "application/json".to_string(),
        );
        Ok(())
    }
}

#[derive(Debug, Deserialize)]
struct GcpServiceAccountCreds {
    client_email: String,
    private_key: String,
    project_id: String,
}

#[derive(Debug, Serialize)]
struct GcpJwtClaims {
    iss: String,
    scope: String,
    aud: String,
    exp: i64,
    iat: i64,
}

impl GcpAuth {
    fn sign_jwt(&self, claims: &GcpJwtClaims) -> Result<String, GcpAuthError> {
        // Implement JWT signing using RSA-SHA256
        // JWT format: header.payload.signature (base64url encoded)
        
        let header = GcpJwtHeader {
            alg: "RS256".to_string(),
            typ: "JWT".to_string(),
        };
        
        let header_b64 = self.base64url_encode(serde_json::to_vec(&header)?);
        let claims_b64 = self.base64url_encode(serde_json::to_vec(claims)?);
        
        let signing_input = format!("{}.{}", header_b64, claims_b64);
        
        // Sign with RSA private key (PKCS#1 or PKCS#8 format)
        let signature = self.rsa_sign_sha256(&signing_input)?;
        let signature_b64 = self.base64url_encode(&signature);
        
        Ok(format!("{}.{}.{}", header_b64, claims_b64, signature_b64))
    }

    fn exchange_jwt_for_token(&self, jwt: &str) -> Result<String, GcpAuthError> {
        // POST to https://oauth2.googleapis.com/token
        // grant_type=urn:ietf:params:oauth:grant-type:jwt-bearer&assertion={jwt}
        todo!("Implement token exchange")
    }

    fn base64url_encode(&self, data: &[u8]) -> String {
        use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
        URL_SAFE_NO_PAD.encode(data)
    }

    fn rsa_sign_sha256(&self, data: &str) -> Result<Vec<u8>, GcpAuthError> {
        // Implement RSA-SHA256 signing
        // Parse PEM private key and sign
        todo!("Implement RSA signing")
    }
}
```

### API Client Structure

```rust
// providers/gcp_api/client.rs

use foundation_core::simple_http::client::SimpleHttpClient;

const CLOUD_RUN_API_BASE: &str = "https://run.googleapis.com/v2";

/// GCP Cloud Run REST API client using SimpleHttpClient.
/// All methods are API-only — no CLI fallback.
pub struct GcpCloudRunApiClient {
    client: SimpleHttpClient,
    auth: GcpAuth,
}

impl GcpCloudRunApiClient {
    pub fn new(auth: GcpAuth) -> Result<Self, GcpAuthError> {
        let mut client = SimpleHttpClient::new();
        auth.configure_client(&mut client)?;
        Ok(Self { client, auth })
    }

    // =======================================================================
    // Cloud Run Services API
    // =======================================================================

    /// POST /v2/projects/{project}/locations/{location}/services
    ///
    /// Create a new Cloud Run service.
    /// Long-running operation - returns Operation resource.
    pub async fn create_service(
        &self,
        project_id: &str,
        location: &str,
        service_id: &str,
        service: &CloudRunService,
    ) -> Result<Operation, GcpApiError> {
        let url = format!(
            "{}/projects/{}/locations/{}/services",
            CLOUD_RUN_API_BASE, project_id, location
        );

        let body = serde_json::json!({
            "service": service,
            "serviceId": service_id,
        });

        let response = self.client.post(&url, body.to_string().as_bytes()).await?;
        serde_json::from_slice(response.body()).map_err(|e| {
            GcpApiError::ParseError(format!("Failed to parse response: {}", e))
        })
    }

    /// PATCH /v2/projects/{project}/locations/{location}/services/{service_id}
    ///
    /// Update an existing Cloud Run service.
    pub async fn update_service(
        &self,
        project_id: &str,
        location: &str,
        service_id: &str,
        service: &CloudRunService,
        update_mask: &str,
    ) -> Result<Operation, GcpApiError> {
        let url = format!(
            "{}/projects/{}/locations/{}/services/{}",
            CLOUD_RUN_API_BASE, project_id, location, service_id
        );

        let body = serde_json::json!({
            "service": service,
            "updateMask": update_mask,
        });

        let response = self.client.patch(&url, body.to_string().as_bytes()).await?;
        serde_json::from_slice(response.body()).map_err(|e| {
            GcpApiError::ParseError(format!("Failed to parse response: {}", e))
        })
    }

    /// GET /v2/projects/{project}/locations/{location}/services/{service_id}
    pub async fn get_service(
        &self,
        project_id: &str,
        location: &str,
        service_id: &str,
    ) -> Result<CloudRunService, GcpApiError> {
        let url = format!(
            "{}/projects/{}/locations/{}/services/{}",
            CLOUD_RUN_API_BASE, project_id, location, service_id
        );

        let response = self.client.get(&url).await?;
        serde_json::from_slice(response.body()).map_err(|e| {
            GcpApiError::ParseError(format!("Failed to parse response: {}", e))
        })
    }

    /// DELETE /v2/projects/{project}/locations/{location}/services/{service_id}
    pub async fn delete_service(
        &self,
        project_id: &str,
        location: &str,
        service_id: &str,
    ) -> Result<Operation, GcpApiError> {
        let url = format!(
            "{}/projects/{}/locations/{}/services/{}",
            CLOUD_RUN_API_BASE, project_id, location, service_id
        );

        let response = self.client.delete(&url).await?;
        serde_json::from_slice(response.body()).map_err(|e| {
            GcpApiError::ParseError(format!("Failed to parse response: {}", e))
        })
    }

    // =======================================================================
    // Cloud Run Jobs API
    // =======================================================================

    /// POST /v2/projects/{project}/locations/{location}/jobs
    pub async fn create_job(
        &self,
        project_id: &str,
        location: &str,
        job_id: &str,
        job: &CloudRunJob,
    ) -> Result<Operation, GcpApiError> {
        let url = format!(
            "{}/projects/{}/locations/{}/jobs",
            CLOUD_RUN_API_BASE, project_id, location
        );

        let body = serde_json::json!({
            "job": job,
            "jobId": job_id,
        });

        let response = self.client.post(&url, body.to_string().as_bytes()).await?;
        serde_json::from_slice(response.body()).map_err(|e| {
            GcpApiError::ParseError(format!("Failed to parse response: {}", e))
        })
    }

    /// POST /v2/projects/{project}/locations/{location}/jobs/{job_id}:run
    ///
    /// Create an execution of the job.
    pub async fn run_job(
        &self,
        project_id: &str,
        location: &str,
        job_id: &str,
    ) -> Result<Operation, GcpApiError> {
        let url = format!(
            "{}/projects/{}/locations/{}/jobs/{}:run",
            CLOUD_RUN_API_BASE, project_id, location, job_id
        );

        let response = self.client.post(&url, &[]).await?;
        serde_json::from_slice(response.body()).map_err(|e| {
            GcpApiError::ParseError(format!("Failed to parse response: {}", e))
        })
    }

    /// GET /v2/projects/{project}/locations/{location}/jobs/{job_id}
    pub async fn get_job(
        &self,
        project_id: &str,
        location: &str,
        job_id: &str,
    ) -> Result<CloudRunJob, GcpApiError> {
        let url = format!(
            "{}/projects/{}/locations/{}/jobs/{}",
            CLOUD_RUN_API_BASE, project_id, location, job_id
        );

        let response = self.client.get(&url).await?;
        serde_json::from_slice(response.body()).map_err(|e| {
            GcpApiError::ParseError(format!("Failed to parse response: {}", e))
        })
    }

    /// DELETE /v2/projects/{project}/locations/{location}/jobs/{job_id}
    pub async fn delete_job(
        &self,
        project_id: &str,
        location: &str,
        job_id: &str,
    ) -> Result<Operation, GcpApiError> {
        let url = format!(
            "{}/projects/{}/locations/{}/jobs/{}",
            CLOUD_RUN_API_BASE, project_id, location, job_id
        );

        let response = self.client.delete(&url).await?;
        serde_json::from_slice(response.body()).map_err(|e| {
            GcpApiError::ParseError(format!("Failed to parse response: {}", e))
        })
    }

    // =======================================================================
    // Long-Running Operations API
    // =======================================================================

    /// GET /v2/{operation_name}
    ///
    /// Poll an operation until complete.
    pub async fn poll_operation(
        &self,
        operation_name: &str,
        max_attempts: u32,
        poll_interval_ms: u64,
    ) -> Result<Operation, GcpApiError> {
        let url = format!("{}/{}", CLOUD_RUN_API_BASE, operation_name);

        for _ in 0..max_attempts {
            let response = self.client.get(&url).await?;
            let operation: Operation = serde_json::from_slice(response.body())?;

            if operation.done {
                return Ok(operation);
            }

            tokio::time::sleep(tokio::time::Duration::from_millis(poll_interval_ms)).await;
        }

        Err(GcpApiError::OperationTimeout(operation_name.to_string()))
    }
}
```

### Error Types

```rust
// providers/gcp_api/error.rs

use foundation_core::simple_http::client::HttpClientError;

/// GCP API-specific error types.
#[derive(Debug)]
pub enum GcpApiError {
    /// Invalid service account credentials.
    InvalidCredentials(String),

    /// JWT signing failed.
    JwtSigningError(String),

    /// Token exchange failed.
    TokenExchangeError(String),

    /// API returned an error response.
    ApiError(GcpStatus),

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

    /// Permission denied (insufficient IAM permissions).
    PermissionDenied(String),

    /// Long-running operation timed out.
    OperationTimeout(String),

    /// Quota exceeded.
    QuotaExceeded(String),
}

impl std::error::Error for GcpApiError {}

impl std::fmt::Display for GcpApiError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidCredentials(reason) => {
                write!(f, "Invalid GCP service account credentials: {}", reason)
            }
            Self::JwtSigningError(msg) => write!(f, "JWT signing failed: {}", msg),
            Self::TokenExchangeError(msg) => write!(f, "Token exchange failed: {}", msg),
            Self::ApiError(status) => {
                write!(f, "GCP API error [{}]: {}", status.code, status.message)
            }
            Self::HttpError(e) => write!(f, "HTTP error: {}", e),
            Self::UnexpectedResponse => write!(f, "Unexpected response from GCP API"),
            Self::ParseError(msg) => write!(f, "Parse error: {}", msg),
            Self::SerializeError(msg) => write!(f, "Serialization error: {}", msg),
            Self::NotFound(resource) => write!(f, "Resource not found: {}", resource),
            Self::PermissionDenied(reason) => write!(f, "Permission denied: {}", reason),
            Self::OperationTimeout(op) => write!(f, "Operation timed out: {}", op),
            Self::QuotaExceeded(quota) => write!(f, "Quota exceeded: {}", quota),
        }
    }
}

impl From<HttpClientError> for GcpApiError {
    fn from(e: HttpClientError) -> Self {
        Self::HttpError(e)
    }
}

impl From<serde_json::Error> for GcpApiError {
    fn from(e: serde_json::Error) -> Self {
        Self::SerializeError(e.to_string())
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct GcpStatus {
    pub code: i32,
    pub message: String,
    pub details: Option<Vec<serde_json::Value>>,
}
```

### GcpCloudRunApiProvider Implementation

```rust
// providers/gcp_api/mod.rs

use std::path::{Path, PathBuf};
use crate::core::traits::DeploymentProvider;
use crate::core::types::{BuildOutput, DeploymentResult};
use crate::error::DeploymentError;
use super::gcp_cloud_run::CloudRunServiceConfig; // Reuse config from CLI provider

pub struct GcpCloudRunApiProvider {
    working_dir: PathBuf,
    auth: GcpAuth,
    client: GcpCloudRunApiClient,
}

impl GcpCloudRunApiProvider {
    /// Create new API provider from explicit credentials.
    pub fn new(
        working_dir: &Path,
        service_account_email: &str,
        private_key: &str,
        project_id: &str,
        region: &str,
    ) -> Result<Self, GcpAuthError> {
        let auth = GcpAuth::new(service_account_email, private_key, project_id, region);
        let client = GcpCloudRunApiClient::new(auth.clone())?;
        Ok(Self {
            working_dir: working_dir.to_path_buf(),
            auth,
            client,
        })
    }

    /// Create from GOOGLE_APPLICATION_CREDENTIALS environment variable.
    pub fn from_env(working_dir: &Path) -> Option<Self> {
        let auth = GcpAuth::from_env()?;
        let client = GcpCloudRunApiClient::new(auth.clone()).ok()?;
        Some(Self {
            working_dir: working_dir.to_path_buf(),
            auth,
            client,
        })
    }

    /// Detect GCP project by finding service.yaml.
    pub fn detect(project_dir: &Path) -> Option<CloudRunServiceConfig> {
        let config_path = project_dir.join("service.yaml");
        CloudRunServiceConfig::load(&config_path).ok()
    }
}

impl DeploymentProvider for GcpCloudRunApiProvider {
    type Config = CloudRunServiceConfig;
    type Resources = GcpApiResources;

    fn name(&self) -> &str {
        "gcp-api"
    }

    fn detect(project_dir: &Path) -> Option<Self::Config> {
        Self::detect(project_dir)
    }

    fn validate(&self, config: &Self::Config) -> Result<(), DeploymentError> {
        config.validate()?;
        // Validate credentials by making a test API call
        Ok(())
    }

    fn build(&self, config: &Self::Config, _env: Option<&str>) -> Result<BuildOutput, DeploymentError> {
        // Build container image using Docker API
        let image_tag = self.build_container_image(config)?;
        
        Ok(BuildOutput {
            artifacts: vec![BuildArtifact {
                path: PathBuf::from("image"),
                size_bytes: 0,
                artifact_type: ArtifactType::ContainerImage { tag: image_tag },
            }],
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
            return Ok(DeploymentResult::dry_run("gcp-api", config.service_name()));
        }

        let service_name = config.service_name();
        let location = config.location().unwrap_or(&self.auth.region);

        // Build and push container image
        let image_tag = self.build_and_push_image(config)?;

        // Create or update service with new image
        let service = self.config_to_service(config, &image_tag)?;

        use futures::executor::block_on;
        let operation = block_on(self.client.create_service(
            &self.auth.project_id,
            location,
            service_name,
            &service,
        ))?;

        // Poll operation until complete
        let result = block_on(self.client.poll_operation(&operation.name, 60, 5000))?;

        if let Some(error) = result.error {
            return Err(DeploymentError::Gcp {
                status: error.code as u16,
                message: error.message,
            });
        }

        // Get the deployed service to extract URL
        let deployed_service = block_on(self.client.get_service(
            &self.auth.project_id,
            location,
            service_name,
        ))?;

        Ok(DeploymentResult {
            deployment_id: deployed_service.uri.clone(),
            provider: "gcp-api".to_string(),
            resource_name: service_name.to_string(),
            environment: env.map(String::from),
            url: Some(deployed_service.uri),
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
                // Route traffic back to previous revision
                self.traffic_shift_to_revision(config, env, &prev.previous_revision)
            }
            None => self.destroy(config, env),
        }
    }

    fn logs(&self, config: &Self::Config, _env: Option<&str>) -> Result<(), DeploymentError> {
        // Cloud Run logs are available via Cloud Logging API
        // For now, return error suggesting use of gcloud or Cloud Console
        Err(DeploymentError::ConfigInvalid {
            file: "logs".into(),
            reason: "Use Cloud Console or gcloud run services logs read for logs".into(),
        })
    }

    fn destroy(&self, config: &Self::Config, _env: Option<&str>) -> Result<(), DeploymentError> {
        let service_name = config.service_name();
        let location = config.location().unwrap_or(&self.auth.region);

        use futures::executor::block_on;
        let operation = block_on(self.client.delete_service(
            &self.auth.project_id,
            location,
            service_name,
        ))?;

        // Wait for deletion to complete
        block_on(self.client.poll_operation(&operation.name, 60, 5000))?;

        Ok(())
    }

    fn status(&self, config: &Self::Config, _env: Option<&str>) -> Result<Self::Resources, DeploymentError> {
        let service_name = config.service_name();
        let location = config.location().unwrap_or(&self.auth.region);

        use futures::executor::block_on;
        let service = block_on(self.client.get_service(
            &self.auth.project_id,
            location,
            service_name,
        ))?;

        Ok(GcpApiResources {
            service_name: service_name.to_string(),
            url: Some(service.uri.clone()),
            latest_revision: service.latest_created_revision,
            traffic: self.extract_traffic(&service.traffic),
        })
    }

    fn verify(&self, result: &DeploymentResult) -> Result<bool, DeploymentError> {
        if let Some(url) = &result.url {
            use foundation_core::simple_http::client::SimpleHttpClient;
            let client = SimpleHttpClient::new();
            match client.get(url) {
                Ok(resp) if resp.status().is_success() => Ok(true),
                Ok(resp) if resp.status() == 404 || resp.status() == 503 => Ok(false),
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
   - [ ] Review `providers/gcp/resources/*.rs` - generated resource types from GCP APIs
   - [ ] Identify Cloud Run v2 types: `Service`, `Job`, `Revision`, `Execution`, `Operation`
   - [ ] Check if `providers/gcp/clients/` exists with generated client functions
   - [ ] Identify gaps: what endpoints need to be added for full provider support

2. **Create module structure**
   - [ ] Create `src/providers/gcp/api/mod.rs`, `auth.rs`, `error.rs`, `provider.rs`, `jwt.rs`
   - [ ] Register in `src/providers/gcp/mod.rs` (alongside existing `fetch`, `provider`, `resources`)
   - [ ] Add to feature flags (separate from CLI provider)

3. **Implement JWT authentication**
   - [ ] Implement JWT claim set creation for GCP OAuth2
   - [ ] Implement RSA-SHA256 JWT signing
   - [ ] Implement JWT-to-token exchange
   - [ ] Implement token caching and refresh

4. **Implement authentication**
   - [ ] Implement `GcpAuth` struct with service account credentials
   - [ ] Implement `from_env()` loading from `GOOGLE_APPLICATION_CREDENTIALS`
   - [ ] Implement `configure_client()` for Bearer token setup

5. **Implement error types**
   - [ ] Define `GcpApiError` enum (wrap generated `ApiError` if available)
   - [ ] Implement `Display` and `Error` traits
   - [ ] Implement `From` conversions
   - [ ] Handle GCP-specific errors (quota, IAM, etc.)

6. **Implement GcpCloudRunApiProvider trait**
   - [ ] Implement `detect()` - find service.yaml
   - [ ] Implement `validate()` - validate config + credentials
   - [ ] Implement `build()` - build container image via Docker API
   - [ ] Implement `deploy()` - create/update service via API (use generated types)
   - [ ] Implement `destroy()` - delete service
   - [ ] Implement `status()` - get service info
   - [ ] Implement `rollback()` - traffic shift to previous revision
   - [ ] Implement `verify()` - HTTP health check

7. **Implement container image build**
   - [ ] Detect Dockerfile in project
   - [ ] Build via Docker API (not CLI)
   - [ ] Push to Artifact Registry or GCR
   - [ ] Handle image tag generation

8. **Implement Jobs support**
   - [ ] Detect `kind: Job` in config
   - [ ] Implement job create/update via Jobs API
   - [ ] Implement job execution trigger
   - [ ] Handle execution status polling

9. **Write unit tests**
   - [ ] Test JWT creation and signing
   - [ ] Test API response parsing with generated types
   - [ ] Test error type conversions
   - [ ] Test config-to-service conversion
   - [ ] Test operation polling logic

10. **Write integration tests**
    - [ ] Test service deploy (requires credentials, mark `#[ignore]`)
    - [ ] Test job deploy (requires credentials, mark `#[ignore]`)
    - [ ] Test full deploy-verify-destroy cycle

11. **Documentation**
    - [ ] Document all public API methods
    - [ ] Add usage examples for API provider
    - [ ] Document required environment variables
    - [ ] Document IAM permissions required

## GCP API Endpoints Used

| Method | Endpoint | Purpose |
|--------|----------|---------|
| `POST` | `/v2/projects/{project}/locations/{location}/services` | Create service |
| `PATCH` | `/v2/projects/{project}/locations/{location}/services/{id}` | Update service |
| `GET` | `/v2/projects/{project}/locations/{location}/services/{id}` | Get service |
| `DELETE` | `/v2/projects/{project}/locations/{location}/services/{id}` | Delete service |
| `POST` | `/v2/projects/{project}/locations/{location}/jobs` | Create job |
| `GET` | `/v2/projects/{project}/locations/{location}/jobs/{id}` | Get job |
| `POST` | `/v2/projects/{project}/locations/{location}/jobs/{id}:run` | Execute job |
| `DELETE` | `/v2/projects/{project}/locations/{location}/jobs/{id}` | Delete job |
| `GET` | `/v2/{operation_name}` | Poll operation |

## Implementation Notes

### JWT Signing

GCP service account authentication requires JWT signing with RSA-SHA256. The private key from the service account JSON is in PEM format:

```
-----BEGIN PRIVATE KEY-----
MIIEvQIBADANBgkq...
-----END PRIVATE KEY-----
```

### Token Exchange

After creating the JWT, exchange it for an OAuth2 access token:

```
POST https://oauth2.googleapis.com/token
Content-Type: application/x-www-form-urlencoded

grant_type=urn:ietf:params:oauth:grant-type:jwt-bearer&assertion={jwt}
```

### Long-Running Operations

All Cloud Run v2 operations are long-running. The response contains an `Operation` resource with a `name` field that must be polled until `done: true`.

### Container Image Building

For API-first image building:
1. Use Docker Engine API directly (not CLI)
2. Or use Cloud Build API for serverless builds
3. Push to Artifact Registry or Container Registry

### IAM Permissions Required

The service account needs:
- `run.services.*` - Full Cloud Run Services access
- `run.jobs.*` - Full Cloud Run Jobs access
- `artifactregistry.repositories.uploadArtifacts` - Push images
- `iam.serviceAccounts.actAs` - For certain operations

## Success Criteria

- [ ] All 10 tasks completed
- [ ] `cargo clippy -p foundation_deployment -- -D warnings -W clippy::pedantic` — zero warnings
- [ ] `cargo doc -p foundation_deployment --no-deps` — zero rustdoc warnings
- [ ] No `#[allow(...)]` or `#[expect(...)]` anywhere
- [ ] JWT authentication works correctly
- [ ] Service create/update/delete via API
- [ ] Job create/execute/delete via API
- [ ] Container image build via Docker API
- [ ] Operation polling handles timeouts correctly
- [ ] Error handling provides clear messages

## Verification

```bash
cd backends/foundation_deployment
cargo test gcp_api -- --nocapture

# Integration (requires GCP credentials)
export GOOGLE_APPLICATION_CREDENTIALS="/path/to/service-account.json"
export GOOGLE_CLOUD_REGION="us-central1"
cargo test gcp_api_integration -- --ignored --nocapture
```

---

_Created: 2026-04-06_
_Status: pending — API-first GCP Cloud Run provider implementation_
