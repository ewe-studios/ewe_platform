---
workspace_name: "ewe_platform"
spec_directory: "specifications/11-cloudflare-wrangler-deployment"
feature_directory: "specifications/11-cloudflare-wrangler-deployment/features/04-cf-rust-template"
this_file: "specifications/11-cloudflare-wrangler-deployment/features/04-cf-rust-template/feature.md"

status: pending
priority: high
created: 2026-03-26

depends_on: ["01-foundation-deployment-crate"]

tasks:
  completed: 0
  uncompleted: 6
  total: 6
  completion_percentage: 0%
---


# Cloudflare Rust App Template

## Overview

Create a template for deploying pure Rust applications to Cloudflare Workers using the Workers Runtime (Deno-based Rust runtime). This template is for serverless Rust APIs that don't require WASM compilation.

## Dependencies

This feature depends on:
- `01-foundation-deployment-crate` - For deployment tooling

This feature is required by:
- `07-mise-integration` - Uses template structure
- `08-examples-documentation` - Example projects based on template

## Requirements

### Template Structure

```
templates/cf-rust-app/
├── Cargo.toml
├── wrangler.toml
├── mise.toml
├── README.md
├── .gitignore
├── src/
│   └── main.rs
├── public/
│   └── index.html
└── .github/
    └── workflows/
        └── deploy.yml
```

### Cargo.toml Template

```toml
[package]
name = "{{PROJECT_NAME}}"
version = "0.1.0"
edition = "2021"
license = "Apache-2.0"
authors = ["{{AUTHOR}}"]
repository = "{{GITHUB_URL}}/{{PROJECT_NAME}}"

[dependencies]
# Worker runtime
worker = "0.4"

# Serialization
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"

# HTTP types
http = "1.0"

# Logging
console_error_panic_hook = "0.1"
tracing = "0.1"
worker-tracing = "0.1"

# Utilities
anyhow = "1.0"
thiserror = "2.0"

[lib]
crate-type = ["cdylib"]
path = "src/main.rs"

[profile.release]
opt-level = "s"
lto = true
strip = true
```

### wrangler.toml Template

```toml
name = "{{WORKER_NAME}}"
main = "build/worker/shim.mjs"
compatibility_date = "2024-01-01"

# Build configuration
[build]
command = "cargo install -q worker-build && worker-build --release"

# Environment variables
[vars]
ENVIRONMENT = "production"

# Routes (optional - uncomment and configure for custom domains)
# routes = [
#   { pattern = "api.example.com/*", zone_name = "example.com" }
# ]

# KV Namespaces (optional)
# [[kv_namespaces]]
# binding = "MY_KV"
# id = "xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx"

# D1 Databases (optional)
# [[d1_databases]]
# binding = "DB"
# database_name = "my-database"
# database_id = "xxxxxxxx-xxxx-xxxx-xxxx-xxxxxxxxxxxx"

# R2 Buckets (optional)
# [[r2_buckets]]
# binding = "BUCKET"
# bucket_name = "my-bucket"

# Environment-specific configuration
[env.staging]
name = "{{WORKER_NAME}}-staging"
vars = { ENVIRONMENT = "staging" }

[env.dev]
name = "{{WORKER_NAME}}-dev"
vars = { ENVIRONMENT = "development" }
```

### Main Entry Point (src/main.rs)

```rust
use worker::*;
use serde::{Deserialize, Serialize};

// Initialize panic hook for better error messages
#[worker::init]
fn init() {
    console_error_panic_hook::set_once();
}

// Request handler
#[event(fetch)]
async fn main(req: Request, env: Env, ctx: Context) -> Result<Response> {
    // Set up tracing
    tracing::info!("Received request: {} {}", req.method(), req.path());

    // Create router
    let router = Router::new();

    // Define routes
    router
        .get("/", |_, ctx| async move {
            Response::ok("Hello from Rust on Cloudflare Workers!")
        })
        .get("/api/health", |_, ctx| async move {
            let health = HealthResponse {
                status: "healthy".to_string(),
                timestamp: Date::now().to_string(),
            };
            Response::from_json(&health)
        })
        .get("/api/greet/:name", |req, ctx| async move {
            let name = ctx.param("name").unwrap_or("stranger");
            let greeting = GreetResponse {
                message: format!("Hello, {}!", name),
                from: "Cloudflare Workers".to_string(),
            };
            Response::from_json(&greeting)
        })
        .post("/api/echo", |mut req, ctx| async move {
            let body = req.text().await?;
            Response::ok(body)
        })
        .run(req, env)
        .await
}

// Response types
#[derive(Serialize)]
struct HealthResponse {
    status: String,
    timestamp: String,
}

#[derive(Serialize)]
struct GreetResponse {
    message: String,
    from: String,
}

#[derive(Deserialize)]
struct EchoRequest {
    data: String,
}
```

### mise.toml Template

```toml
[tools]
rust = "1.87"
nodejs = "20"
wrangler = "latest"

[tasks.install-worker-build]
description = "Install worker-build tool"
run = "cargo install worker-build"

[tasks.build]
description = "Build the worker"
depends = ["install-worker-build"]
run = """
worker-build --release
"""

[tasks.dev]
description = "Run local development server"
depends = ["build"]
run = "wrangler dev"

[tasks.deploy_cf]
description = "Deploy to Cloudflare Workers"
depends = ["build"]
run = "wrangler deploy"

[tasks.deploy_cf_staging]
description = "Deploy to staging environment"
depends = ["build"]
run = "wrangler deploy --env staging"

[tasks.logs]
description = "Tail worker logs"
run = "wrangler tail"

[tasks.secret]
description = "Manage secrets (usage: mise run secret <KEY>)"
run = "wrangler secret put"

[tasks.check]
description = "Run all checks"
run = """
cargo check
cargo fmt --check
cargo clippy -- -D warnings
"""

[tasks.test]
description = "Run tests"
run = "cargo test"
```

### README.md Template

```markdown
# {{PROJECT_NAME}}

A Rust application deployed to Cloudflare Workers.

## Prerequisites

- Rust 1.87+
- [mise](https://mise.jdx.dev/) for tooling management
- Cloudflare account

## Setup

1. Install dependencies:
   ```bash
   mise install
   ```

2. Authenticate with Cloudflare:
   ```bash
   wrangler login
   ```

3. Configure `wrangler.toml` with your account ID if needed.

## Development

Run local development server:
```bash
mise run dev
```

The worker will be available at `http://localhost:8787`.

## Deployment

Deploy to production:
```bash
mise run deploy_cf
```

Deploy to staging:
```bash
mise run deploy_cf_staging
```

## API Endpoints

| Method | Path | Description |
|--------|------|-------------|
| GET | `/` | Hello world |
| GET | `/api/health` | Health check |
| GET | `/api/greet/:name` | Greeting |
| POST | `/api/echo` | Echo request body |

## Environment Variables

Configure in `wrangler.toml`:

```toml
[vars]
MY_VAR = "value"
```

Access in code:
```rust
let value = env.var("MY_VAR")?;
```

## Resources

- [Cloudflare Workers Documentation](https://developers.cloudflare.com/workers/)
- [worker-rust Documentation](https://github.com/cloudflare/workers-rs)
```

### GitHub Actions Workflow

```yaml
# .github/workflows/deploy.yml
name: Deploy to Cloudflare Workers

on:
  push:
    branches: [main]
  pull_request:
    branches: [main]

jobs:
  deploy:
    runs-on: ubuntu-latest

    steps:
      - uses: actions/checkout@v4

      - name: Install Rust
        uses: dtolnay/rust-action@stable

      - name: Install mise
        uses: jdx/mise-action@v2

      - name: Install dependencies
        run: mise install

      - name: Build worker
        run: mise run build

      - name: Deploy to Cloudflare
        uses: cloudflare/wrangler-action@v3
        with:
          apiToken: ${{ secrets.CLOUDFLARE_API_TOKEN }}
          account-id: ${{ secrets.CLOUDFLARE_ACCOUNT_ID }}
```

## Tasks

1. **Create template directory structure**
   - [ ] Create `templates/cf-rust-app/` directory
   - [ ] Create all subdirectories (src/, public/, .github/workflows/)
   - [ ] Add .gitignore

2. **Create Cargo.toml template**
   - [ ] Write Cargo.toml with template variables
   - [ ] Configure cdylib crate type
   - [ ] Add all required dependencies
   - [ ] Set release profile optimizations

3. **Create wrangler.toml template**
   - [ ] Write base configuration
   - [ ] Add build command
   - [ ] Configure environment overrides
   - [ ] Add commented examples for KV, D1, R2

4. **Create main.rs template**
   - [ ] Write worker entry point with `#[event(fetch)]`
   - [ ] Add example routes
   - [ ] Include response type definitions
   - [ ] Add panic hook initialization

5. **Create mise.toml template**
   - [ ] Define tool versions
   - [ ] Add build task
   - [ ] Add dev task
   - [ ] Add deploy_cf task
   - [ ] Add utility tasks (logs, secret, check, test)

6. **Create documentation**
   - [ ] Write README.md with template variables
   - [ ] Create GitHub Actions workflow
   - [ ] Add .gitignore for Rust/Node projects

## Implementation Notes

- Uses `worker` crate (cloudflare/workers-rs) for Rust Workers
- `worker-build` tool compiles Rust to WASM and creates shim
- Template variables: `{{PROJECT_NAME}}`, `{{WORKER_NAME}}`, `{{AUTHOR}}`, `{{GITHUB_URL}}`
- Release profile optimized for size (`opt-level = "s"`)

## Success Criteria

- [ ] All 6 tasks completed
- [ ] Generated project builds successfully with `worker-build`
- [ ] Generated project deploys with `mise run deploy_cf`
- [ ] All example routes respond correctly
- [ ] README renders correctly after template substitution

## Verification

```bash
# Generate project from template
cargo run --bin ewe_platform generate \
  --template_name="cf-rust-app" \
  -p "my-rust-worker" \
  -o /tmp/my-rust-worker

# Build generated project
cd /tmp/my-rust-worker
mise install
mise run build

# Deploy (requires Cloudflare auth)
mise run deploy_cf
```

---

_Created: 2026-03-26_
