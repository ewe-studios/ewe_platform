---
workspace_name: "ewe_platform"
spec_directory: "specifications/11-foundation-deployment"
feature_directory: "specifications/11-foundation-deployment/features/12-mise-integration"
this_file: "specifications/11-foundation-deployment/features/12-mise-integration/feature.md"

status: pending
priority: high
created: 2026-03-26
updated: 2026-04-11

depends_on: ["11-templates", "34-plan-executor"]

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

Define provider-agnostic mise.toml task definitions that work with **deployment plans**. Task names are stable (`deploy`, `logs`, `status`) regardless of provider - the underlying `ewe_platform` CLI reads the deployment plan and executes accordingly.

## Updated for Deployment Plans

**Key change:** mise tasks now support deployment plan execution via `--plan` flag:

```toml
[tasks.deploy]
description = "Deploy to detected provider"
depends = ["build"]
run = "ewe_platform deploy --plan .deployment-plan.yaml"
```

## Dependencies

Depends on:
- `11-templates` - Templates include mise.toml files
- `34-plan-executor` - Plan execution engine

Required by:
- `13-examples-documentation` - Examples use mise tasks

## Requirements

### Provider-Agnostic Task Model

```toml
# mise.toml - Provider-agnostic tasks with deployment plan support

[tasks.build]
description = "Build the project"
run = "ewe_platform build"

[tasks.dev]
description = "Start local development server"
depends = ["build"]
run = "ewe_platform dev"

[tasks.deploy]
description = "Deploy using .deployment-plan.yaml"
depends = ["build"]
run = "ewe_platform deploy --plan .deployment-plan.yaml"

[tasks.deploy_staging]
description = "Deploy to staging environment"
depends = ["build"]
run = "ewe_platform deploy --plan .deployment-plan.yaml --stage staging"

[tasks.deploy_dry]
description = "Dry-run deployment"
run = "ewe_platform deploy --plan .deployment-plan.yaml --dry-run"

[tasks.deploy_force]
description = "Force deploy (skip change detection)"
depends = ["build"]
run = "ewe_platform deploy --plan .deployment-plan.yaml --force"

[tasks.logs]
description = "Tail logs from deployed service"
run = "ewe_platform logs --plan .deployment-plan.yaml"

[tasks.status]
description = "Show deployment status"
run = "ewe_platform status --plan .deployment-plan.yaml"

[tasks.destroy]
description = "Tear down deployed resources"
run = "ewe_platform destroy --plan .deployment-plan.yaml"

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

[tasks.init_plan]
description = "Initialize a new deployment plan"
run = "ewe_platform init-plan --provider {{provider | default('cloudflare')}}"
```

### Infrastructure Plan Tasks

```toml
# Additional tasks for infrastructure management

[tasks.infra_up]
description = "Create/update infrastructure from plan"
run = "ewe_platform infra apply --plan .infrastructure-plan.yaml"

[tasks.infra_destroy]
description = "Destroy infrastructure from plan"
run = "ewe_platform infra destroy --plan .infrastructure-plan.yaml"

[tasks.infra_status]
description = "Show infrastructure status"
run = "ewe_platform infra status --plan .infrastructure-plan.yaml"

[tasks.infra_drift]
description = "Detect infrastructure drift"
run = "ewe_platform infra drift --plan .infrastructure-plan.yaml"
```

### Tool Definitions by Provider

**Cloudflare:**
```toml
[tools]
rust = "1.87"
nodejs = "20"
wrangler = "latest"
```

**GCP Cloud Run:**
```toml
[tools]
rust = "1.87"
# gcloud typically system-wide
```

**AWS Lambda:**
```toml
[tools]
rust = "1.87"
cargo-lambda = "latest"
```

### ewe_platform CLI Commands

```bash
# Deploy using deployment plan
ewe_platform deploy --plan .deployment-plan.yaml
ewe_platform deploy --plan .deployment-plan.yaml --stage staging
ewe_platform deploy --plan .deployment-plan.yaml --dry-run

# Infrastructure management
ewe_platform infra apply --plan .infrastructure-plan.yaml
ewe_platform infra destroy --plan .infrastructure-plan.yaml
ewe_platform infra status --plan .infrastructure-plan.yaml

# Initialize new deployment plan
ewe_platform init-plan --provider gcp --project my-app

# Legacy mode (provider config file detection)
ewe_platform deploy  # Auto-detects from wrangler.toml, service.yaml, etc.
```

## Tasks

1. **Define mise.toml templates**
   - [ ] Create provider-agnostic task definitions
   - [ ] Add infrastructure plan tasks
   - [ ] Include tool definitions per provider
   - [ ] Write tests

2. **Update ewe_platform CLI**
   - [ ] Add `--plan` flag to deploy command
   - [ ] Add `--plan` flag to logs/status/destroy
   - [ ] Add `infra` subcommand
   - [ ] Add `init-plan` command
   - [ ] Write tests

3. **Backward compatibility**
   - [ ] Support legacy config file detection
   - [ ] Deprecation warnings for old format
   - [ ] Migration guide in CLI help
   - [ ] Write tests

4. **Environment variable expansion**
   - [ ] Support `${env:VAR}` in mise.toml
   - [ ] Document variable usage
   - [ ] Write tests

5. **Documentation**
   - [ ] Document all mise tasks
   - [ ] Document CLI commands
   - [ ] Provide migration examples

## Success Criteria

- [ ] All 5 tasks completed
- [ ] `cargo clippy -p foundation_deployment -- -D warnings -W clippy::pedantic` — zero warnings
- [ ] mise tasks work with deployment plans
- [ ] Legacy config file detection still works
- [ ] CLI help documents both modes
- [ ] Examples use new deployment plan format

## Verification

```bash
cd backends/foundation_deployment
cargo check
cargo clippy -- -D warnings
cargo test mise -- --nocapture
```

---

_Created: 2026-03-26_
_Updated: 2026-04-11 - Added deployment plan support_

