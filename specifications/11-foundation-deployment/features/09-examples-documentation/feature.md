---
workspace_name: "ewe_platform"
spec_directory: "specifications/11-foundation-deployment"
feature_directory: "specifications/11-foundation-deployment/features/09-examples-documentation"
this_file: "specifications/11-foundation-deployment/features/09-examples-documentation/feature.md"

status: pending
priority: high
created: 2026-03-26

depends_on: ["04-cloudflare-provider", "05-gcp-cloud-run-provider", "06-aws-lambda-provider", "07-templates", "08-mise-integration"]

tasks:
  completed: 0
  uncompleted: 6
  total: 6
  completion_percentage: 0%
---


# Examples and Documentation

## Iron Law: Zero Warnings

> **All code must compile with zero warnings and pass all lints. No suppression. No exceptions.**
>
> - `cargo clippy -p foundation_deployment -- -D warnings -W clippy::pedantic` — zero warnings
> - `cargo doc -p foundation_deployment --no-deps` — zero rustdoc warnings
> - `cargo test -p foundation_deployment` — zero compilation warnings
> - **No `#[allow(...)]`, `#[expect(...)]`, or `#![allow(...)]` anywhere.** Fix the code, never suppress.
>
> Standalone example crates (not workspace members) follow their own lint rules,
> but should aspire to the same standard.

## Overview

Create working example projects for each provider and comprehensive documentation covering deployment workflows, state management, troubleshooting, and migration guides.

## Dependencies

Depends on all previous features (complete implementation required for working examples).

## Async Runtime Policy

> **tokio and async/await are banned from the deployment tool itself.** All deployment
> orchestration uses valtron exclusively.
>
> Example projects are standalone crates with their own `Cargo.toml` files. They are
> **not workspace members by default** — they live outside the workspace `members` list
> and are only compiled when explicitly requested. Each example provider directory is a
> self-contained project that the user `cd`s into and builds independently.
>
> Examples that depend on tokio (GCP/axum, AWS/lambda_http) must:
> 1. Be in their own crate with their own `Cargo.toml` (not a workspace member)
> 2. Never be referenced from `foundation_deployment`'s `Cargo.toml`
> 3. Be clearly documented as requiring tokio for the target framework

### Example Isolation

```toml
# Workspace Cargo.toml — examples are NOT listed here
[workspace]
members = [
    "backends/foundation_deployment",
    # ... other workspace crates
    # examples/* are NOT workspace members
]

# Each example is self-contained:
# examples/gcp/rust-cloud-run/Cargo.toml      → depends on tokio, axum
# examples/aws/rust-lambda/Cargo.toml          → depends on tokio, lambda_http
# examples/cloudflare/rust-worker/Cargo.toml   → depends on worker (wasm async, no tokio)
```

## Requirements

### Example Projects

```
examples/
|-- cloudflare/
|   |-- rust-worker/          # REST API on Cloudflare Workers
|   +-- rust-wasm-worker/     # WASM compute on Cloudflare Workers
|
|-- gcp/
|   |-- rust-cloud-run/       # REST API on Cloud Run
|   +-- rust-cloud-run-job/   # Batch job on Cloud Run Jobs
|
|-- aws/
|   |-- rust-lambda/          # REST API on Lambda + API Gateway
|   +-- rust-lambda-turso/    # Lambda with Turso state store
|
+-- multi-provider/
    +-- rust-api/             # Same API deployed to all 3 providers
```

### Example: Multi-Provider Rust API

Demonstrates the same application deployable to any provider:

```
examples/multi-provider/rust-api/
|-- Cargo.toml
|-- src/
|   +-- main.rs               # Shared application code (axum)
|
|-- configs/
|   |-- wrangler.toml          # Cloudflare config
|   |-- service.yaml           # GCP config
|   +-- template.yaml          # AWS config
|
|-- Dockerfile                 # For GCP Cloud Run
|-- mise.toml                  # Provider-agnostic tasks
+-- README.md                  # How to deploy to each
```

The README shows how the same `mise run deploy` command works with each config:

```bash
# Deploy to Cloudflare
cp configs/wrangler.toml .
mise run deploy

# Deploy to GCP
rm wrangler.toml
cp configs/service.yaml .
mise run deploy

# Deploy to AWS
rm service.yaml
cp configs/template.yaml .
mise run deploy
```

### Example: Lambda with Turso State

Demonstrates the Turso state store integration.

> **Note**: This is a standalone example crate (not a workspace member). The `async`
> and tokio usage is required by `lambda_http` — the deployed Lambda runtime, not the
> deployment tool. This crate has its own `Cargo.toml` with `tokio` and `lambda_http`
> as direct dependencies.

```rust
// examples/aws/rust-lambda-turso/src/main.rs
// Standalone crate — NOT part of foundation_deployment.
// async/tokio required by lambda_http framework.

use lambda_http::{run, service_fn, Body, Error, Request, Response};
use libsql::Database;

async fn handler(event: Request) -> Result<Response<Body>, Error> {
    // Connect to Turso embedded replica
    let db = Database::open_with_remote_sync(
        "local.db",
        std::env::var("TURSO_DATABASE_URL")?,
        std::env::var("TURSO_AUTH_TOKEN")?,
    ).await?;

    let conn = db.connect()?;

    // Read deployment state from Turso
    let mut rows = conn.query(
        "SELECT id, status, output FROM resources WHERE provider = 'aws'",
        [],
    ).await?;

    let mut deployments = vec![];
    while let Some(row) = rows.next().await? {
        deployments.push(serde_json::json!({
            "id": row.get::<String>(0)?,
            "status": row.get::<String>(1)?,
            "output": row.get::<String>(2)?,
        }));
    }

    Ok(Response::builder()
        .status(200)
        .header("content-type", "application/json")
        .body(serde_json::to_string(&deployments)?.into())?)
}
```

### Documentation Structure

```
documentation/deployment/
|-- README.md                    # Index
|-- getting-started.md           # 5-minute quickstart
|
|-- providers/
|   |-- cloudflare.md            # Cloudflare Workers guide
|   |-- gcp-cloud-run.md         # GCP Cloud Run guide
|   +-- aws-lambda.md            # AWS Lambda guide
|
|-- state-management/
|   |-- overview.md              # State store concepts
|   |-- file-store.md            # FileStateStore usage
|   |-- sqlite-store.md          # SqliteStateStore + Turso
|   +-- remote-sync.md           # Syncing state to R2/S3/GCS
|
|-- workflows/
|   |-- ci-cd.md                 # GitHub Actions setup
|   |-- environments.md          # Multi-env (dev/staging/prod)
|   +-- rollback.md              # Rollback procedures
|
+-- troubleshooting/
    |-- common-issues.md         # FAQ
    +-- debugging.md             # Debugging deployment failures
```

### Getting Started Guide

```markdown
# Getting Started with Foundation Deployment

## Choose Your Provider

| Provider | Best For | Config File |
|----------|----------|-------------|
| Cloudflare Workers | Edge compute, low latency | `wrangler.toml` |
| GCP Cloud Run | Containers, long-running services | `service.yaml` |
| AWS Lambda | Event-driven, pay-per-invocation | `template.yaml` |

## Quick Start

### 1. Generate a project

\`\`\`bash
# Cloudflare Workers
ewe_platform generate --lang rust --target cloudflare -p my-api -o .

# GCP Cloud Run
ewe_platform generate --lang rust --target gcp -p my-api -o . --region us-central1

# AWS Lambda
ewe_platform generate --lang rust-lambda --target aws -p my-api -o .
\`\`\`

### 2. Install tools

\`\`\`bash
cd my-api
mise install
\`\`\`

### 3. Authenticate

\`\`\`bash
# Cloudflare
wrangler login

# GCP
gcloud auth login

# AWS
aws configure
\`\`\`

### 4. Deploy

\`\`\`bash
mise run deploy
\`\`\`

That's it. The same command works regardless of provider.

## State Management

By default, deployment state is stored as JSON files in `.deployment/`.

For team workflows, enable Turso (replicated SQLite):

\`\`\`bash
export TURSO_DATABASE_URL=libsql://your-db.turso.io
export TURSO_AUTH_TOKEN=your-token
mise run deploy  # State automatically uses Turso
\`\`\`
```

### Integration Tests

Integration tests for the deployment tool itself use **valtron only** — no tokio.
The `PoolGuard` is initialized in test setup for valtron executor tests.

```rust
// tests/integration/deployment_tests.rs
// Part of foundation_deployment — valtron only, no tokio.

use foundation_deployment::{
    DeploymentExecutor, DeploymentTarget,
    state::{FileStateStore, create_state_store},
};

#[test]
fn test_cloudflare_dry_run() {
    let project_dir = PathBuf::from("../cloudflare/rust-worker");
    let store = Box::new(FileStateStore::new(&project_dir, "cloudflare", "test"));
    let result = DeploymentExecutor::deploy(&project_dir, None, true, store);
    assert!(result.is_ok());
}

#[test]
fn test_provider_detection() {
    // Cloudflare
    assert_eq!(
        DeploymentTarget::detect(Path::new("../cloudflare/rust-worker")),
        Some(DeploymentTarget::Cloudflare)
    );
    // GCP
    assert_eq!(
        DeploymentTarget::detect(Path::new("../gcp/rust-cloud-run")),
        Some(DeploymentTarget::GcpCloudRun)
    );
    // AWS
    assert_eq!(
        DeploymentTarget::detect(Path::new("../aws/rust-lambda")),
        Some(DeploymentTarget::AwsLambda)
    );
}

#[test]
fn test_state_round_trip() {
    let store = FileStateStore::new(Path::new("/tmp/test-state"), "test", "dev");
    store.init().unwrap();

    let state = ResourceState {
        id: "test-resource".into(),
        kind: "test::resource".into(),
        provider: "test".into(),
        status: StateStatus::Created,
        environment: Some("dev".into()),
        config_hash: "abc123".into(),
        output: serde_json::json!({"url": "https://example.com"}),
        config_snapshot: serde_json::json!({}),
        created_at: chrono::Utc::now(),
        updated_at: chrono::Utc::now(),
    };

    store.set("test-resource", &state).unwrap();
    let loaded = store.get("test-resource").unwrap().unwrap();
    assert_eq!(loaded.id, "test-resource");
    assert_eq!(loaded.status, StateStatus::Created);
}
```

## Tasks

1. **Create Cloudflare examples** (standalone crate, `worker` crate — wasm async, no tokio)
   - [ ] `examples/cloudflare/rust-worker/` - REST API with own `Cargo.toml`
   - [ ] `examples/cloudflare/rust-wasm-worker/` - WASM compute with own `Cargo.toml`
   - [ ] Each with wrangler.toml, mise.toml, README.md
   - [ ] Verify NOT listed in workspace `members`
   - [ ] Verify build with `mise run build`

2. **Create GCP examples** (standalone crate, tokio + axum)
   - [ ] `examples/gcp/rust-cloud-run/` - HTTP service with own `Cargo.toml`
   - [ ] `examples/gcp/rust-cloud-run-job/` - Batch job with own `Cargo.toml`
   - [ ] Each with service.yaml, Dockerfile, mise.toml, README.md
   - [ ] Verify NOT listed in workspace `members`
   - [ ] Document that tokio is required by axum (framework requirement)

3. **Create AWS examples** (standalone crate, tokio + lambda_http)
   - [ ] `examples/aws/rust-lambda/` - HTTP API with own `Cargo.toml`
   - [ ] `examples/aws/rust-lambda-turso/` - Lambda with Turso state, own `Cargo.toml`
   - [ ] Each with template.yaml, mise.toml, README.md
   - [ ] Verify NOT listed in workspace `members`
   - [ ] Document that tokio is required by lambda_http (framework requirement)

4. **Create multi-provider example** (standalone crate)
   - [ ] `examples/multi-provider/rust-api/` - Same code, 3 configs, own `Cargo.toml`
   - [ ] Demonstrate provider switching via config file

5. **Create documentation**
   - [ ] Getting started guide
   - [ ] Per-provider deployment guides
   - [ ] State management documentation
   - [ ] CI/CD workflow guide
   - [ ] Troubleshooting guide
   - [ ] Document async runtime policy: valtron-only for deployment tool, framework-required for examples

6. **Write integration tests** (valtron only — no tokio)
   - [ ] Provider detection tests
   - [ ] State store round-trip tests
   - [ ] Dry-run deployment tests for each provider (use valtron `PoolGuard` in test setup)
   - [ ] Live deployment tests (mark `#[ignore]`)
   - [ ] Verify `cargo build -p foundation_deployment` has zero tokio deps

## Success Criteria

- [ ] All 6 tasks completed
- [ ] `cargo clippy -p foundation_deployment -- -D warnings -W clippy::pedantic` — zero warnings, zero suppression
- [ ] `cargo doc -p foundation_deployment --no-deps` — zero rustdoc warnings
- [ ] No `#[allow(...)]` or `#[expect(...)]` anywhere in the code
- [ ] All examples build with `mise run build` (from their own directory)
- [ ] No example crate is a workspace member
- [ ] `cargo build -p foundation_deployment` pulls in zero tokio/async runtime deps
- [ ] Multi-provider example works with all 3 configs
- [ ] Integration tests pass (dry-run) using valtron executor only
- [ ] Documentation renders correctly and is accurate

## Verification

```bash
# Build all examples
for dir in examples/cloudflare/rust-worker examples/gcp/rust-cloud-run examples/aws/rust-lambda; do
  echo "=== Building $dir ==="
  cd $dir && mise install && mise run build && cd -
done

# Run integration tests
cd examples/integration-tests
cargo test -- --nocapture
```

---

_Created: 2026-03-26_
