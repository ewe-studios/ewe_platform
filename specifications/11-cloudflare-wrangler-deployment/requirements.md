---
description: "Create a Cloudflare deployment system with wrangler integration, including foundation_deployment crate, project templates, and mise.toml-based tooling for deploying Rust, Rust+WASM, and generic WASM projects to Cloudflare Workers."
status: "pending"
priority: "high"
created: 2026-03-26
author: "Specification Agent"
metadata:
  version: "1.0"
  estimated_effort: "high"
  tags:
    - cloudflare
    - wrangler
    - deployment
    - workers
    - wasm
    - templates
    - mise
  skills: []
  tools:
    - Rust
    - cargo
    - wrangler
    - mise
has_features: true
has_fundamentals: true
builds_on: "specifications/02-build-http-client"
related_specs:
  - "specifications/07-foundation-ai"
  - "specifications/08-valtron-async-iterators"
features:
  completed: 0
  uncompleted: 8
  total: 8
  completion_percentage: 0%
---

# Cloudflare Wrangler Deployment Specification

## Overview

This specification defines a comprehensive deployment system for Cloudflare Workers using wrangler, integrated with the ewe_platform ecosystem. It includes:

1. **foundation_deployment crate** - Core deployment utilities, process management, and Cloudflare API clients
2. **Project templates** - Ready-to-use templates for Rust, Rust+WASM, and generic WASM projects
3. **mise.toml integration** - Declarative tooling configuration and deployment tasks
4. **Examples** - Self-contained example projects demonstrating each template type

## Goals

- Create `backends/foundation_deployment` crate with deployment tooling
- Build Cloudflare API clients using `simple_http` and valtron async patterns
- Provide templates for rapid project scaffolding via `ewe_platform generate`
- Enable one-command deployment with `mise run deploy_cf`
- Document Cloudflare Workers deployment patterns and best practices

## Implementation Location

- **Deployment crate**: `backends/foundation_deployment/src/`
- **Templates**: `templates/cf-rust-app/`, `templates/cf-rust-wasm-app/`, `templates/cf-wasm-app/`
- **Examples**: `examples/cloudflare/rust-worker/`, `examples/cloudflare/rust-wasm-worker/`, `examples/cloudflare/wasm-worker/`
- **Specification**: `specifications/11-cloudflare-wrangler-deployment/`

---

## Feature Index

### Pending Features (0/8 completed)

1. **[foundation-deployment-crate](./features/01-foundation-deployment-crate/feature.md)** - Core deployment crate structure
2. **[wrangler-process-wrapper](./features/02-wrangler-process-wrapper/feature.md)** - Process execution for wrangler commands
3. **[cloudflare-api-client](./features/03-cloudflare-api-client/feature.md)** - Cloudflare REST API integration
4. **[cf-rust-template](./features/04-cf-rust-template/feature.md)** - Rust serverless function template
5. **[cf-rust-wasm-template](./features/05-cf-rust-wasm-template/feature.md)** - Rust+WASM worker template with foundation_wasm
6. **[cf-generic-wasm-template](./features/06-cf-generic-wasm-template/feature.md)** - Generic WASM worker template
7. **[mise-integration](./features/07-mise-integration/feature.md)** - mise.toml configuration and tasks
8. **[examples-documentation](./features/08-examples-documentation/feature.md)** - Example projects and documentation

---

## High-Level Architecture

```
┌─────────────────────────────────────────────────────────────────┐
│                     User / Developer                            │
│              mise run deploy_cf / ewe_platform generate         │
└─────────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────────┐
│                  foundation_deployment                          │
│  ┌─────────────────┐  ┌─────────────────┐  ┌─────────────────┐ │
│  │ WranglerRunner  │  │ CloudflareAPI   │  │ DeployPlanner   │ │
│  │ (process exec)  │  │ (simple_http)   │  │ (state machine) │ │
│  └─────────────────┘  └─────────────────┘  └─────────────────┘ │
│  ┌─────────────────┐  ┌─────────────────┐  ┌─────────────────┐ │
│  │ ProjectScanner  │  │ ConfigValidator │  │ SecretManager   │ │
│  └─────────────────┘  └─────────────────┘  └─────────────────┘ │
└─────────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────────┐
│                    Templates (ewe_temple)                       │
│  ┌──────────────┐  ┌──────────────────┐  ┌─────────────────┐   │
│  │ cf-rust-app  │  │ cf-rust-wasm-app │  │ cf-wasm-app     │   │
│  │ (Axum/Hono)  │  │ (foundation_wasm)│  │ (generic wasm)  │   │
│  └──────────────┘  └──────────────────┘  └─────────────────┘   │
└─────────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────────┐
│                   Cloudflare Workers                            │
│  ┌──────────────┐  ┌──────────────┐  ┌──────────────────────┐  │
│  │ Worker       │  │ Durable      │  │ KV / R2 / D1         │  │
│  │ (JS/Rust)    │  │ Objects      │  │ Storage              │  │
│  └──────────────┘  └──────────────┘  └──────────────────────┘  │
└─────────────────────────────────────────────────────────────────┘
```

---

## Success Criteria (Spec-Wide)

This specification is considered complete when:

### Functionality
- [ ] All 8 features implemented and verified
- [ ] `ewe_platform generate --template_name="cf-rust-app"` generates working project
- [ ] `mise run deploy_cf` deploys projects to Cloudflare Workers
- [ ] Cloudflare API client can manage workers, KV, secrets, and deployments
- [ ] All three templates (Rust, Rust+WASM, WASM) deploy successfully

### Code Quality
- [ ] `cargo clippy -- -D warnings` passes for foundation_deployment
- [ ] `cargo fmt -- --check` passes for all modified files
- [ ] All unit and integration tests pass
- [ ] Example projects build and deploy without errors

### Documentation
- [ ] Each template includes README.md with setup instructions
- [ ] API documentation for foundation_deployment crate
- [ ] LEARNINGS.md with deployment patterns and troubleshooting
- [ ] VERIFICATION.md with deployment check procedures

---

## Prerequisites

### System Requirements

Users deploying to Cloudflare will need:

1. **wrangler CLI** - Installed via mise or npm
2. **Cloudflare account** - Free tier sufficient for development
3. **API token** - With Workers write permissions
4. **mise** - For tooling management (recommended)

### Cloudflare Authentication

Authentication is handled via:

```bash
# Interactive login (recommended for development)
wrangler login

# Or API token in environment (CI/CD)
export CLOUDFLARE_API_TOKEN="your_token_here"
```

---

## Cloudflare Workers Fundamentals

### Worker Types

| Type | Description | Use Case |
|------|-------------|----------|
| **JavaScript** | Standard ES modules | Quick prototyping, JS-heavy logic |
| **Rust+WASM** | Rust compiled to WebAssembly | Performance-critical, type safety |
| **Static Assets** | SPA hosting with worker routing | Frontend apps with API needs |

### Worker Limits (Free Tier)

- **CPU time**: 10ms per request
- **Memory**: 128MB per request
- **Requests**: 100,000 per day
- **Script size**: 1MB (compressed)

### Deployment Targets

| Target | Description |
|--------|-------------|
| **workers.dev** | Free subdomain (`<name>.<subdomain>.workers.dev`) |
| **Custom domain** | Your own domain with Workers routing |
| **Zones** | Cloudflare-managed DNS zones |

---

## Feature Dependencies

```
01-foundation-deployment-crate (base)
         │
    ┌────┴────┐
    │         │
    ▼         ▼
02-wrangler-process-wrapper  03-cloudflare-api-client
    │         │
    └────┬────┘
         │
         ▼
04-cf-rust-template  05-cf-rust-wasm-template  06-cf-generic-wasm-template
         │                    │                        │
         └────────────────────┼────────────────────────┘
                              │
                              ▼
                    07-mise-integration
                              │
                              ▼
                    08-examples-documentation
```

---

## Implementation Guidelines

### foundation_deployment Crate Structure

```
backends/foundation_deployment/
├── Cargo.toml
├── src/
│   ├── lib.rs              # Crate root, re-exports
│   ├── wrangler/
│   │   ├── mod.rs          # Wrangler runner module
│   │   ├── commands.rs     # Command builders (deploy, tail, secret)
│   │   ├── config.rs       # wrangler.toml parsing
│   │   └── version.rs      # Version detection
│   ├── cloudflare/
│   │   ├── mod.rs          # API client module
│   │   ├── client.rs       # Cloudflare HTTP client
│   │   ├── auth.rs         # API token handling
│   │   ├── workers.rs      # Workers API (CRUD operations)
│   │   ├── kv.rs           # KV namespace operations
│   │   ├── d1.rs           # D1 database operations
│   │   ├── r2.rs           # R2 bucket operations
│   │   ├── secrets.rs      # Secret management
│   │   └── types.rs        # Shared API types
│   ├── process/
│   │   ├── mod.rs          # Process execution utilities
│   │   ├── executor.rs     # Async process runner
│   │   └── output.rs       # Output capture and streaming
│   ├── project/
│   │   ├── mod.rs          # Project analysis module
│   │   ├── scanner.rs      # Project structure scanner
│   │   ├── validator.rs    # Config validation
│   │   └── builder.rs      # Build command orchestration
│   └── deploy/
│       ├── mod.rs          # Deployment orchestration
│       ├── planner.rs      # Deployment state machine
│       ├── executor.rs     # Multi-step deployment
│       └── rollback.rs     # Rollback handling
├── tests/
│   ├── wrangler_tests.rs   # Wrangler integration tests
│   ├── api_tests.rs        # Cloudflare API tests
│   └── deployment_tests.rs # End-to-end deployment
└── examples/
    └── deploy_worker.rs    # Deployment example
```

### Template Structure

Each template follows the `ewe_temple` format:

```
templates/cf-rust-app/
├── Cargo.toml              # Project manifest (template variables)
├── wrangler.toml           # Wrangler configuration
├── mise.toml               # Tooling and tasks
├── README.md               # Template documentation
├── src/
│   └── main.rs             # Rust entry point (Axum/Hono style)
├── public/                 # Static assets (optional)
└── .github/
    └── workflows/
        └── deploy.yml      # CI/CD workflow template
```

### mise.toml Pattern

```toml
[tools]
rust = "1.87"
wasm-pack = "latest"
wrangler = "latest"
nodejs = "20"

[tasks]
# Build the project
build = "cargo build --release --target wasm32-unknown-unknown"

# Run development server
dev = "wrangler dev"

# Deploy to Cloudflare
deploy_cf = """
#!/bin/bash
set -e

echo "Building worker..."
cargo build --release --target wasm32-unknown-unknown
wasm-pack build --target no-modules --out-dir pkg

echo "Deploying to Cloudflare..."
wrangler deploy

echo "Deployment complete!"
"""

# Tail worker logs
logs = "wrangler tail"

# Manage secrets
secrets = "wrangler secret put"
```

---

## Cloudflare API Endpoints

The `cloudflare` module will wrap these REST APIs:

### Workers API

| Method | Endpoint | Description |
|--------|----------|-------------|
| `PUT` | `/accounts/{account_id}/workers/scripts/{script_name}` | Upload worker script |
| `GET` | `/accounts/{account_id}/workers/scripts/{script_name}` | Get worker script |
| `DELETE` | `/accounts/{account_id}/workers/scripts/{script_name}` | Delete worker |
| `POST` | `/accounts/{account_id}/workers/scripts/{script_name}/subdomains` | Deploy to workers.dev |

### Secrets Management

| Method | Endpoint | Description |
|--------|----------|-------------|
| `PUT` | `/accounts/{account_id}/workers/scripts/{script_name}/secrets` | Create/update secret |
| `DELETE` | `/accounts/{account_id}/workers/scripts/{script_name}/secrets/{secret_name}` | Delete secret |
| `GET` | `/accounts/{account_id}/workers/scripts/{script_name}/secrets` | List secrets |

### KV Namespaces

| Method | Endpoint | Description |
|--------|----------|-------------|
| `POST` | `/accounts/{account_id}/storage/kv/namespaces` | Create namespace |
| `PUT` | `/accounts/{account_id}/storage/kv/namespaces/{namespace_id}/values/{key}` | Set key |
| `GET` | `/accounts/{account_id}/storage/kv/namespaces/{namespace_id}/values/{key}` | Get key |
| `DELETE` | `/accounts/{account_id}/storage/kv/namespaces/{namespace_id}/values/{key}` | Delete key |

### D1 Databases

| Method | Endpoint | Description |
|--------|----------|-------------|
| `POST` | `/accounts/{account_id}/d1/database` | Create database |
| `POST` | `/accounts/{account_id}/d1/database/{database_id}/query` | Execute query |

---

## Valtron Integration

The deployment system uses valtron async patterns for:

### Deployment State Machine

```rust
enum DeployState {
    Initializing,
    ValidatingConfig,
    BuildingWorker,
    UploadingScript,
    Deploying,
    Verifying,
    Complete { deployment_id: String },
    Failed { error: DeployError },
}

impl StateMachine for DeployPlanner {
    type State = DeployState;
    type Output = DeploymentResult;
    type Error = DeployError;
    type Action = NoAction;

    fn transition(&mut self, state: DeployState) -> StateTransition<...> {
        match state {
            DeployState::Initializing => {
                StateTransition::Continue(DeployState::ValidatingConfig)
            }
            DeployState::ValidatingConfig => {
                match self.validate_config() {
                    Ok(_) => StateTransition::Continue(DeployState::BuildingWorker),
                    Err(e) => StateTransition::Error(e),
                }
            }
            // ... additional states
            DeployState::Complete { deployment_id } => {
                StateTransition::Complete(DeploymentResult { deployment_id })
            }
        }
    }
}
```

### Parallel Resource Checks

```rust
// Check multiple resources in parallel
let tasks = vec![
    CheckWorkerTask::new(worker_name),
    CheckKVTask::new(namespace_id),
    CheckSecretsTask::new(worker_name),
];

let collected = execute_collect_all(tasks, None)?;
```

---

## Template Variables

Templates use `ewe_temple` variable substitution:

| Variable | Description | Example |
|----------|-------------|---------|
| `{{PROJECT_NAME}}` | Project name | `my-worker` |
| `{{GITHUB_NAMESPACE}}` | GitHub repo URL | `github.com/user/repo` |
| `{{CLOUDFLARE_ACCOUNT_ID}}` | CF Account ID | `abc123...` |
| `{{WORKER_NAME}}` | Worker script name | `my-worker` |
| `{{WORKER_ROUTE}}` | Optional route pattern | `api.example.com/*` |

---

## Testing Strategy

### Unit Tests

```rust
#[test]
fn test_wrangler_command_builder() {
    let cmd = WranglerCommand::Deploy { env: None, dry_run: true };
    assert_eq!(cmd.to_args(), vec!["deploy", "--dry-run"]);
}

#[test]
fn test_cloudflare_auth_header() {
    let auth = CloudflareAuth::new("test_token");
    assert!(auth.header().starts_with("Bearer "));
}
```

### Integration Tests

```rust
#[test]
#[ignore] // Requires Cloudflare account
fn test_deploy_worker() {
    let _guard = valtron::initialize_pool(4, None);

    let result = deploy_worker("test-worker", "./examples/test");
    assert!(result.is_ok());
}
```

### Template Tests

```bash
# Generate from template
cargo run --bin ewe_platform generate \
  --template_name="cf-rust-app" \
  -p "test-worker" \
  -o /tmp/test-worker

# Build generated project
cd /tmp/test-worker
mise run build
```

---

## Security Considerations

### API Token Handling

- Never commit API tokens to version control
- Use environment variables or secrets manager
- Rotate tokens periodically
- Use minimal permission scopes

### Worker Security

- Validate all input in workers
- Use CSP headers for static assets
- Implement rate limiting
- Log security-relevant events

### Secret Management

```toml
# wrangler.toml - DO NOT commit secrets
[vars]
PUBLIC_KEY = "public_value"

# Use wrangler secret put for sensitive values
# This prompts for the secret value interactively
```

---

## Migration Path

For existing projects:

1. Add `wrangler.toml` to project root
2. Add `mise.toml` with tooling configuration
3. Create Cloudflare Worker entry point
4. Configure build target (wasm32 for Rust)
5. Run `mise run deploy_cf`

---

## Related Documentation

- `documentation/wrangler/quickstart.md` - Getting started guide
- `documentation/cloudflare/workers-api.md` - API reference
- `documentation/templates/cf-rust-app.md` - Rust template guide
- `documentation/templates/cf-rust-wasm-app.md` - Rust+WASM guide
- `documentation/templates/cf-wasm-app.md` - Generic WASM guide

---

## Open Questions

1. **Multi-environment support**: How to handle dev/staging/prod environments?
2. **Database migrations**: Best practices for D1 schema changes?
3. **Rollback strategy**: Automated rollback on deployment failure?
4. **Monitoring integration**: Built-in health checks and alerting?

---

_This specification builds on the existing `simple_http` client, valtron async patterns, and ewe_temple template system. It extends the platform's deployment capabilities to include Cloudflare Workers as a first-class target._

---

_Created: 2026-03-26_
_Structure: Feature-based (has_features: true)_
