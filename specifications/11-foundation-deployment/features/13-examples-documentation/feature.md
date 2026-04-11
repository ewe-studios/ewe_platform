---
workspace_name: "ewe_platform"
spec_directory: "specifications/11-foundation-deployment"
feature_directory: "specifications/11-foundation-deployment/features/13-examples-documentation"
this_file: "specifications/11-foundation-deployment/features/13-examples-documentation/feature.md"

status: pending
priority: high
created: 2026-03-26
updated: 2026-04-11

depends_on: ["01-foundation-deployment-core", "11-templates", "12-mise-integration", "31-deployment-plan-schema", "34-plan-executor"]

tasks:
  completed: 0
  uncompleted: 7
  total: 7
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

## Overview

Create working example projects demonstrating deployment plans in action, plus comprehensive documentation covering deployment workflows, state management, troubleshooting, and migration guides.

## Updated for Deployment Plans

**Key change:** All examples now use `.deployment-plan.yaml` as the primary configuration format. Legacy configs are shown for reference but marked as deprecated.

## Dependencies

Depends on all previous features (complete implementation required for working examples).

## Requirements

### Example Projects

```
examples/
├── cloudflare/
│   ├── rust-worker/          # REST API on Cloudflare Workers
│   └── rust-wasm-worker/     # WASM compute on Cloudflare Workers
│
├── gcp/
│   ├── rust-cloud-run/       # REST API on Cloud Run
│   └── rust-cloud-run-job/   # Batch job on Cloud Run Jobs
│
├── aws/
│   ├── rust-lambda/          # REST API on Lambda + API Gateway
│   └── rust-lambda-turso/    # Lambda with Turso state store
│
└── multi-provider/
    └── rust-api/             # Same API deployed to all 3 providers
```

### Example: Multi-Provider Rust API

Demonstrates the same application with different deployment plans:

```
examples/multi-provider/rust-api/
├── Cargo.toml
├── src/
│   └── main.rs               # Shared application code
│
├── plans/
│   ├── cloudflare.yaml       # Cloudflare deployment plan
│   ├── gcp.yaml              # GCP deployment plan
│   └── aws.yaml              # AWS deployment plan
│
├── Dockerfile                 # For GCP Cloud Run
├── mise.toml                  # Provider-agnostic tasks
└── README.md                  # How to deploy to each provider
```

**Deployment plans:**

```yaml
# plans/cloudflare.yaml
schema_version: "1.0"
provider: cloudflare
project: multi-provider-api
stage: production

service:
  name: multi-api-cf
  main: src/lib.rs
  compatibility_date: "2026-01-01"
  
state_store:
  backend: d1
  database_name: multi-api-state
```

```yaml
# plans/gcp.yaml
schema_version: "1.0"
provider: gcp
project: multi-provider-api
stage: production

service:
  name: multi-api-gcp
  image: gcr.io/my-project/multi-api:latest
  region: us-central1
  
  service_account:
    create_new: true
    
  iam:
    roles:
      - roles/run.invoker
      - roles/logging.logWriter

state_store:
  backend: r2
  bucket: multi-api-state
```

```yaml
# plans/aws.yaml
schema_version: "1.0"
provider: aws
project: multi-provider-api
stage: production

service:
  name: multi-api-lambda
  handler: bootstrap
  runtime: provided.al2
  region: us-east-1
  
  service_account:
    use_existing: "lambda-executor@my-project.iam.gserviceaccount.com"
    
  iam:
    policies:
      - arn:aws:iam::aws:policy/service-role/AWSLambdaBasicExecutionRole

state_store:
  backend: sqlite
```

**mise.toml tasks:**

```toml
[tasks.deploy_cf]
description = "Deploy to Cloudflare"
run = "ewe_platform deploy --plan plans/cloudflare.yaml"

[tasks.deploy_gcp]
description = "Deploy to GCP"
run = "ewe_platform deploy --plan plans/gcp.yaml"

[tasks.deploy_aws]
description = "Deploy to AWS"
run = "ewe_platform deploy --plan plans/aws.yaml"
```

### Example: Infrastructure Plan

Demonstrates creating infrastructure before deployment:

```yaml
# .infrastructure-plan.yaml
schema_version: "1.0"
provider: gcp
project: my-app
stage: production

resources:
  - type: service_account
    name: my-service-sa
    display_name: "My Service Account"
    iam:
      roles:
        - roles/run.invoker
        - roles/logging.logWriter
        
  - type: database
    engine: postgres
    version: "15"
    tier: db-custom-1-2048
    storage: 10Gi
    region: us-central1
    
  - type: bucket
    name: my-assets-bucket
    location: us-central1
    storage_class: STANDARD
    
outputs:
  database_url: ${resources.database.my-db.connection_string}
  bucket_name: ${resources.bucket.my-assets-bucket.name}
```

### Documentation Sections

1. **Getting Started**
   - Installation
   - Quick start (5 minutes)
   - Your first deployment

2. **Deployment Plans**
   - Schema overview
   - Provider-specific options
   - Variable substitution
   - Infrastructure plans

3. **State Management**
   - State store backends (File, SQLite, Turso, R2, D1)
   - Change detection
   - Rollback procedures

4. **Provider Guides**
   - Cloudflare Workers
   - GCP Cloud Run
   - AWS Lambda
   - Multi-provider deployments

5. **Infrastructure Plans**
   - Creating service accounts
   - Managing IAM roles
   - Databases and buckets
   - Drift detection

6. **CI/CD Integration**
   - GitHub Actions
   - GitLab CI
   - CircleCI

7. **Troubleshooting**
   - Common errors
   - Debug mode
   - Support channels

8. **Migration Guide**
   - From legacy configs to deployment plans
   - From Terraform/Pulumi
   - From other deployment tools

## Tasks

1. **Create multi-provider example**
   - [ ] Shared Rust application code
   - [ ] Three deployment plans (CF, GCP, AWS)
   - [ ] mise.toml with all deploy tasks
   - [ ] README with deployment instructions
   - [ ] Write tests

2. **Create infrastructure plan example**
   - [ ] Sample infrastructure plan
   - [ ] Deployment plan referencing infra outputs
   - [ ] Step-by-step README
   - [ ] Write tests

3. **Create provider-specific examples**
   - [ ] Cloudflare Worker example
   - [ ] GCP Cloud Run example
   - [ ] AWS Lambda example
   - [ ] Each with working deployment plan
   - [ ] Write tests

4. **Write documentation**
   - [ ] Getting Started guide
   - [ ] Deployment Plans reference
   - [ ] State Management guide
   - [ ] Provider guides
   - [ ] CI/CD integration guide
   - [ ] Troubleshooting guide
   - [ ] Migration guide

5. **Record demos/tutorials**
   - [ ] Screen recording of first deployment
   - [ ] Multi-provider deployment demo
   - [ ] Infrastructure plan walkthrough

6. **Create quickstart templates**
   - [ ] `ewe_platform init` command
   - [ ] Interactive setup wizard
   - [ ] Pre-configured templates

7. **API documentation**
   - [ ] Generate rustdoc for `foundation_deployment`
   - [ ] Document public API
   - [ ] Add examples to docstrings

## Success Criteria

- [ ] All 7 tasks completed
- [ ] `cargo clippy -p foundation_deployment -- -D warnings -W clippy::pedantic` — zero warnings
- [ ] All examples deploy successfully
- [ ] Documentation is comprehensive and accurate
- [ ] Migration guide works for legacy users
- [ ] Quickstart completes in under 5 minutes

## Verification

```bash
cd backends/foundation_deployment
cargo check
cargo clippy -- -D warnings
cargo doc --no-deps
```

---

_Created: 2026-03-26_
_Updated: 2026-04-11 - Updated for deployment plan model_

