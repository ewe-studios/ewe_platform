---
description: "Multi-provider deployment system with state management, supporting Cloudflare Workers, GCP Cloud Run/Jobs, and AWS Lambda. API-first deployment via SimpleHttpClient and valtron, with pluggable state stores (Turso, SQLite, JSON file, Cloudflare R2, Cloudflare D1)."
status: "pending"
priority: "high"
created: 2026-03-26
updated: 2026-03-26
author: "Specification Agent"
metadata:
  version: "2.1"
  estimated_effort: "high"
  tags:
    - deployment
    - cloudflare
    - gcp
    - aws
    - lambda
    - cloud-run
    - state-management
    - turso
    - sqlite
    - templates
    - mise
  skills: []
  tools:
    - Rust
    - cargo
    - mise
has_features: true
has_fundamentals: true
builds_on: "specifications/02-build-http-client"
related_specs:
  - "specifications/07-foundation-ai"
  - "specifications/08-valtron-async-iterators"
features:
  completed: 0
  uncompleted: 9
  total: 9
  completion_percentage: 0%
---

# Foundation Deployment Specification

## Overview

This specification defines a **multi-provider deployment system** for the ewe_platform ecosystem. It builds an **API-first** deployment tool that talks directly to cloud provider APIs using `SimpleHttpClient` and valtron async patterns from `foundation_core`. The tool manages its own deployment state via pluggable state stores.

Supported providers:
1. **Cloudflare Workers** (+ Pages, KV, D1, R2)
2. **GCP Cloud Run** (+ Cloud Run Jobs)
3. **AWS Lambda** (+ API Gateway, S3)

## Design Philosophy

### API-First, Not CLI-Wrapping

The primary deployment path calls provider REST APIs **directly** using `SimpleHttpClient` and valtron state machines. This means:

- **No CLI tools required** - no `wrangler`, `gcloud`, or `aws` CLI needed to deploy
- **Full programmatic control** - deploy from within Rust code, CI/CD, or other tools
- **State captured from API responses** - deployment IDs, URLs, versions, resource bindings all tracked in state

Provider config files (`wrangler.toml`, `service.yaml`, `template.yaml`) are **generated when needed** (e.g., for local dev with `wrangler dev` or `sam local`), not treated as the source of truth. The deployment tool owns the state.

### Pluggable State Stores

Deployment state (what's deployed, where, what version) is persisted in a **state store**. Three interchangeable backends:

| Backend | Description | Best For |
|---------|-------------|----------|
| **Turso** | Turso-hosted SQLite with embedded replicas | Teams, CI/CD, cross-machine sync |
| **SQLite** | Plain local SQLite file | Single-machine, no external deps (via turso sqlite) |
| **JSON files** | One JSON file per resource in `.deployment/` | Simplest, git-friendly |

All three implement the same `StateStore` trait. The user picks one via config or environment variables. There is no special relationship between any state store and any provider.

## Goals

- Create `backends/foundation_deployment` crate with a **provider trait** abstraction
- Build **API clients** for each provider using `SimpleHttpClient` + valtron patterns
- Build **pluggable state stores** (Turso, SQLite, JSON file) for tracking deployment state
- Build a **deployment engine** using valtron state machines for orchestration
- Implement **three providers** (Cloudflare, GCP, AWS) as API clients
- Generate provider config files when needed for local dev tooling
- Provide **composable templates** for project scaffolding
- Enable provider-agnostic deployment via `mise run deploy`

## Architecture

```
                        User / Developer
            mise run deploy / ewe_platform deploy
                              |
                              v
                  +---------------------------+
                  |   foundation_deployment   |
                  |                           |
                  |  +---------------------+  |
                  |  | DeploymentEngine    |  |
                  |  | (valtron state      |  |
                  |  |  machine)           |  |
                  |  +---------------------+  |
                  |            |               |
                  |  +---------+----------+   |
                  |  |         |          |   |
                  |  v         v          v   |
                  | +----+  +-----+  +-----+ |
                  | | CF |  | GCP |  | AWS | |
                  | | API|  | API |  | API | |
                  | +----+  +-----+  +-----+ |
                  |  all use SimpleHttpClient |
                  |  + valtron async patterns |
                  |                           |
                  |  +---------------------+  |
                  |  | StateStore (trait)   |  |
                  |  | - TursoStore        |  |
                  |  | - SqliteStore       |  |
                  |  | - JsonFileStore     |  |
                  |  +---------------------+  |
                  +---------------------------+
                              |
                  +-----------+-----------+
                  |           |           |
                  v           v           v
            Cloudflare    GCP Cloud    AWS Lambda
            Workers       Run / Jobs   + API GW
```

## Implementation Location

- **Deployment crate**: `backends/foundation_deployment/`
- **Templates**: `templates/{cf,gcp,aws}-{rust,wasm}-app/`
- **Examples**: `examples/{cloudflare,gcp,aws}/`
- **Specification**: `specifications/11-foundation-deployment/`

---

## Feature Index

### Pending Features (0/9 completed)

1. **[foundation-deployment-core](./features/01-foundation-deployment-core/feature.md)** - Provider trait, errors, shared types
2. **[state-stores](./features/02-state-stores/feature.md)** - Turso, SQLite, and JSON file state store backends
3. **[deployment-engine](./features/03-deployment-engine/feature.md)** - Valtron-based deployment state machine and orchestrator
4. **[cloudflare-provider](./features/04-cloudflare-provider/feature.md)** - Cloudflare API client (Workers, KV, Secrets)
5. **[gcp-cloud-run-provider](./features/05-gcp-cloud-run-provider/feature.md)** - GCP Cloud Run Admin API client
6. **[aws-lambda-provider](./features/06-aws-lambda-provider/feature.md)** - AWS Lambda API client
7. **[templates](./features/07-templates/feature.md)** - Composable project templates (language x provider)
8. **[mise-integration](./features/08-mise-integration/feature.md)** - Provider-agnostic mise tasks
9. **[examples-documentation](./features/09-examples-documentation/feature.md)** - Working examples and documentation

---

## Feature Dependencies

```
01-foundation-deployment-core (base)
         |
    +----+----+
    |         |
    v         v
02-state-stores  03-deployment-engine
    |                    |
    +--------+-----------+
             |
    +--------+---------+
    |        |         |
    v        v         v
04-cf    05-gcp    06-aws
    |        |         |
    +--------+---------+
             |
        07-templates
             |
        08-mise-integration
             |
        09-examples-documentation
```

---

## Key Design Decisions

### 1. API-First Deployment

Each provider is an API client built with `SimpleHttpClient`. The tool calls provider APIs directly:

| Provider | API Used | Auth |
|----------|----------|------|
| Cloudflare | `api.cloudflare.com/client/v4` | Bearer token |
| GCP | `run.googleapis.com/v2` | OAuth2 / Service Account |
| AWS | `lambda.{region}.amazonaws.com` | SigV4 |

Config files like `wrangler.toml` or `service.yaml` are **generated artifacts** when the user needs them (e.g., for `wrangler dev` or `gcloud run services replace`). They are not the deployment source of truth.

### 2. State Store as Source of Truth

The **state store** is the source of truth for what's deployed. It records:
- Resource identity (name, provider, kind)
- Current status (creating, created, updating, failed)
- Config hash (for change detection)
- API response data (deployment IDs, URLs, versions, bindings)

Three interchangeable backends — Turso, SQLite, JSON files — all implement the same trait.

### 3. Provider Trait

```rust
pub trait DeploymentProvider {
    type Config: DeserializeOwned + Serialize;
    type Resources: Debug;

    fn name(&self) -> &str;
    fn validate(&self, config: &Self::Config) -> Result<(), DeploymentError>;
    fn build(&self, config: &Self::Config, env: Option<&str>) -> Result<BuildOutput, DeploymentError>;
    fn deploy(&self, config: &Self::Config, env: Option<&str>, dry_run: bool) -> Result<DeploymentResult, DeploymentError>;
    fn logs(&self, config: &Self::Config, env: Option<&str>) -> Result<(), DeploymentError>;
    fn destroy(&self, config: &Self::Config, env: Option<&str>) -> Result<(), DeploymentError>;
    fn status(&self, config: &Self::Config, env: Option<&str>) -> Result<Self::Resources, DeploymentError>;

    /// Generate provider-specific config files for local dev tooling.
    /// E.g., generate wrangler.toml so `wrangler dev` works locally.
    fn generate_config(&self, config: &Self::Config, output_dir: &Path) -> Result<(), DeploymentError>;
}
```

### 4. Config File Generation

The tool can **generate** provider config files when needed:

```rust
// Generate wrangler.toml for local dev
provider.generate_config(&config, project_dir)?;

// Now the user can run:
//   wrangler dev       (Cloudflare)
//   sam local start-api (AWS)
//   docker compose up   (GCP)
```

This keeps the deployment tool in control while still supporting provider-native local dev workflows.

---

## Success Criteria (Spec-Wide)

### Functionality
- [ ] All 9 features implemented and verified
- [ ] Provider trait implemented with 3 providers, all API-first
- [ ] State stores (Turso, SQLite, JSON file) are interchangeable
- [ ] `mise run deploy` deploys via provider API
- [ ] No CLI tools required for deployment (API-only)
- [ ] Config files can be generated for local dev
- [ ] Templates generate working projects

### Code Quality
- [ ] `cargo clippy -- -D warnings` passes for foundation_deployment
- [ ] `cargo fmt -- --check` passes
- [ ] All unit and integration tests pass
- [ ] State store round-trips are lossless

### Documentation
- [ ] Each provider has deployment guide
- [ ] State store setup documented (Turso, SQLite, JSON)
- [ ] LEARNINGS.md with patterns and troubleshooting

---

## Prerequisites

### Authentication

| Provider | Environment Variables |
|----------|---------------------|
| Cloudflare | `CLOUDFLARE_API_TOKEN`, `CLOUDFLARE_ACCOUNT_ID` |
| GCP | `GOOGLE_APPLICATION_CREDENTIALS` or `GOOGLE_ACCESS_TOKEN`, `GOOGLE_CLOUD_PROJECT`, `GOOGLE_CLOUD_REGION` |
| AWS | `AWS_ACCESS_KEY_ID`, `AWS_SECRET_ACCESS_KEY`, `AWS_REGION` |

### State Store

| Backend | Environment Variables |
|---------|---------------------|
| Turso | `TURSO_DATABASE_URL`, `TURSO_AUTH_TOKEN` |
| SQLite | `DEPLOYMENT_STATE_DB` (path, default: `.deployment/state.db`) |
| JSON file | None (default: `.deployment/{provider}/{stage}/`) |

---

## Open Questions

1. **Cross-provider state**: Should state be shared across providers or isolated per-provider?
2. **Rollback strategy**: Provider-specific (CF versions, GCR revisions, Lambda aliases) or generic?
3. **Secret encryption**: Follow alchemy's XSalsa20-Poly1305 pattern for encrypted state at rest?
4. **Config file generation**: Should `generate_config` be automatic on `deploy` or explicit?

---

_Created: 2026-03-26_
_Structure: Feature-based (has_features: true)_
_Inspiration: alchemy IaC framework (state stores, resource lifecycle, provider model)_
