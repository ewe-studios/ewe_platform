---
workspace_name: "ewe_platform"
spec_directory: "specifications/11-cloudflare-wrangler-deployment"
feature_directory: "specifications/11-cloudflare-wrangler-deployment/features/07-mise-integration"
this_file: "specifications/11-cloudflare-wrangler-deployment/features/07-mise-integration/feature.md"

status: pending
priority: high
created: 2026-03-26

depends_on: ["04-cf-rust-template", "05-cf-rust-wasm-template", "06-cf-generic-wasm-template"]

tasks:
  completed: 0
  uncompleted: 5
  total: 5
  completion_percentage: 0%
---


# Mise Integration

## Overview

Integrate mise.toml configuration across all Cloudflare templates, providing consistent tooling management, task definitions, and deployment workflows. This feature ensures all templates use mise for reproducible development environments.

## Dependencies

This feature depends on:
- `04-cf-rust-template` - Uses mise in Rust template
- `05-cf-rust-wasm-template` - Uses mise in Rust+WASM template
- `06-cf-generic-wasm-template` - Uses mise in generic WASM template

This feature is required by:
- `08-examples-documentation` - Examples use mise tasks

## Requirements

### Mise.toml Structure

All templates should include a comprehensive mise.toml:

```toml
[tools]
# Required tools
rust = "1.87"
nodejs = "20"
wrangler = "latest"

# Optional tools (template-specific)
# wasm-pack = "latest"
# tinygo = "latest"
# binaryen = "latest"

# Python for some Cloudflare tools
# python = "3.12"

[env]
# Environment variables
CARGO_TERM_COLOR = "always"
RUSTFLAGS = "-C target-feature=+bulk-memory"

# Cloudflare (users should set these in .env.local)
# CLOUDFLARE_API_TOKEN = "{{env.CLOUDFLARE_API_TOKEN}}"
# CLOUDFLARE_ACCOUNT_ID = "{{env.CLOUDFLARE_ACCOUNT_ID}}"

# Task-specific settings
WORKER_NAME = "{{PROJECT_NAME}}"

[tasks]
# ============================================
# Build Tasks
# ============================================

[tasks.build]
description = "Build the project"
run = """
cargo build --release --target wasm32-unknown-unknown
"""

[tasks.build_debug]
description = "Build debug version (faster)"
run = """
cargo build --target wasm32-unknown-unknown
"""

# ============================================
# Development Tasks
# ============================================

[tasks.dev]
description = "Start local development server"
depends = ["build"]
run = "wrangler dev"

[tasks.dev_remote]
description = "Start dev server with remote bindings"
depends = ["build"]
run = "wrangler dev --remote"

[tasks.dev_no_build]
description = "Start dev server without rebuilding"
run = "wrangler dev"

# ============================================
# Deployment Tasks
# ============================================

[tasks.deploy_cf]
description = "Deploy to production Cloudflare Workers"
depends = ["build"]
run = """
#!/bin/bash
set -e

echo "=== Deploying to Cloudflare Workers ==="
echo "Worker: $WORKER_NAME"
echo "Environment: production"
echo ""

wrangler deploy

echo ""
echo "=== Deployment Complete ==="
"""

[tasks.deploy_cf_dry]
description = "Dry-run deployment"
depends = ["build"]
run = "wrangler deploy --dry-run"

[tasks.deploy_cf_staging]
description = "Deploy to staging environment"
depends = ["build"]
run = "wrangler deploy --env staging"

[tasks.deploy_cf_dev]
description = "Deploy to dev environment"
depends = ["build"]
run = "wrangler deploy --env dev"

# ============================================
# Observability Tasks
# ============================================

[tasks.logs]
description = "Tail worker logs in real-time"
run = "wrangler tail"

[tasks.logs_dev]
description = "Tail dev environment logs"
run = "wrangler tail --env dev"

[tasks.logs_staging]
description = "Tail staging environment logs"
run = "wrangler tail --env staging"

[tasks.list_routes]
description = "List configured routes"
run = "wrangler routes list"

# ============================================
# Secret Management Tasks
# ============================================

[tasks.secret_put]
description = "Add or update a secret (usage: mise run secret_put KEY_NAME)"
run = "wrangler secret put"

[tasks.secret_list]
description = "List all secrets (names only, values hidden)"
run = "wrangler secret list"

[tasks.secret_delete]
description = "Delete a secret (usage: mise run secret_delete KEY_NAME)"
run = "wrangler secret delete"

# ============================================
# KV Management Tasks
# ============================================

[tasks.kv_list]
description = "List KV namespaces"
run = "wrangler kv namespace list"

[tasks.kv_key_put]
description = "Put a KV value"
run = "wrangler kv:key put"

[tasks.kv_key_get]
description = "Get a KV value"
run = "wrangler kv:key get"

[tasks.kv_key_delete]
description = "Delete a KV key"
run = "wrangler kv:key delete"

# ============================================
# D1 Database Tasks
# ============================================

[tasks.d1_list]
description = "List D1 databases"
run = "wrangler d1 list"

[tasks.d1_execute]
description = "Execute SQL against D1 database"
run = "wrangler d1 execute"

# ============================================
# Maintenance Tasks
# ============================================

[tasks.check]
description = "Run all code checks"
depends = ["check_fmt", "check_clippy"]
run = """
echo "All checks passed!"
"""

[tasks.check_fmt]
description = "Check code formatting"
run = "cargo fmt --check"

[tasks.check_clippy]
description = "Run clippy lints"
run = "cargo clippy -- -D warnings"

[tasks.fmt]
description = "Format code"
run = "cargo fmt"

[tasks.test]
description = "Run tests"
run = "cargo test"

[tasks.clean]
description = "Clean build artifacts"
run = "cargo clean && rm -rf pkg/"

[tasks.size]
description = "Check WASM bundle size"
run = """
if [ -f pkg/*.wasm ]; then
  echo "=== WASM Bundle Size ==="
  ls -lh pkg/*.wasm
  wc -c pkg/*.wasm
else
  echo "No WASM bundle found. Run 'mise run build' first."
fi
"""

# ============================================
# Setup Tasks
# ============================================

[tasks.setup]
description = "Initial project setup"
run = """
#!/bin/bash
echo "=== Project Setup ==="
echo ""
echo "1. Installing tools..."
mise install

echo ""
echo "2. Authenticating with Cloudflare..."
wrangler login

echo ""
echo "3. Setup complete!"
echo "Run 'mise run dev' to start developing."
"""

[tasks.whoami]
description = "Show Cloudflare account info"
run = "wrangler whoami"
```

### Mise Configuration in CI/CD

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

      - name: Install mise
        uses: jdx/mise-action@v2
        with:
          experimental: true

      - name: Install tools
        run: mise install

      - name: Check code
        run: mise run check

      - name: Build
        run: mise run build

      - name: Deploy to Cloudflare
        uses: cloudflare/wrangler-action@v3
        with:
          apiToken: ${{ secrets.CLOUDFLARE_API_TOKEN }}
          environment: ${{ github.ref == 'refs/heads/main' && 'production' || 'staging' }}
```

### Environment-Specific Configuration

```toml
# .mise.toml (project-local overrides)
[tools]
# Override versions for this project only
# wrangler = "3.25.0"  # Pin specific version

[env]
# Project-specific environment variables
# DEBUG = "true"
# LOG_LEVEL = "debug"
```

### Shared Task Definitions

Create reusable task definitions that can be imported:

```toml
# tasks/@cloudflare/deploy.toml (shared task library)
[tasks._cf_deploy_base]
description = "Base Cloudflare deployment task"
script = """
#!/bin/bash
set -e

ENV=${1:-production}
echo "Deploying to $ENV"

wrangler deploy ${ENV:+--env $ENV}
"""
```

## Tasks

1. **Standardize mise.toml across templates**
   - [ ] Ensure cf-rust-app has complete mise.toml
   - [ ] Ensure cf-rust-wasm-app has complete mise.toml
   - [ ] Ensure cf-wasm-app has complete mise.toml
   - [ ] All templates use consistent task naming

2. **Implement deployment tasks**
   - [ ] `deploy_cf` - Production deployment
   - [ ] `deploy_cf_staging` - Staging deployment
   - [ ] `deploy_cf_dry` - Dry run
   - [ ] Add deployment output parsing

3. **Implement observability tasks**
   - [ ] `logs` - Tail production logs
   - [ ] `logs_staging` - Tail staging logs
   - [ ] `list_routes` - Show configured routes

4. **Implement management tasks**
   - [ ] `secret_put`, `secret_list`, `secret_delete`
   - [ ] `kv_list`, `kv_key_put`, `kv_key_get`, `kv_key_delete`
   - [ ] `d1_list`, `d1_execute`

5. **Create CI/CD integration**
   - [ ] GitHub Actions workflow using mise
   - [ ] Document CI/CD setup in README
   - [ ] Add environment-specific deployment

## Implementation Notes

- mise provides faster, more reliable tooling management vs npm/cargo global installs
- Tasks should be composable (use `depends` for task dependencies)
- Environment variables can be overridden per-environment
- Use `.env.local` for secrets (never commit)

## Success Criteria

- [ ] All 5 tasks completed
- [ ] All templates have consistent mise.toml
- [ ] `mise run deploy_cf` works in all templates
- [ ] CI/CD workflow uses mise tasks
- [ ] Documentation explains all available tasks

## Verification

```bash
# Test mise setup in each template
for template in cf-rust-app cf-rust-wasm-app cf-wasm-app; do
  echo "=== Testing $template ==="

  # Generate project
  cargo run --bin ewe_platform generate \
    --template_name="$template" \
    -p "test-$template" \
    -o /tmp/test-$template

  cd /tmp/test-$template

  # Install tools
  mise install

  # Run check task
  mise run check

  # Run build task
  mise run build

  echo ""
done
```

---

_Created: 2026-03-26_
