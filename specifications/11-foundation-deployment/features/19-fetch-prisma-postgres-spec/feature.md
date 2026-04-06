---
workspace_name: "ewe_platform"
spec_directory: "specifications/11-foundation-deployment"
feature_directory: "specifications/11-foundation-deployment/features/15-fetch-prisma-postgres-spec"
this_file: "specifications/11-foundation-deployment/features/15-fetch-prisma-postgres-spec/feature.md"

status: completed
priority: medium
created: 2026-03-27

depends_on: ["10-provider-spec-fetcher-core"]

tasks:
  completed: 4
  uncompleted: 0
  total: 4
  completion_percentage: 100%
---


# Fetch Prisma Postgres OpenAPI Spec

## Iron Law: Zero Warnings

> **All code must compile with zero warnings and pass all lints. No suppression. No exceptions.**

## Overview

Implement the Prisma Postgres OpenAPI spec fetcher. Prisma Postgres provides a single OpenAPI spec at `https://api.prisma.io/v1/doc`.

## Prisma Postgres Spec Details

| Property | Value |
|----------|-------|
| URL | `https://api.prisma.io/v1/doc` |
| Format | OpenAPI JSON |
| Auth Required | No |
| Rate Limits | Standard HTTP rate limits |

## Architecture

Provider implementations live in `backends/foundation_deployment/src/providers/`, **not** in `bin/platform/`. Each provider gets its own directory:

```
backends/foundation_deployment/src/providers/
├── prisma_postgres/
│   ├── mod.rs       # Module declaration (pub mod fetch;)
│   ├── fetch.rs     # Fetch logic, constants, process_spec
│   └── resources/
│       └── mod.rs   # (future) Auto-generated resource types
├── openapi.rs       # Shared OpenAPI 3.x extraction utilities
└── standard/
    └── fetch.rs     # Generic HTTP fetch (used by all standard providers)
```

### Prisma Postgres-Specific Fetcher

```rust
// backends/foundation_deployment/src/providers/prisma_postgres/fetch.rs

pub const SPEC_URL: &str = "https://api.prisma.io/v1/doc";
pub const PROVIDER_NAME: &str = "prisma-postgres";

pub fn fetch_prisma_postgres_specs(output_dir: PathBuf) -> Result<impl StreamIterator<...>, DeploymentError> {
    standard::fetch::fetch_standard_spec(PROVIDER_NAME, SPEC_URL, output_dir)
}

pub fn process_spec(spec: &serde_json::Value) -> ProcessedSpec {
    openapi::process_spec(spec)
}
```

## Tasks

1. **Create Prisma Postgres provider module**
   - [x] Create `backends/foundation_deployment/src/providers/prisma_postgres/mod.rs`
   - [x] Create `backends/foundation_deployment/src/providers/prisma_postgres/fetch.rs`
   - [x] Implement fetch and process_spec functions

2. **Register in module tree**
   - [x] Add `pub mod prisma_postgres;` to `providers/mod.rs`
   - [x] Wire into fetcher

3. **Write unit tests**
   - [x] Test constants, version extraction, endpoint extraction

4. **Integration test**
   - [x] Verify `cargo run -- gen_provider_specs --provider prisma-postgres` works

## Success Criteria

- [x] All 4 tasks completed
- [x] Zero warnings
- [x] Spec fetches successfully to `artefacts/cloud_providers/prisma-postgres/`

## Verification

```bash
cargo test -p foundation_deployment -- resources::prisma_postgres
cargo run -- gen_provider_specs --provider prisma-postgres
```

---

_Created: 2026-03-27_
_Updated: 2026-04-04 - Corrected directory structure to backends/foundation_deployment_
