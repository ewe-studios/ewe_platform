---
workspace_name: "ewe_platform"
spec_directory: "specifications/11-foundation-deployment"
feature_directory: "specifications/11-foundation-deployment/features/35-trait-based-deployments"
this_file: "specifications/11-foundation-deployment/features/35-trait-based-deployments/feature.md"

status: implemented
priority: high
created: 2026-04-11
completed: 2026-04-11

depends_on: ["01-foundation-deployment-core", "04-cloudflare-provider", "05-gcp-cloud-run-provider", "06-aws-lambda-provider"]

tasks:
  completed: 5
  uncompleted: 0
  total: 5
  completion_percentage: 100%
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

## Architecture

```rust
// User defines their infrastructure as Rust types
pub struct MyWorker {
    pub name: String,
    pub script: String,
}

// Implement Deployable — deploy_task() returns TaskIterator, deploy() executes it
// Uses existing provider clients from foundation_deployment::providers::cloudflare::clients
impl Deployable for MyWorker {
    type Output = WorkerDeployment;
    type Error = ApiError;
    
    // deploy() uses default implementation which calls deploy_task() and executes
    fn deploy(&self) -> Result<
        impl StreamIterator<D = Result<Self::Output, Self::Error>, P = Deploying> + Send + 'static,
        Self::Error,
    > {
        deploy_from_task::<Self>(self.deploy_task())
    }
    
    // deploy_task() contains the actual deployment logic using provider clients
    fn deploy_task(&self) -> Result<
        impl TaskIterator<Ready = Result<Self::Output, Self::Error>, Pending = Deploying, Spawner = BoxedSendExecutionAction> + Send + 'static,
        Self::Error,
    > {
        // Construct client inside the method
        let (config, pool) = setup_client();
        let client = SimpleHttpClient::new(config, pool);
        
        // Use generated provider client - already handles HTTP and valtron
        let task = put_workers_script_task(&client, &PutWorkersScriptArgs {
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
}

// Usage: deploy() executes the task automatically
let worker = MyWorker {
    name: "my-worker".into(),
    script: include_str!("worker.js").into(),
};

// Simple usage - deploy() handles execution
let stream = worker.deploy()?;
for item in stream {
    match item {
        Stream::Next(result) => println!("Deployed: {:?}", result),
        Stream::Done(_) => {}
    }
}

// Advanced usage - compose tasks before execution
let task = worker.deploy_task()?;
let customized = task
    .map_done(|r| println!("Result: {:?}", r))
    .map_pending(|p| println!("Progress: {:?}", p));
let result = execute(customized, None)?;
```

**Key Points:**
- Provider clients (`foundation_deployment::providers::*`) handle HTTP, serialization, and valtron combinators
- `Deployable` implementations compose provider client calls into higher-level deployment units
- Users get type-safe deployment APIs without duplicating HTTP logic
- Client construction happens inside the trait methods - no need to pass it around

## Requirements

### Deployable Trait

```rust
// foundation_core/traits.rs

use foundation_core::valtron::{TaskIterator, StreamIterator, BoxedSendExecutionAction};

/// Generic progress states for deployment execution.
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

/// Trait for deployable infrastructure.
///
/// Users implement this on their own structs to define deployment logic.
/// The associated `Output` type contains deployment artifacts (URLs, IDs, etc.).
/// The associated `Error` type is the error type for this deployment.
///
/// The trait provides two methods:
/// - `deploy_task()` - Returns a `TaskIterator` containing the deployment logic
/// - `deploy()` - Executes the task via valtron and returns a `StreamIterator` with the result
pub trait Deployable {
    /// Deployment output type — contains URLs, IDs, and other artifacts.
    type Output: Send + Sync;
    
    /// Error type for this deployment.
    type Error: std::error::Error + Send + Sync + std::fmt::Debug;
    
    /// Deploy the resource and return a StreamIterator with the result.
    ///
    /// This is the convenience method that creates the task via `deploy_task()`
    /// and executes it via valtron's `execute()`.
    ///
    /// # Returns
    ///
    /// Returns `Ok(StreamIterator)` which yields `Result<Output, Error>` when iterated.
    fn deploy(
        &self,
    ) -> Result<
        impl StreamIterator<
                D = Result<Self::Output, Self::Error>,
                P = Deploying,
            > + Send
            + 'static,
        Self::Error;
    
    /// Deploy the resource and return a TaskIterator for customization.
    ///
    /// This is the core method that contains the actual deployment logic.
    /// Users can call this directly when they need to compose tasks or
    /// apply custom valtron combinators before execution.
    ///
    /// # Returns
    ///
    /// Returns `Ok(TaskIterator)` which can be executed via `execute()`.
    fn deploy_task(
        &self,
    ) -> Result<
        impl TaskIterator<
                Ready = Result<Self::Output, Self::Error>,
                Pending = Deploying,
                Spawner = BoxedSendExecutionAction,
            > + Send
            + 'static,
        Self::Error,
    >;
}
```

### Default Implementation Helper

```rust
// For simple cases, use this helper to implement deploy() in terms of deploy_task()

pub fn deploy_from_task<T>(
    task_result: Result<impl TaskIterator<
        Ready = Result<T::Output, T::Error>,
        Pending = Deploying,
        Spawner = BoxedSendExecutionAction,
    > + Send + 'static, T::Error>
) -> Result<
    impl StreamIterator<D = Result<T::Output, T::Error>, P = Deploying> + Send + 'static,
    T::Error,
>
where
    T: Deployable,
{
    use foundation_core::valtron::execute;
    task_result.and_then(|task| {
        execute(task, None).map_err(|e| {
            // Convert valtron execution error to your error type
            // Most deployments will use a custom error that can represent this
            use std::io::{Error, ErrorKind};
            T::Error::from(Error::new(ErrorKind::Other, format!("Valtron execution failed: {}", e)))
        })
    })
}
```

### User Implementation Pattern

```rust
// Users construct their own client inside the trait methods

impl Deployable for MyWorker {
    type Output = WorkerDeployment;
    type Error = ApiError;
    
    fn deploy(&self) -> Result<
        impl StreamIterator<D = Result<Self::Output, Self::Error>, P = Deploying> + Send + 'static,
        Self::Error,
    > {
        deploy_from_task::<Self>(self.deploy_task())
    }
    
    fn deploy_task(&self) -> Result<
        impl TaskIterator<Ready = Result<Self::Output, Self::Error>, Pending = Deploying, Spawner = BoxedSendExecutionAction> + Send + 'static,
        Self::Error,
    > {
        // Construct client inside the method
        let (config, pool) = setup_client(); // User's setup function
        let client = SimpleHttpClient::new(config, pool);
        
        // Use generated provider client task function
        let task = put_workers_script_task(&client, &PutWorkersScriptArgs {
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

### Provider Clients (Already Exist)

Provider clients in `backends/foundation_deployment/src/providers/` expose valtron-native functions:
- `cloudflare::clients::*` — Cloudflare Workers, KV, D1, R2
- `gcp::clients::*` — Cloud Run, Cloud Run Jobs
- `aws::clients::*` — Lambda, S3, API Gateway

Each generated client provides four functions per endpoint:
1. `*_builder()` — Returns `ClientRequestBuilder` for customization
2. `*_task()` — Returns `TaskIterator` for composition
3. `*_execute()` — Returns `StreamIterator` via valtron
4. `*()` — Convenience function combining all

Users call these from their `deploy()` implementation.

### State Store Integration (via Provider Wrappers)

State stores in `foundation_db` track deployed resources. Provider wrappers automatically:
- Read state before deployment (change detection)
- Write state after successful deployment
- Support rollback on failure

Users don't interact with state stores directly — it's handled by provider clients.

## Examples

### Example 1: Simple Cloudflare Worker

```rust
// deploy/my_worker.rs

use foundation_core::valtron::{TaskIterator, TaskIteratorExt, BoxedSendExecutionAction};
use foundation_core::wire::simple_http::client::SimpleHttpClient;
use foundation_deployment::{providers::cloudflare::clients::*, types::*};
use foundation_core::traits::Deploying;

pub struct MyWorker {
    pub name: String,
    pub script: String,
}

impl Deployable for MyWorker {
    type Output = WorkerDeployment;
    type Error = ApiError;
    
    // deploy() uses default implementation which calls deploy_task() and executes
    fn deploy(&self) -> Result<
        impl foundation_core::valtron::StreamIterator<D = Result<Self::Output, Self::Error>, P = Deploying> + Send + 'static,
        Self::Error,
    > {
        deploy_from_task::<Self>(self.deploy_task())
    }
    
    // deploy_task() contains the actual deployment logic using provider clients
    fn deploy_task(&self) -> Result<
        impl TaskIterator<Ready = Result<Self::Output, Self::Error>, Pending = Deploying, Spawner = BoxedSendExecutionAction> + Send + 'static,
        Self::Error,
    > {
        // Construct client inside the method
        let (config, pool) = setup_client();
        let client = SimpleHttpClient::new(config, pool);
        
        // Use generated provider client task function
        let task = put_workers_script_task(&client, &PutWorkersScriptArgs {
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
}

// Usage
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let worker = MyWorker {
        name: "my-worker".into(),
        script: include_str!("worker.js").into(),
    };
    
    // Simple: deploy() returns StreamIterator with result
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

### Example 2: GCP Cloud Run Service

```rust
// deploy/my_service.rs

use foundation_core::valtron::{TaskIterator, TaskIteratorExt, BoxedSendExecutionAction};
use foundation_core::wire::simple_http::client::SimpleHttpClient;
use foundation_deployment::{providers::gcp::clients::*, types::*};
use foundation_core::traits::Deploying;

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
        deploy_from_task::<Self>(self.deploy_task())
    }
    
    fn deploy_task(&self) -> Result<
        impl TaskIterator<Ready = Result<Self::Output, Self::Error>, Pending = Deploying, Spawner = BoxedSendExecutionAction> + Send + 'static,
        Self::Error,
    > {
        // Construct client inside the method
        let (config, pool) = setup_client();
        let client = SimpleHttpClient::new(config, pool);
        
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

use foundation_core::valtron::{TaskIterator, TaskIteratorExt, BoxedSendExecutionAction, collect_from_streams};
use foundation_core::wire::simple_http::client::SimpleHttpClient;
use foundation_deployment::{providers::cloudflare::clients::*, types::*};
use foundation_core::traits::Deploying;

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
        deploy_from_task::<Self>(self.deploy_task())
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

use foundation_core::valtron::{TaskIterator, TaskIteratorExt, BoxedSendExecutionAction};
use foundation_core::wire::simple_http::client::SimpleHttpClient;
use foundation_deployment::{providers::cloudflare::clients::*, types::*};
use foundation_core::traits::Deploying;

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
        deploy_from_task::<Self>(self.deploy_task())
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

use foundation_core::valtron::{TaskIterator, StreamIterator, BoxedSendExecutionAction};
use foundation_core::traits::Deploying;

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
        use foundation_core::valtron::one_shot;
        Ok(one_shot(Ok(())).map_pending(|_| Deploying::Init))
    }
    
    fn destroy(&self, output: &Self::Output) -> Result<
        impl StreamIterator<D = Result<(), Self::Error>, P = Deploying> + Send + 'static,
        Self::Error,
    > {
        deploy_from_task::<Self>(
            self.destroy_task(output)
                .map_err(|e| Self::Error::from(std::io::Error::new(
                    std::io::ErrorKind::Other,
                    format!("Destroy task failed: {}", e),
                )))
        )
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
        deploy_from_task::<Self>(self.deploy_task())
    }
    
    fn deploy_task(&self) -> Result<
        impl TaskIterator<Ready = Result<Self::Output, Self::Error>, Pending = Deploying, Spawner = BoxedSendExecutionAction> + Send + 'static,
        Self::Error,
    > {
        // Construct client inside
        let (config, pool) = setup_client();
        let client = SimpleHttpClient::new(config, pool);
        
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
        // Construct client inside
        let (config, pool) = setup_client();
        let client = SimpleHttpClient::new(config, pool);
        
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

1. **Define Deployable trait**
   - [x] Create trait in `foundation_core/traits.rs`
   - [x] Add associated types: `Output`, `Error`, `Pending`
   - [x] Add two methods: `deploy()` (StreamIterator) and `deploy_task()` (TaskIterator)
   - [x] Add optional `destroy()` and `destroy_task()` methods with default no-op impl
   - [x] Document usage with examples
   - [x] Write unit tests

2. **Create common output types**
   - [x] `WorkerDeployment` for Cloudflare
   - [x] `CloudRunDeployment` for GCP
   - [x] `LambdaDeployment` for AWS
   - [x] `DeploymentOutput` generic type
   - [x] Write unit tests

3. **Update provider clients**
   - [x] Ensure generated clients expose `*_task()` functions for composition
   - [x] Verify all provider clients return compatible types
   - [x] Write integration tests

4. **Add destroy support**
   - [x] Add `destroy_task()` method to trait with default no-op implementation
   - [x] Add `destroy()` convenience method
   - [x] Implement for all providers
   - [x] Write tests

5. **Documentation**
   - [x] Document trait in rustdoc
   - [x] Add examples to documentation
   - [x] Create example deploy scripts

## Success Criteria

- [x] All 5 tasks completed
- [x] `cargo clippy -p foundation_deployment -- -D warnings -W clippy::pedantic` — zero warnings
- [x] `cargo doc -p foundation_deployment --no-deps` — zero rustdoc warnings
- [x] No `#[allow(...)]` or `#[expect(...)]` anywhere
- [x] Users can implement `Deployable` on custom structs
- [x] Provider clients compose correctly in `deploy_task()` implementations
- [x] Examples compile and run

## Lessons Learned

- **Generic `Deploying` type**: No need for every implementation to define its own Pending enum. A single `Deploying { Init, Processing, Done, Failed }` covers all cases - reduces boilerplate and keeps user code cleaner.

- **Error type needs Debug**: The `Error` associated type requires `Debug` bound (`std::error::Error + Send + Sync + Debug`) for proper error handling and logging.

- **Client construction inside trait methods**: Initially considered passing `SimpleHttpClient` as a parameter, but this added unnecessary complexity. Users construct the client inside `deploy_task()` - this keeps the trait simpler and more flexible.

- **Two-method design**: The `deploy()` / `deploy_task()` split provides both convenience (call `deploy()` for immediate execution) and flexibility (call `deploy_task()` for composition with valtron combinators before execution).

- **Default implementation helper**: `deploy_from_task()` eliminates boilerplate - most implementations just delegate to this helper for `deploy()` and focus all their logic in `deploy_task()`.

- **Valtron-native composition**: Provider clients expose `*_task()` functions that return `TaskIterator`, allowing users to compose deployments using valtron combinators (`map_done`, `map_pending`, `map_err`) rather than async/await patterns.

- **Destroy with state**: The `destroy(&self, output: &Self::Output)` signature requires the deployment output, which typically contains IDs/names needed for deletion. State stores track this output automatically.

## Verification

```bash
cd backends/foundation_deployment
cargo check
cargo clippy -- -D warnings
cargo doc --no-deps
cargo test trait -- --nocapture
```

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

---

_Created: 2026-04-11_

