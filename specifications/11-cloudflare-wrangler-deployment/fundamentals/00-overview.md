# Cloudflare Wrangler Deployment - Overview

**Created:** 2026-03-26
**Specification:** `specifications/11-cloudflare-wrangler-deployment`

---

## What This Specification Covers

This specification defines a complete deployment system for Cloudflare Workers, including:

1. **foundation_deployment crate** - Core deployment utilities
2. **Project templates** - Ready-to-use scaffolds for different project types
3. **mise.toml integration** - Consistent tooling and task management
4. **Examples and documentation** - Working examples and comprehensive guides

---

## Architecture Overview

```
┌─────────────────────────────────────────────────────────────────┐
│                     Developer Experience                        │
│                                                                 │
│   ewe_platform generate --template_name="cf-rust-app"          │
│   mise run deploy_cf                                           │
│                                                                 │
└─────────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────────┐
│                  Templates (ewe_temple)                         │
│                                                                 │
│   cf-rust-app        → Pure Rust worker (worker crate)         │
│   cf-rust-wasm-app   → Rust+WASM worker (foundation_wasm)       │
│   cf-wasm-app        → Generic WASM (any language)              │
│                                                                 │
└─────────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────────┐
│              foundation_deployment Crate                        │
│                                                                 │
│   ┌─────────────────┐  ┌─────────────────┐  ┌────────────────┐ │
│   │ WranglerRunner  │  │ CloudflareAPI   │  │ DeployPlanner  │ │
│   │                 │  │                 │  │                │ │
│   │ • deploy        │  │ • workers       │  │ • state machine│ │
│   │ • dev           │  │ • kv            │  │ • validation   │ │
│   │ • tail          │  │ • secrets       │  │ • orchestration│ │
│   │ • secret *      │  │ • d1            │  │                │ │
│   └─────────────────┘  └─────────────────┘  └────────────────┘ │
│                                                                 │
│   Uses: foundation_core::simple_http, valtron async patterns   │
└─────────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────────┐
│                   Cloudflare Workers                            │
│                                                                 │
│   • JavaScript/TypeScript workers                               │
│   • Rust workers (via worker crate)                             │
│   • WASM workers (any language → WASM)                          │
│                                                                 │
│   Resources: KV, D1, R2, Secrets, Bindings                      │
└─────────────────────────────────────────────────────────────────┘
```

---

## Feature Summary

| Feature | Description | Status |
|---------|-------------|--------|
| foundation-deployment-crate | Core deployment utilities | Pending |
| wrangler-process-wrapper | wrangler CLI wrapper | Pending |
| cloudflare-api-client | REST API client | Pending |
| cf-rust-template | Pure Rust worker template | Pending |
| cf-rust-wasm-template | Rust+WASM worker template | Pending |
| cf-generic-wasm-template | Language-agnostic WASM template | Pending |
| mise-integration | Tooling and task management | Pending |
| examples-documentation | Working examples and guides | Pending |

---

## Template Comparison

| Template | Use Case | Build Tool | Entry Point |
|----------|----------|------------|-------------|
| `cf-rust-app` | REST APIs, serverless functions | worker-build | `src/main.rs` |
| `cf-rust-wasm-app` | Compute-intensive, type-safe | wasm-pack | `src/lib.rs` |
| `cf-wasm-app` | Multi-language, custom toolchains | Language-specific | `wasm/*` |

---

## Quick Start

### Generate a Project

```bash
# Pure Rust worker
cargo run --bin ewe_platform generate \
  --template_name="cf-rust-app" \
  -p "my-worker" \
  -o .

# Rust+WASM worker
cargo run --bin ewe_platform generate \
  --template_name="cf-rust-wasm-app" \
  -p "my-wasm-worker" \
  -o .

# Generic WASM worker
cargo run --bin ewe_platform generate \
  --template_name="cf-wasm-app" \
  -p "my-generic-wasm" \
  -o .
```

### Deploy

```bash
cd my-worker
mise install          # Install rust, wrangler, nodejs
wrangler login        # Authenticate with Cloudflare
mise run deploy_cf    # Build and deploy
```

---

## Key Concepts

### Workers Runtime

Cloudflare Workers runs on Isolates (V8-based), not traditional containers:

- **Cold start:** ~50ms (near-instant)
- **CPU time:** 10ms per request (free tier)
- **Memory:** 128MB per request
- **Locations:** 300+ edge locations worldwide

### Deployment Methods

| Method | Description | When to Use |
|--------|-------------|-------------|
| `wrangler deploy` | Direct CLI deployment | Development, manual deploys |
| `CloudflareAPI::upload_worker()` | Programmatic deployment | CI/CD, automation |
| GitHub Actions | Automated deployment | Production workflows |

### Environments

Workers supports multiple environments per worker:

```toml
# wrangler.toml
[env.staging]
name = "my-worker-staging"
vars = { ENV = "staging" }

[env.production]
name = "my-worker"
vars = { ENV = "production" }
```

Deploy to specific environment:
```bash
mise run deploy_cf_staging    # deploys to staging
wrangler deploy --env production  # deploys to production
```

---

## Project Structure

### Pure Rust Worker

```
my-worker/
├── Cargo.toml          # cdylib crate type, worker deps
├── wrangler.toml       # Cloudflare configuration
├── mise.toml           # Tooling and tasks
├── src/main.rs         # Worker entry point
└── public/             # Static assets (optional)
```

### Rust+WASM Worker

```
my-wasm-worker/
├── Cargo.toml          # cdylib + rlib, wasm-bindgen
├── wrangler.toml       # Points to pkg/*.js
├── mise.toml           # Includes wasm-pack
├── src/lib.rs          # WASM entry point
└── pkg/                # wasm-pack output
```

---

## Common Patterns

### Request Handling

```rust
use worker::*;

#[event(fetch)]
async fn main(req: Request, env: Env, ctx: Context) -> Result<Response> {
    Router::new()
        .get("/", |_, _| async { Response::ok("Hello!") })
        .post("/api/data", |mut req, _| async {
            let json: MyData = req.json().await?;
            Response::from_json(&process(json))
        })
        .run(req, env)
        .await
}
```

### KV Storage

```rust
// In worker
let kv = env.kv("MY_KV")?;
kv.put("key", "value")?.execute().await?;
let value: String = kv.get("key").json().await?;
```

### Secret Management

```bash
# Interactively add secret
mise run secret_put API_KEY

# Access in worker
let api_key = env.secret("API_KEY")?;
```

### D1 Database

```rust
let db = env.d1("MY_DB")?;
db.prepare("SELECT * FROM users WHERE id = ?")
    .bind(&[user_id])?
    .all()
    .await?;
```

---

## Troubleshooting

### Build Fails

```bash
# Clean and rebuild
mise run clean
mise run build

# Check Rust target
rustup target add wasm32-unknown-unknown
```

### Deployment Fails

```bash
# Verify authentication
wrangler whoami

# Dry-run deployment
mise run deploy_cf_dry

# Check wrangler.toml
cat wrangler.toml
```

### Worker Returns 500

```bash
# Tail logs
mise run logs

# Check error in Cloudflare dashboard
# https://dash.cloudflare.com/ > Workers > your-worker > Logs
```

---

## Resources

### Official Documentation

- [Cloudflare Workers Docs](https://developers.cloudflare.com/workers/)
- [wrangler CLI](https://developers.cloudflare.com/workers/wrangler/)
- [Workers Runtime API](https://developers.cloudflare.com/workers/runtime-apis/)

### Rust-Specific

- [worker-rust (cloudflare/workers-rs)](https://github.com/cloudflare/workers-rs)
- [wasm-pack](https://rustwasm.github.io/wasm-pack/)
- [Rust WASM Book](https://rustwasm.github.io/docs/book/)

### Examples in This Repo

- `examples/cloudflare/rust-worker/` - Pure Rust REST API
- `examples/cloudflare/rust-wasm-worker/` - Compute-intensive WASM
- `examples/cloudflare/wasm-worker/` - Generic WASM with JS shim

---

_Related Specifications:_

- `specifications/02-build-http-client` - simple_http client used by CloudflareAPI
- `specifications/07-foundation-ai` - foundation_ai patterns used in deployment
- `specifications/08-valtron-async-iterators` - valtron async patterns

---

_Created: 2026-03-26_
