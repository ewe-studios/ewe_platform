---
spec_id: "11-foundation-deployment"
title: "Foundation Deployment"
status: "pending"
priority: "high"
created: "2026-03-26"
---

# Foundation Deployment - Start Here

## Specification Summary

This specification creates a **multi-provider deployment system** with:

1. **foundation_deployment** crate (`backends/foundation_deployment/`)
2. **Provider trait** with implementations for Cloudflare, GCP Cloud Run, and AWS Lambda
3. **State stores** - Five interchangeable backends: Turso, plain SQLite, JSON files, Cloudflare R2, Cloudflare D1
4. **Deployment engine** - Valtron state machine for orchestration
5. **Composable templates** - Language x Provider matrix
6. **mise.toml integration** - Provider-agnostic task definitions

## Implementation Order

```
1. foundation-deployment-core     (base - no dependencies)
2. state-stores                   (depends on #1)
3. deployment-engine              (depends on #1, #2)
4. cloudflare-provider            (depends on #1, #2, #3)
5. gcp-cloud-run-provider         (depends on #1, #2, #3)
6. aws-lambda-provider            (depends on #1, #2, #3)
7. templates                      (depends on #4, #5, #6)
8. mise-integration               (depends on #7)
9. examples-documentation         (depends on all above)
```

## Where to Start

### Recommended: Start with the core + state stores

These are the foundation everything else builds on:

```bash
mkdir -p backends/foundation_deployment/src/{core,state,engine,providers/{cloudflare,gcp,aws},template}
```

1. Define the `DeploymentProvider` trait (`src/core/traits.rs`)
2. Define error types (`src/error.rs`)
3. Build `ProcessExecutor` (`src/core/process.rs`)
4. Implement `StateStore` trait + `JsonFileStateStore` (`src/state/`)
5. Then add `SqliteStateStore`, `TursoStateStore`, `R2StateStore`, `D1StateStore`

See: `features/01-foundation-deployment-core/feature.md`

### Alternative: Start with a single provider

If you want to validate the design with a real provider:

1. Implement core trait + errors (minimal)
2. Implement Cloudflare provider (API-first via SimpleHttpClient, CLI as fallback)
3. Backfill state stores and engine once the provider works

See: `features/04-cloudflare-provider/feature.md`

## Feature Files

| Feature | File |
|---------|------|
| foundation-deployment-core | `features/01-foundation-deployment-core/feature.md` |
| state-stores | `features/02-state-stores/feature.md` |
| deployment-engine | `features/03-deployment-engine/feature.md` |
| cloudflare-provider | `features/04-cloudflare-provider/feature.md` |
| gcp-cloud-run-provider | `features/05-gcp-cloud-run-provider/feature.md` |
| aws-lambda-provider | `features/06-aws-lambda-provider/feature.md` |
| templates | `features/07-templates/feature.md` |
| mise-integration | `features/08-mise-integration/feature.md` |
| examples-documentation | `features/09-examples-documentation/feature.md` |

## Fundamentals

- `fundamentals/00-overview.md` - Architecture, state model, config formats

## Key Dependencies

| Dependency | Usage |
|------------|-------|
| `foundation_core::simple_http::client::SimpleHttpClient` | API clients for all providers |
| `foundation_core::valtron` | Deployment state machine, async execution |
| `ewe_temple` | Template generation system |
| `rusqlite` / `libsql` | SQLite/Turso state store |
| `mise` | Tooling and task management |

## Success Criteria

- [ ] All 9 features implemented and verified
- [ ] `DeploymentProvider` trait works for all 3 providers
- [ ] State stores persist and detect changes correctly
- [ ] `mise run deploy` auto-detects provider and deploys
- [ ] Templates generate working projects for all provider x language combos
- [ ] Programmatic API deployment works without CLI tools

## First Steps

1. Read `fundamentals/00-overview.md` for architecture
2. Start with Feature 01 (core crate)
3. Then Feature 02 (state stores)
4. Pick a provider (recommend Cloudflare first)
5. Verify with the commands in each feature file

---

_Created: 2026-03-26_
