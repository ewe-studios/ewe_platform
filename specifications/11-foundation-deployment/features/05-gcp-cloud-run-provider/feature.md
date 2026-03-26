---
workspace_name: "ewe_platform"
spec_directory: "specifications/11-foundation-deployment"
feature_directory: "specifications/11-foundation-deployment/features/05-gcp-cloud-run-provider"
this_file: "specifications/11-foundation-deployment/features/05-gcp-cloud-run-provider/feature.md"

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


# GCP Cloud Run Provider

## Overview

Implement the GCP Cloud Run deployment provider. This provider is **API-first** — it deploys by calling the Cloud Run Admin API v2 directly via `SimpleHttpClient`, with no CLI tools required.

The provider:
- **Deploys via API** - creates/updates Cloud Run services and jobs via `run.googleapis.com/v2`
- **Captures state from API responses** - revision names, service URLs, traffic splits stored in state store
- **Generates `service.yaml` on demand** - for local use with `gcloud run services replace`, not as deployment input
- **Falls back to CLI** - can optionally shell out to `gcloud` if the user prefers

Supports both **Cloud Run services** (long-running HTTP) and **Cloud Run Jobs** (batch/scheduled).

## Dependencies

Depends on:
- `01-foundation-deployment-core` - `DeploymentProvider` trait, `ProcessExecutor`
- `02-state-stores` - `StateStore` for persistence
- `03-deployment-engine` - `DeploymentPlanner` for orchestration

Required by:
- `07-templates` - GCP-specific template configs
- `09-examples-documentation` - GCP examples

## Requirements

### service.yaml Config Parsing

```rust
// providers/gcp/config.rs

use serde::{Deserialize, Serialize};

/// Parsed service.yaml - Knative Service manifest used by Cloud Run.
/// This is the source of truth for GCP Cloud Run deployments.
///
/// Reference: https://cloud.google.com/run/docs/reference/rest/v2/projects.locations.services
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CloudRunServiceConfig {
    pub api_version: String,               // "serving.knative.dev/v1"
    pub kind: String,                      // "Service" or "Job"
    pub metadata: ServiceMetadata,
    pub spec: ServiceSpec,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ServiceMetadata {
    pub name: String,
    pub labels: Option<HashMap<String, String>>,
    pub annotations: Option<HashMap<String, String>>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ServiceSpec {
    pub template: RevisionTemplate,
    pub traffic: Option<Vec<TrafficTarget>>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RevisionTemplate {
    pub metadata: Option<RevisionMetadata>,
    pub spec: RevisionSpec,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RevisionMetadata {
    pub annotations: Option<HashMap<String, String>>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RevisionSpec {
    pub container_concurrency: Option<u32>,
    pub timeout_seconds: Option<u32>,
    pub service_account_name: Option<String>,
    pub containers: Vec<Container>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Container {
    pub image: String,
    pub ports: Option<Vec<ContainerPort>>,
    pub resources: Option<ResourceRequirements>,
    pub env: Option<Vec<EnvVar>>,
    pub command: Option<Vec<String>>,
    pub args: Option<Vec<String>>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ContainerPort {
    pub container_port: u16,
    pub name: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ResourceRequirements {
    pub limits: Option<HashMap<String, String>>,  // cpu: "1", memory: "512Mi"
    pub requests: Option<HashMap<String, String>>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EnvVar {
    pub name: String,
    pub value: Option<String>,
    pub value_from: Option<EnvVarSource>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EnvVarSource {
    pub secret_key_ref: Option<SecretKeyRef>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SecretKeyRef {
    pub name: String,
    pub key: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TrafficTarget {
    pub percent: u32,
    pub revision_name: Option<String>,
    pub latest_revision: Option<bool>,
}

impl CloudRunServiceConfig {
    pub fn load(path: &Path) -> Result<Self, DeploymentError> {
        let content = std::fs::read_to_string(path)?;
        serde_yaml::from_str(&content).map_err(|e| DeploymentError::ConfigInvalid {
            file: path.display().to_string(),
            reason: e.to_string(),
        })
    }

    pub fn service_name(&self) -> &str {
        &self.metadata.name
    }

    pub fn location(&self) -> Option<&str> {
        self.metadata.labels.as_ref()
            .and_then(|l| l.get("cloud.googleapis.com/location"))
            .map(|s| s.as_str())
    }

    pub fn is_job(&self) -> bool {
        self.kind == "Job"
    }

    pub fn container_image(&self) -> &str {
        &self.spec.template.spec.containers[0].image
    }

    /// Replace the image placeholder with the actual built image tag.
    pub fn with_image(&self, image: &str) -> Self {
        let mut config = self.clone();
        config.spec.template.spec.containers[0].image = image.to_string();
        config
    }

    pub fn validate(&self) -> Result<(), DeploymentError> {
        if self.metadata.name.is_empty() {
            return Err(DeploymentError::ConfigInvalid {
                file: "service.yaml".into(),
                reason: "metadata.name is required".into(),
            });
        }
        if self.spec.template.spec.containers.is_empty() {
            return Err(DeploymentError::ConfigInvalid {
                file: "service.yaml".into(),
                reason: "at least one container is required".into(),
            });
        }
        Ok(())
    }
}
```

### GcpCloudRunProvider Implementation

```rust
// providers/gcp/mod.rs

pub struct GcpCloudRunProvider {
    mode: GcpMode,
    working_dir: PathBuf,
    project_id: Option<String>,
    region: Option<String>,
}

pub enum GcpMode {
    /// Shell out to gcloud CLI.
    Cli,
    /// Use Cloud Run Admin API directly.
    Api {
        access_token: String,
        project_id: String,
        region: String,
    },
}

impl GcpCloudRunProvider {
    pub fn cli(working_dir: &Path) -> Self;
    pub fn api(working_dir: &Path, access_token: &str, project_id: &str, region: &str) -> Self;

    /// Auto-detect from environment.
    /// Uses GOOGLE_CLOUD_PROJECT, GOOGLE_CLOUD_REGION, GOOGLE_APPLICATION_CREDENTIALS.
    pub fn auto(working_dir: &Path) -> Self;
}

impl DeploymentProvider for GcpCloudRunProvider {
    type Config = CloudRunServiceConfig;
    type Resources = GcpResources;

    fn name(&self) -> &str { "gcp" }

    fn detect(project_dir: &Path) -> Option<CloudRunServiceConfig> {
        let config_path = project_dir.join("service.yaml");
        CloudRunServiceConfig::load(&config_path).ok()
    }

    fn validate(&self, config: &CloudRunServiceConfig) -> Result<(), DeploymentError> {
        config.validate()
    }

    fn build(&self, config: &CloudRunServiceConfig, _env: Option<&str>) -> Result<BuildOutput, DeploymentError> {
        // Build container image
        // 1. Check for Dockerfile
        // 2. docker build -t {image_tag} .
        // 3. docker push {image_tag}
        // OR: gcloud builds submit --tag {image_tag}
        let image = config.container_image();
        if image == "IMAGE_PLACEHOLDER" {
            // Build from Dockerfile
            let tag = format!("gcr.io/{}/{}", self.project_id_or_detect(), config.service_name());
            let output = ProcessExecutor::new("docker")
                .args(["build", "-t", &tag, "."])
                .current_dir(&self.working_dir)
                .execute()?;
            if !output.success {
                return Err(DeploymentError::BuildFailed(output.stderr));
            }
            // Push
            ProcessExecutor::new("docker")
                .args(["push", &tag])
                .current_dir(&self.working_dir)
                .execute()?;
        }
        Ok(BuildOutput { artifacts: vec![], duration_ms: 0 })
    }

    fn deploy(
        &self,
        config: &CloudRunServiceConfig,
        env: Option<&str>,
        dry_run: bool,
    ) -> Result<DeploymentResult, DeploymentError> {
        match &self.mode {
            GcpMode::Cli => self.deploy_cli(config, env, dry_run),
            GcpMode::Api { access_token, project_id, region } => {
                self.deploy_api(config, env, dry_run, access_token, project_id, region)
            }
        }
    }

    fn logs(&self, config: &CloudRunServiceConfig, _env: Option<&str>) -> Result<(), DeploymentError> {
        ProcessExecutor::new("gcloud")
            .args(["run", "services", "logs", "read", config.service_name(), "--limit=100"])
            .current_dir(&self.working_dir)
            .execute_streaming(|line| println!("{}", line))?;
        Ok(())
    }

    fn destroy(&self, config: &CloudRunServiceConfig, _env: Option<&str>) -> Result<(), DeploymentError> {
        ProcessExecutor::new("gcloud")
            .args(["run", "services", "delete", config.service_name(), "--quiet"])
            .current_dir(&self.working_dir)
            .execute()?;
        Ok(())
    }

    fn status(&self, config: &CloudRunServiceConfig, _env: Option<&str>) -> Result<GcpResources, DeploymentError> {
        todo!()
    }
}
```

### CLI Mode

```rust
// providers/gcp/gcloud.rs

impl GcpCloudRunProvider {
    fn deploy_cli(
        &self,
        config: &CloudRunServiceConfig,
        env: Option<&str>,
        dry_run: bool,
    ) -> Result<DeploymentResult, DeploymentError> {
        if dry_run {
            return Ok(DeploymentResult::dry_run("gcp", config.service_name()));
        }

        // Write resolved service.yaml to temp file (with actual image tag)
        let resolved_yaml = serde_yaml::to_string(config)
            .map_err(|e| DeploymentError::ConfigInvalid {
                file: "service.yaml".into(),
                reason: e.to_string(),
            })?;

        // gcloud run services replace service.yaml --region {region}
        let mut cmd = ProcessExecutor::new("gcloud")
            .args(["run", "services", "replace", "-"]) // Read from stdin
            .current_dir(&self.working_dir);

        if let Some(region) = &self.region {
            cmd = cmd.args(["--region", region]);
        }

        // Execute and parse output
        let output = cmd.execute()?;
        if !output.success {
            return Err(DeploymentError::ProcessFailed {
                command: "gcloud run services replace".into(),
                exit_code: output.exit_code,
                stdout: output.stdout,
                stderr: output.stderr,
            });
        }

        Ok(DeploymentResult {
            deployment_id: chrono::Utc::now().timestamp().to_string(),
            provider: "gcp".to_string(),
            resource_name: config.service_name().to_string(),
            environment: env.map(String::from),
            url: extract_service_url(&output.stdout),
            deployed_at: chrono::Utc::now(),
        })
    }
}
```

### Cloud Run Admin API

```rust
// providers/gcp/api.rs

use foundation_core::simple_http::client::SimpleHttpClient;

const CLOUD_RUN_API_BASE: &str = "https://run.googleapis.com/v2";

impl GcpCloudRunProvider {
    fn deploy_api(
        &self,
        config: &CloudRunServiceConfig,
        env: Option<&str>,
        dry_run: bool,
        access_token: &str,
        project_id: &str,
        region: &str,
    ) -> Result<DeploymentResult, DeploymentError> {
        if dry_run {
            return Ok(DeploymentResult::dry_run("gcp", config.service_name()));
        }

        let parent = format!("projects/{}/locations/{}", project_id, region);
        let url = format!("{}/{}/services", CLOUD_RUN_API_BASE, parent);

        // Check if service exists (GET) or needs creation (POST)
        // Then create or update accordingly
        // Uses SimpleHttpClient with Bearer token auth

        // For Cloud Run Jobs:
        if config.is_job() {
            let jobs_url = format!("{}/{}/jobs", CLOUD_RUN_API_BASE, parent);
            // POST/PATCH to jobs endpoint
        }

        Ok(DeploymentResult {
            deployment_id: "api-deploy".to_string(),
            provider: "gcp".to_string(),
            resource_name: config.service_name().to_string(),
            environment: env.map(String::from),
            url: None, // Extracted from API response
            deployed_at: chrono::Utc::now(),
        })
    }
}

#[derive(Debug)]
pub struct GcpResources {
    pub service_name: String,
    pub url: Option<String>,
    pub latest_revision: Option<String>,
    pub traffic: Vec<TrafficAllocation>,
}

#[derive(Debug)]
pub struct TrafficAllocation {
    pub revision: String,
    pub percent: u32,
}
```

### Cloud Run Jobs Support

```rust
/// Cloud Run Jobs are detected by `kind: Job` in service.yaml.
/// Jobs use a different API path and support execution triggers.
impl GcpCloudRunProvider {
    fn deploy_job(
        &self,
        config: &CloudRunServiceConfig,
        env: Option<&str>,
    ) -> Result<DeploymentResult, DeploymentError> {
        ProcessExecutor::new("gcloud")
            .args(["run", "jobs", "replace", "service.yaml"])
            .current_dir(&self.working_dir)
            .execute()?;
        // ...
    }

    fn execute_job(
        &self,
        config: &CloudRunServiceConfig,
    ) -> Result<(), DeploymentError> {
        ProcessExecutor::new("gcloud")
            .args(["run", "jobs", "execute", config.service_name()])
            .current_dir(&self.working_dir)
            .execute()?;
        Ok(())
    }
}
```

## Tasks

1. **Create module structure**
   - [ ] Create `src/providers/gcp/mod.rs`, `config.rs`, `gcloud.rs`, `api.rs`
   - [ ] Register in `src/providers/mod.rs`
   - [ ] Add `serde_yaml` dependency

2. **Implement service.yaml parsing**
   - [ ] Define all Knative Service config structs
   - [ ] Implement `CloudRunServiceConfig::load()`
   - [ ] Implement `service_name()`, `location()`, `is_job()`, `container_image()`
   - [ ] Implement `with_image()` for image tag substitution
   - [ ] Implement `validate()`
   - [ ] Write unit tests with sample service.yaml files

3. **Implement GcpCloudRunProvider trait**
   - [ ] Implement `detect()`, `validate()`, `build()`, `deploy()`, `logs()`, `destroy()`, `status()`
   - [ ] Handle Cloud Run Services and Jobs via `kind` field
   - [ ] Auto-detect project ID and region from environment

4. **Implement CLI mode (gcloud wrapper)**
   - [ ] Implement `deploy_cli()` using `gcloud run services replace`
   - [ ] Handle image tag substitution in service.yaml
   - [ ] Parse gcloud output for service URL and revision
   - [ ] Write tests with mock output

5. **Implement API mode**
   - [ ] Implement `deploy_api()` using Cloud Run Admin API v2
   - [ ] Handle service create vs update
   - [ ] Handle job create/execute
   - [ ] Write tests with mock HTTP responses

6. **Container build support**
   - [ ] Detect Dockerfile in project
   - [ ] Build via `docker build` or `gcloud builds submit`
   - [ ] Push to GCR/Artifact Registry
   - [ ] Handle image tag generation

7. **Write integration tests**
   - [ ] Test service.yaml parsing with real configs
   - [ ] Test CLI deploy (requires gcloud, mark `#[ignore]`)
   - [ ] Test API deploy (requires GCP credentials, mark `#[ignore]`)

## GCP API Endpoints Used

| Method | Endpoint | Purpose |
|--------|----------|---------|
| `POST` | `/v2/projects/*/locations/*/services` | Create service |
| `PATCH` | `/v2/projects/*/locations/*/services/*` | Update service |
| `DELETE` | `/v2/projects/*/locations/*/services/*` | Delete service |
| `GET` | `/v2/projects/*/locations/*/services/*` | Get service |
| `POST` | `/v2/projects/*/locations/*/jobs` | Create job |
| `POST` | `/v2/projects/*/locations/*/jobs/*/executions` | Execute job |

## Success Criteria

- [ ] All 7 tasks completed
- [ ] service.yaml parsing handles Services and Jobs
- [ ] CLI deploy works with `gcloud run services replace`
- [ ] Image tag substitution works correctly
- [ ] Cloud Run Jobs support works

## Verification

```bash
cd backends/foundation_deployment
cargo test gcp -- --nocapture

# Integration (requires gcloud + GCP project)
cargo test gcp_integration -- --ignored --nocapture
```

---

_Created: 2026-03-26_
