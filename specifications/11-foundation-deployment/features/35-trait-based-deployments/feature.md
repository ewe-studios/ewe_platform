---
workspace_name: "ewe_platform"
spec_directory: "specifications/11-foundation-deployment"
feature_directory: "specifications/11-foundation-deployment/features/35-trait-based-deployments"
this_file: "specifications/11-foundation-deployment/features/35-trait-based-deployments/feature.md"

status: pending
priority: high
created: 2026-04-11

depends_on: ["01-foundation-deployment-core", "04-cloudflare-provider", "05-gcp-cloud-run-provider", "06-aws-lambda-provider"]

tasks:
  completed: 0
  uncompleted: 5
  total: 5
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

**No YAML, no TOML, no custom configuration format** — just Rust structs and trait implementations that call provider clients directly.

## Architecture

```rust
// User defines their infrastructure as Rust types
struct MyWorker {
    name: String,
    script: String,
}

// Implement Deployable — returns deployment artifacts on success
impl Deployable for MyWorker {
    type Output = WorkerDeployment;  // User-defined output type
    type Error = DeploymentError;
    
    async fn deploy(&self) -> Result<Self::Output, Self::Error> {
        // Use provider clients directly
        let client = CloudflareClient::from_env()?;
        client.put_worker_script(&self.name, &self.script).await
    }
}

// Use it
#[tokio::main]
async fn main() {
    let worker = MyWorker {
        name: "my-worker".into(),
        script: include_str!("worker.js").into(),
    };
    
    let result = worker.deploy()?;
    println!("Deployed to: {}", result.url);
}
```

## Requirements

### Deployable Trait

```rust
// core/traits.rs

use std::future::Future;

/// Trait for deployable infrastructure.
///
/// Users implement this on their own structs to define deployment logic.
/// The associated `Output` type contains deployment artifacts (URLs, IDs, etc.).
pub trait Deployable: Send + Sync {
    /// Deployment output type — contains URLs, IDs, and other artifacts.
    type Output: Send + Sync;
    
    /// Error type for this deployment.
    type Error: std::error::Error + Send + Sync;
    
    /// Deploy the resource.
    ///
    /// Returns `Ok(Output)` with deployment artifacts on success,
    /// or `Err(Error)` if deployment fails.
    fn deploy(&self) -> impl Future<Output = Result<Self::Output, Self::Error>> + Send;
}
```

### Common Deployment Output Types

```rust
// core/types.rs

/// Common deployment output for Cloudflare Workers.
#[derive(Debug, Clone)]
pub struct WorkerDeployment {
    pub deployment_id: String,
    pub worker_name: String,
    pub url: String,
    pub deployed_at: chrono::DateTime<chrono::Utc>,
}

/// Common deployment output for GCP Cloud Run.
#[derive(Debug, Clone)]
pub struct CloudRunDeployment {
    pub deployment_id: String,
    pub service_name: String,
    pub url: String,
    pub image: String,
    pub deployed_at: chrono::DateTime<chrono::Utc>,
}

/// Common deployment output for AWS Lambda.
#[derive(Debug, Clone)]
pub struct LambdaDeployment {
    pub deployment_id: String,  // Function ARN
    pub function_name: String,
    pub url: Option<String>,     // Function URL if configured
    pub runtime: String,
    pub deployed_at: chrono::DateTime<chrono::Utc>,
}

/// Generic deployment output when you just need basic info.
#[derive(Debug, Clone)]
pub struct DeploymentOutput {
    pub id: String,
    pub name: String,
    pub url: Option<String>,
    pub metadata: serde_json::Value,
    pub deployed_at: chrono::DateTime<chrono::Utc>,
}
```

### Provider Clients (Already Exist)

Provider clients are already implemented in `backends/foundation_deployment/src/providers/`:
- `CloudflareClient` — Cloudflare Workers, KV, D1, R2
- `GcpClient` — Cloud Run, Cloud Run Jobs
- `AwsClient` — Lambda, S3, API Gateway

Users call these directly from their `deploy()` implementation.

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

use foundation_deployment::prelude::*;

pub struct MyWorker {
    pub name: String,
    pub script: String,
}

impl Deployable for MyWorker {
    type Output = WorkerDeployment;
    type Error = foundation_deployment::error::DeploymentError;
    
    async fn deploy(&self) -> Result<Self::Output, Self::Error> {
        let client = CloudflareClient::from_env()
            .ok_or_else(|| DeploymentError::ConfigInvalid {
                file: "env".into(),
                reason: "CLOUDFLARE_API_TOKEN or CLOUDFLARE_ACCOUNT_ID not set".into(),
            })?;
        
        client
            .put_worker_script(&self.name, &self.script, None)
            .await
            .map(|r| WorkerDeployment {
                deployment_id: r.deployment_id,
                worker_name: r.name,
                url: format!("https://{}.workers.dev", r.name),
                deployed_at: chrono::Utc::now(),
            })
    }
}
```

### Example 2: GCP Cloud Run Service

```rust
// deploy/my_service.rs

use foundation_deployment::prelude::*;

pub struct CloudRunService {
    pub name: String,
    pub image: String,
    pub region: String,
    pub project_id: String,
}

impl Deployable for CloudRunService {
    type Output = CloudRunDeployment;
    type Error = foundation_deployment::error::DeploymentError;
    
    async fn deploy(&self) -> Result<Self::Output, Self::Error> {
        let client = GcpClient::from_env()
            .ok_or_else(|| DeploymentError::ConfigInvalid {
                file: "env".into(),
                reason: "GOOGLE_APPLICATION_CREDENTIALS not set".into(),
            })?;
        
        client
            .deploy_service(&self.name, &self.region, &self.image, None)
            .await
            .map(|r| CloudRunDeployment {
                deployment_id: r.deployment_id,
                service_name: r.name,
                url: r.uri,
                image: self.image.clone(),
                deployed_at: chrono::Utc::now(),
            })
    }
}
```

### Example 3: Composite Infrastructure

```rust
// deploy/full_stack.rs

use foundation_deployment::prelude::*;

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
    type Error = foundation_deployment::error::DeploymentError;
    
    async fn deploy(&self) -> Result<Self::Output, Self::Error> {
        // Deploy dependencies first
        let kv = self.kv_namespace.deploy().await?;
        let d1 = self.d1_database.deploy().await?;
        
        // Then deploy worker with bindings
        let worker = MyWorker {
            name: self.worker.name.clone(),
            script: format!(
                "const KV = '{}'; const DB = '{}';\n{}",
                kv.id, d1.id, self.worker.script
            ),
        }
        .deploy()
        .await?;
        
        Ok(FullStackOutput {
            worker,
            kv_id: kv.id,
            d1_id: d1.id,
        })
    }
}
```

### Example 4: Environment-Specific Deployments

```rust
// deploy/main.rs

use foundation_deployment::prelude::*;

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
    type Error = foundation_deployment::error::DeploymentError;
    
    async fn deploy(&self) -> Result<Self::Output, Self::Error> {
        let name = format!("{}-{}", self.env.prefix(), self.base_name);
        
        MyWorker {
            name,
            script: self.script.clone(),
        }
        .deploy()
        .await
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
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
    
    let result = worker.deploy().await?;
    println!("Deployed {} to {}", result.worker_name, result.url);
    
    Ok(())
}
```

### Example 5: Destroy / Rollback

```rust
// Add destroy capability to Deployable

pub trait Deployable: Send + Sync {
    type Output: Send + Sync;
    type Error: std::error::Error + Send + Sync;
    
    fn deploy(&self) -> impl Future<Output = Result<Self::Output, Self::Error>> + Send;
    
    /// Optional: destroy deployed resources.
    fn destroy(&self, output: &Self::Output) -> impl Future<Output = Result<(), Self::Error>> + Send {
        async { Ok(()) }  // Default: no-op
    }
}

// Usage
impl Deployable for MyWorker {
    type Output = WorkerDeployment;
    type Error = DeploymentError;
    
    async fn deploy(&self) -> Result<Self::Output, Self::Error> {
        // ... deploy logic
    }
    
    async fn destroy(&self, output: &Self::Output) -> Result<(), Self::Error> {
        let client = CloudflareClient::from_env()?;
        client.delete_worker_script(&output.worker_name).await
    }
}

// Destroy in main
async fn destroy_worker() -> Result<(), DeploymentError> {
    let worker = MyWorker { /* ... */ };
    let deployment = worker.deploy().await?;
    worker.destroy(&deployment).await
}
```

## Tasks

1. **Define Deployable trait**
   - [ ] Create trait in `core/traits.rs`
   - [ ] Add associated types `Output` and `Error`
   - [ ] Document usage with examples
   - [ ] Write unit tests

2. **Create common output types**
   - [ ] `WorkerDeployment` for Cloudflare
   - [ ] `CloudRunDeployment` for GCP
   - [ ] `LambdaDeployment` for AWS
   - [ ] `DeploymentOutput` generic type
   - [ ] Write unit tests

3. **Update provider clients**
   - [ ] Ensure `CloudflareClient` returns proper types
   - [ ] Ensure `GcpClient` returns proper types
   - [ ] Ensure `AwsClient` returns proper types
   - [ ] Write integration tests

4. **Add destroy support**
   - [ ] Add default `destroy()` method to trait
   - [ ] Implement for all providers
   - [ ] Write tests

5. **Documentation**
   - [ ] Document trait in rustdoc
   - [ ] Add examples to documentation
   - [ ] Create example deploy scripts

## Success Criteria

- [ ] All 5 tasks completed
- [ ] `cargo clippy -p foundation_deployment -- -D warnings -W clippy::pedantic` — zero warnings
- [ ] `cargo doc -p foundation_deployment --no-deps` — zero rustdoc warnings
- [ ] No `#[allow(...)]` or `#[expect(...)]` anywhere
- [ ] Users can implement `Deployable` on custom structs
- [ ] Provider clients work correctly from trait implementations
- [ ] Examples compile and run

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
let result = worker.deploy().await?;
```

**Benefits:**
- No state machine complexity
- Full type safety
- Compose deployments with regular Rust code
- Test with regular Rust tests
- No custom configuration format

---

_Created: 2026-04-11_

