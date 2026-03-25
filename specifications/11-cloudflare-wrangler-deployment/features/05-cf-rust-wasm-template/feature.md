---
workspace_name: "ewe_platform"
spec_directory: "specifications/11-cloudflare-wrangler-deployment"
feature_directory: "specifications/11-cloudflare-wrangler-deployment/features/05-cf-rust-wasm-template"
this_file: "specifications/11-cloudflare-wrangler-deployment/features/05-cf-rust-wasm-template/feature.md"

status: pending
priority: high
created: 2026-03-26

depends_on: ["01-foundation-deployment-crate", "04-cf-rust-template"]

tasks:
  completed: 0
  uncompleted: 7
  total: 7
  completion_percentage: 0%
---


# Cloudflare Rust+WASM App Template

## Overview

Create a template for deploying Rust applications compiled to WebAssembly for Cloudflare Workers, leveraging `foundation_wasm` for WASM-specific utilities and patterns.

## Dependencies

This feature depends on:
- `01-foundation-deployment-crate` - For deployment tooling
- `04-cf-rust-template` - Builds on Rust template structure

This feature is required by:
- `07-mise-integration` - Uses template structure
- `08-examples-documentation` - Example projects based on template

## Requirements

### Template Structure

```
templates/cf-rust-wasm-app/
├── Cargo.toml
├── wrangler.toml
├── mise.toml
├── README.md
├── .gitignore
├── src/
│   ├── lib.rs              # WASM entry point
│   ├── handlers/
│   │   └── mod.rs          # Request handlers
│   └── utils/
│       └── mod.rs          # Utilities
├── pkg/                    # Built WASM output (gitignored)
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

[lib]
crate-type = ["cdylib", "rlib"]
path = "src/lib.rs"

[dependencies]
# WASM bindings
wasm-bindgen = "0.2"
wasm-bindgen-futures = "0.4"
js-sys = "0.3"

# Foundation crates
foundation_wasm = { path = "../../backends/foundation_wasm", version = "0.0.2" }
foundation_core = { path = "../../backends/foundation_core", version = "0.0.3" }

# Serialization
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"

# HTTP handling
worker = "0.4"
http = "1.0"

# Logging
console_error_panic_hook = "0.1"
tracing = "0.1"
worker-tracing = "0.1"

# Utilities
anyhow = "1.0"
thiserror = "2.0"
getrandom = { version = "0.2", features = ["js"] }

[dev-dependencies]
wasm-bindgen-test = "0.3"

[profile.release]
opt-level = "s"
lto = true
strip = true
codegen-units = 1
```

### wrangler.toml Template

```toml
name = "{{WORKER_NAME}}"
main = "pkg/{{PROJECT_NAME}}.js"
compatibility_date = "2024-01-01"

# Build configuration - uses wasm-pack
[build]
command = "wasm-pack build --target no-modules --out-dir pkg"

# Environment variables
[vars]
ENVIRONMENT = "production"

# Workers.dev subdomain
workers_dev = true

# KV Namespaces
# [[kv_namespaces]]
# binding = "MY_KV"
# id = "xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx"
# preview_id = "yyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyy"

# D1 Databases
# [[d1_databases]]
# binding = "DB"
# database_name = "my-database"
# database_id = "xxxxxxxx-xxxx-xxxx-xxxx-xxxxxxxxxxxx"

# R2 Buckets
# [[r2_buckets]]
# binding = "BUCKET"
# bucket_name = "my-bucket"

# Service Bindings (for multi-worker architectures)
# [[services]]
# binding = "AUTH_SERVICE"
# service = "auth-worker"

# Environment-specific configuration
[env.staging]
name = "{{WORKER_NAME}}-staging"
vars = { ENVIRONMENT = "staging" }

[env.dev]
name = "{{WORKER_NAME}}-dev"
vars = { ENVIRONMENT = "development" }
```

### Main Entry Point (src/lib.rs)

```rust
use wasm_bindgen::prelude::*;
use worker::*;
use foundation_wasm::WasmInit;

// Initialize WASM and panic hook
#[wasm_bindgen(start)]
pub fn main() -> Result<(), JsValue> {
    // Set up panic hook for better error messages in WASM
    console_error_panic_hook::set_once();

    // Initialize foundation_wasm utilities
    foundation_wasm::init();

    // Set up logging
    tracing::info!("WASM module initialized");

    Ok(())
}

// Request handler - entry point for Cloudflare Workers
#[event(fetch)]
async fn main(req: Request, env: Env, ctx: Context) -> Result<Response> {
    tracing::info!("Received request: {} {}", req.method(), req.path());

    let router = Router::new();

    router
        .get("/", handle_index)
        .get("/api/health", handle_health)
        .get("/api/wasm-info", handle_wasm_info)
        .post("/api/compute", handle_compute)
        .run(req, env)
        .await
}

// Handler: Index page
async fn handle_index(req: Request, ctx: RouteContext<()>) -> Result<Response> {
    Response::html(include_str!("../public/index.html"))
}

// Handler: Health check
async fn handle_health(req: Request, ctx: RouteContext<()>) -> Result<Response> {
    let health = HealthResponse {
        status: "healthy".to_string(),
        runtime: "wasm".to_string(),
        timestamp: Date::now().to_string(),
        memory: get_memory_usage(),
    };
    Response::from_json(&health)
}

// Handler: WASM information
async fn handle_wasm_info(req: Request, ctx: RouteContext<()>) -> Result<Response> {
    let info = WasmInfoResponse {
        wasm_supported: true,
        memory_pages: wasm_bindgen::wasm_memory_size(0),
        table_size: wasm_bindgen::wasm_table_size(0),
        features: vec![
            "bulk-memory".to_string(),
            "reference-types".to_string(),
        ],
    };
    Response::from_json(&info)
}

// Handler: Compute-intensive operation (demonstrates WASM performance)
async fn handle_compute(mut req: Request, ctx: RouteContext<()>) -> Result<Response> {
    let body: ComputeRequest = req.json().await?;

    // Example: CPU-intensive computation
    let result = compute_heavy(&body.data, body.iterations);

    let response = ComputeResponse {
        result,
        iterations: body.iterations,
        computation_time_ms: 0,  // Could measure with performance.now()
    };

    Response::from_json(&response)
}

// Helper: Get memory usage
fn get_memory_usage() -> MemoryInfo {
    MemoryInfo {
        memory_pages: wasm_bindgen::wasm_memory_size(0),
        heap_used: 0,  // Could use performance.memory if available
    }
}

// Example compute function (demonstrates WASM computation)
fn compute_heavy(data: &str, iterations: usize) -> String {
    let mut result = data.to_string();
    for _ in 0..iterations {
        result = format!("{:x}", md5::compute(&result));
    }
    result
}

// Response types
#[derive(serde::Serialize)]
struct HealthResponse {
    status: String,
    runtime: String,
    timestamp: String,
    memory: MemoryInfo,
}

#[derive(serde::Serialize)]
struct MemoryInfo {
    memory_pages: usize,
    heap_used: usize,
}

#[derive(serde::Serialize)]
struct WasmInfoResponse {
    wasm_supported: bool,
    memory_pages: usize,
    table_size: usize,
    features: Vec<String>,
}

#[derive(serde::Deserialize)]
struct ComputeRequest {
    data: String,
    iterations: usize,
}

#[derive(serde::Serialize)]
struct ComputeResponse {
    result: String,
    iterations: usize,
    computation_time_ms: u128,
}
```

### mise.toml Template

```toml
[tools]
rust = "1.87"
nodejs = "20"
wrangler = "latest"
wasm-pack = "latest"

[tasks.build]
description = "Build WASM module"
run = """
wasm-pack build --target no-modules --out-dir pkg --release
"""

[tasks.build_debug]
description = "Build WASM module (debug)"
run = """
wasm-pack build --target no-modules --out-dir pkg --dev
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
description = "Manage secrets"
run = "wrangler secret put"

[tasks.check]
description = "Run all checks"
run = """
cargo check --target wasm32-unknown-unknown
cargo fmt --check
cargo clippy --target wasm32-unknown-unknown -- -D warnings
"""

[tasks.test]
description = "Run WASM tests"
run = """
wasm-pack test --headless --firefox
"""

[tasks.size]
description = "Check WASM bundle size"
run = """
ls -lh pkg/*.wasm
"""
```

### README.md Template

```markdown
# {{PROJECT_NAME}}

A Rust + WebAssembly application deployed to Cloudflare Workers.

## Features

- Rust compiled to WebAssembly for maximum performance
- Uses `foundation_wasm` for WASM-specific utilities
- Automatic tooling management via mise.toml
- Multi-environment deployment (dev/staging/production)

## Prerequisites

- Rust 1.87+
- [mise](https://mise.jdx.dev/)
- Cloudflare account

## Quick Start

1. Install dependencies:
   ```bash
   mise install
   ```

2. Authenticate with Cloudflare:
   ```bash
   wrangler login
   ```

3. Build the WASM module:
   ```bash
   mise run build
   ```

4. Run locally:
   ```bash
   mise run dev
   ```

5. Deploy:
   ```bash
   mise run deploy_cf
   ```

## Development

### Building

```bash
# Release build (optimized)
mise run build

# Debug build (faster compilation)
mise run build_debug
```

### Local Development

```bash
mise run dev
```

Worker available at `http://localhost:8787`.

### Testing

```bash
# Run WASM tests in headless browser
mise run test
```

### Size Optimization

Check WASM bundle size:
```bash
mise run size
```

Tips for reducing size:
- Enable `lto = true` and `opt-level = "s"`
- Use `strip = true` to remove debug info
- Minimize dependencies
- Use `wee_alloc` for smaller allocator (optional)

## API Endpoints

| Method | Path | Description |
|--------|------|-------------|
| GET | `/` | Index page |
| GET | `/api/health` | Health check |
| GET | `/api/wasm-info` | WASM runtime info |
| POST | `/api/compute` | Compute-intensive operation |

## Configuration

Edit `wrangler.toml` to configure:
- Worker name and routes
- KV namespaces
- D1 databases
- R2 buckets
- Environment variables

## Resources

- [Cloudflare Workers Docs](https://developers.cloudflare.com/workers/)
- [wasm-pack](https://rustwasm.github.io/wasm-pack/)
- [Rust WASM Book](https://rustwasm.github.io/docs/book/)
```

## Tasks

1. **Create template directory structure**
   - [ ] Create `templates/cf-rust-wasm-app/` directory
   - [ ] Create src/, pkg/, public/ subdirectories
   - [ ] Add .gitignore (exclude pkg/, target/)

2. **Create Cargo.toml template**
   - [ ] Configure cdylib and rlib crate types
   - [ ] Add foundation_wasm dependency
   - [ ] Add wasm-bindgen and related deps
   - [ ] Configure WASM-optimized release profile

3. **Create wrangler.toml template**
   - [ ] Configure for wasm-pack output
   - [ ] Add build command
   - [ ] Configure environment overrides
   - [ ] Add commented examples for bindings

4. **Create lib.rs template**
   - [ ] Write WASM entry point with `#[wasm_bindgen(start)]`
   - [ ] Add `#[event(fetch)]` handler
   - [ ] Implement example handlers
   - [ ] Add foundation_wasm initialization
   - [ ] Include response types

5. **Create mise.toml template**
   - [ ] Define tool versions (rust, wasm-pack, wrangler)
   - [ ] Add build task with wasm-pack
   - [ ] Add dev and deploy tasks
   - [ ] Add size check task

6. **Create documentation**
   - [ ] Write README.md with WASM-specific guidance
   - [ ] Create GitHub Actions workflow
   - [ ] Add size optimization tips

7. **Create public assets**
   - [ ] Create public/index.html template
   - [ ] Add basic styling

## Implementation Notes

- Uses `wasm-pack` for building WASM
- Target: `no-modules` for Cloudflare Workers compatibility
- foundation_wasm provides initialization and utilities
- Release profile optimized for minimal WASM size

## Success Criteria

- [ ] All 7 tasks completed
- [ ] Generated project builds with `wasm-pack build`
- [ ] WASM bundle deploys successfully
- [ ] All handlers respond correctly
- [ ] Bundle size under 1MB (Cloudflare limit)

## Verification

```bash
# Generate project
cargo run --bin ewe_platform generate \
  --template_name="cf-rust-wasm-app" \
  -p "my-wasm-worker" \
  -o /tmp/my-wasm-worker

# Build
cd /tmp/my-wasm-worker
mise install
mise run build

# Check size
mise run size

# Deploy
mise run deploy_cf
```

---

_Created: 2026-03-26_
