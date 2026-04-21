---
workspace_name: "ewe_platform"
spec_directory: "specifications/11-foundation-deployment"
feature_directory: "specifications/11-foundation-deployment/features/35-trait-based-deployments"
this_file: "specifications/11-foundation-deployment/features/35-trait-based-deployments/feature.md"

status: in-progress
priority: high
created: 2026-04-11
updated: 2026-04-20

depends_on: ["01-foundation-deployment-core", "02-state-stores", "07-provider-api-clients"]

tasks:
  completed: 4
  uncompleted: 10
  total: 14
  completion_percentage: 29%
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
- What destruction artifacts are returned

**No YAML, no TOML, no custom configuration format** — just Rust structs and trait implementations that return Valtron `TaskIterator` types.

**Key Design:** The `Deployable` trait requires a `const NAMESPACE: &'static str` that uniquely identifies each deployable implementation. This namespace is:
- Set once by the implementor, never changes
- Automatically prefixes all state store keys for this deployable's resources
- Uses a hierarchical `provider/group/resource` convention (e.g., `"cloudflare/workers/script"`, `"gcp/run/service"`)

The trait has two methods (`deploy`, `destroy`) that each take an `instance_id: usize` parameter identifying which instance of the resource to operate on. This supports deploying multiple instances of the same resource type (e.g., 3 copies of a worker). Both methods return `impl TaskIterator<...>` — users decide how to orchestrate execution (valtron combinators, sequential iteration, parallel spawning, etc).

The trait also has five associated types (`DeployOutput`, `DestroyOutput`, `Error`, `Store`, `Resolver`) and receives `ProviderClient<Store, Resolver>` as a parameter to all methods. The `Deployable` trait provides two default methods that wrap `ProviderClient` with `NAMESPACE` automatically:
1. **`self.store(client)`** — returns a `NamespacedStore<S>` that prefixes all keys with `NAMESPACE`, so resources are isolated per-deployable. Implemented as a wrapper in `foundation_db`.
2. **`self.update(client, instance_id, input, task)`** — wraps a valtron task to persist its result + input under `"{NAMESPACE}/{instance_id}"`. The deploy path stores what was deployed; the destroy path reads it back via `self.store(client).get(instance_id)` to know what to tear down.
3. **`client.http_client()`** — shared HTTP client for calling generated API functions directly
4. Because all keys are prefixed by `NAMESPACE`, listing all resources for a deployable is a simple prefix scan on the state store.

**Generated API functions** are standalone `_request` functions that take `&SimpleHttpClient<R>`, an args struct, and an optional builder-modifier closure. They return `Result<impl TaskIterator<...>, ApiError>`. There are no provider wrapper structs — users call the generated functions directly.

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

// Deploy a Cloudflare Worker (instance 0)
let worker = CloudflareWorker::new("my-worker", "./worker.js", "account-id");
let stream = worker.deploy(0, client.clone())?;

// Deploy a second instance (instance 1)
let stream = worker.deploy(1, client.clone())?;

// Deploy a GCP Cloud Run service
let service = CloudRunService::new("my-service", "gcr.io/project/image:latest", "us-central1", "project-id");
let stream = service.deploy(0, client)?;

// Destroy instance 0 — reads state stored during deploy
let destroy_stream = worker.destroy(0, client)?;
```

## Architecture

### Deployable Trait Definition

```rust
// foundation_deployment/src/traits.rs

use foundation_core::valtron::{BoxedSendExecutionAction, TaskIterator};
use foundation_core::wire::simple_http::client::DnsResolver;
use foundation_db::state::traits::StateStore;
use crate::provider_client::ProviderClient;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Deploying {
    #[default]
    Init,
    Processing,
    Done,
    Failed,
}

pub trait Deployable {
    /// Namespace for this deployable's state store keys.
    ///
    /// Convention: `"provider/group/resource"` — e.g., `"cloudflare/workers/script"`.
    /// All state store operations via `self.store(client)` are automatically
    /// prefixed with this value, isolating resources per-deployable.
    /// Must be a compile-time constant so it cannot drift between deploys.
    const NAMESPACE: &'static str;

    /// Deployment output type — contains URLs, IDs, and other artifacts.
    type DeployOutput: Send + Sync;

    /// Destroy output type — contains metadata about the destruction (often `()`).
    type DestroyOutput: Send + Sync;

    /// Error type for this deployment.
    type Error: std::error::Error + Send + Sync + std::fmt::Debug;

    /// State store type for persistence.
    type Store: StateStore + Send + Sync + 'static;

    /// DNS resolver type for HTTP calls.
    type Resolver: DnsResolver + Clone + 'static;

    /// Deploy a specific instance, returning a TaskIterator for composition.
    ///
    /// Users decide how to execute it — valtron combinators, sequential iteration,
    /// parallel spawning, etc.
    fn deploy(
        &self,
        instance_id: usize,
        client: ProviderClient<Self::Store, Self::Resolver>,
    ) -> Result<
        impl TaskIterator<
                Ready = Result<Self::DeployOutput, Self::Error>,
                Pending = Deploying,
                Spawner = BoxedSendExecutionAction,
            > + Send + 'static,
        Self::Error,
    >;

    /// Destroy a specific instance, returning a TaskIterator for composition.
    ///
    /// Users decide how to execute it — valtron combinators, sequential iteration,
    /// parallel spawning, etc.
    fn destroy(
        &self,
        instance_id: usize,
        client: ProviderClient<Self::Store, Self::Resolver>,
    ) -> Result<
        impl TaskIterator<
                Ready = Result<Self::DestroyOutput, Self::Error>,
                Pending = Deploying,
                Spawner = BoxedSendExecutionAction,
            > + Send + 'static,
        Self::Error,
    >;

    // -- Default methods --

    /// Returns a `NamespacedStore` scoped to `Self::NAMESPACE`.
    ///
    /// All `get()`, `list()`, `delete()` operations on the returned wrapper
    /// are automatically prefixed with `NAMESPACE`.
    fn store<'a>(
        &self,
        client: &'a ProviderClient<Self::Store, Self::Resolver>,
    ) -> NamespacedStore<'a, Self::Store> {
        NamespacedStore::new(client.state_store(), Self::NAMESPACE)
    }

    /// Wraps a valtron task to persist `(input, result)` under `"{NAMESPACE}/{instance_id}"`.
    ///
    /// On task success, stores the input and result in the state store. The deploy
    /// path calls this to record what was deployed; the destroy path reads it back
    /// via `self.store(client).get(instance_id)`.
    fn update<T, I>(
        &self,
        client: &ProviderClient<Self::Store, Self::Resolver>,
        instance_id: usize,
        input: I,
        task: T,
    ) -> impl TaskIterator<...>
    where
        T: TaskIterator,
        I: Serialize,
    {
        let store = self.store(client);
        // Wraps task: on success, store (input, result) under instance_id
        // Returns the original result unchanged
    }
}
```

### ProviderClient Structure

```rust
// foundation_deployment/src/provider_client.rs

use foundation_db::state::traits::StateStore;
use foundation_core::wire::simple_http::client::{SimpleHttpClient, DnsResolver};
use std::sync::Arc;

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

### NamespacedStore

`NamespacedStore` is a thin wrapper around any `StateStore` that automatically prefixes all keys with a namespace. Implemented in `foundation_db`.

```rust
// foundation_db/src/state/namespaced.rs

pub struct NamespacedStore<'a, S: StateStore> {
    inner: &'a S,
    prefix: String,
}

impl<'a, S: StateStore> NamespacedStore<'a, S> {
    pub fn new(inner: &'a S, namespace: &str) -> Self {
        Self {
            inner,
            prefix: format!("{namespace}/"),
        }
    }
}

impl<S: StateStore> StateStore for NamespacedStore<'_, S> {
    // All key-based operations prepend `self.prefix`:
    //   store(key, val)  → inner.store("{prefix}{key}", val)
    //   get(key)         → inner.get("{prefix}{key}")
    //   delete(key)      → inner.delete("{prefix}{key}")
    //   list()           → inner.list() filtered to keys starting with prefix,
    //                       with prefix stripped from returned keys
}
```

**Why a wrapper instead of convention?** Convention-based prefixing (e.g., documenting "always prefix your keys") is easy to forget and impossible to enforce. The wrapper makes it structural — once you have a `NamespacedStore`, every operation is scoped. You can't accidentally write to another deployable's namespace.

**Key design decisions:**

- **Generic over `R: DnsResolver + Clone`** — Allows `SystemDnsResolver` in production and `StaticSocketAddr` in tests
- **`Arc<SimpleHttpClient<R>>`** — Cheap cloning and connection pool sharing across API calls
- **Manual `Clone` impl** — Required due to `Arc` fields; `Debug` not derived to avoid requiring `Debug` bounds on generics
- **No provider wrapper structs** — Users call generated `_request` functions directly with `client.http_client()`
- **`state_store()` is raw** — `ProviderClient` exposes the raw `StateStore`. Namespacing is handled by `Deployable::store()` default method, not `ProviderClient`.

### Generated API Functions

Each provider endpoint is a single `_request` function. No `_builder`, `_task`, `_execute` splits. No wrapper structs.

```rust
// Generated pattern (e.g., cloudflare/workers/mod.rs)

pub fn worker_script_upload_worker_module_request<R, F>(
    client: &SimpleHttpClient<R>,
    args: &WorkerScriptUploadWorkerModuleArgs,
    builder_mod: Option<F>,
) -> Result<
    impl TaskIterator<
            Ready = Result<ApiResponse<WorkerScriptUploadResponse>, ApiError>,
            Pending = ApiPending,
            Spawner = BoxedSendExecutionAction,
        > + Send + 'static,
    ApiError,
>
where
    R: DnsResolver + Clone + Default + 'static,
    F: FnOnce(&mut ClientRequestBuilder<R>),
{
    // 1. Build URL from args
    // 2. Construct request via client.get/post/put/delete
    // 3. Apply builder_mod closure if provided
    // 4. Return TaskIterator that handles response parsing
}
```

**Shared types per provider** (e.g., `cloudflare/shared/mod.rs`):

```rust
pub enum ApiError {
    RequestBuildFailed(String),
    RequestSendFailed(String),
    HttpStatus { code: u16, headers: SimpleHeaders, body: Option<String> },
    ParseFailed(String),
}

pub enum ApiPending {
    Building,
    Sending,
}

pub struct ApiResponse<T> {
    pub status: u16,
    pub headers: SimpleHeaders,
    pub body: T,
}
```

### Example 1: Cloudflare Worker

```rust
// crates/ewe_deployables/src/cloudflare/worker.rs

use foundation_deployment::traits::{Deployable, Deploying};
use foundation_core::valtron::{TaskIterator, TaskIteratorExt, BoxedSendExecutionAction};
use foundation_deployment::provider_client::ProviderClient;
use foundation_db::state::FileStateStore;
use foundation_deployment::providers::cloudflare::workers::{
    worker_script_upload_worker_module_request, worker_script_delete_worker_request,
    WorkerScriptUploadWorkerModuleArgs, WorkerScriptDeleteWorkerArgs,
};
use foundation_core::wire::simple_http::client::{DnsResolver, SystemDnsResolver};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkerDeployment {
    pub account_id: String,
    pub script_name: String,
    pub deployment_id: String,
    pub url: String,
    pub deployed_at: chrono::DateTime<chrono::Utc>,
}

pub struct CloudflareWorker {
    pub name: String,
    pub script_path: String,
    pub account_id: String,
}

impl CloudflareWorker {
    pub fn new(name: impl Into<String>, script_path: impl Into<String>, account_id: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            script_path: script_path.into(),
            account_id: account_id.into(),
        }
    }
}

impl Deployable for CloudflareWorker {
    const NAMESPACE: &'static str = "cloudflare/workers/script";

    type DeployOutput = WorkerDeployment;
    type DestroyOutput = ();
    type Error = DeploymentError;
    type Store = FileStateStore;
    type Resolver = SystemDnsResolver;

    fn deploy(
        &self,
        instance_id: usize,
        client: ProviderClient<Self::Store, Self::Resolver>,
    ) -> Result<impl TaskIterator<Ready = Result<Self::DeployOutput, Self::Error>, Pending = Deploying, Spawner = BoxedSendExecutionAction> + Send + 'static, Self::Error> {
        let _script = std::fs::read_to_string(&self.script_path)
            .map_err(|e| DeploymentError::Io { path: self.script_path.clone(), source: e })?;

        let name = self.name.clone();
        let account_id = self.account_id.clone();

        let args = WorkerScriptUploadWorkerModuleArgs {
            account_id: account_id.clone(),
            script_name: name.clone(),
        };

        let task = worker_script_upload_worker_module_request(
            client.http_client(),
            &args,
            None,
        ).map_err(|e| DeploymentError::Generic(e.to_string()))?;

        let task = task
            .map_ready(move |result| {
                result
                    .map(|response| WorkerDeployment {
                        account_id,
                        script_name: name,
                        deployment_id: response.body.data.get("id")
                            .and_then(|v| v.as_str())
                            .unwrap_or("unknown")
                            .to_string(),
                        url: format!("https://{}.workers.dev", name),
                        deployed_at: chrono::Utc::now(),
                    })
                    .map_err(|e| DeploymentError::Generic(e.to_string()))
            })
            .map_pending(|_| Deploying::Processing);

        // Wrap with self.update() — persists (args, WorkerDeployment) under NAMESPACE/instance_id.
        // In destroy, we read this back via self.store() to know what to delete.
        Ok(self.update(&client, instance_id, &args, task))
    }

    fn destroy(
        &self,
        instance_id: usize,
        client: ProviderClient<Self::Store, Self::Resolver>,
    ) -> Result<impl TaskIterator<Ready = Result<Self::DestroyOutput, Self::Error>, Pending = Deploying, Spawner = BoxedSendExecutionAction> + Send + 'static, Self::Error> {
        // Read stored state for this instance to find what was deployed
        let store = self.store(&client);
        let state: WorkerDeployment = store.get(&instance_id.to_string())?
            .ok_or_else(|| DeploymentError::Generic(
                format!("no state found for worker '{}' instance {} — nothing to destroy", self.name, instance_id)
            ))?;

        let task = worker_script_delete_worker_request(
            client.http_client(),
            &WorkerScriptDeleteWorkerArgs {
                account_id: state.account_id.clone(),
                script_name: state.script_name.clone(),
            },
            None,
        ).map_err(|e| DeploymentError::Generic(e.to_string()))?;

        Ok(task
            .map_ready(|result| {
                result
                    .map(|_| ())
                    .map_err(|e| DeploymentError::Generic(e.to_string()))
            })
            .map_pending(|_| Deploying::Processing))
    }
}
```

### Example 2: GCP Cloud Run Service

```rust
// crates/ewe_deployables/src/gcp/cloud_run.rs

use foundation_deployment::traits::{Deployable, Deploying};
use foundation_core::valtron::{TaskIterator, TaskIteratorExt, BoxedSendExecutionAction};
use foundation_deployment::provider_client::ProviderClient;
use foundation_db::state::FileStateStore;
use foundation_deployment::providers::gcp::run::{
    run_projects_locations_services_patch_request, run_projects_locations_services_delete_request,
    RunProjectsLocationsServicesPatchArgs, RunProjectsLocationsServicesDeleteArgs,
};
use foundation_core::wire::simple_http::client::{DnsResolver, SystemDnsResolver};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CloudRunDeployment {
    pub resource_name: String,
    pub service_name: String,
    pub region: String,
    pub project_id: String,
    pub url: String,
    pub image: String,
    pub deployed_at: chrono::DateTime<chrono::Utc>,
}

pub struct CloudRunService {
    pub name: String,
    pub image: String,
    pub region: String,
    pub project_id: String,
}

impl CloudRunService {
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
    const NAMESPACE: &'static str = "gcp/run/service";

    type DeployOutput = CloudRunDeployment;
    type DestroyOutput = ();
    type Error = DeploymentError;
    type Store = FileStateStore;
    type Resolver = SystemDnsResolver;

    fn deploy(
        &self,
        instance_id: usize,
        client: ProviderClient<Self::Store, Self::Resolver>,
    ) -> Result<impl TaskIterator<Ready = Result<Self::DeployOutput, Self::Error>, Pending = Deploying, Spawner = BoxedSendExecutionAction> + Send + 'static, Self::Error> {
        let resource_name = format!("projects/{}/locations/{}/services/{}", self.project_id, self.region, self.name);
        let service_name = self.name.clone();
        let region = self.region.clone();
        let project_id = self.project_id.clone();
        let image = self.image.clone();

        let args = RunProjectsLocationsServicesPatchArgs {
            name: resource_name.clone(),
            allow_missing: Some(true),
            force_new_revision: Some(true),
            update_mask: Some("spec.template.spec.containers".to_string()),
            validate_only: None,
        };

        let task = run_projects_locations_services_patch_request(
            client.http_client(),
            &args,
            None,
        ).map_err(|e| DeploymentError::Generic(e.to_string()))?;

        let task = task
            .map_ready(move |result| {
                result
                    .map(|_| CloudRunDeployment {
                        resource_name,
                        service_name,
                        region,
                        project_id,
                        url: format!("https://{}-{}.a.run.app", service_name, region),
                        image,
                        deployed_at: chrono::Utc::now(),
                    })
                    .map_err(|e| DeploymentError::Generic(e.to_string()))
            })
            .map_pending(|_| Deploying::Processing);

        Ok(self.update(&client, instance_id, &args, task))
    }

    fn destroy(
        &self,
        instance_id: usize,
        client: ProviderClient<Self::Store, Self::Resolver>,
    ) -> Result<impl TaskIterator<Ready = Result<Self::DestroyOutput, Self::Error>, Pending = Deploying, Spawner = BoxedSendExecutionAction> + Send + 'static, Self::Error> {
        let store = self.store(&client);
        let state: CloudRunDeployment = store.get(&instance_id.to_string())?
            .ok_or_else(|| DeploymentError::Generic(
                format!("no state found for service '{}' instance {} — nothing to destroy", self.name, instance_id)
            ))?;

        let task = run_projects_locations_services_delete_request(
            client.http_client(),
            &RunProjectsLocationsServicesDeleteArgs {
                name: state.resource_name,
                etag: None,
                validate_only: None,
            },
            None,
        ).map_err(|e| DeploymentError::Generic(e.to_string()))?;

        Ok(task
            .map_ready(|result| {
                result
                    .map(|_| ())
                    .map_err(|e| DeploymentError::Generic(e.to_string()))
            })
            .map_pending(|_| Deploying::Processing))
    }
}
```

### Example 3: GCP Cloud Run Job

```rust
// crates/ewe_deployables/src/gcp/cloud_job.rs

use foundation_deployment::traits::{Deployable, Deploying};
use foundation_core::valtron::{TaskIterator, TaskIteratorExt, BoxedSendExecutionAction};
use foundation_deployment::provider_client::ProviderClient;
use foundation_db::state::FileStateStore;
use foundation_deployment::providers::gcp::run::{
    run_projects_locations_jobs_create_request, run_projects_locations_jobs_delete_request,
    RunProjectsLocationsJobsCreateArgs, RunProjectsLocationsJobsDeleteArgs,
};
use foundation_core::wire::simple_http::client::{DnsResolver, SystemDnsResolver};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CloudRunJobDeployment {
    pub resource_name: String,
    pub job_name: String,
    pub region: String,
    pub project_id: String,
    pub deployed_at: chrono::DateTime<chrono::Utc>,
}

pub struct CloudRunJob {
    pub name: String,
    pub image: String,
    pub region: String,
    pub project_id: String,
    pub schedule: Option<String>,
    pub command: Option<Vec<String>>,
}

impl CloudRunJob {
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

    pub fn with_schedule(mut self, schedule: impl Into<String>) -> Self {
        self.schedule = Some(schedule.into());
        self
    }

    pub fn with_command(mut self, command: Vec<String>) -> Self {
        self.command = Some(command);
        self
    }
}

impl Deployable for CloudRunJob {
    const NAMESPACE: &'static str = "gcp/run/job";

    type DeployOutput = CloudRunJobDeployment;
    type DestroyOutput = ();
    type Error = DeploymentError;
    type Store = FileStateStore;
    type Resolver = SystemDnsResolver;

    fn deploy(
        &self,
        instance_id: usize,
        client: ProviderClient<Self::Store, Self::Resolver>,
    ) -> Result<impl TaskIterator<Ready = Result<Self::DeployOutput, Self::Error>, Pending = Deploying, Spawner = BoxedSendExecutionAction> + Send + 'static, Self::Error> {
        let parent = format!("projects/{}/locations/{}", self.project_id, self.region);
        let job_name = self.name.clone();
        let region = self.region.clone();
        let project_id = self.project_id.clone();

        let args = RunProjectsLocationsJobsCreateArgs {
            parent,
            job_id: self.name.clone(),
            validate_only: None,
        };

        let task = run_projects_locations_jobs_create_request(
            client.http_client(),
            &args,
            None,
        ).map_err(|e| DeploymentError::Generic(e.to_string()))?;

        let task = task
            .map_ready(move |result| {
                result
                    .map(|_| CloudRunJobDeployment {
                        resource_name: format!("projects/{}/locations/{}/jobs/{}", project_id, region, job_name),
                        job_name,
                        region,
                        project_id,
                        deployed_at: chrono::Utc::now(),
                    })
                    .map_err(|e| DeploymentError::Generic(e.to_string()))
            })
            .map_pending(|_| Deploying::Processing);

        Ok(self.update(&client, instance_id, &args, task))
    }

    fn destroy(
        &self,
        instance_id: usize,
        client: ProviderClient<Self::Store, Self::Resolver>,
    ) -> Result<impl TaskIterator<Ready = Result<Self::DestroyOutput, Self::Error>, Pending = Deploying, Spawner = BoxedSendExecutionAction> + Send + 'static, Self::Error> {
        let store = self.store(&client);
        let state: CloudRunJobDeployment = store.get(&instance_id.to_string())?
            .ok_or_else(|| DeploymentError::Generic(
                format!("no state found for job '{}' instance {} — nothing to destroy", self.name, instance_id)
            ))?;

        let task = run_projects_locations_jobs_delete_request(
            client.http_client(),
            &RunProjectsLocationsJobsDeleteArgs {
                name: state.resource_name,
                etag: None,
                validate_only: None,
            },
            None,
        ).map_err(|e| DeploymentError::Generic(e.to_string()))?;

        Ok(task
            .map_ready(|result| {
                result
                    .map(|_| ())
                    .map_err(|e| DeploymentError::Generic(e.to_string()))
            })
            .map_pending(|_| Deploying::Processing))
    }
}
```

## Usage Pattern

```rust
use foundation_db::state::FileStateStore;
use foundation_core::wire::simple_http::client::SimpleHttpClient;
use foundation_core::valtron::execute;
use foundation_deployment::provider_client::ProviderClient;

// Create state store and HTTP client
let state_store = FileStateStore::new("/path/to/state", "my-project", "dev")?;
let http_client = SimpleHttpClient::default();
let client = ProviderClient::new("my-project", "dev", state_store, http_client);

// Create and deploy instance 0
let worker = CloudflareWorker::new("my-worker", "./worker.js", "account-id");
let task = worker.deploy(0, client.clone())?;

// Execute the task — users decide how to run it
let result = execute(task, None)?;
println!("Deployed {} to {}", result.script_name, result.url);

// Deploy another instance (instance 1)
let task = worker.deploy(1, client.clone())?;
// ... compose with valtron combinators, run in parallel, etc.

// Destroy instance 0
let destroy_task = worker.destroy(0, client)?;
execute(destroy_task, None)?;
println!("Destroyed worker instance 0");
```

## Common Deployment Output Types

```rust
// foundation_deployment/src/types.rs

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkerDeployment {
    pub deployment_id: String,
    pub worker_name: String,
    pub url: String,
    pub deployed_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CloudRunDeployment {
    pub deployment_id: String,
    pub service_name: String,
    pub url: String,
    pub image: String,
    pub deployed_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LambdaDeployment {
    pub deployment_id: String,
    pub function_name: String,
    pub url: Option<String>,
    pub runtime: String,
    pub deployed_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeploymentOutput {
    pub id: String,
    pub name: String,
    pub url: Option<String>,
    pub metadata: serde_json::Value,
    pub deployed_at: chrono::DateTime<chrono::Utc>,
}
```

## Tests

All tests use `foundation_testing::http::TestHttpServer` for mocking provider APIs and
temporary directories for state stores. Tests validate HTTP requests, state persistence,
and idempotent destroy behavior.

### Test Infrastructure

Tests use `TestHttpsServer` from `foundation_testing` — a real HTTPS server with a self-signed localhost certificate. This is needed because generated `_request` functions hardcode `https://` URLs (e.g., `https://api.cloudflare.com/...`), so the test server must speak TLS.

**Components:**

- **`TestHttpsServer`** (`foundation_testing::http`) — TLS-enabled test server using `RustlsAcceptor` with embedded self-signed cert/key
- **`test_tls_connector()`** — Returns an `SSLConnector` that trusts the self-signed test certificate
- **`SimpleHttpClient::with_tls_connector()`** — Builder method on `SimpleHttpClient` to inject a custom TLS connector
- **`StaticSocketAddr`** — DNS resolver that always returns the test server's IP, redirecting all hostnames to localhost

```rust
// crates/ewe_deployables/tests/common.rs (or test helper module)

use foundation_testing::http::{TestHttpsServer, HttpResponse, test_tls_connector};
use foundation_db::state::FileStateStore;
use foundation_deployment::provider_client::ProviderClient;
use foundation_core::valtron;
use foundation_core::wire::simple_http::client::{SimpleHttpClient, StaticSocketAddr};
use std::net::SocketAddr;
use std::sync::{Arc, Mutex};
use serde_json::json;

fn init_valtron() -> valtron::PoolGuard {
    valtron::initialize_pool(42, Some(4))
}

fn make_client(
    server: &TestHttpsServer,
    state_store: FileStateStore,
) -> ProviderClient<FileStateStore, StaticSocketAddr> {
    let port = server.port();
    let addr = SocketAddr::from(([127, 0, 0, 1], port));
    let dns = StaticSocketAddr::new(addr);
    let http_client = SimpleHttpClient::with_resolver(dns)
        .with_tls_connector(test_tls_connector())
        .with_connection_pool();

    ProviderClient::new("test-project", "test-stage", state_store, http_client)
}
```

**How it works:** `StaticSocketAddr` ignores the hostname and always resolves to the test server's `127.0.0.1:{port}`. The `test_tls_connector()` provides a `rustls::ClientConfig` that trusts the self-signed test certificate, so the TLS handshake succeeds despite the hostname mismatch. The generated `_request` functions use their normal hardcoded `https://` URLs and everything works end-to-end.

**Implementation details:**

- `HttpConnectionPool` gained an optional `tls_connector: Option<Arc<SSLConnector>>` field
- `HttpClientConnection::connect_with_tls_config()` accepts an optional `&SSLConnector` and uses it instead of the default Mozilla root store
- `SimpleHttpClient::with_tls_connector()` sets the custom connector on the pool
- `TestHttpsServer` uses `RustlsAcceptor::from_pem()` with embedded cert/key PEM files
- The embedded cert is a self-signed localhost certificate (valid until Jan 2027)

### Tests use `TestHttpsServer` + `test_tls_connector()` + `StaticSocketAddr`

See `crates/ewe_deployables/tests/` for full test implementations.

### Test Coverage

1. **State management:** Namespace isolation, key listing, deletion via `NamespacedStore`
2. **Deploy operations:** HTTP request correctness via `TestHttpsServer`, response parsing into typed `DeployOutput`, state persistence via `self.update()`
3. **Destroy operations:** State retrieval, delete request with correct parameters, `DestroyOutput` returned
4. **Error handling:** Missing script files, missing state on destroy, API errors (401, 500) propagate correctly
5. **Test infrastructure:** `TestHttpsServer` + `test_tls_connector()` + `StaticSocketAddr` for full HTTPS testing

## Implementation Changes Summary

### Completed

1. **Deployable trait** (`backends/foundation_deployment/src/traits.rs`)
   - [x] Separate `DeployOutput` and `DestroyOutput` associated types
   - [x] `Resolver` associated type alongside `Store`, `Error`
   - [x] Two methods: `deploy()` and `destroy()`, both returning `TaskIterator`
   - [x] `Deploying` enum with `Init`, `Processing`, `Done`, `Failed`
   - [x] `const NAMESPACE: &'static str` on trait definition
   - [x] `instance_id: usize` parameter on both methods
   - [x] `store(&self, client)` default method returning `NamespacedStore`
   - [x] `update(&self, client, instance_id, input, task)` default method returning `UpdateTask<T, S>` wrapper

2. **ProviderClient** (`backends/foundation_deployment/src/provider_client.rs`)
   - [x] Generic `ProviderClient<S, R>` over `StateStore` and `DnsResolver`
   - [x] `Arc<SimpleHttpClient<R>>` for shared HTTP client
   - [x] Manual `Clone`, getters for `state_store()`, `project()`, `stage()`, `http_client()`

3. **NamespacedStore** (`backends/foundation_db/src/state/namespaced.rs`)
   - [x] `NamespacedStore<S>` wrapper struct (owns `Arc<S>`, not a borrow)
   - [x] Full `StateStore` trait impl with automatic key prefixing
   - [x] `store_typed()` / `get_typed()` / `remove()` convenience methods
   - [x] `list()` filters by prefix and strips prefix from returned keys
   - [x] 6 unit tests: prefix isolation, list filtering, delete, struct serialization, all(), count()

4. **Generated API functions** (unified generator)
   - [x] Single `_request<R, F>` function per endpoint
   - [x] No state store dependency in generated code
   - [x] `builder_mod: Option<F>` closure for request customization
   - [x] Returns `Result<impl TaskIterator<...>, ApiError>`

5. **SimpleHttpClient** (`backends/foundation_core/src/wire/simple_http/client/`)
   - [x] `with_resolver(R)` constructor for custom DNS resolvers
   - [x] `with_tls_connector(SSLConnector)` for custom TLS config (test self-signed certs)
   - [x] `HttpConnectionPool` gained optional `tls_connector: Option<Arc<SSLConnector>>`
   - [x] `HttpClientConnection::connect_with_tls_config()` threads custom TLS through connection path

6. **HTTPS Test Infrastructure** (`backends/foundation_testing/src/http/tls_server.rs`)
   - [x] `TestHttpsServer` — TLS-enabled test server using `RustlsAcceptor` with embedded self-signed cert
   - [x] `test_tls_connector()` — Returns `SSLConnector` trusting the test certificate
   - [x] Embedded cert/key PEM files in `foundation_testing/src/http/fixtures/`
   - [x] Integration test verifying full HTTPS roundtrip

7. **Common output types** (`backends/foundation_deployment/src/types.rs`)
   - [x] `WorkerDeployment`, `CloudRunDeployment`, `CloudRunJobDeployment`, `DeploymentOutput`
   - [x] Re-exported via `crates/ewe_deployables/src/common/types.rs`

8. **ewe_deployables crate** (`crates/ewe_deployables/`)
   - [x] `CloudflareWorker<R>` — deploy + destroy with state persistence
   - [x] `CloudRunService<R>` and `CloudRunJob<R>` — both in `cloud_run.rs`
   - [x] Module declarations and re-exports in `lib.rs`

9. **Documentation**
   - [x] Rustdoc on `Deployable` trait with full example
   - [x] Rustdoc on all associated types and methods

### Remaining

10. **Tests**
   - [ ] Cloudflare Worker: deploy success, deploy API error, destroy success (with `TestHttpsServer`)
   - [ ] GCP Cloud Run Service: deploy success, destroy success (with `TestHttpsServer`)
   - [ ] GCP Cloud Run Job: deploy success (with `TestHttpsServer`)

11. **Verification**
   - [ ] `cargo clippy -p foundation_deployment -- -D warnings -W clippy::pedantic` — zero warnings
   - [ ] `cargo doc -p foundation_deployment --no-deps` — zero rustdoc warnings
   - [ ] All tests pass

## Key Design Decisions

1. **`const NAMESPACE: &'static str`** — Every `Deployable` impl must declare a constant namespace like `"cloudflare/workers/script"`. This is the identity of the deployable: it prefixes all state store keys, appears in logs, and enables prefix-scan to list all resources for a deployable. Being a `const` means it's baked in at compile time — it can't drift between deploys, can't be accidentally changed at runtime, and the compiler enforces that every implementor provides one. The `provider/group/resource` convention keeps namespaces hierarchical and collision-free across providers.

2. **`NamespacedStore` wrapper** — `self.store(client)` returns a `NamespacedStore<S>` that implements `StateStore` by prefixing all keys with `NAMESPACE`. This is structural isolation, not convention — once you have a `NamespacedStore`, every operation is scoped. You can't accidentally write to another deployable's namespace. `list()` returns only keys within the namespace (with prefix stripped). Implemented in `foundation_db` so it's reusable outside of deployments.

3. **`self.store()` and `self.update()` are default trait methods on `Deployable`** — Not on `ProviderClient`. This keeps `ProviderClient` simple (just holds state + HTTP client) and lets the trait wire in `Self::NAMESPACE` automatically. Implementors call `self.store(&client)` and `self.update(&client, instance_id, input, task)` — the namespace is always correct because it comes from the const.

4. **`instance_id: usize` on all methods** — Supports deploying multiple instances of the same resource type. The instance ID is part of the state store key (`"{NAMESPACE}/{instance_id}"`), so each instance's state is independently tracked. `deploy(0, client)` and `deploy(1, client)` create two separate state entries. `destroy(0, client)` reads instance 0's state to know what to tear down. For single-instance deployables, callers just pass `0`.

5. **Separate `DeployOutput` / `DestroyOutput`** — Destroy may return different metadata than deploy (e.g., `()` vs a full deployment record). Keeping them separate avoids forcing `Output` to serve both purposes.

6. **No provider wrapper structs** — Generated API functions are standalone. Users call `worker_script_upload_request(client.http_client(), &args, None)` directly. This removes an abstraction layer and makes the generated code simpler.

7. **Single `_request` function per endpoint** — Merged the old `_builder` + `_task` + `_execute` triple into one function with an optional `builder_mod` closure. Reduces generated code size and simplifies the API.

8. **No state store in generated clients** — Generated functions take `&SimpleHttpClient<R>` only. State persistence is handled by `NamespacedStore` + `self.update()`, not the generated API layer.

9. **`Arc<SimpleHttpClient>` in ProviderClient** — Enables cheap cloning and connection pool sharing across multiple `_request` calls within a single deployment.

10. **Associated type for Resolver** — `type Resolver = SystemDnsResolver` in production, `type Resolver = StaticSocketAddr` in tests. Cleaner than adding a generic parameter to every method signature.

11. **Generic `Deploying` enum** — A single shared enum (`Init`, `Processing`, `Done`, `Failed`) covers all deployment types. No need for custom `Pending` types per implementation.

12. **Two-method trait, not four** — Only `deploy` and `destroy`, both returning `TaskIterator`. No `StreamIterator` convenience methods. Users orchestrate execution themselves via valtron combinators (`execute`, `map_ready`, `map_pending`, etc), deciding whether to run sequentially, in parallel, or with custom composition.

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
let worker = CloudflareWorker::new("my-worker", "./worker.js", "account-id");
let client = ProviderClient::new("my-project", "dev", state_store, http_client);
let stream = worker.deploy(client)?;
```

**Benefits:**
- No state machine complexity
- Full type safety with separate deploy/destroy output types
- Compose deployments using valtron combinators (`map_ready`, `map_pending`, `execute`)
- Test with `StaticSocketAddr` + `TestHttpsServer` + `test_tls_connector()`
- No custom configuration format
- Generated `_request` functions handle all HTTP and serialization

## Verification

```bash
cd backends/foundation_deployment
cargo check
cargo clippy -- -D warnings
cargo doc --no-deps
cargo test trait -- --nocapture
```

---

_Created: 2026-04-11_
_Updated: 2026-04-20 - Added HTTPS test infrastructure: `TestHttpsServer`, `test_tls_connector()`, `SimpleHttpClient::with_tls_connector()`. Unblocks integration tests for all deployables. Updated implementation status to reflect completed items._
