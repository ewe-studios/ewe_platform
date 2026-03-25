---
workspace_name: "ewe_platform"
spec_directory: "specifications/11-cloudflare-wrangler-deployment"
feature_directory: "specifications/11-cloudflare-wrangler-deployment/features/08-examples-documentation"
this_file: "specifications/11-cloudflare-wrangler-deployment/features/08-examples-documentation/feature.md"

status: pending
priority: high
created: 2026-03-26

depends_on: ["01-foundation-deployment-crate", "04-cf-rust-template", "05-cf-rust-wasm-template", "06-cf-generic-wasm-template", "07-mise-integration"]

tasks:
  completed: 0
  uncompleted: 6
  total: 6
  completion_percentage: 0%
---


# Examples and Documentation

## Overview

Create self-contained example projects in `examples/cloudflare/` that demonstrate each template type, along with comprehensive documentation for deployment workflows, troubleshooting, and best practices.

## Dependencies

This feature depends on:
- `01-foundation-deployment-crate` - Uses deployment tooling
- `04-cf-rust-template` - Examples based on Rust template
- `05-cf-rust-wasm-template` - Examples based on Rust+WASM template
- `06-cf-generic-wasm-template` - Examples based on generic WASM template
- `07-mise-integration` - Examples use mise tasks

## Requirements

### Example Projects Structure

```
examples/cloudflare/
├── README.md                     # Overview of all examples
├── rust-worker/                  # Pure Rust worker example
│   ├── Cargo.toml               # References foundation_deployment via path
│   ├── wrangler.toml
│   ├── mise.toml
│   ├── README.md
│   ├── src/
│   │   └── main.rs
│   └── public/
│       └── index.html
│
├── rust-wasm-worker/            # Rust+WASM worker example
│   ├── Cargo.toml
│   ├── wrangler.toml
│   ├── mise.toml
│   ├── README.md
│   ├── src/
│   │   └── lib.rs
│   └── pkg/
│
├── wasm-worker/                 # Generic WASM example (Rust-based)
│   ├── wrangler.toml
│   ├── mise.toml
│   ├── README.md
│   ├── src/
│   │   └── index.js
│   └── wasm/
│       └── main.rs
│
└── integration-tests/           # End-to-end deployment tests
    ├── Cargo.toml
    └── src/
        └── main.rs
```

### Example 1: Rust Worker (REST API)

A complete REST API worker demonstrating common patterns:

```rust
// examples/cloudflare/rust-worker/src/main.rs

use worker::*;
use serde::{Deserialize, Serialize};

#[event(fetch)]
async fn main(req: Request, env: Env, ctx: Context) -> Result<Response> {
    let router = Router::new();

    router
        .get("/", handle_index)
        .get("/api/todos", handle_list_todos)
        .post("/api/todos", handle_create_todo)
        .get("/api/todos/:id", handle_get_todo)
        .put("/api/todos/:id", handle_update_todo)
        .delete("/api/todos/:id", handle_delete_todo)
        .run(req, env)
        .await
}

// In-memory store (in production, use KV or D1)
static TODOS: std::sync::OnceLock<parking_lot::Mutex<Vec<Todo>>> = std::sync::OnceLock::new();

fn get_store() -> &'static parking_lot::Mutex<Vec<Todo>> {
    TODOS.get_or_init(|| parking_lot::Mutex::new(Vec::new()))
}

#[derive(Serialize, Deserialize, Clone)]
struct Todo {
    id: u64,
    title: String,
    completed: bool,
}

// Implement handlers...
```

### Example 2: Rust+WASM Worker (Compute-Intensive)

Demonstrates WASM-specific use cases:

```rust
// examples/cloudflare/rust-wasm-worker/src/lib.rs

use wasm_bindgen::prelude::*;
use worker::*;

#[wasm_bindgen(start)]
pub fn main() -> Result<(), JsValue> {
    console_error_panic_hook::set_once();
    Ok(())
}

#[event(fetch)]
async fn main(req: Request, env: Env, ctx: Context) -> Result<Response> {
    let router = Router::new();

    router
        .get("/api/hash", handle_hash)
        .get("/api/compress", handle_compress)
        .get("/api/image-process", handle_image_process)
        .run(req, env)
        .await
}

// Hash computation (demonstrates WASM performance)
async fn handle_hash(mut req: Request, _ctx: RouteContext<()>) -> Result<Response> {
    let body: HashRequest = req.json().await?;

    let result = match body.algorithm.as_str() {
        "md5" => format!("{:x}", md5::compute(&body.data)),
        "sha256" => format!("{:x}", sha2::Sha256::digest(&body.data)),
        _ => return Response::error("Unknown algorithm", 400),
    };

    Response::from_json(&HashResponse {
        algorithm: body.algorithm,
        hash: result,
        input_size: body.data.len(),
    })
}
```

### Example 3: Generic WASM Worker

Demonstrates the JS shim approach:

```javascript
// examples/cloudflare/wasm-worker/src/index.js

import wasmModule from '../wasm/bundle.wasm';

export default {
  async fetch(request, env, ctx) {
    const wasm = await WebAssembly.instantiate(wasmModule);

    const url = new URL(request.url);

    // Route handling in JS, computation in WASM
    if (url.pathname === '/api/fibonacci') {
      const n = parseInt(url.searchParams.get('n') || '10');
      const result = wasm.instance.exports.fibonacci(n);
      return Response.json({ n, result });
    }

    return Response.json({ message: 'WASM worker' });
  },
};
```

```rust
// examples/cloudflare/wasm-worker/wasm/main.rs

#[no_mangle]
pub extern "C" fn fibonacci(n: u32) -> u32 {
    match n {
        0 => 0,
        1 => 1,
        _ => {
            let mut a = 0;
            let mut b = 1;
            for _ in 2..=n {
                let temp = a + b;
                a = b;
                b = temp;
            }
            b
        }
    }
}
```

### Integration Tests

```rust
// examples/cloudflare/integration-tests/src/main.rs

use foundation_deployment::{WranglerRunner, DeployConfig};
use std::path::PathBuf;

#[test]
fn test_rust_worker_deploy() {
    let _guard = valtron::initialize_pool(4, None);

    let project_dir = PathBuf::from("../rust-worker");

    // Build
    let mut runner = WranglerRunner::new(&project_dir);
    let result = runner.build();
    assert!(result.is_ok(), "Build failed: {:?}", result);

    // Deploy (dry-run)
    let deploy_result = runner.deploy(None, true);  // dry_run = true
    assert!(deploy_result.is_ok(), "Deploy failed: {:?}", deploy_result);
}

#[test]
#[ignore]  // Requires Cloudflare account
fn test_rust_worker_live_deploy() {
    let _guard = valtron::initialize_pool(4, None);

    let project_dir = PathBuf::from("../rust-worker");
    let runner = WranglerRunner::new(&project_dir);

    let result = runner.deploy(None, false);
    assert!(result.is_ok());
}
```

### Documentation Structure

```
documentation/cloudflare/
├── README.md                    # Documentation index
├── getting-started.md           # Quick start guide
├── templates/
│   ├── rust-app.md             # Rust template guide
│   ├── rust-wasm-app.md        # Rust+WASM template guide
│   └── wasm-app.md             # Generic WASM template guide
├── deployment/
│   ├── workflows.md            # Deployment workflows
│   ├── environments.md         # Multi-environment setup
│   ├── ci-cd.md               # CI/CD integration
│   └── rollback.md            # Rollback procedures
├── management/
│   ├── secrets.md             # Secret management
│   ├── kv-storage.md          # KV namespace usage
│   ├── d1-databases.md        # D1 database usage
│   └── r2-storage.md          # R2 bucket usage
├── troubleshooting/
│   ├── common-issues.md       # Common problems and solutions
│   ├── debugging.md           # Debugging techniques
│   └── performance.md         # Performance optimization
└── best-practices/
    ├── security.md            # Security best practices
    ├── monitoring.md          # Monitoring and alerting
    └── cost-optimization.md   # Cost management
```

### Getting Started Guide

```markdown
# Getting Started with Cloudflare Workers

## Prerequisites

- Rust 1.87+
- [mise](https://mise.jdx.dev/)
- Cloudflare account (free tier works)

## Quick Start (5 minutes)

### 1. Generate a project

```bash
cargo run --bin ewe_platform generate \
  --template_name="cf-rust-app" \
  -p "my-first-worker" \
  -o .
```

### 2. Install dependencies

```bash
cd my-first-worker
mise install
```

### 3. Authenticate with Cloudflare

```bash
wrangler login
```

### 4. Run locally

```bash
mise run dev
```

Visit `http://localhost:8787` to see your worker.

### 5. Deploy

```bash
mise run deploy_cf
```

Your worker is now live at `https://my-first-worker.<your-subdomain>.workers.dev`.

## Next Steps

- [Templates overview](./templates/) - Choose the right template
- [Deployment workflows](./deployment/workflows.md) - Learn deployment options
- [Secret management](./management/secrets.md) - Secure your credentials
- [Examples](../../examples/cloudflare/) - See working examples
```

## Tasks

1. **Create example projects**
   - [ ] Create `examples/cloudflare/rust-worker/` with full REST API
   - [ ] Create `examples/cloudflare/rust-wasm-worker/` with compute examples
   - [ ] Create `examples/cloudflare/wasm-worker/` with JS shim
   - [ ] Ensure all examples reference ewe_platform via path dependencies

2. **Create integration tests**
   - [ ] Create `examples/cloudflare/integration-tests/`
   - [ ] Add dry-run deployment tests
   - [ ] Add live deployment tests (marked `#[ignore]`)
   - [ ] Test all three template types

3. **Create documentation index**
   - [ ] Create `documentation/cloudflare/README.md`
   - [ ] Link all documentation sections
   - [ ] Add quick reference

4. **Create getting started guide**
   - [ ] Write `documentation/cloudflare/getting-started.md`
   - [ ] Include 5-minute quick start
   - [ ] Add troubleshooting tips

5. **Create template documentation**
   - [ ] Write `documentation/cloudflare/templates/rust-app.md`
   - [ ] Write `documentation/cloudflare/templates/rust-wasm-app.md`
   - [ ] Write `documentation/cloudflare/templates/wasm-app.md`

6. **Create deployment documentation**
   - [ ] Write `documentation/cloudflare/deployment/workflows.md`
   - [ ] Write `documentation/cloudflare/deployment/environments.md`
   - [ ] Write `documentation/cloudflare/deployment/ci-cd.md`
   - [ ] Write `documentation/cloudflare/troubleshooting/common-issues.md`

## Implementation Notes

- Examples are NOT part of workspace (avoid crates.io publishing)
- Examples use path dependencies to reference ewe_platform crates
- Each example should be a complete, working project
- Documentation should be copy-paste ready where possible

## Success Criteria

- [ ] All 6 tasks completed
- [ ] All example projects build successfully
- [ ] Integration tests pass (dry-run)
- [ ] Documentation renders correctly
- [ ] Getting started guide works end-to-end

## Verification

```bash
# Build all examples
for example in rust-worker rust-wasm-worker wasm-worker; do
  echo "=== Building $example ==="
  cd examples/cloudflare/$example
  mise install
  mise run build
  cd ../../..
done

# Run integration tests (dry-run)
cd examples/cloudflare/integration-tests
cargo test -- --nocapture

# Check documentation builds (if using mdBook or similar)
# mdbook build documentation/cloudflare
```

---

_Created: 2026-03-26_
