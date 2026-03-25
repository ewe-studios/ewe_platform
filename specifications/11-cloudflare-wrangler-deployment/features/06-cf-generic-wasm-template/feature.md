---
workspace_name: "ewe_platform"
spec_directory: "specifications/11-cloudflare-wrangler-deployment"
feature_directory: "specifications/11-cloudflare-wrangler-deployment/features/06-cf-generic-wasm-template"
this_file: "specifications/11-cloudflare-wrangler-deployment/features/06-cf-generic-wasm-template/feature.md"

status: pending
priority: high
created: 2026-03-26

depends_on: ["01-foundation-deployment-crate", "04-cf-rust-template"]

tasks:
  completed: 0
  uncompleted: 6
  total: 6
  completion_percentage: 0%
---


# Cloudflare Generic WASM App Template

## Overview

Create a template for deploying generic WebAssembly modules to Cloudflare Workers, supporting any language that compiles to WASM (Rust, AssemblyScript, TinyGo, etc.) with automatic tooling setup via mise.toml.

## Dependencies

This feature depends on:
- `01-foundation-deployment-crate` - For deployment tooling
- `04-cf-rust-template` - Shares template structure patterns

This feature is required by:
- `07-mise-integration` - Uses template structure
- `08-examples-documentation` - Example projects based on template

## Requirements

### Template Structure

```
templates/cf-wasm-app/
├── wrangler.toml
├── mise.toml
├── README.md
├── .gitignore
├── src/
│   └── index.js          # JS shim for WASM loading
├── wasm/                 # WASM source (language-agnostic)
│   └── .gitkeep
├── pkg/                  # Built WASM output (gitignored)
├── public/
│   └── index.html
└── .github/
    └── workflows/
        └── deploy.yml
```

### wrangler.toml Template

```toml
name = "{{WORKER_NAME}}"
main = "src/index.js"
compatibility_date = "2024-01-01"

# Build configuration - generic, customize for your WASM toolchain
[build]
command = "mise run build:wasm"

# Environment variables
[vars]
ENVIRONMENT = "production"

# Workers.dev subdomain
workers_dev = true

# KV Namespaces
# [[kv_namespaces]]
# binding = "MY_KV"
# id = "xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx"

# D1 Databases
# [[d1_databases]]
# binding = "DB"
# database_name = "my-database"
# database_id = "xxxxxxxx-xxxx-xxxx-xxxx-xxxxxxxxxxxx"

# Environment-specific configuration
[env.staging]
name = "{{WORKER_NAME}}-staging"

[env.dev]
name = "{{WORKER_NAME}}-dev"
```

### JS Shim (src/index.js)

```javascript
// Cloudflare Worker entry point that loads WASM
import wasmModule from '../pkg/bundle.wasm';

export default {
  async fetch(request, env, ctx) {
    // Instantiate WASM module
    const wasm = await WebAssembly.instantiate(wasmModule, {
      env: {
        // Import functions available to WASM
        log: (ptr, len) => {
          const bytes = new Uint8Array(wasm.memory.buffer, ptr, len);
          const message = new TextDecoder().decode(bytes);
          console.log('WASM:', message);
        },
      },
    });

    const exports = wasm.instance.exports;

    // Route request through WASM
    const url = new URL(request.url);
    const path = url.pathname;

    // Call WASM handler
    if (exports.handle_request) {
      const response = exports.handle_request(path);
      return new Response(response, {
        headers: { 'content-type': 'application/json' },
      });
    }

    return new Response('WASM module loaded', {
      headers: { 'content-type': 'text/plain' },
    });
  },
};
```

### mise.toml Template

```toml
[tools]
# Core tools
nodejs = "20"
wrangler = "latest"

# WASM tooling (examples - customize for your language)
# Rust/WASM
# wasm-pack = "latest"

# TinyGo
# tinygo = "latest"

# AssemblyScript
# assemblyscript = "latest"

# Binaryen (WASM tools)
# binaryen = "latest"

[tasks.build:wasm]
description = "Build WASM module (customize for your toolchain)"
# Example for Rust:
# run = "wasm-pack build --target no-modules --out-dir pkg"
# Example for TinyGo:
# run = "tinygo build -o pkg/bundle.wasm -target=wasi wasm/"
# Example for AssemblyScript:
# run = "asc wasm/index.ts --target wasm"
run = """
echo "Configure your WASM build command in mise.toml"
echo "Examples provided in comments for Rust, TinyGo, AssemblyScript"
exit 1
"""

[tasks.build]
description = "Build the project"
depends = ["build:wasm"]
run = """
echo "WASM built successfully"
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
description = "Deploy to staging"
depends = ["build"]
run = "wrangler deploy --env staging"

[tasks.logs]
description = "Tail worker logs"
run = "wrangler tail"

[tasks.check]
description = "Check WASM file"
run = """
if [ -f pkg/bundle.wasm ]; then
  echo "WASM bundle found"
  ls -lh pkg/bundle.wasm
else
  echo "Warning: pkg/bundle.wasm not found"
fi
"""
```

### README.md Template

```markdown
# {{PROJECT_NAME}}

A generic WebAssembly application for Cloudflare Workers.

## Language Support

This template is language-agnostic. Configure your WASM toolchain in `mise.toml`:

### Rust
```toml
[tools]
rust = "1.87"
wasm-pack = "latest"

[tasks.build:wasm]
run = "wasm-pack build --target no-modules --out-dir pkg"
```

### TinyGo
```toml
[tools]
tinygo = "latest"

[tasks.build:wasm]
run = "tinygo build -o pkg/bundle.wasm -target=wasi wasm/"
```

### AssemblyScript
```toml
[tools]
nodejs = "20"

[dependencies]
assemblyscript = "latest"

[tasks.build:wasm]
run = "asc wasm/index.ts --target wasm -o pkg/bundle.wasm"
```

## Quick Start

1. Install dependencies:
   ```bash
   mise install
   ```

2. Configure your WASM toolchain in `mise.toml`

3. Add your WASM source to `wasm/` directory

4. Build:
   ```bash
   mise run build
   ```

5. Deploy:
   ```bash
   mise run deploy_cf
   ```

## Project Structure

```
wasm/           # Your WASM source code
pkg/            # Built WASM output (auto-generated)
src/            # JS shim for loading WASM
public/         # Static assets
```

## Resources

- [Cloudflare Workers + WASM](https://developers.cloudflare.com/workers/examples/using-webassembly/)
- [WebAssembly.org](https://webassembly.org/)
```

## Tasks

1. **Create template directory structure**
   - [ ] Create `templates/cf-wasm-app/` directory
   - [ ] Create wasm/, pkg/, src/, public/ subdirectories
   - [ ] Add comprehensive .gitignore

2. **Create wrangler.toml template**
   - [ ] Configure for generic WASM loading
   - [ ] Add build command placeholder
   - [ ] Add environment configurations

3. **Create JS shim**
   - [ ] Write src/index.js with WASM loading
   - [ ] Add import function examples
   - [ ] Handle request routing

4. **Create mise.toml template**
   - [ ] Add commented tool configurations for multiple languages
   - [ ] Create build:wasm task with examples
   - [ ] Add dev and deploy tasks
   - [ ] Add check task for WASM verification

5. **Create documentation**
   - [ ] Write README.md with multi-language examples
   - [ ] Include Rust, TinyGo, AssemblyScript examples
   - [ ] Add WASM best practices

6. **Create example WASM modules**
   - [ ] Add Rust example in wasm/rust-example/
   - [ ] Add TinyGo example in wasm/tinygo-example/
   - [ ] Document example usage

## Implementation Notes

- Template is intentionally language-agnostic
- Provides examples for popular WASM languages
- JS shim handles WASM instantiation
- mise.toml makes toolchain switching easy

## Success Criteria

- [ ] All 6 tasks completed
- [ ] Template works with Rust WASM
- [ ] Template works with TinyGo WASM
- [ ] Template works with AssemblyScript
- [ ] Documentation clearly explains setup

## Verification

```bash
# Generate project
cargo run --bin ewe_platform generate \
  --template_name="cf-wasm-app" \
  -p "my-generic-wasm" \
  -o /tmp/my-generic-wasm

# Configure for Rust and build
cd /tmp/my-generic-wasm
# Edit mise.toml to enable Rust
mise install
mise run build

# Deploy
mise run deploy_cf
```

---

_Created: 2026-03-26_
