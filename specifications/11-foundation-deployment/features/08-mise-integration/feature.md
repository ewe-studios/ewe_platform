---
workspace_name: "ewe_platform"
spec_directory: "specifications/11-foundation-deployment"
feature_directory: "specifications/11-foundation-deployment/features/08-mise-integration"
this_file: "specifications/11-foundation-deployment/features/08-mise-integration/feature.md"

status: pending
priority: high
created: 2026-03-26

depends_on: ["07-templates"]

tasks:
  completed: 0
  uncompleted: 5
  total: 5
  completion_percentage: 0%
---


# Mise Integration

## Iron Law: Zero Warnings

> **All code must compile with zero warnings and pass all lints. No suppression. No exceptions.**
>
> - `cargo clippy -p foundation_deployment -- -D warnings -W clippy::pedantic` — zero warnings
> - `cargo doc -p foundation_deployment --no-deps` — zero rustdoc warnings
> - `cargo test -p foundation_deployment` — zero compilation warnings
> - **No `#[allow(...)]`, `#[expect(...)]`, or `#![allow(...)]` anywhere.** Fix the code, never suppress.

## Overview

Define provider-agnostic mise.toml task definitions that work across all deployment targets. Task names are stable (`deploy`, `logs`, `status`) regardless of provider - the underlying commands detect the provider from the project's config file and dispatch accordingly.

## Dependencies

Depends on:
- `07-templates` - Templates include mise.toml files

Required by:
- `09-examples-documentation` - Examples use mise tasks

## Requirements

### Provider-Agnostic Task Model

The key insight: task names should be **the same** regardless of provider. The `ewe_platform` CLI (or direct provider detection) handles dispatching to the right tool.

```toml
# mise.toml - Provider-agnostic tasks
# Works with wrangler.toml (Cloudflare), service.yaml (GCP), or template.yaml (AWS)

[tasks.build]
description = "Build the project"
run = "ewe_platform build"

[tasks.dev]
description = "Start local development server"
depends = ["build"]
run = "ewe_platform dev"

[tasks.deploy]
description = "Deploy to detected provider"
depends = ["build"]
run = "ewe_platform deploy"

[tasks.deploy_staging]
description = "Deploy to staging environment"
depends = ["build"]
run = "ewe_platform deploy --env staging"

[tasks.deploy_dry]
description = "Dry-run deployment (validate + build only)"
run = "ewe_platform deploy --dry-run"

[tasks.logs]
description = "Tail logs from deployed service"
run = "ewe_platform logs"

[tasks.status]
description = "Show deployment status"
run = "ewe_platform status"

[tasks.destroy]
description = "Tear down deployed resources"
run = "ewe_platform destroy"

[tasks.check]
description = "Run all code checks"
depends = ["check_fmt", "check_clippy"]

[tasks.check_fmt]
description = "Check formatting"
run = "cargo fmt --check"

[tasks.check_clippy]
description = "Run clippy"
run = "cargo clippy -- -D warnings"

[tasks.fmt]
description = "Format code"
run = "cargo fmt"

[tasks.test]
description = "Run tests"
run = "cargo test"

[tasks.clean]
description = "Clean build artifacts"
run = "cargo clean"
```

### Provider-Specific Tool Sections

Each provider's mise.toml includes only the tools it needs:

**Cloudflare:**
```toml
[tools]
rust = "1.87"
nodejs = "20"
wrangler = "latest"
# wasm-pack = "latest"  # Uncomment for WASM templates
```

**GCP Cloud Run:**
```toml
[tools]
rust = "1.87"
# gcloud is typically installed system-wide or via mise plugin
# docker is system-wide
```

**AWS Lambda:**
```toml
[tools]
rust = "1.87"
cargo-lambda = "latest"
# aws-sam-cli installed via pip or brew
```

### ewe_platform CLI Commands

The `ewe_platform` binary dispatches to the correct provider based on config file detection:

```
ewe_platform deploy             # Auto-detect provider, deploy to default env
ewe_platform deploy --env staging   # Deploy to staging
ewe_platform deploy --target cf     # Force Cloudflare even if multiple configs exist
ewe_platform deploy --dry-run       # Validate + build without deploying
ewe_platform build                  # Build only
ewe_platform dev                    # Start local dev server
ewe_platform logs                   # Tail logs
ewe_platform status                 # Show deployment state
ewe_platform destroy                # Tear down resources
```

### Provider-Specific Overrides

Templates can include provider-specific tasks alongside the generic ones:

```toml
# Cloudflare-specific extras
[tasks.secret_put]
description = "Add a secret (usage: mise run secret_put KEY)"
run = "wrangler secret put"

[tasks.secret_list]
description = "List secrets"
run = "wrangler secret list"

# GCP-specific extras
[tasks.job_execute]
description = "Execute a Cloud Run Job"
run = "gcloud run jobs execute $(yq .metadata.name service.yaml)"

# AWS-specific extras
[tasks.invoke]
description = "Invoke Lambda function locally"
run = "sam local invoke"

[tasks.api_local]
description = "Start local API Gateway"
run = "sam local start-api"
```

### CI/CD Workflow Template

Provider-agnostic GitHub Actions workflow:

```yaml
# .github/workflows/deploy.yml
name: Deploy

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

      - name: Install tools
        run: mise install

      - name: Check
        run: mise run check

      - name: Build
        run: mise run build

      - name: Deploy
        if: github.ref == 'refs/heads/main'
        run: mise run deploy
        env:
          # Cloudflare
          CLOUDFLARE_API_TOKEN: ${{ secrets.CLOUDFLARE_API_TOKEN }}
          CLOUDFLARE_ACCOUNT_ID: ${{ secrets.CLOUDFLARE_ACCOUNT_ID }}
          # GCP
          GOOGLE_APPLICATION_CREDENTIALS: ${{ secrets.GCP_SA_KEY }}
          # AWS
          AWS_ACCESS_KEY_ID: ${{ secrets.AWS_ACCESS_KEY_ID }}
          AWS_SECRET_ACCESS_KEY: ${{ secrets.AWS_SECRET_ACCESS_KEY }}
          AWS_REGION: ${{ secrets.AWS_REGION }}
```

## Tasks

1. **Define standard task names**
   - [ ] Document the canonical task names (build, dev, deploy, logs, status, destroy)
   - [ ] Document environment/flag conventions (--env, --target, --dry-run)
   - [ ] Ensure all templates use consistent naming

2. **Implement ewe_platform CLI commands**
   - [ ] Add `deploy` subcommand to ewe_platform binary
   - [ ] Add `build`, `dev`, `logs`, `status`, `destroy` subcommands
   - [ ] Implement `--target` flag for explicit provider selection
   - [ ] Implement `--env` flag for environment selection
   - [ ] Implement `--dry-run` flag

3. **Create mise.toml for each template**
   - [ ] Cloudflare Rust worker mise.toml
   - [ ] Cloudflare Rust WASM mise.toml
   - [ ] GCP Cloud Run Rust mise.toml
   - [ ] AWS Lambda Rust mise.toml
   - [ ] All share the same task names, differ only in [tools]

4. **Create CI/CD workflow templates**
   - [ ] Provider-agnostic deploy.yml
   - [ ] Document which secrets to configure per provider

5. **Write tests**
   - [ ] Test task execution for each template
   - [ ] Test `ewe_platform deploy` auto-detection
   - [ ] Test `--target` override
   - [ ] Test `--dry-run` mode

## Success Criteria

- [ ] All 5 tasks completed
- [ ] `cargo clippy -p foundation_deployment -- -D warnings -W clippy::pedantic` — zero warnings, zero suppression
- [ ] `cargo doc -p foundation_deployment --no-deps` — zero rustdoc warnings
- [ ] No `#[allow(...)]` or `#[expect(...)]` anywhere in the code
- [ ] `mise run deploy` works identically across all templates
- [ ] `ewe_platform deploy` auto-detects provider correctly
- [ ] CI/CD workflow deploys to the correct provider
- [ ] Provider-specific tasks are available alongside generic ones

## Verification

```bash
# Test each template's mise tasks
for dir in /tmp/test-cloudflare /tmp/test-gcp /tmp/test-aws; do
  cd $dir
  mise run check
  mise run build
  mise run deploy --dry-run
done
```

---

_Created: 2026-03-26_
