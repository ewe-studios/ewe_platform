---
spec_id: "11-cloudflare-wrangler-deployment"
title: "Cloudflare Wrangler Deployment"
status: "pending"
priority: "high"
created: "2026-03-26"
---

# Cloudflare Wrangler Deployment - Start Here

## Specification Summary

This specification creates a complete deployment system for Cloudflare Workers with:

1. **foundation_deployment** crate (`backends/foundation_deployment/`)
2. **Three project templates** (Rust, Rust+WASM, Generic WASM)
3. **mise.toml integration** for consistent tooling
4. **Examples and documentation** for each template type

## Implementation Order

Implement features in this order (dependencies first):

```
1. foundation-deployment-crate        (base - no dependencies)
2. wrangler-process-wrapper           (depends on #1)
3. cloudflare-api-client              (depends on #1)
4. cf-rust-template                   (depends on #1)
5. cf-rust-wasm-template              (depends on #1, #4)
6. cf-generic-wasm-template           (depends on #1, #4)
7. mise-integration                   (depends on #4, #5, #6)
8. examples-documentation             (depends on all above)
```

## Where to Start

### Option 1: Start with foundation_deployment crate

Begin with the core crate that all other features depend on:

```bash
# Create crate directory
mkdir -p backends/foundation_deployment/src/{wrangler,cloudflare,process,project,deploy}
touch backends/foundation_deployment/src/{lib.rs,error.rs}
```

Then implement:
1. Error types (`src/error.rs`)
2. Process executor (`src/process/mod.rs`)
3. Project scanner (`src/project/mod.rs`)

See: `features/01-foundation-deployment-crate/feature.md`

### Option 2: Start with a template

If you want quick wins, start with the Rust template:

```bash
mkdir -p templates/cf-rust-app/{src,public,.github/workflows}
```

Create:
1. `Cargo.toml` with template variables
2. `wrangler.toml` with build configuration
3. `mise.toml` with tasks
4. `src/main.rs` with example handlers

See: `features/04-cf-rust-template/feature.md`

## Feature Files

| Feature | File |
|---------|------|
| foundation-deployment-crate | `features/01-foundation-deployment-crate/feature.md` |
| wrangler-process-wrapper | `features/02-wrangler-process-wrapper/feature.md` |
| cloudflare-api-client | `features/03-cloudflare-api-client/feature.md` |
| cf-rust-template | `features/04-cf-rust-template/feature.md` |
| cf-rust-wasm-template | `features/05-cf-rust-wasm-template/feature.md` |
| cf-generic-wasm-template | `features/06-cf-generic-wasm-template/feature.md` |
| mise-integration | `features/07-mise-integration/feature.md` |
| examples-documentation | `features/08-examples-documentation/feature.md` |

## Fundamentals

- `fundamentals/00-overview.md` - Architecture and concepts

## Key Dependencies

The implementation relies on:

- `foundation_core::simple_http::client::SimpleHttpClient` - HTTP client for Cloudflare API
- `valtron` async patterns - For deployment state machines
- `ewe_temple` - Template generation system
- `mise` - Tooling management

## Success Criteria

The specification is complete when:

- [ ] All 8 features implemented and verified
- [ ] `ewe_platform generate --template_name="cf-*"` works for all templates
- [ ] `mise run deploy_cf` deploys successfully
- [ ] Examples build and deploy without errors
- [ ] Documentation is complete and accurate

## First Steps

1. Read `fundamentals/00-overview.md` for architecture
2. Pick a starting feature (recommend #1 or #4)
3. Read the feature's `feature.md` file
4. Implement tasks in order
5. Verify with the commands in the feature file

---

_Created: 2026-03-26_
