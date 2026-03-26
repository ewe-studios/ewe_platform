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

## Overview

Create working example projects for each provider and comprehensive documentation covering deployment workflows, state management, troubleshooting, and migration guides.

## Dependencies

Depends on all previous features (complete implementation required for working examples).

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

Demonstrates the Turso state store integration:

```rust
// examples/aws/rust-lambda-turso/src/main.rs

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

```rust
// examples/integration-tests/src/main.rs

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

1. **Create Cloudflare examples**
   - [ ] `examples/cloudflare/rust-worker/` - REST API
   - [ ] `examples/cloudflare/rust-wasm-worker/` - WASM compute
   - [ ] Each with wrangler.toml, mise.toml, README.md
   - [ ] Verify build with `mise run build`

2. **Create GCP examples**
   - [ ] `examples/gcp/rust-cloud-run/` - HTTP service
   - [ ] `examples/gcp/rust-cloud-run-job/` - Batch job
   - [ ] Each with service.yaml, Dockerfile, mise.toml, README.md

3. **Create AWS examples**
   - [ ] `examples/aws/rust-lambda/` - HTTP API
   - [ ] `examples/aws/rust-lambda-turso/` - Lambda with Turso state
   - [ ] Each with template.yaml, mise.toml, README.md

4. **Create multi-provider example**
   - [ ] `examples/multi-provider/rust-api/` - Same code, 3 configs
   - [ ] Demonstrate provider switching via config file

5. **Create documentation**
   - [ ] Getting started guide
   - [ ] Per-provider deployment guides
   - [ ] State management documentation
   - [ ] CI/CD workflow guide
   - [ ] Troubleshooting guide

6. **Write integration tests**
   - [ ] Provider detection tests
   - [ ] State store round-trip tests
   - [ ] Dry-run deployment tests for each provider
   - [ ] Live deployment tests (mark `#[ignore]`)

## Success Criteria

- [ ] All 6 tasks completed
- [ ] All examples build with `mise run build`
- [ ] Multi-provider example works with all 3 configs
- [ ] Integration tests pass (dry-run)
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
