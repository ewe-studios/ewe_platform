---
workspace_name: "ewe_platform"
spec_directory: "specifications/11-foundation-deployment"
feature_directory: "specifications/11-foundation-deployment/features/35-trait-based-deployments"
this_file: "specifications/11-foundation-deployment/features/35-trait-based-deployments/feature.md"

status: proposed
priority: high
created: 2026-04-11
updated: 2026-04-15

depends_on: ["01-foundation-deployment-core", "02-state-stores", "07-provider-api-clients"]

tasks:
  completed: 0
  uncompleted: 10
  total: 10
  completion_percentage: 0%
---


# Trait-Based Deployments

## Iron Law: Zero Warnings

> **All code must compile with zero warnings and pass all lints. No suppression. No exceptions.**
>
> - `cargo clippy -p foundation_deployment -- -D warnings -W clippy::pedantic` — zero warnings
> - `cargo doc -p foundation_deployment --no-deps` — zero rustdoc warnings
> - `cargo test -p foundation_deployment` — zero compilation warnings
> - **No `#[allow(...)]`, `#[expect(...)]`, or `#![allow(...)]` anywhere.** Fix the code, never suppress.

## Overview

Replace configuration-driven deployments with **pure Rust code**. Users define structs that implement the `Deployable` trait, specifying:
- What resources to deploy
- Which provider to use
- What deployment artifacts are returned

**No YAML, no TOML, no custom configuration format** — just Rust structs and trait implementations that return Valtron `TaskIterator`/`StreamIterator` types.

**Key Design:** The `Deployable` trait uses an associated `Store` type and receives `ProviderClient<Store>` as a parameter to all methods. This provides:
1. Type-safe state store access
2. Provider client with HTTP connection pooling
3. Unified interface for deploy and destroy operations

## The `ewe_deployables` Crate

Ready-to-use `Deployable` implementations are provided in a new crate:

```
crates/ewe_deployables/
├── Cargo.toml
└── src/
    ├── lib.rs           # Re-exports
    ├── cloudflare/
    │   ├── mod.rs
    │   └── worker.rs    # CloudflareWorker deployable
    ├── gcp/
    │   ├── mod.rs
    │   ├── cloud_run.rs # CloudRunService deployable
    │   └── cloud_job.rs # CloudRunJob deployable
    └── common/
        └── types.rs     # Shared deployment output types
```

**Usage:**

```rust
use ewe_deployables::cloudflare::CloudflareWorker;
use ewe_deployables::gcp::CloudRunService;

// Deploy a Cloudflare Worker
let worker = CloudflareWorker::new("my-worker", "./worker.js");
let stream = worker.deploy(client.clone())?;

// Deploy a GCP Cloud Run service
let service = CloudRunService::new("my-service", "gcr.io/project/image:latest");
let stream = service.deploy(client)?;
```

**Destroy uses state store:**

```rust
// Destroy reads state to get resource info
let destroy_stream = worker.destroy(client)?;
// State store provides: deployment_id, resource_name, bindings, etc.
```

## Architecture

### Example 1: Cloudflare Worker (`crates/ewe_deployables/src/cloudflare/worker.rs`)

```rust
use foundation_core::traits::Deploying;
use foundation_core::valtron::{execute, StreamIterator, TaskIterator, TaskIteratorExt, BoxedSendExecutionAction};
use foundation_deployment::provider_client::ProviderClient;
use foundation_db::state::FileStateStore;
use foundation_deployment::providers::cloudflare::api::cloudflare::CloudflareProvider;
use foundation_deployment::providers::cloudflare::clients::cloudflare::{
    worker_script_upload_worker_module, worker_script_delete_worker,
    types::{WorkerScriptUploadWorkerModuleArgs, WorkerScriptDeleteWorkerArgs},
};
use foundation_core::wire::simple_http::client::DnsResolver;
use serde::{Deserialize, Serialize};

/// Cloudflare Worker deployment output.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkerDeployment {
    pub account_id: String,
    pub script_name: String,
    pub deployment_id: String,
    pub url: String,
    pub deployed_at: chrono::DateTime<chrono::Utc>,
}

/// Cloudflare Worker deployable - deploys a worker script to Cloudflare Workers.
///
/// Reads the script from disk, deploys via Cloudflare API.
/// The generated client handles HTTP, serialization, and state persistence internally.
pub struct CloudflareWorker {
    pub name: String,
    pub script_path: String,
    pub account_id: String,
}

impl CloudflareWorker {
    /// Create a new Cloudflare Worker deployable.
    pub fn new(name: impl Into<String>, script_path: impl Into<String>, account_id: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            script_path: script_path.into(),
            account_id: account_id.into(),
        }
    }
}

impl Deployable for CloudflareWorker {
    type Output = WorkerDeployment;
    type Error = DeploymentError;
    type Store = FileStateStore;
    type Resolver = SystemDnsResolver;  // Or StaticSocketAddr for tests
    
    fn deploy(
        &self,
        client: ProviderClient<Self::Store, Self::Resolver>,
    ) -> Result<impl StreamIterator<D = Result<Self::Output, Self::Error>, P = Deploying> + Send + 'static, Self::Error> {
        self.deploy_task(client)
            .and_then(|task| execute(task, None))
            .map_err(|e| DeploymentError::ExecutorFailed(e.to_string()))
    }
    
    fn deploy_task(
        &self,
        client: ProviderClient<Self::Store, Self::Resolver>,
    ) -> Result<impl TaskIterator<Ready = Result<Self::Output, Self::Error>, Pending = Deploying, Spawner = BoxedSendExecutionAction> + Send + 'static, Self::Error> {
        // Read script content from disk
        let script = std::fs::read_to_string(&self.script_path)
            .map_err(|e| DeploymentError::IoFailed(format!("Failed to read script from {}: {}", self.script_path, e)))?;
        
        // Create CloudflareProvider from client - extracts HTTP client automatically
        let cloudflare = CloudflareProvider::from_provider_client(client);
        
        // Use generated Cloudflare client - handles HTTP, state persistence internally
        let result = cloudflare.worker_script_upload_worker_module(&WorkerScriptUploadWorkerModuleArgs {
            account_id: self.account_id.clone(),
            script_name: self.name.clone(),
            bindings_inherit: Some(true),
            // Script content passed in request body
        })?;
        
        // Transform API response into deployment output
        Ok(result.map(|response| WorkerDeployment {
            account_id: self.account_id.clone(),
            script_name: self.name.clone(),
            deployment_id: response.result.id.clone(),
            url: format!("https://{}.workers.dev", self.name),
            deployed_at: chrono::Utc::now(),
        }))
    }
    
    fn destroy(
        &self,
        client: ProviderClient<Self::Store, Self::Resolver>,
    ) -> Result<impl StreamIterator<D = Result<(), Self::Error>, P = Deploying> + Send + 'static, Self::Error> {
        self.destroy_task(client)
            .and_then(|task| execute(task, None))
            .map_err(|e| DeploymentError::ExecutorFailed(e.to_string()))
    }
    
    fn destroy_task(
        &self,
        client: ProviderClient<Self::Store, Self::Resolver>,
    ) -> Result<impl TaskIterator<Ready = Result<(), Self::Error>, Pending = Deploying, Spawner = BoxedSendExecutionAction> + Send + 'static, Self::Error> {
        use foundation_core::valtron::ThreadedValue;
        use foundation_db::state::traits::StateStore;
        
        let state_store = client.state_store();
        let resource_id = format!("cloudflare:worker:{}:{}", client.project(), self.name);
        
        // Read state from store to confirm resource was deployed through this system
        let existing_state = state_store.get(&resource_id)
            .map_err(|e| DeploymentError::StateFailed(format!("Failed to get state for {}: {}", resource_id, e)))?
            .find_map(|v| match v {
                ThreadedValue::Value(Ok(state)) => Some(state),
                ThreadedValue::Value(Err(e)) => {
                    tracing::warn!("State store error during destroy for {}: {}", resource_id, e);
                    None
                }
                _ => None,
            })
            .flatten();
        
        match existing_state {
            Some(state) => {
                // Deserialize stored state into typed output
                let output: WorkerDeployment = serde_json::from_value(state.output.clone())
                    .map_err(|e| DeploymentError::StateFailed(format!("Failed to deserialize state: {}", e)))?;
                
                // Create CloudflareProvider from client
                let cloudflare = CloudflareProvider::from_provider_client(client);
                
                // Use the script_name from stored state - ensures we delete exactly what was deployed
                let result = cloudflare.worker_script_delete_worker(&WorkerScriptDeleteWorkerArgs {
                    account_id: output.account_id,
                    script_name: output.script_name,
                    force: None,
                })?;
                
                Ok(result.map(|_| ()))
            }
            None => {
                // No state found - resource was never deployed through this system
                // Return idempotent success (destroy is a no-op)
                tracing::warn!("No state found for worker '{}' - skipping destroy (idempotent)", self.name);
                Ok(Box::new(std::iter::once(Stream::Next(Ok(())))))
            }
        }
    }
}
```

### Example 2: GCP Cloud Run Service (`crates/ewe_deployables/src/gcp/cloud_run.rs`)

```rust
use foundation_core::traits::Deploying;
use foundation_core::valtron::{execute, StreamIterator, TaskIteratorExt, BoxedSendExecutionAction};
use foundation_deployment::provider_client::ProviderClient;
use foundation_db::state::FileStateStore;
use foundation_deployment::providers::gcp::api::run::RunProvider;
use foundation_deployment::providers::gcp::clients::run::{
    RunProjectsLocationsServicesPatchArgs, RunProjectsLocationsServicesDeleteArgs,
};
use foundation_core::wire::simple_http::client::DnsResolver;
use serde::{Deserialize, Serialize};

/// GCP Cloud Run deployment output.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CloudRunDeployment {
    pub resource_name: String,  // Full resource path: projects/.../services/...
    pub service_name: String,
    pub region: String,
    pub project_id: String,
    pub url: String,
    pub image: String,
    pub deployed_at: chrono::DateTime<chrono::Utc>,
}

/// GCP Cloud Run service deployable - deploys a container to Cloud Run.
pub struct CloudRunService {
    pub name: String,
    pub image: String,
    pub region: String,
    pub project_id: String,
}

impl CloudRunService {
    /// Create a new Cloud Run service deployable.
    pub fn new(
        name: impl Into<String>,
        image: impl Into<String>,
        region: impl Into<String>,
        project_id: impl Into<String>,
    ) -> Self {
        Self {
            name: name.into(),
            image: image.into(),
            region: region.into(),
            project_id: project_id.into(),
        }
    }
}

impl Deployable for CloudRunService {
    type Output = CloudRunDeployment;
    type Error = DeploymentError;
    type Store = FileStateStore;
    type Resolver = SystemDnsResolver;
    
    fn deploy(
        &self,
        client: ProviderClient<Self::Store, Self::Resolver>,
    ) -> Result<impl StreamIterator<D = Result<Self::Output, Self::Error>, P = Deploying> + Send + 'static, Self::Error> {
        self.deploy_task(client)
            .and_then(|task| execute(task, None))
            .map_err(|e| DeploymentError::ExecutorFailed(e.to_string()))
    }
    
    fn deploy_task(
        &self,
        client: ProviderClient<Self::Store, Self::Resolver>,
    ) -> Result<impl TaskIterator<Ready = Result<Self::Output, Self::Error>, Pending = Deploying, Spawner = BoxedSendExecutionAction> + Send + 'static, Self::Error> {
        let name = format!("projects/{}/locations/{}/services/{}", self.project_id, self.region, self.name);
        
        // Create RunProvider from client - extracts HTTP client automatically
        let run = RunProvider::from_provider_client(client);
        
        // Use generated GCP Cloud Run client
        let result = run.run_projects_locations_services_patch(&RunProjectsLocationsServicesPatchArgs {
            name,
            allow_missing: Some(true),  // Create if doesn't exist
            force_new_revision: Some(true),
            update_mask: Some("spec.template.spec.containers".to_string()),
            validate_only: None,
            // Body with container image
            body: serde_json::json!({
                "spec": {
                    "template": {
                        "spec": {
                            "containers": [{
                                "image": self.image,
                            }]
                        }
                    }
                }
            }),
        })?;
        
        Ok(result.map(|operation| CloudRunDeployment {
            resource_name: format!("projects/{}/locations/{}/services/{}", self.project_id, self.region, self.name),
            service_name: self.name.clone(),
            region: self.region.clone(),
            project_id: self.project_id.clone(),
            url: format!("https://{}-{}.a.run.app", self.name, self.region),
            image: self.image.clone(),
            deployed_at: chrono::Utc::now(),
        }))
    }
    
    fn destroy(
        &self,
        client: ProviderClient<Self::Store, Self::Resolver>,
    ) -> Result<impl StreamIterator<D = Result<(), Self::Error>, P = Deploying> + Send + 'static, Self::Error> {
        self.destroy_task(client)
            .and_then(|task| execute(task, None))
            .map_err(|e| DeploymentError::ExecutorFailed(e.to_string()))
    }
    
    fn destroy_task(
        &self,
        client: ProviderClient<Self::Store, Self::Resolver>,
    ) -> Result<impl TaskIterator<Ready = Result<(), Self::Error>, Pending = Deploying, Spawner = BoxedSendExecutionAction> + Send + 'static, Self::Error> {
        use foundation_core::valtron::ThreadedValue;
        use foundation_db::state::traits::StateStore;
        
        let state_store = client.state_store();
        let resource_id = format!("gcp:cloud-run:{}:{}:{}:{}", client.project(), self.project_id, self.region, self.name);
        
        // Read state from store to confirm resource was deployed through this system
        let existing_state = state_store.get(&resource_id)
            .map_err(|e| DeploymentError::StateFailed(format!("Failed to get state for {}: {}", resource_id, e)))?
            .find_map(|v| match v {
                ThreadedValue::Value(Ok(state)) => Some(state),
                ThreadedValue::Value(Err(e)) => {
                    tracing::warn!("State store error during destroy for {}: {}", resource_id, e);
                    None
                }
                _ => None,
            })
            .flatten();
        
        match existing_state {
            Some(state) => {
                // Deserialize stored state into typed output
                let output: CloudRunDeployment = serde_json::from_value(state.output.clone())
                    .map_err(|e| DeploymentError::StateFailed(format!("Failed to deserialize state: {}", e)))?;
                
                // Create RunProvider from client
                let run = RunProvider::from_provider_client(client);
                
                // Use the full resource name from stored state
                let result = run.run_projects_locations_services_delete(&RunProjectsLocationsServicesDeleteArgs {
                    name: output.resource_name,
                    etag: None,
                    validate_only: None,
                })?;
                
                Ok(result.map(|_| ()))
            }
            None => {
                // No state - idempotent success
                tracing::warn!("No state found for Cloud Run service '{}' - skipping destroy (idempotent)", self.name);
                Ok(Box::new(std::iter::once(Stream::Next(Ok(())))))
            }
        }
    }
}
```

### Example 3: GCP Cloud Run Job (`crates/ewe_deployables/src/gcp/cloud_job.rs`)

```rust
use foundation_core::traits::Deploying;
use foundation_core::valtron::{execute, StreamIterator, TaskIteratorExt, BoxedSendExecutionAction};
use foundation_deployment::provider_client::ProviderClient;
use foundation_db::state::FileStateStore;
use foundation_deployment::providers::gcp::api::run::RunProvider;
use foundation_deployment::providers::gcp::clients::run::{
    RunProjectsLocationsJobsCreateArgs, RunProjectsLocationsJobsDeleteArgs,
};
use serde::{Deserialize, Serialize};

/// GCP Cloud Run Job deployment output.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CloudRunJobDeployment {
    pub resource_name: String,  // Full resource path: projects/.../jobs/...
    pub job_name: String,
    pub region: String,
    pub project_id: String,
    pub deployed_at: chrono::DateTime<chrono::Utc>,
}

/// GCP Cloud Run Job deployable - creates/schedules a Cloud Run Job.
pub struct CloudRunJob {
    pub name: String,
    pub image: String,
    pub region: String,
    pub project_id: String,
    pub schedule: Option<String>,  // Optional cron schedule
    pub command: Option<Vec<String>>,
}

impl CloudRunJob {
    /// Create a new Cloud Run Job deployable.
    pub fn new(
        name: impl Into<String>,
        image: impl Into<String>,
        region: impl Into<String>,
        project_id: impl Into<String>,
    ) -> Self {
        Self {
            name: name.into(),
            image: image.into(),
            region: region.into(),
            project_id: project_id.into(),
            schedule: None,
            command: None,
        }
    }
    
    /// Set the cron schedule for this job.
    pub fn with_schedule(mut self, schedule: impl Into<String>) -> Self {
        self.schedule = Some(schedule.into());
        self
    }
    
    /// Set the command to run in the job container.
    pub fn with_command(mut self, command: Vec<String>) -> Self {
        self.command = Some(command);
        self
    }
}

impl Deployable for CloudRunJob {
    type Output = CloudRunJobDeployment;
    type Error = DeploymentError;
    type Store = FileStateStore;
    type Resolver = SystemDnsResolver;
    
    fn deploy(
        &self,
        client: ProviderClient<Self::Store, Self::Resolver>,
    ) -> Result<impl StreamIterator<D = Result<Self::Output, Self::Error>, P = Deploying> + Send + 'static, Self::Error> {
        self.deploy_task(client)
            .and_then(|task| execute(task, None))
            .map_err(|e| DeploymentError::ExecutorFailed(e.to_string()))
    }
    
    fn deploy_task(
        &self,
        client: ProviderClient<Self::Store, Self::Resolver>,
    ) -> Result<impl TaskIterator<Ready = Result<Self::Output, Self::Error>, Pending = Deploying, Spawner = BoxedSendExecutionAction> + Send + 'static, Self::Error> {
        let run = RunProvider::from_provider_client(client);
        let parent = format!("projects/{}/locations/{}", self.project_id, self.region);
        
        // Build job spec
        let mut job_spec = serde_json::json!({
            "template": {
                "spec": {
                    "containers": [{
                        "image": self.image,
                    }]
                }
            }
        });
        
        // Add optional command
        if let Some(ref cmd) = self.command {
            job_spec["template"]["spec"]["containers"][0]["command"] = serde_json::json!(cmd);
        }
        
        // Add optional schedule
        if let Some(ref schedule) = self.schedule {
            job_spec["schedule"] = serde_json::json!({ "cron": schedule });
        }
        
        // Use generated GCP Cloud Run client - handles HTTP, state persistence internally
        let result = run.run_projects_locations_jobs_create(&RunProjectsLocationsJobsCreateArgs {
            parent,
            job_id: self.name.clone(),
            body: job_spec,
            validate_only: None,
        })?;
        
        Ok(result.map(|operation| CloudRunJobDeployment {
            resource_name: format!("projects/{}/locations/{}/jobs/{}", self.project_id, self.region, self.name),
            job_name: self.name.clone(),
            region: self.region.clone(),
            project_id: self.project_id.clone(),
            deployed_at: chrono::Utc::now(),
        }))
    }
    
    fn destroy(
        &self,
        client: ProviderClient<Self::Store, Self::Resolver>,
    ) -> Result<impl StreamIterator<D = Result<(), Self::Error>, P = Deploying> + Send + 'static, Self::Error> {
        self.destroy_task(client)
            .and_then(|task| execute(task, None))
            .map_err(|e| DeploymentError::ExecutorFailed(e.to_string()))
    }
    
    fn destroy_task(
        &self,
        client: ProviderClient<Self::Store, Self::Resolver>,
    ) -> Result<impl TaskIterator<Ready = Result<(), Self::Error>, Pending = Deploying, Spawner = BoxedSendExecutionAction> + Send + 'static, Self::Error> {
        use foundation_core::valtron::ThreadedValue;
        use foundation_db::state::traits::StateStore;
        
        let state_store = client.state_store();
        let resource_id = format!("gcp:cloud-job:{}:{}:{}:{}", client.project(), self.project_id, self.region, self.name);
        
        // Read state from store to confirm resource was deployed through this system
        let existing_state = state_store.get(&resource_id)
            .map_err(|e| DeploymentError::StateFailed(format!("Failed to get state for {}: {}", resource_id, e)))?
            .find_map(|v| match v {
                ThreadedValue::Value(Ok(state)) => Some(state),
                ThreadedValue::Value(Err(e)) => {
                    tracing::warn!("State store error during destroy for {}: {}", resource_id, e);
                    None
                }
                _ => None,
            })
            .flatten();
        
        match existing_state {
            Some(state) => {
                // Deserialize stored state into typed output
                let output: CloudRunJobDeployment = serde_json::from_value(state.output.clone())
                    .map_err(|e| DeploymentError::StateFailed(format!("Failed to deserialize state: {}", e)))?;
                
                // Create RunProvider from client
                let run = RunProvider::from_provider_client(client);
                let result = run.run_projects_locations_jobs_delete(&RunProjectsLocationsJobsDeleteArgs {
                    name: output.resource_name,
                    etag: None,
                    validate_only: None,
                })?;
                
                Ok(result.map(|_| ()))
            }
            None => {
                // No state - idempotent success
                tracing::warn!("No state found for Cloud Run Job '{}' - skipping destroy (idempotent)", self.name);
                Ok(Box::new(std::iter::once(Stream::Next(Ok(())))))
            }
        }
    }
}
```

### Deployable Trait Definition

```rust
// foundation_core/traits.rs

use foundation_core::valtron::{TaskIterator, StreamIterator, BoxedSendExecutionAction};
use foundation_deployment::provider_client::ProviderClient;
use foundation_db::state::traits::StateStore;
use foundation_core::wire::simple_http::client::DnsResolver;

/// Generic progress states for deployment and destroy execution.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Deploying {
    #[default]
    Init,
    Processing,
    Done,
    Failed,
}

/// Trait for deployable and destroyable infrastructure.
///
/// Users implement this on their own structs to define deployment and destruction logic.
/// The associated `Output` type contains deployment artifacts (URLs, IDs, etc.).
/// The associated `Error` type is the error type for this deployment.
/// The associated `Store` type is the state store implementation used for persistence.
/// The associated `Resolver` type is the DNS resolver used for HTTP calls.
///
/// Using an associated type for `Resolver` simplifies the trait signature:
/// - Production: `type Resolver = SystemDnsResolver`
/// - Test: `type Resolver = StaticSocketAddr`
pub trait Deployable {
    /// Deployment output type — contains URLs, IDs, and other artifacts.
    type Output: Send + Sync;
    
    /// Error type for this deployment.
    type Error: std::error::Error + Send + Sync + std::fmt::Debug;
    
    /// State store type for persistence.
    type Store: StateStore + Send + Sync + 'static;
    
    /// DNS resolver type for HTTP calls.
    type Resolver: DnsResolver + Clone + 'static;
    
    /// Deploy the resource and return a StreamIterator with the result.
    fn deploy(
        &self,
        client: ProviderClient<Self::Store, Self::Resolver>,
    ) -> Result<
        impl StreamIterator<D = Result<Self::Output, Self::Error>, P = Deploying> + Send + 'static,
        Self::Error,
    >;
    
    /// Deploy the resource and return a TaskIterator for customization.
    fn deploy_task(
        &self,
        client: ProviderClient<Self::Store, Self::Resolver>,
    ) -> Result<
        impl TaskIterator<Ready = Result<Self::Output, Self::Error>, Pending = Deploying, Spawner = BoxedSendExecutionAction> + Send + 'static,
        Self::Error,
    >;
    
    /// Destroy the resource and return a StreamIterator with the result.
    fn destroy(
        &self,
        client: ProviderClient<Self::Store, Self::Resolver>,
    ) -> Result<
        impl StreamIterator<D = Result<(), Self::Error>, P = Deploying> + Send + 'static,
        Self::Error,
    >;
    
    /// Destroy the resource and return a TaskIterator for customization.
    fn destroy_task(
        &self,
        client: ProviderClient<Self::Store, Self::Resolver>,
    ) -> Result<
        impl TaskIterator<Ready = Result<(), Self::Error>, Pending = Deploying, Spawner = BoxedSendExecutionAction> + Send + 'static,
        Self::Error,
    >;
}
```

## Requirements

### ProviderClient Structure

`ProviderClient<S, R>` wraps the state store and HTTP client, providing access to both state persistence and API functionality:

```rust
// foundation_deployment/provider_client.rs

pub struct ProviderClient<S, R>
where
    S: StateStore + Send + Sync + 'static,
    R: DnsResolver + Clone + 'static,
{
    pub state_store: Arc<S>,
    pub project: String,
    pub stage: String,
    pub http_client: Arc<SimpleHttpClient<R>>,
}

impl<S, R> Clone for ProviderClient<S, R>
where
    S: StateStore + Send + Sync + 'static,
    R: DnsResolver + Clone + 'static,
{
    fn clone(&self) -> Self {
        Self {
            state_store: self.state_store.clone(),
            project: self.project.clone(),
            stage: self.stage.clone(),
            http_client: self.http_client.clone(),
        }
    }
}

impl<S, R> ProviderClient<S, R>
where
    S: StateStore + Send + Sync + 'static,
    R: DnsResolver + Clone + 'static,
{
    pub fn new(project: &str, stage: &str, state_store: S, http_client: SimpleHttpClient<R>) -> Self;
    pub fn state_store(&self) -> &S;
    pub fn project(&self) -> &str;
    pub fn stage(&self) -> &str;
    pub fn http_client(&self) -> &SimpleHttpClient<R>;
}
```

**Key design decisions:**

- **Generic over `R: DnsResolver + Clone`** - Allows using `SystemDnsResolver` in production and `StaticSocketAddr` in tests
- **`Arc<SimpleHttpClient<R>>`** - HTTP client is wrapped in `Arc` for cheap cloning and connection pool sharing across provider instances
- **Manual `Clone` impl** - Required due to `Arc` fields; `Debug` not derived to avoid requiring `Debug` bounds on generics

**Usage in Deployable implementations:**

```rust
impl Deployable for MyWorker {
    type Store = FileStateStore;
    
    fn deploy_task(
        &self,
        client: ProviderClient<Self::Store, impl DnsResolver + Clone + 'static>,
    ) -> Result<...> {
        // Access state store
        let state_store = client.state_store();
        
        // Access HTTP client for API calls
        let http_client = client.http_client();
        
        // Create provider from client - extracts HTTP client automatically
        let cloudflare = CloudflareProvider::from_provider_client(client);
        
        // Use generated task function
        let task = cloudflare.worker_script_upload_worker_module(&args)?;
        // ...
    }
}
```

**Usage in Deployable implementations:**

```rust
impl Deployable for MyWorker {
    type Store = FileStateStore;
    
    fn deploy_task(&self, client: ProviderClient<Self::Store>) -> Result<...> {
        // Access state store directly
        let state_store = client.state_store();
        
        // Pass to generated task function
        let task = put_workers_script_task(state_store, args)?;
        // ...
    }
}
```

### Deployable Trait

```rust
// foundation_core/traits.rs

use foundation_core::valtron::{TaskIterator, StreamIterator, BoxedSendExecutionAction};
use foundation_deployment::provider_client::ProviderClient;
use foundation_db::state::traits::StateStore;

/// Generic progress states for deployment and destroy execution.
///
/// All implementations use this same enum - no need to define custom Pending types.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Deploying {
    #[default]
    Init,
    Processing,
    Done,
    Failed,
}

/// Trait for deployable and destroyable infrastructure.
///
/// Users implement this on their own structs to define deployment and destruction logic.
/// The associated `Output` type contains deployment artifacts (URLs, IDs, etc.).
/// The associated `Error` type is the error type for this deployment.
/// The associated `Store` type is the state store implementation used for persistence.
///
/// The trait provides four methods:
/// - `deploy()` - Executes the deploy task via valtron and returns a `StreamIterator`
/// - `deploy_task()` - Returns a `TaskIterator` containing the deployment logic
/// - `destroy()` - Executes the destroy task via valtron and returns a `StreamIterator`
/// - `destroy_task()` - Returns a `TaskIterator` containing the destroy logic
///
/// All methods receive `ProviderClient<Self::Store>` as a parameter, providing:
/// - Access to the state store for persistence and change detection
/// - HTTP client for API calls
/// - Type-safe access to provider-specific resources
pub trait Deployable {
    /// Deployment output type — contains URLs, IDs, and other artifacts.
    type Output: Send + Sync;
    
    /// Error type for this deployment.
    type Error: std::error::Error + Send + Sync + std::fmt::Debug;
    
    /// State store type for persistence.
    type Store: StateStore + Send + Sync + 'static;
    
    /// Deploy the resource and return a StreamIterator with the result.
    ///
    /// This is the convenience method that creates the task via `deploy_task()`
    /// and executes it via valtron's `execute()`.
    ///
    /// # Arguments
    ///
    /// * `client` - ProviderClient with access to state store and HTTP client
    ///
    /// # Returns
    ///
    /// Returns `Ok(StreamIterator)` which yields `Result<Output, Error>` when iterated.
    fn deploy(
        &self,
        client: ProviderClient<Self::Store>,
    ) -> Result<
        impl StreamIterator<
                D = Result<Self::Output, Self::Error>,
                P = Deploying,
            > + Send
            + 'static,
        Self::Error,
    >;
    
    /// Deploy the resource and return a TaskIterator for customization.
    ///
    /// This is the core method that contains the actual deployment logic.
    /// Users can call this directly when they need to compose tasks or
    /// apply custom valtron combinators before execution.
    ///
    /// # Arguments
    ///
    /// * `client` - ProviderClient with access to state store and HTTP client
    ///
    /// # Returns
    ///
    /// Returns `Ok(TaskIterator)` which can be executed via `execute()`.
    fn deploy_task(
        &self,
        client: ProviderClient<Self::Store>,
    ) -> Result<
        impl TaskIterator<
                Ready = Result<Self::Output, Self::Error>,
                Pending = Deploying,
                Spawner = BoxedSendExecutionAction,
            > + Send
            + 'static,
        Self::Error,
    >;
    
    /// Destroy the resource and return a StreamIterator with the result.
    ///
    /// This is the convenience method that creates the task via `destroy_task()`
    /// and executes it via valtron's `execute()`.
    ///
    /// # Arguments
    ///
    /// * `client` - ProviderClient with access to state store and HTTP client
    ///
    /// # Returns
    ///
    /// Returns `Ok(StreamIterator)` which yields `Result<(), Error>` when iterated.
    fn destroy(
        &self,
        client: ProviderClient<Self::Store>,
    ) -> Result<
        impl StreamIterator<
                D = Result<(), Self::Error>,
                P = Deploying,
            > + Send
            + 'static,
        Self::Error,
    >;
    
    /// Destroy the resource and return a TaskIterator for customization.
    ///
    /// This is the core method that contains the actual destroy logic.
    /// Users can call this directly when they need to compose tasks or
    /// apply custom valtron combinators before execution.
    ///
    /// # Arguments
    ///
    /// * `client` - ProviderClient with access to state store and HTTP client
    ///
    /// # Returns
    ///
    /// Returns `Ok(TaskIterator)` which can be executed via `execute()`.
    fn destroy_task(
        &self,
        client: ProviderClient<Self::Store>,
    ) -> Result<
        impl TaskIterator<
                Ready = Result<(), Self::Error>,
                Pending = Deploying,
                Spawner = BoxedSendExecutionAction,
            > + Send
            + 'static,
        Self::Error,
    >;
}
```

### User Implementation Pattern

```rust
// Users receive ProviderClient<Store> as argument - no need to construct it

impl Deployable for MyWorker {
    type Output = WorkerDeployment;
    type Error = ApiError;
    type Store = FileStateStore;  // Specify the state store type
    
    fn deploy(
        &self,
        client: ProviderClient<Self::Store>,
    ) -> Result<
        impl StreamIterator<D = Result<Self::Output, Self::Error>, P = Deploying> + Send + 'static,
        Self::Error,
    > {
        use foundation_core::valtron::execute;
        self.deploy_task(client)
            .and_then(|task| execute(task, None).map_err(|e| {
                Self::Error::from(std::io::Error::new(
                    std::io::ErrorKind::Other,
                    format!("Valtron execution failed: {}", e),
                ))
            }))
    }
    
    fn deploy_task(
        &self,
        client: ProviderClient<Self::Store>,
    ) -> Result<
        impl TaskIterator<Ready = Result<Self::Output, Self::Error>, Pending = Deploying, Spawner = BoxedSendExecutionAction> + Send + 'static,
        Self::Error,
    > {
        // Access state store through client
        let state_store = client.state_store();
        
        // Use generated provider client task function
        let task = put_workers_script_task(state_store, &PutWorkersScriptArgs {
            name: self.name.clone(),
            script: self.script.clone(),
            ..Default::default()
        })?;
        
        // Transform provider response into deployment output
        Ok(task.map_done(|result| {
            result.map(|response| WorkerDeployment {
                deployment_id: response.id.clone(),
                worker_name: self.name.clone(),
                url: format!("https://{}.workers.dev", self.name),
                deployed_at: chrono::Utc::now(),
            })
        }))
    }
    
    fn destroy(
        &self,
        client: ProviderClient<Self::Store>,
    ) -> Result<
        impl StreamIterator<D = Result<(), Self::Error>, P = Deploying> + Send + 'static,
        Self::Error,
    > {
        self.destroy_task(client)
            .and_then(|task| execute(task, None).map_err(|e| {
                Self::Error::from(std::io::Error::new(
                    std::io::ErrorKind::Other,
                    format!("Valtron execution failed: {}", e),
                ))
            }))
    }
    
    fn destroy_task(
        &self,
        client: ProviderClient<Self::Store>,
    ) -> Result<
        impl TaskIterator<Ready = Result<(), Self::Error>, Pending = Deploying, Spawner = BoxedSendExecutionAction> + Send + 'static,
        Self::Error,
    > {
        let task = delete_workers_script_task(client.state_store(), &DeleteWorkersScriptArgs {
            name: self.name.clone(),
        })?;
        
        Ok(task.map_done(|result| result.map_err(Into::into)))
    }
}
```

### Usage Pattern

```rust
// Create state store
let state_store = FileStateStore::new("/path/to/state", "my-project", "dev")?;

// Create provider client with the store
let client = ProviderClient::new("my-project", "dev", state_store);

// Create deployable resource
let worker = MyWorker {
    name: "my-worker".into(),
    script: include_str!("worker.js").into(),
};

// Deploy with client
let stream = worker.deploy(client.clone())?;
for item in stream {
    if let Stream::Next(result) = item {
        let deployment = result?;
        println!("Deployed {} to {}", deployment.worker_name, deployment.url);
    }
}

// Later: destroy with same client pattern
let destroy_stream = worker.destroy(client)?;
for item in destroy_stream {
    if let Stream::Next(result) = item {
        result?;  // () on success, Error on failure
        println!("Destroyed worker");
    }
}
```

### Common Deployment Output Types

```rust
// foundation_deployment/types.rs

/// Common deployment output for Cloudflare Workers.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkerDeployment {
    pub deployment_id: String,
    pub worker_name: String,
    pub url: String,
    pub deployed_at: chrono::DateTime<chrono::Utc>,
}

/// Common deployment output for GCP Cloud Run.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CloudRunDeployment {
    pub deployment_id: String,
    pub service_name: String,
    pub url: String,
    pub image: String,
    pub deployed_at: chrono::DateTime<chrono::Utc>,
}

/// Common deployment output for AWS Lambda.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LambdaDeployment {
    pub deployment_id: String,  // Function ARN
    pub function_name: String,
    pub url: Option<String>,     // Function URL if configured
    pub runtime: String,
    pub deployed_at: chrono::DateTime<chrono::Utc>,
}

/// Generic deployment output when you just need basic info.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeploymentOutput {
    pub id: String,
    pub name: String,
    pub url: Option<String>,
    pub metadata: serde_json::Value,
    pub deployed_at: chrono::DateTime<chrono::Utc>,
}
```

### Provider Clients

Provider clients in `backends/foundation_deployment/src/providers/` expose valtron-native functions that take `&dyn StateStore` as first argument:

- `cloudflare::clients::*` — Cloudflare Workers, KV, D1, R2
- `gcp::clients::*` — Cloud Run, Cloud Run Jobs
- `aws::clients::*` — Lambda, S3, API Gateway

Each generated client provides task functions per endpoint:

```rust
// Function signature pattern for all generated clients
fn put_workers_script_task(
    state_store: &dyn StateStore,
    args: &PutWorkersScriptArgs,
) -> Result<impl TaskIterator<...>, ApiError>;

fn delete_workers_script_task(
    state_store: &dyn StateStore,
    args: &DeleteWorkersScriptArgs,
) -> Result<impl TaskIterator<...>, ApiError>;
```

Users call these from their `deploy_task()` and `destroy_task()` implementations, passing `client.state_store()`.

## Implementation Changes Summary

### Phase 1: Core Trait and ProviderClient Updates

**Completed:**
- Added `Resolver` associated type to `Deployable` trait alongside `Store`, `Output`, and `Error` types
- Updated `ProviderClient<S, R>` to be generic over DNS resolver type `R: DnsResolver + Clone + 'static`
- Added `Arc<SimpleHttpClient<R>>` to `ProviderClient` for shared HTTP client access
- Manual `Clone` implementation for `ProviderClient` (no `Debug` derive to avoid generic bounds issues)
- Added `http_client()` getter method to `ProviderClient`

### Phase 2: Generator Updates

**Client Generator (`bin/platform/src/gen_resources/clients.rs`):**
- Made all builder functions generic over DNS resolver type `R` instead of hardcoding `SystemDnsResolver`
- Changed signature from `ClientRequestBuilder<SystemDnsResolver>` to `ClientRequestBuilder<R>`
- Added `DnsResolver` to imports in generated code
- Fixed deduplication logic to check ALL generated function names (`_builder`, `_task`, `_execute`, and base function) to prevent collisions
  - Previous logic only checked Args struct names, missing function name collisions
  - New logic prevents endpoints like `/tools` and `/tools/{id}:execute` from generating conflicting functions

**Provider Wrapper Generator (`bin/platform/src/gen_resources/provider_wrappers.rs`):**
- Updated `new()` to accept `Arc<SimpleHttpClient<R>>` instead of creating its own
- Added `from_provider_client()` convenience method that extracts HTTP client from `ProviderClient`
- Made providers generic over `R: DnsResolver + Clone + 'static`
- Updated all generated providers to use `from_provider_client()` pattern

### Phase 3: Regenerated Providers

All providers regenerated with new patterns:
- **Cloudflare**: `CloudflareProvider<S, R>` with `from_provider_client()` method
- **GCP**: All API providers (`RunProvider`, `CloudRunProvider`, etc.) updated
- **Fly.io**, **Neon**, **PlanetScale**, **Prisma Postgres**, **Stripe**, **Supabase**: All updated

### Phase 4: Test Infrastructure Support

The generic DNS resolver pattern enables test mocking:
- Production: `type Resolver = SystemDnsResolver` for real API calls
- Tests: `type Resolver = StaticSocketAddr` to redirect all API calls to `TestHttpServer`

Example test pattern:
```rust
#[test]
fn test_worker_deploy() {
    let (client, _guard) = test_client_with_server(
        StaticSocketAddr::new(server.local_addr()),
        "my-project",
        "test",
    );
    
    let provider = CloudflareProvider::from_provider_client(client);
    // All API calls go to test server
}
```

### Key Design Decisions

1. **Associated type vs `impl Trait`**: Used `type Resolver` associated type instead of `impl DnsResolver + Clone` in trait definition because:
   - `impl Trait` in parameter position is invalid syntax in trait definitions
   - Associated types provide cleaner implementation syntax
   - Users specify resolver type once in impl block

2. **`Arc<SimpleHttpClient>`**: Wrapped HTTP client in `Arc` for:
   - Cheap cloning of `ProviderClient`
   - Connection pool sharing across provider instances
   - Thread-safe sharing in async contexts

3. **`from_provider_client()` pattern**: Added convenience method because:
   - Reduces boilerplate in deployable implementations
   - Automatically extracts HTTP client from `ProviderClient`
   - Ensures consistent provider construction

4. **Function name deduplication**: Fixed by checking all generated function names because:
   - Endpoints with `:execute` suffix in path create naming conflicts
   - `/tools` generates `tools_execute` function
   - `/tools/{id}:execute` generates `tools_execute` convenience function
   - Collision detection prevents duplicate definitions

## Tasks

1. **Update Deployable trait definition**
   - [ ] Add `Store` associated type to trait
   - [ ] Add `client: ProviderClient<Self::Store>` parameter to `deploy()` and `deploy_task()`
   - [ ] Add `destroy()` and `destroy_task()` methods with same pattern
   - [ ] Update `Deploying` enum if needed for destroy states

2. **Update ProviderClient**
   - [ ] Ensure `state_store()` method returns `&S`
   - [ ] Verify `ProviderClient` is re-exported from crate root
   - [ ] Add `Clone` impl if needed for repeated use

3. **Update generated provider clients**
   - [ ] Task functions take `&dyn StateStore` as first argument
   - [ ] Both deploy and destroy task functions generated
   - [ ] Update examples in documentation

4. **Update examples**
   - [ ] Example 1: Cloudflare Worker with deploy + destroy
   - [ ] Example 2: GCP Cloud Run with state store
   - [ ] Example 3: Composite infrastructure with sequential deploy/destroy

5. **Write tests**
   - [ ] Unit test: Deployable impl with mock state store
   - [ ] Integration test: Full deploy + destroy cycle
   - [ ] Test: State persistence across deployments

6. **Documentation**
   - [ ] Document `Deployable` trait with examples
   - [ ] Document `ProviderClient` usage pattern
   - [ ] Migration guide from old CLI-provider approach

7. **Verification**
   - [ ] `cargo clippy -p foundation_deployment -- -D warnings -W clippy::pedantic` — zero warnings
   - [ ] `cargo doc -p foundation_deployment --no-deps` — zero rustdoc warnings
   - [ ] All tests pass

8. **Cleanup**
   - [ ] Remove old `DeploymentProvider` trait exports (keep code, just don't re-export)
   - [ ] Update `lib.rs` to export new `Deployable` trait
   - [ ] Update README with new approach

## Examples

### Example 1: Simple Cloudflare Worker

```rust
// deploy/my_worker.rs

use foundation_core::traits::{Deploying, Deployable};
use foundation_core::valtron::{execute, TaskIterator, TaskIteratorExt, BoxedSendExecutionAction};
use foundation_deployment::provider_client::ProviderClient;
use foundation_db::state::FileStateStore;

pub struct MyWorker {
    pub name: String,
    pub script: String,
}

impl Deployable for MyWorker {
    type Output = WorkerDeployment;
    type Error = ApiError;
    type Store = FileStateStore;
    
    fn deploy(
        &self,
        client: ProviderClient<Self::Store>,
    ) -> Result<
        impl StreamIterator<D = Result<Self::Output, Self::Error>, P = Deploying> + Send + 'static,
        Self::Error,
    > {
        self.deploy_task(client)
            .and_then(|task| execute(task, None).map_err(|e| {
                Self::Error::from(std::io::Error::new(
                    std::io::ErrorKind::Other,
                    format!("Valtron execution failed: {}", e),
                ))
            }))
    }
    
    fn deploy_task(
        &self,
        client: ProviderClient<Self::Store>,
    ) -> Result<
        impl TaskIterator<Ready = Result<Self::Output, Self::Error>, Pending = Deploying, Spawner = BoxedSendExecutionAction> + Send + 'static,
        Self::Error,
    > {
        // Access state store through client
        let state_store = client.state_store();
        
        // Use generated provider client task function
        let task = put_workers_script_task(state_store, &PutWorkersScriptArgs {
            name: self.name.clone(),
            script: self.script.clone(),
            ..Default::default()
        })?;
        
        // Transform provider response into deployment output
        Ok(task.map_done(|result| {
            result.map(|response| WorkerDeployment {
                deployment_id: response.id.clone(),
                worker_name: self.name.clone(),
                url: format!("https://{}.workers.dev", self.name),
                deployed_at: chrono::Utc::now(),
            })
        }))
    }
    
    fn destroy(
        &self,
        client: ProviderClient<Self::Store>,
    ) -> Result<
        impl StreamIterator<D = Result<(), Self::Error>, P = Deploying> + Send + 'static,
        Self::Error,
    > {
        self.destroy_task(client)
            .and_then(|task| execute(task, None).map_err(|e| {
                Self::Error::from(std::io::Error::new(
                    std::io::ErrorKind::Other,
                    format!("Valtron execution failed: {}", e),
                ))
            }))
    }
    
    fn destroy_task(
        &self,
        client: ProviderClient<Self::Store>,
    ) -> Result<
        impl TaskIterator<Ready = Result<(), Self::Error>, Pending = Deploying, Spawner = BoxedSendExecutionAction> + Send + 'static,
        Self::Error,
    > {
        let task = delete_workers_script_task(client.state_store(), &DeleteWorkersScriptArgs {
            name: self.name.clone(),
        })?;
        
        Ok(task.map_done(|result| result.map_err(Into::into)))
    }
}

// Usage
fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Create state store and client
    let state_store = FileStateStore::new("/path/to/state", "my-project", "dev")?;
    let client = ProviderClient::new("my-project", "dev", state_store);
    
    let worker = MyWorker {
        name: "my-worker".into(),
        script: include_str!("worker.js").into(),
    };
    
    // Deploy with client - pass ownership or clone
    let stream = worker.deploy(client.clone())?;
    for item in stream {
        if let Stream::Next(result) = item {
            let deployment = result?;
            println!("Deployed {} to {}", deployment.worker_name, deployment.url);
        }
    }
    
    // Destroy with same client
    let destroy_stream = worker.destroy(client)?;
    for item in destroy_stream {
        if let Stream::Next(result) = item {
            result?;
            println!("Destroyed worker");
        }
    }
    
    Ok(())
}
```

### Example 2: GCP Cloud Run Service

```rust
// deploy/my_service.rs

use foundation_core::traits::Deploying;
use foundation_core::valtron::{execute, TaskIterator, TaskIteratorExt, BoxedSendExecutionAction};
use foundation_core::wire::simple_http::client::SimpleHttpClient;
use foundation_deployment::{providers::gcp::clients::*, types::*};
use std::time::Duration;

pub struct CloudRunService {
    pub name: String,
    pub image: String,
    pub region: String,
    pub project_id: String,
}

impl Deployable for CloudRunService {
    type Output = CloudRunDeployment;
    type Error = ApiError;
    
    fn deploy(&self) -> Result<
        impl foundation_core::valtron::StreamIterator<D = Result<Self::Output, Self::Error>, P = Deploying> + Send + 'static,
        Self::Error,
    > {
        self.deploy_task()
            .and_then(|task| execute(task, None).map_err(|e| {
                Self::Error::from(std::io::Error::new(
                    std::io::ErrorKind::Other,
                    format!("Valtron execution failed: {}", e),
                ))
            }))
    }
    
    fn deploy_task(&self) -> Result<
        impl TaskIterator<Ready = Result<Self::Output, Self::Error>, Pending = Deploying, Spawner = BoxedSendExecutionAction> + Send + 'static,
        Self::Error,
    > {
        // Construct client with shared connection pool
        let config = HttpClientConfig::default()
            .with_base_url("https://run.googleapis.com")
            .with_timeout(Duration::from_secs(60));
        let client = SimpleHttpClient::pooled(config);
        
        // Use generated GCP client for Cloud Run deploy
        let task = run_projects_locations_services_replace_service_task(&client, &RunProjectsLocationsServicesReplaceServiceArgs {
            parent: format!("projects/{}/locations/{}", self.project_id, self.region),
            service_id: self.name.clone(),
            ..Default::default()
        })?;
        
        Ok(task.map_done(|result| {
            result.map(|operation| CloudRunDeployment {
                deployment_id: operation.name,
                service_name: self.name.clone(),
                url: format!("https://{}-{}.a.run.app", self.name, self.region),
                image: self.image.clone(),
                deployed_at: chrono::Utc::now(),
            })
        }))
    }
}
```

### Example 3: Composite Infrastructure (Sequential Deployment)

```rust
// deploy/full_stack.rs

use foundation_core::traits::Deploying;
use foundation_core::valtron::{execute, TaskIterator, TaskIteratorExt, BoxedSendExecutionAction, collect_from_streams};
use foundation_core::wire::simple_http::client::SimpleHttpClient;
use foundation_deployment::{providers::cloudflare::clients::*, types::*};
use std::time::Duration;

pub struct FullStack {
    pub worker: MyWorker,
    pub kv_namespace: KvNamespace,
    pub d1_database: D1Database,
}

pub struct FullStackOutput {
    pub worker: WorkerDeployment,
    pub kv_id: String,
    pub d1_id: String,
}

impl Deployable for FullStack {
    type Output = FullStackOutput;
    type Error = ApiError;
    
    fn deploy(&self) -> Result<
        impl foundation_core::valtron::StreamIterator<D = Result<Self::Output, Self::Error>, P = Deploying> + Send + 'static,
        Self::Error,
    > {
        self.deploy_task()
            .and_then(|task| execute(task, None).map_err(|e| {
                Self::Error::from(std::io::Error::new(
                    std::io::ErrorKind::Other,
                    format!("Valtron execution failed: {}", e),
                ))
            }))
    }
    
    fn deploy_task(&self) -> Result<
        impl TaskIterator<Ready = Result<Self::Output, Self::Error>, Pending = Deploying, Spawner = BoxedSendExecutionAction> + Send + 'static,
        Self::Error,
    > {
        // Sequential: KV -> D1 -> Worker (each depends on previous)
        // For sequential composition, chain transformations
        
        // Step 1: Deploy KV
        let kv_task = self.kv_namespace.deploy_task()
            .map_err(|e| ApiError::RequestBuildFailed(e.to_string()))
            .map_done(|r| r.map(|kv| kv.id));
        
        // Step 2: Deploy D1  
        let d1_task = self.d1_database.deploy_task()
            .map_err(|e| ApiError::RequestBuildFailed(e.to_string()))
            .map_done(|r| r.map(|d1| d1.id));
        
        // Step 3: Deploy worker (KV and D1 IDs available after execution)
        // Note: For true sequential execution with data passing, use a wrapper task
        let worker_task = self.worker.deploy_task()
            .map_err(|e| ApiError::RequestBuildFailed(e.to_string()))
            .map_done(|r| r.map(|w| FullStackOutput {
                worker: w,
                kv_id: "kv-id-from-state".to_string(), // Would come from state store
                d1_id: "d1-id-from-state".to_string(),
            }));
        
        Ok(worker_task.map_pending(|_| Deploying::Processing))
    }
}
```

### Example 4: Environment-Specific Deployments

```rust
// deploy/main.rs

use foundation_core::traits::Deploying;
use foundation_core::valtron::{execute, TaskIterator, TaskIteratorExt, BoxedSendExecutionAction};
use foundation_core::wire::simple_http::client::SimpleHttpClient;
use foundation_deployment::{providers::cloudflare::clients::*, types::*};
use std::time::Duration;

#[derive(Debug, Clone, Copy)]
pub enum Env {
    Staging,
    Production,
}

impl Env {
    fn prefix(&self) -> &'static str {
        match self {
            Env::Staging => "staging",
            Env::Production => "prod",
        }
    }
}

pub struct EnvWorker {
    pub env: Env,
    pub base_name: String,
    pub script: String,
}

impl Deployable for EnvWorker {
    type Output = WorkerDeployment;
    type Error = ApiError;
    
    fn deploy(&self) -> Result<
        impl foundation_core::valtron::StreamIterator<D = Result<Self::Output, Self::Error>, P = Deploying> + Send + 'static,
        Self::Error,
    > {
        self.deploy_task()
            .and_then(|task| execute(task, None).map_err(|e| {
                Self::Error::from(std::io::Error::new(
                    std::io::ErrorKind::Other,
                    format!("Valtron execution failed: {}", e),
                ))
            }))
    }
    
    fn deploy_task(&self) -> Result<
        impl TaskIterator<Ready = Result<Self::Output, Self::Error>, Pending = Deploying, Spawner = BoxedSendExecutionAction> + Send + 'static,
        Self::Error,
    > {
        let name = format!("{}-{}", self.env.prefix(), self.base_name);
        
        // Delegate to MyWorker with environment-prefixed name
        MyWorker {
            name,
            script: self.script.clone(),
        }.deploy_task()
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let env = match std::env::args().nth(1).as_deref() {
        Some("staging") => Env::Staging,
        Some("production") => Env::Production,
        _ => Env::Staging,
    };
    
    let worker = EnvWorker {
        env,
        base_name: "api".into(),
        script: include_str!("../src/worker.js").into(),
    };
    
    let stream = worker.deploy()?;
    for item in stream {
        if let Stream::Next(result) = item {
            let deployment = result?;
            println!("Deployed {} to {}", deployment.worker_name, deployment.url);
        }
    }
    
    Ok(())
}
```

### Example 5: Destroy / Rollback

```rust
// Add destroy capability to Deployable trait

use foundation_core::traits::Deploying;
use foundation_core::valtron::{execute, one_shot, StreamIterator, TaskIterator, BoxedSendExecutionAction};

pub trait Deployable {
    type Output: Send + Sync;
    type Error: std::error::Error + Send + Sync + std::fmt::Debug;
    
    fn deploy(&self) -> Result<
        impl StreamIterator<D = Result<Self::Output, Self::Error>, P = Deploying> + Send + 'static,
        Self::Error,
    >;
    
    fn deploy_task(&self) -> Result<
        impl TaskIterator<Ready = Result<Self::Output, Self::Error>, Pending = Deploying, Spawner = BoxedSendExecutionAction> + Send + 'static,
        Self::Error,
    >;
    
    /// Optional: destroy deployed resources.
    /// Default implementation is no-op.
    fn destroy_task(&self, _output: &Self::Output) -> Result<
        impl TaskIterator<Ready = Result<(), Self::Error>, Pending = Deploying, Spawner = BoxedSendExecutionAction> + Send + 'static,
        Self::Error,
    > {
        Ok(one_shot(Ok(())).map_pending(|_| Deploying::Init))
    }
    
    fn destroy(&self, output: &Self::Output) -> Result<
        impl StreamIterator<D = Result<(), Self::Error>, P = Deploying> + Send + 'static,
        Self::Error,
    > {
        self.destroy_task(output)
            .and_then(|task| execute(task, None).map_err(|e| {
                Self::Error::from(std::io::Error::new(
                    std::io::ErrorKind::Other,
                    format!("Destroy task failed: {}", e),
                ))
            }))
    }
}

// Usage example
impl Deployable for MyWorker {
    type Output = WorkerDeployment;
    type Error = ApiError;
    
    fn deploy(&self) -> Result<
        impl foundation_core::valtron::StreamIterator<D = Result<Self::Output, Self::Error>, P = Deploying> + Send + 'static,
        Self::Error,
    > {
        self.deploy_task()
            .and_then(|task| execute(task, None).map_err(|e| {
                Self::Error::from(std::io::Error::new(
                    std::io::ErrorKind::Other,
                    format!("Valtron execution failed: {}", e),
                ))
            }))
    }
    
    fn deploy_task(&self) -> Result<
        impl TaskIterator<Ready = Result<Self::Output, Self::Error>, Pending = Deploying, Spawner = BoxedSendExecutionAction> + Send + 'static,
        Self::Error,
    > {
        // Construct client with shared connection pool
        let config = HttpClientConfig::default()
            .with_base_url("https://api.cloudflare.com")
            .with_timeout(Duration::from_secs(30));
        let client = SimpleHttpClient::pooled(config);
        
        // ... deploy logic using provider client
        put_workers_script_task(&client, &PutWorkersScriptArgs {
            name: self.name.clone(),
            script: self.script.clone(),
            ..Default::default()
        })?.map_done(|r| r.map(|resp| WorkerDeployment {
            deployment_id: resp.id,
            worker_name: self.name.clone(),
            url: format!("https://{}.workers.dev", self.name),
            deployed_at: chrono::Utc::now(),
        }))
    }
    
    fn destroy_task(&self, output: &Self::Output) -> Result<
        impl TaskIterator<Ready = Result<(), Self::Error>, Pending = Deploying, Spawner = BoxedSendExecutionAction> + Send + 'static,
        Self::Error,
    > {
        // Construct client with shared connection pool
        let config = HttpClientConfig::default()
            .with_base_url("https://api.cloudflare.com")
            .with_timeout(Duration::from_secs(30));
        let client = SimpleHttpClient::pooled(config);
        
        // Use generated delete function from provider client
        let task = delete_workers_script_task(&client, &DeleteWorkersScriptArgs {
            name: output.worker_name.clone(),
        })?;
        
        Ok(task.map_done(|result| {
            result.map(|_| ())
        }))
    }
}

// Destroy in main
fn destroy_worker() -> Result<(), ApiError> {
    let worker = MyWorker { /* ... */ };
    
    // First deploy to get output
    let stream = worker.deploy()?;
    let deployment = stream.into_iter().next()
        .and_then(|item| if let Stream::Next(r) = item { r.ok() } else { None })
        .ok_or_else(|| ApiError::ParseFailed("No deployment result".into()))?;
    
    // Then destroy
    let destroy_stream = worker.destroy(&deployment)?;
    for item in destroy_stream {
        if let Stream::Next(result) = item {
            result?;
        }
    }
    
    Ok(())
}
```

## Tasks

1. **Add reusable connection pool to SimpleHttpClient**
   - [ ] Add a static/thread-local connection pool that is initialized once
   - [ ] Add `SimpleHttpClient::pooled()` or similar method that reuses the shared pool
   - [ ] Ensure pool is properly cleaned up on drop (or use lazy_static/once_cell for lifetime management)
   - [ ] Users calling the pooled method get consistent connection reuse across multiple client creations
   - [ ] Write unit tests for pool reuse
   - [ ] Update examples to use the pooled client method

2. **Define Deployable trait**
   - [ ] Create trait in `foundation_core/traits.rs`
   - [ ] Add associated types: `Output`, `Error` (with Debug bound)
   - [ ] Add generic `Deploying` enum with `Init, Processing, Done, Failed` variants
   - [ ] Add two methods: `deploy()` (StreamIterator) and `deploy_task()` (TaskIterator)
   - [ ] Add optional `destroy()` and `destroy_task()` methods with default no-op impl
   - [ ] Document usage with examples
   - [ ] Write unit tests

3. **Create common output types**
   - [ ] `WorkerDeployment` for Cloudflare
   - [ ] `CloudRunDeployment` for GCP
   - [ ] `LambdaDeployment` for AWS
   - [ ] `DeploymentOutput` generic type
   - [ ] Write unit tests

4. **Update provider clients**
   - [ ] Ensure generated clients expose `*_task()` functions for composition
   - [ ] Verify all provider clients return compatible types
   - [ ] Write integration tests

5. **Add destroy support**
   - [ ] Add `destroy_task()` method to trait with default no-op implementation
   - [ ] Add `destroy()` convenience method
   - [ ] Implement for all providers
   - [ ] Write tests

6. **Documentation**
   - [ ] Document trait in rustdoc
   - [ ] Add examples to documentation
   - [ ] Create example deploy scripts

## Success Criteria

- [ ] All 6 tasks completed
- [ ] `cargo clippy -p foundation_deployment -- -D warnings -W clippy::pedantic` — zero warnings
- [ ] `cargo doc -p foundation_deployment --no-deps` — zero rustdoc warnings
- [ ] No `#[allow(...)]` or `#[expect(...)]` anywhere
- [ ] Users can implement `Deployable` on custom structs
- [ ] Provider clients compose correctly in `deploy_task()` implementations
- [ ] Examples compile and run
- [ ] `SimpleHttpClient::pooled()` reuses connection pool across multiple calls

## Lessons Learned

- **Reusable connection pool**: Users calling `SimpleHttpClient::pooled()` across multiple `deploy_task()` implementations share the same underlying connection pool. This provides connection reuse, reduced latency, and lower resource consumption without requiring users to manage pool lifetime.

- **Generic `Deploying` type**: No need for every implementation to define its own Pending enum. A single `Deploying { Init, Processing, Done, Failed }` covers all cases - reduces boilerplate and keeps user code cleaner.

- **Error type needs Debug**: The `Error` associated type requires `Debug` bound (`std::error::Error + Send + Sync + Debug`) for proper error handling and logging.

- **Client construction inside trait methods**: Initially considered passing `SimpleHttpClient` as a parameter, but this added unnecessary complexity. Users construct the client inside `deploy_task()` - this keeps the trait simpler and more flexible.

- **Two-method design**: The `deploy()` / `deploy_task()` split provides both convenience (call `deploy()` for immediate execution) and flexibility (call `deploy_task()` for composition with valtron combinators before execution).

- **No helper functions**: `deploy()` implementations inline the valtron `execute()` call directly - no intermediate helper functions needed, keeping the code straightforward.

- **Imports at top**: All imports are at the top of each example - no local `use` statements inside function bodies. This improves readability and follows Rust best practices.

- **Valtron-native composition**: Provider clients expose `*_task()` functions that return `TaskIterator`, allowing users to compose deployments using valtron combinators (`map_done`, `map_pending`, `map_err`) rather than async/await patterns.

- **Destroy with state**: The `destroy(&self, output: &Self::Output)` signature requires the deployment output, which typically contains IDs/names needed for deletion. State stores track this output automatically.

## Iron Law: Imports at Top

**All imports must be at the top of the file or module - no local `use` statements inside function bodies.**

```rust
// ✅ CORRECT: Imports at top
use foundation_core::traits::Deploying;
use foundation_core::valtron::{execute, StreamIterator, TaskIterator};

impl Deployable for MyWorker {
    fn deploy(&self) -> Result<...> {
        self.deploy_task()
            .and_then(|task| execute(task, None).map_err(...))
    }
}

// ❌ WRONG: Local import inside function
impl Deployable for MyWorker {
    fn deploy(&self) -> Result<...> {
        use foundation_core::valtron::execute;  // Don't do this
        execute(...)
    }
}
```

**Why:**
- Easier to see all dependencies at a glance
- Follows Rust community conventions
- Better IDE support for navigation and refactoring
- No hidden dependencies buried in function bodies

## Verification

```bash
cd backends/foundation_deployment
cargo check
cargo clippy -- -D warnings
cargo doc --no-deps
cargo test trait -- --nocapture
```

## Tests

All tests use `foundation_testing::http::TestHttpServer` for mocking provider APIs and
temporary directories for state stores. Tests validate HTTP requests, state persistence,
and idempotent destroy behavior.

### Test Infrastructure

```rust
// crates/ewe_deployables/tests/deployables_test.rs

use foundation_testing::http::{TestHttpServer, HttpRequest, HttpResponse};
use foundation_db::state::{FileStateStore, traits::StateStore};
use foundation_deployment::provider_client::ProviderClient;
use ewe_deployables::cloudflare::CloudflareWorker;
use ewe_deployables::gcp::{CloudRunService, CloudRunJob};
use foundation_core::valtron;
use foundation_core::wire::simple_http::client::{SimpleHttpClient, StaticSocketAddr};
use std::sync::{Arc, Mutex};
use serde_json::json;

/// Initialize valtron pool for tests.
/// Returns PoolGuard which must be held for the duration of the test.
fn init_valtron() -> valtron::PoolGuard {
    valtron::initialize_pool(42, Some(4))
}

/// Create a temporary state store directory for tests.
/// Automatically cleaned up when dropped.
fn temp_state_store(project: &str, stage: &str) -> (tempfile::TempDir, FileStateStore) {
    let temp_dir = tempfile::tempdir().expect("Failed to create temp dir");
    let store = FileStateStore::new(
        temp_dir.path().to_str().unwrap(),
        project,
        stage,
    ).expect("Failed to create state store");
    (temp_dir, store)
}

/// Create provider client with HTTP client for tests.
/// The HTTP client uses StaticSocketAddr to redirect all API calls to the test server.
fn test_client_with_server(
    store: FileStateStore,
    server: &TestHttpServer,
) -> ProviderClient<FileStateStore, StaticSocketAddr> {
    let server_addr: std::net::SocketAddr = server.base_url()
        .strip_prefix("http://")
        .unwrap()
        .parse()
        .expect("Valid server address");
    
    ProviderClient::new(
        "test-project",
        "test-stage",
        store,
        SimpleHttpClient::with_resolver(StaticSocketAddr::new(server_addr)),
    )
}
```

### SimpleHttpClient with Custom DNS Resolver

Add a constructor to `SimpleHttpClient` for creating a client with a custom DNS resolver:

```rust
// foundation_core/src/wire/simple_http/client/client.rs

impl<R: DnsResolver + Clone> SimpleHttpClient<R> {
    /// Creates a new HTTP client with a custom DNS resolver.
    ///
    /// WHY: Tests need to redirect API calls to mock servers.
    ///      StaticSocketAddr ignores hostname and returns test server IP.
    ///
    /// # Arguments
    ///
    /// * `resolver` - DNS resolver implementation
    ///
    /// # Returns
    ///
    /// A new `SimpleHttpClient` with the specified resolver and default config.
    #[must_use]
    pub fn with_resolver(resolver: R) -> Self {
        Self {
            config: ClientConfig::default(),
            pool: Some(Arc::new(HttpConnectionPool::new(
                ConnectionPool::default(),
                resolver,
            ))),
            middleware_chain: Arc::new(MiddlewareChain::new()),
        }
    }
}
```

### ProviderClient with Arc<SimpleHttpClient>

Update `ProviderClient` to include `Arc<SimpleHttpClient>` for shared HTTP client access:

```rust
// foundation_deployment/src/provider_client.rs

pub struct ProviderClient<S, R>
where
    S: StateStore + Send + Sync + 'static,
    R: DnsResolver + Clone + 'static,
{
    pub state_store: Arc<S>,
    pub project: String,
    pub stage: String,
    pub http_client: Arc<SimpleHttpClient<R>>,
}

impl<S, R> ProviderClient<S, R>
where
    S: StateStore + Send + Sync + 'static,
    R: DnsResolver + Clone + 'static,
{
    pub fn new(project: &str, stage: &str, state_store: S, http_client: SimpleHttpClient<R>) -> Self {
        Self {
            state_store: Arc::new(state_store),
            project: project.to_string(),
            stage: stage.to_string(),
            http_client: Arc::new(http_client),
        }
    }

    #[must_use]
    pub fn http_client(&self) -> &SimpleHttpClient<R> {
        &self.http_client
    }
}
```

### Provider Generator Updates

Update the provider generator (`bin/platform/src/gen_resources/provider_wrappers.rs`) to:

1. Accept `Arc<SimpleHttpClient<R>>` in `new()` method
2. Add `from_provider_client()` convenience method

```rust
// Generated provider struct

pub struct CloudflareProvider<S, R>
where
    S: StateStore + Send + Sync + 'static,
    R: DnsResolver + Clone + 'static,
{
    client: ProviderClient<S, R>,
    http_client: Arc<SimpleHttpClient<R>>,
}

impl<S, R> CloudflareProvider<S, R>
where
    S: StateStore + Send + Sync + 'static,
    R: DnsResolver + Clone + 'static,
{
    /// Create new CloudflareProvider.
    pub fn new(client: ProviderClient<S, R>, http_client: Arc<SimpleHttpClient<R>>) -> Self {
        Self {
            client,
            http_client,
        }
    }

    /// Create new CloudflareProvider from ProviderClient, extracting the HTTP client.
    ///
    /// This is a convenience method that calls `Self::new()` with `client.http_client()`.
    pub fn from_provider_client(client: ProviderClient<S, R>) -> Self {
        Self::new(client, client.http_client.clone())
    }
}
```

The key insight: `StaticSocketAddr` ignores the hostname and always returns the test server's IP. This means the deployables can use their normal hardcoded API hostnames (e.g., `api.cloudflare.com`), but the DNS resolver will redirect them to the test server.

### Cloudflare Worker Tests

```rust
mod cloudflare_worker_tests {
    use super::*;
    use foundation_testing::http::{HttpRequest, HttpResponse};
    use foundation_core::valtron::{StreamIterator, collect_result};
    use ewe_deployables::cloudflare::types::WorkerDeployment;

    #[test]
    fn test_deploy_worker_success() {
        let _guard = init_valtron();
        let request_log = Arc::new(Mutex::new(Vec::new()));
        let log_clone = Arc::clone(&request_log);
        
        // Mock Cloudflare API server
        let server = TestHttpServer::with_response(move |req| {
            log_clone.lock().unwrap().push((
                req.method.clone(),
                req.path.clone(),
                req.body.clone(),
            ));
            
            // Validate PUT /accounts/{id}/workers/scripts/{name}
            if req.path.path_str().contains("/accounts/test-account-id/workers/scripts/test-worker") 
                && req.method.as_str() == "PUT" 
            {
                HttpResponse {
                    status: 200,
                    status_text: "OK".to_string(),
                    headers: vec![("Content-Type".to_string(), "application/json".to_string())],
                    body: json!({
                        "success": true,
                        "result": {
                            "id": "worker-script-id-123",
                            "etag": "abc123",
                        }
                    }).to_string().into_bytes(),
                }
            } else {
                HttpResponse::status(404, "Not Found")
            }
        });
        
        // Create temp state store
        let (_temp_dir, store) = temp_state_store("cloudflare-test", "dev");
        
        // Create worker deployable with normal constructor
        let script_path = "/tmp/test-worker.js";
        std::fs::write(script_path, "export default { fetch() {} }").unwrap();
        let worker = CloudflareWorker::new("test-worker", script_path, "test-account-id");
        
        // Create client with HTTP client pointing to test server
        let client = test_client_with_server(store.clone(), &server);
        
        // Deploy - worker uses client.http_client for API calls
        let stream = worker.deploy(client).expect("Deploy failed");
        let results: Vec<_> = collect_result(stream);
        
        // Validate results
        assert_eq!(results.len(), 1);
        let deployment = results[0].as_ref().unwrap();
        assert_eq!(deployment.script_name, "test-worker");
        assert_eq!(deployment.deployment_id, "worker-script-id-123");
        
        // Validate state was persisted
        let resource_id = "cloudflare:worker:test-project:test-worker";
        let state = store.get(&resource_id)
            .unwrap()
            .find_map(|v| match v {
                foundation_core::valtron::ThreadedValue::Value(Ok(s)) => Some(s),
                _ => None,
            })
            .flatten()
            .expect("State should be persisted");
        
        let output: WorkerDeployment = serde_json::from_value(state.output).unwrap();
        assert_eq!(output.script_name, "test-worker");
        assert_eq!(output.account_id, "test-account-id");
        
        // Validate HTTP request was made correctly
        let requests = request_log.lock().unwrap();
        assert_eq!(requests.len(), 1);
        assert_eq!(requests[0].0.as_str(), "PUT");
        assert!(requests[0].1.path_str().contains("/workers/scripts/"));
    }

    #[test]
    fn test_destroy_worker_success() {
        let _guard = init_valtron();
        let delete_called = Arc::new(Mutex::new(false));
        let called_clone = Arc::clone(&delete_called);
        
        // Mock Cloudflare API for delete
        let server = TestHttpServer::with_response(move |req| {
            if req.method.as_str() == "DELETE" 
                && req.path.path_str().contains("/workers/scripts/test-worker") 
            {
                *called_clone.lock().unwrap() = true;
                HttpResponse {
                    status: 200,
                    status_text: "OK".to_string(),
                    headers: vec![],
                    body: json!({"success": true}).to_string().into_bytes(),
                }
            } else {
                HttpResponse::status(404, "Not Found")
            }
        });
        
        // Create state store with pre-existing state (simulating prior deploy)
        let (_temp_dir, store) = temp_state_store("cloudflare-test", "dev");
        
        // Pre-populate state
        let worker = CloudflareWorker::new("test-worker", "/tmp/worker.js", "test-account-id");
        let client = test_client_with_server(store.clone(), &server);
        
        // First deploy to create state
        let deploy_stream = worker.deploy(client.clone()).expect("Deploy failed");
        let _results: Vec<_> = collect_result(deploy_stream);
        
        // Destroy
        let stream = worker.destroy(client).expect("Destroy failed");
        let results: Vec<_> = collect_result(stream);
        
        // Validate destroy succeeded
        assert_eq!(results.len(), 1);
        assert!(results[0].is_ok());
        
        // Validate delete API was called
        assert!(*delete_called.lock().unwrap());
    }

    #[test]
    fn test_destroy_worker_idempotent_no_state() {
        let _guard = init_valtron();
        // Mock server that should NOT be called
        let server = TestHttpServer::with_response(|_req| {
            panic!("Delete should not be called when no state exists");
        });
        
        // Create empty state store (no prior deploy)
        let (_temp_dir, store) = temp_state_store("cloudflare-test", "dev");
        
        let worker = CloudflareWorker::new("nonexistent-worker", "/tmp/worker.js", "test-account-id");
        let client = test_client_with_server(store, &server);
        
        // Destroy should succeed idempotently
        let stream = worker.destroy(client).expect("Destroy failed");
        let results: Vec<_> = collect_result(stream);
        
        assert_eq!(results.len(), 1);
        assert!(results[0].is_ok());
    }

    #[test]
    fn test_deploy_worker_api_error() {
        let _guard = init_valtron();
        // Mock server returning 401 Unauthorized
        let server = TestHttpServer::with_response(|req| {
            if req.method.as_str() == "PUT" && req.path.path_str().contains("/workers/scripts/") {
                HttpResponse {
                    status: 401,
                    status_text: "Unauthorized".to_string(),
                    headers: vec![],
                    body: json!({
                        "success": false,
                        "errors": [{"code": 10000, "message": "Invalid API token"}]
                    }).to_string().into_bytes(),
                }
            } else {
                HttpResponse::status(404, "Not Found")
            }
        });
        
        let (_temp_dir, store) = temp_state_store("cloudflare-test", "dev");
        
        let worker = CloudflareWorker::new("test-worker", "/tmp/worker.js", "test-account-id");
        let client = test_client_with_server(store, &server);
        
        // Deploy should fail with API error
        let stream = worker.deploy(client).expect("Deploy stream creation failed");
        let results: Vec<_> = collect_result(stream);
        
        assert_eq!(results.len(), 1);
        assert!(results[0].is_err());
    }
}
```

### GCP Cloud Run Service Tests

```rust
mod cloud_run_service_tests {
    use super::*;
    use foundation_testing::http::{HttpRequest, HttpResponse};
    use foundation_core::valtron::{StreamIterator, collect_result};
    use ewe_deployables::gcp::{CloudRunService, types::CloudRunDeployment};

    #[test]
    fn test_deploy_cloud_run_service_success() {
        let _guard = init_valtron();
        let request_log = Arc::new(Mutex::new(Vec::new()));
        let log_clone = Arc::clone(&request_log);
        
        // Mock GCP Cloud Run API server
        let server = TestHttpServer::with_response(move |req| {
            log_clone.lock().unwrap().push((
                req.method.clone(),
                req.path.clone(),
                req.body.clone(),
            ));
            
            // Validate PATCH request for service deployment
            if req.method.as_str() == "PATCH" 
                && req.path.path_str().contains("/projects/test-project/locations/us-central1/services/test-service")
            {
                HttpResponse {
                    status: 200,
                    status_text: "OK".to_string(),
                    headers: vec![("Content-Type".to_string(), "application/json".to_string())],
                    body: json!({
                        "name": "projects/test-project/locations/us-central1/services/test-service",
                        "operation": "long-running-op-id",
                        "status": "in_progress"
                    }).to_string().into_bytes(),
                }
            } else {
                HttpResponse::status(404, "Not Found")
            }
        });
        
        let (_temp_dir, store) = temp_state_store("gcp-test", "dev");
        
        // Create service with normal constructor
        let service = CloudRunService::new("test-service", "gcr.io/test-project/image:latest", "us-central1", "test-project");
        
        // Create client with HTTP client pointing to test server
        let client = test_client_with_server(store.clone(), &server);
        
        // Deploy - service uses client.http_client for API calls
        let stream = service.deploy(client).expect("Deploy failed");
        let results: Vec<_> = collect_result(stream);
        
        // Validate results
        assert_eq!(results.len(), 1);
        let deployment = results[0].as_ref().unwrap();
        assert_eq!(deployment.service_name, "test-service");
        assert_eq!(deployment.project_id, "test-project");
        
        // Validate state was persisted
        let resource_id = "gcp:cloud-run:test-project:test-project:us-central1:test-service";
        let state = store.get(&resource_id).unwrap()
            .find_map(|v| match v {
                foundation_core::valtron::ThreadedValue::Value(Ok(s)) => Some(s),
                _ => None,
            })
            .flatten()
            .expect("State should be persisted");
        
        let output: CloudRunDeployment = serde_json::from_value(state.output).unwrap();
        assert_eq!(output.service_name, "test-service");
        
        // Validate HTTP request
        let requests = request_log.lock().unwrap();
        assert_eq!(requests.len(), 1);
        assert_eq!(requests[0].0.as_str(), "PATCH");
    }

    #[test]
    fn test_destroy_cloud_run_service_from_state() {
        let _guard = init_valtron();
        let delete_log = Arc::new(Mutex::new(Vec::new()));
        let log_clone = Arc::clone(&delete_log);
        
        let server = TestHttpServer::with_response(move |req| {
            log_clone.lock().unwrap().push((
                req.method.clone(),
                req.path.clone(),
            ));
            
            // Validate DELETE uses resource name from state
            if req.method.as_str() == "DELETE" 
                && req.path.path_str().contains("/projects/test-project/locations/us-central1/services/test-service")
            {
                HttpResponse {
                    status: 200,
                    status_text: "OK".to_string(),
                    body: json!({
                        "name": "operations/delete-op-id",
                        "done": true,
                    }).to_string().into_bytes(),
                }
            } else {
                HttpResponse::status(404, "Not Found")
            }
        });
        
        let (_temp_dir, store) = temp_state_store("gcp-test", "dev");
        
        let service = CloudRunService::new("test-service", "gcr.io/test-project/image:latest", "us-central1", "test-project");
        let client = test_client_with_server(store.clone(), &server);
        
        // First deploy to create state
        let deploy_stream = service.deploy(client.clone()).expect("Deploy failed");
        let _results: Vec<_> = collect_result(deploy_stream);
        
        // Destroy
        let stream = service.destroy(client).expect("Destroy failed");
        let results: Vec<_> = collect_result(stream);
        
        assert_eq!(results.len(), 1);
        assert!(results[0].is_ok());
        
        // Validate delete was called with correct resource name
        let requests = delete_log.lock().unwrap();
        assert_eq!(requests.len(), 1);
        assert!(requests[0].1.path_str().contains("/services/test-service"));
    }

    #[test]
    fn test_destroy_cloud_run_idempotent() {
        let _guard = init_valtron();
        let server = TestHttpServer::with_response(|_req| {
            panic!("Delete should not be called when no state exists");
        });
        
        let (_temp_dir, store) = temp_state_store("gcp-test", "dev");
        
        let service = CloudRunService::new("nonexistent-service", "gcr.io/test-project/image:latest", "us-central1", "test-project");
        let client = test_client_with_server(store, &server);
        
        // Destroy should succeed idempotently
        let stream = service.destroy(client).expect("Destroy failed");
        let results: Vec<_> = collect_result(stream);
        
        assert_eq!(results.len(), 1);
        assert!(results[0].is_ok());
    }
}
```
        
        // Destroy should succeed idempotently
        let server_addr: std::net::SocketAddr = server.base_url()
            .strip_prefix("http://")
            .unwrap()
            .parse()
            .unwrap();
        let stream = service.destroy_with_resolver(
            store,
            StaticSocketAddr::new(server_addr)
        ).expect("Destroy failed");
        let results: Vec<_> = collect_result(stream);
        
        assert_eq!(results.len(), 1);
        assert!(results[0].is_ok());
    }
}
```

### GCP Cloud Run Job Tests

```rust
mod cloud_run_job_tests {
    use super::*;
    use foundation_testing::http::{HttpRequest, HttpResponse};
    use foundation_core::valtron::{StreamIterator, collect_result};
    use ewe_deployables::gcp::{CloudRunJob, types::CloudRunJobDeployment};

    #[test]
    fn test_deploy_cloud_run_job_with_schedule() {
        let _guard = init_valtron();
        let request_log = Arc::new(Mutex::new(Vec::new()));
        let log_clone = Arc::clone(&request_log);
        
        let server = TestHttpServer::with_response(move |req| {
            log_clone.lock().unwrap().push((
                req.method.clone(),
                req.path.clone(),
                req.body.clone(),
            ));
            
            if req.method.as_str() == "POST" 
                && req.path.path_str().contains("/projects/test-project/locations/us-central1/jobs")
            {
                // Validate job spec includes schedule
                let body_str = String::from_utf8_lossy(&req.body);
                let body: serde_json::Value = serde_json::from_str(&body_str).unwrap();
                assert!(body.get("schedule").is_some(), "Job spec should include schedule");
                
                HttpResponse {
                    status: 200,
                    status_text: "OK".to_string(),
                    body: json!({
                        "name": "projects/test-project/locations/us-central1/jobs/test-job",
                        "operation": "job-create-op",
                    }).to_string().into_bytes(),
                }
            } else {
                HttpResponse::status(404, "Not Found")
            }
        });
        
        let (_temp_dir, store) = temp_state_store("gcp-test", "dev");
        
        let job = CloudRunJob::new(
            "test-job",
            "gcr.io/test-project/job-image:latest",
            "us-central1",
            "test-project",
        )
        .with_schedule("0 * * * *")  // Hourly
        .with_command(vec!["/app/process".to_string()]);
        
        let client = test_client_with_server(store, &server);
        
        let stream = job.deploy(client).expect("Deploy failed");
        let results: Vec<_> = collect_result(stream);
        
        assert_eq!(results.len(), 1);
        assert!(results[0].is_ok());
    }

    #[test]
    fn test_destroy_cloud_run_job_from_state() {
        let _guard = init_valtron();
        let delete_log = Arc::new(Mutex::new(Vec::new()));
        let log_clone = Arc::clone(&delete_log);
        
        let server = TestHttpServer::with_response(move |req| {
            log_clone.lock().unwrap().push((
                req.method.clone(),
                req.path.clone(),
            ));
            
            if req.method.as_str() == "DELETE" 
                && req.path.path_str().contains("/jobs/test-job")
            {
                HttpResponse {
                    status: 200,
                    status_text: "OK".to_string(),
                    body: json!({"done": true}).to_string().into_bytes(),
                }
            } else {
                HttpResponse::status(404, "Not Found")
            }
        });
        
        let (_temp_dir, store) = temp_state_store("gcp-test", "dev");
        
        let job = CloudRunJob::new(
            "test-job",
            "gcr.io/test-project/job-image:latest",
            "us-central1",
            "test-project",
        );
        
        let client = test_client_with_server(store.clone(), &server);
        
        // First deploy to create state
        let deploy_stream = job.deploy(client.clone()).expect("Deploy failed");
        let _results: Vec<_> = collect_result(deploy_stream);
        
        // Destroy
        let stream = job.destroy(client).expect("Destroy failed");
        let results: Vec<_> = collect_result(stream);
        
        assert_eq!(results.len(), 1);
        assert!(results[0].is_ok());
        
        // Validate delete was called
        let requests = delete_log.lock().unwrap();
        assert_eq!(requests.len(), 1);
        assert!(requests[0].1.path_str().contains("/jobs/test-job"));
    }
}
```

### Test Coverage

The test suite validates:

1. **Deploy operations:**
   - HTTP request is sent with correct method, path, and body
   - API response is parsed into typed `Output` struct
   - State is persisted with correct resource identifier
   - Deserialized state contains all expected fields
   - `ProviderClientWithHttp` provides both state store and HTTP client

2. **Destroy operations:**
   - State is read and deserialized into typed output
   - Resource name from state is used for delete API call
   - Delete request is sent with correct parameters
   - Idempotent success when no state exists (no HTTP call made)

3. **Error handling:**
   - API errors (401, 403, 404, 500) propagate correctly
   - State store errors are wrapped with context
   - Deserialization failures produce clear errors

4. **State persistence:**
   - Resource IDs are namespaced by project and stage:
     - Cloudflare: `cloudflare:worker:{project}:{name}`
     - GCP Cloud Run: `gcp:cloud-run:{project}:{project_id}:{region}:{name}`
     - GCP Cloud Run Job: `gcp:cloud-run-job:{project}:{project_id}:{region}:{name}`
   - State survives across deploy/destroy cycles
   - Typed deserialization enforces schema expectations

5. **Test infrastructure patterns:**
   - `init_valtron()` returns `PoolGuard` held for test duration
   - `temp_state_store()` creates isolated temp directories
   - `test_client_with_server()` creates `ProviderClientWithHttp` with test server
   - `StaticSocketAddr` ignores hostname, resolves to test server IP
   - Deployables use `client.http_client` for API calls

## Migration from Old Engine

**Old way (rejected):**
```rust
let planner = DeploymentPlanner::new(provider, config, state_store)
    .environment("production")
    .dry_run(false);
let result = DeploymentExecutor::run(planner)?;
```

**New way:**
```rust
let worker = MyWorker { name: "api".into(), script: code };

// deploy() returns StreamIterator - iterate to get result
let stream = worker.deploy()?;
for item in stream {
    if let Stream::Next(result) = item {
        let deployment = result?;
        println!("Deployed: {}", deployment.url);
    }
}
```

**Benefits:**
- No state machine complexity
- Full type safety
- Compose deployments using valtron combinators
- Test with regular Rust tests
- No custom configuration format
- Provider clients handle all HTTP and serialization

## Implementation Changes Summary

### Phase 1: Core HTTP Client Updates

**File:** `backends/foundation_core/src/wire/simple_http/client/client.rs`

Added `SimpleHttpClient::with_resolver()` constructor:

```rust
impl<R: DnsResolver + Clone> SimpleHttpClient<R> {
    #[must_use]
    pub fn with_resolver(resolver: R) -> Self {
        Self {
            config: ClientConfig::default(),
            pool: Some(Arc::new(HttpConnectionPool::new(
                ConnectionPool::default(),
                resolver,
            ))),
            middleware_chain: Arc::new(MiddlewareChain::new()),
        }
    }
}
```

**Purpose:** Allow creating HTTP clients with custom DNS resolvers (e.g., `StaticSocketAddr` for testing).

### Phase 2: ProviderClient Updates

**File:** `backends/foundation_deployment/src/provider_client.rs`

- Consolidated `ProviderClient` and `ProviderClientWithHttp` into single generic struct
- Added `Arc<SimpleHttpClient<R>>` for shared HTTP client access
- Added `http_client()` getter method
- Removed `Debug` derive (SimpleHttpClient doesn't implement Debug)
- Manual `Clone` implementation

```rust
pub struct ProviderClient<S, R>
where
    S: StateStore + Send + Sync + 'static,
    R: DnsResolver + Clone + 'static,
{
    pub state_store: Arc<S>,
    pub project: String,
    pub stage: String,
    pub http_client: Arc<SimpleHttpClient<R>>,
}
```

### Phase 3: Generator Updates - Clients

**File:** `bin/platform/src/gen_resources/clients.rs`

Made builder functions generic over DNS resolver type:

1. Added `DnsResolver` to imports
2. Changed function signatures from:
   ```rust
   pub fn foo_builder(client: &SimpleHttpClient) -> Result<ClientRequestBuilder<SystemDnsResolver>, ApiError>
   ```
   To:
   ```rust
   pub fn foo_builder<R>(
       client: &SimpleHttpClient<R>,
   ) -> Result<ClientRequestBuilder<R>, ApiError>
   where
       R: DnsResolver + Clone,
   ```

### Phase 4: Generator Updates - Provider Wrappers

**File:** `bin/platform/src/gen_resources/provider_wrappers.rs`

1. Updated `new()` to accept `Arc<SimpleHttpClient<R>>`
2. Added `from_provider_client()` convenience method:
   ```rust
   pub fn from_provider_client(client: ProviderClient<S, R>) -> Self {
       Self::new(client, client.http_client.clone())
   }
   ```
3. Updated struct to be generic over `R: DnsResolver + Clone + 'static`
4. Updated imports to include `DnsResolver`
5. Updated doc examples to show `from_provider_client()` pattern

### Phase 5: Regenerated Code

Ran `cargo run --bin ewe_platform -- gen_resources clients` and `gen_resources providers` to regenerate:

- All provider clients (cloudflare, gcp, fly_io, neon, planetscale, prisma_postgres, stripe, supabase)
- All provider wrappers with new `from_provider_client()` method

### Test Infrastructure Pattern

Tests now use unified pattern:

```rust
fn test_client_with_server(
    store: FileStateStore,
    server: &TestHttpServer,
) -> ProviderClient<FileStateStore, StaticSocketAddr> {
    let server_addr = server.base_url().strip_prefix("http://").unwrap().parse().unwrap();
    ProviderClient::new(
        "test-project",
        "test-stage",
        store,
        SimpleHttpClient::with_resolver(StaticSocketAddr::new(server_addr)),
    )
}
```

### Key Benefits

1. **Test isolation:** `StaticSocketAddr` redirects all API calls to mock test servers
2. **Shared connection pools:** `Arc<SimpleHttpClient>` enables cheap cloning and pool sharing
3. **Cleaner API:** `from_provider_client()` extracts HTTP client from `ProviderClient` automatically
4. **Type safety:** Generic DNS resolver parameter maintains type safety across the stack
5. **No breaking changes:** Existing code using system DNS resolver continues to work

---

_Created: 2026-04-11_
_Updated: 2026-04-15 - Added Arc<SimpleHttpClient> to ProviderClient, generic DNS resolver support_

