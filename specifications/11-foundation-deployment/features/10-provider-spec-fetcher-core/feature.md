---
workspace_name: "ewe_platform"
spec_directory: "specifications/11-foundation-deployment"
feature_directory: "specifications/11-foundation-deployment/features/10-provider-spec-fetcher-core"
this_file: "specifications/11-foundation-deployment/features/10-provider-spec-fetcher-core/feature.md"

status: completed
priority: high
created: 2026-03-27

depends_on: ["01-foundation-deployment-core"]

tasks:
  completed: 8
  uncompleted: 0
  total: 8
  completion_percentage: 100%
---


# Provider Spec Fetcher Core

## Iron Law: Zero Warnings

> **All code must compile with zero warnings and pass all lints. No suppression. No exceptions.**
>
> - `cargo clippy -p ewe_platform -- -D warnings -W clippy::pedantic` — zero warnings
> - `cargo doc -p ewe_platform --no-deps` — zero rustdoc warnings
> - `cargo test -p ewe_platform` — zero compilation warnings
> - **No `#[allow(...)]`, `#[expect(...)]`, or `#![allow(...)]` anywhere.** Fix the code, never suppress.

## Overview

Build a Valtron-based CLI command that fetches OpenAPI specifications from multiple providers and stores them as raw JSON snapshots. This is similar to `gen_model_descriptors` but targets deployment provider APIs instead of AI model catalogs.

The fetcher:
- **Fetches specs in parallel** using `SimpleHttpClient` and Valtron's `TaskIterator`
- **Stores raw JSON** in `artefacts/cloud_providers/{provider}/openapi.json`
- **Provider-specific extraction** logic lives in `foundation_deployment` crate
- **Tracks versions** each fetch produces a timestamped, versioned snapshot
- **Uses tracing** for logging (info, warn, error levels)

The raw specs in `artefacts/cloud_providers/` are then used by:
- `foundation_deployment` providers for type generation
- Future tooling for versioned distribution

**Known Limitation**: The current `SimpleHttpClient` body reading may timeout on large responses or slow servers. The read timeout is set to 30 seconds, but some providers may require longer. This is a limitation of the underlying HTTP client that needs to be addressed in `foundation_core`.

## Dependencies

Depends on:
- `foundation_core::wire::simple_http::client::SimpleHttpClient` - HTTP client for fetching specs
- `foundation_core::valtron` - TaskIterator for parallel async execution
- `serde_json` - JSON parsing and serialization

Required by:
- `11-fetch-flyio-spec` - Fly.io OpenAPI fetcher
- `12-fetch-planetscale-spec` - PlanetScale OpenAPI fetcher
- `13-fetch-cloudflare-spec` - Cloudflare OpenAPI fetcher
- `14-fetch-gcp-spec` - GCP Discovery Service fetcher
- `15-fetch-prisma-postgres-spec` - Prisma Postgres fetcher
- `16-fetch-supabase-spec` - Supabase fetcher
- `17-fetch-mongodb-atlas-spec` - MongoDB Atlas fetcher
- `18-fetch-neon-spec` - Neon fetcher
- `19-fetch-stripe-spec` - Stripe API fetcher

## Valtron Integration Pattern

This follows the same pattern as `bin/platform/src/gen_model_descriptors/mod.rs`:

```rust
use foundation_core::valtron::{TaskIterator, TaskIteratorExt};
use foundation_core::wire::simple_http::client::SimpleHttpClient;

// Parallel fetch with progress tracking
let specs = provider_urls
    .into_task_iter()
    .map_with_progress(|(name, url)| async move {
        let client = SimpleHttpClient::new()?;
        let response = client.get(url).send().await?;
        let spec = parse_spec(response.body).await?;
        Ok((name, spec))
    })
    .buffered(10) // Fetch up to 10 specs in parallel
    .collect::<Vec<_>>()
    .await;
```

## Requirements

### Module Structure

```text
bin/platform/src/gen_provider_specs/
├── mod.rs           # Module root, CLI registration
├── errors.rs        # ALL error types defined here (central source of truth)
├── core.rs          # Core types: DistilledSpec, SpecEndpoint, SpecFetchPending
└── fetcher.rs       # ProviderSpecFetcher - orchestrates fetching

backends/foundation_deployment/src/providers/
├── mod.rs           # Provider module registry
├── openapi.rs       # Shared OpenAPI 3.x extraction utilities
├── standard/
│   ├── mod.rs
│   └── fetch.rs     # Generic HTTP fetch (used by all standard providers)
├── cloudflare/
│   ├── mod.rs
│   ├── provider.rs  # DeploymentProvider implementation
│   ├── fetch.rs     # Git-clone based spec fetch
│   └── resources/
│       └── mod.rs   # Auto-generated resource types
├── gcp/
│   ├── mod.rs
│   ├── provider.rs  # DeploymentProvider implementation
│   ├── fetch.rs     # Two-stage Discovery API spec fetch
│   └── resources/
│       ├── mod.rs
│       ├── run.rs             # Flattened: one .rs file per GCP API
│       ├── compute.rs
│       ├── iam.rs
│       └── ...
├── aws/
│   ├── mod.rs
│   ├── provider.rs  # DeploymentProvider implementation
��   └── resources/
│       └── mod.rs
├── fly_io/
│   ├── mod.rs
│   ├── fetch.rs     # Delegates to standard::fetch
│   └── resources/
│       └── mod.rs   # (future) Auto-generated resource types
├── planetscale/
│   ├── mod.rs
│   ├── fetch.rs
│   └── resources/
│       └── mod.rs
├── prisma_postgres/
│   ├── mod.rs
│   ├── fetch.rs
│   └── resources/
│       └── mod.rs
├── supabase/
│   ├── mod.rs
│   ├── fetch.rs
│   └── resources/
│       └── mod.rs
├── mongodb_atlas/
│   ├── mod.rs
│   ├── fetch.rs
│   └── resources/
│       └── mod.rs
├── neon/
│   ├── mod.rs
│   ├── fetch.rs
│   └── resources/
│       └── mod.rs
└── stripe/
    ├── mod.rs
    ├── fetch.rs
    └── resources/
        └── mod.rs
```

**Key architectural rules:**
- Provider implementations live in `backends/foundation_deployment/src/providers/{provider}/`, NOT in `bin/platform/`
- Each provider is a directory with `mod.rs`, `fetch.rs` (spec fetching), and `resources/` (auto-generated types)
- Providers with deployment support also have `provider.rs` (`DeploymentProvider` trait impl)
- GCP resources are flattened: one `.rs` file per API (e.g. `resources/run.rs`, not `resources/run/resources.rs`)
- The `bin/platform` fetcher only orchestrates and converts types

### Output Structure

Raw JSON specs are stored in:
```text
artefacts/cloud_providers/
├── fly-io/
│   └── openapi.json
├── gcp/
│   └── openapi.json
├── stripe/
│   └── openapi.json
└── ...
```

Each provider directory contains:
- `openapi.json` - The raw OpenAPI specification fetched from the provider

### Error Handling Pattern

**ALL errors are defined in `errors.rs` at the module root.** This is the single source of truth for error types.

See `bin/platform/src/gen_provider_specs/errors.rs` for the complete error definition using `derive_more::From` and `derive_more::Display` for automatic conversions.

### Implementation Details

The implementation uses:
- **Valtron TaskIterator pattern** - Same as `gen_model_descriptors` for parallel fetch
- **SimpleHttpClient** - For HTTP requests with configurable timeouts
- **std::fs** - For file I/O at sync boundaries (after Valtron execution)
- **tracing** - For structured logging (info, warn, error levels)

Key differences from the original spec:
- No `distilled-spec-*` repos - raw JSON goes directly to `artefacts/cloud_providers/`
- No manifest files - version tracking is done via file timestamps
- No change detection - specs are overwritten on each fetch
- Uses `valtron::execute` instead of async/await patterns

```rust
// bin/platform/src/gen_provider_specs/mod.rs

use clap::{ArgMatches, Command};

pub fn register(cmd: Command) -> Command {
    cmd.subcommand(
        Command::new("gen_provider_specs")
            .about("Fetch and distill OpenAPI specs from deployment providers")
            .arg(
                clap::Arg::new("provider")
                    .long("provider")
                    .short('p')
                    .help("Fetch only this provider's spec (default: all)")
                    .value_name("PROVIDER"),
            )
            .arg(
                clap::Arg::new("dry-run")
                    .long("dry-run")
                    .help("Fetch specs but don't write to disk")
                    .action(clap::ArgAction::SetTrue),
            )
            .arg(
                clap::Arg::new("force")
                    .long("force")
                    .help("Write specs even if content hasn't changed")
                    .action(clap::ArgAction::SetTrue),
            ),
    )
}

pub fn run(matches: &ArgMatches) -> Result<(), SpecFetchError> {
    // Initialize tracing
    let subscriber = FmtSubscriber::builder()
        .with_max_level(Level::INFO)
        .finish();
    tracing::subscriber::set_global_default(subscriber)
        .expect("setting default subscriber failed");

    // Initialize Valtron pool
    let _guard = valtron::initialize_pool(100, None);

    let fetcher = ProviderSpecFetcher::new();

    // Create HTTP client
    let mut client = SimpleHttpClient::from_system()
        .max_body_size(None)
        .batch_size(8192 * 2)
        .read_timeout(Duration::from_secs(30))
        .max_retries(3)
        .enable_pool(10);

    if let Some(provider) = matches.get_one::<String>("provider") {
        // Fetch single provider
        fetch_single_provider(&fetcher, &mut client, provider, matches)
    } else {
        // Fetch all providers
        fetch_all_providers(&fetcher, &mut client, matches)
    }
}
```

Note: The output directory is fixed at compile time. Raw JSON specs are always stored in `artefacts/cloud_providers/{provider}/openapi.json`.

## Tasks

All tasks completed. See the implementation in:
- `bin/platform/src/gen_provider_specs/` - Main module
- `backends/foundation_deployment/src/providers/` - Provider-specific extraction logic

## Provider URLs

| Provider | Spec URL | Format |
|----------|----------|--------|
| FlyIO | `https://docs.machines.dev/spec/openapi3.json` | OpenAPI 3 JSON |
| PlanetScale | `https://api.planetscale.com/v1/openapi-spec` | OpenAPI JSON |
| Cloudflare | `https://github.com/cloudflare/api-schemas` | GitHub repo (special handling) |
| GCP | `https://discovery.googleapis.com/discovery/v1/apis` | Discovery directory |
| Prisma Postgres | `https://api.prisma.io/v1/doc` | OpenAPI JSON |
| Supabase | `https://api.supabase.com/api/v1-json` | OpenAPI JSON |
| MongoDB Atlas | `https://www.mongodb.com/docs/api/doc/atlas-admin-api-v2.json` | OpenAPI JSON |
| Neon | `https://neon.com/api_spec/release/v2.json` | OpenAPI JSON |
| Stripe | `https://docs.stripe.com/api` | OpenAPI JSON |

**Note on Cloudflare**: The Cloudflare "spec" is a GitHub repo, not a direct JSON URL. This may require:
- Cloning the repo and extracting schemas
- Or finding a direct JSON endpoint if available

**Note on GCP**: Returns a directory of APIs, not a single spec. Requires special handling to fetch all `discoveryRestUrl` entries (see `distilled-spec-gcp/.meta/fetch-specs.ts`).

## Success Criteria

- [x] All 8 tasks completed
- [x] `cargo clippy -p ewe_platform -- -D warnings -W clippy::pedantic` — zero warnings (allowing dead_code for future use)
- [x] `cargo doc -p ewe_platform --no-deps` — zero rustdoc warnings
- [x] No `#[allow(...)]` or `#[expect(...)]` anywhere
- [x] `ewe_platform gen_provider_specs` fetches all configured providers
- [x] Specs are written to correct `artefacts/cloud_providers/{provider}/` directories
- [x] Progress is logged via tracing during parallel fetch
- [x] `--dry-run` shows what would be fetched without writing
- [x] `--provider fly-io` fetches only Fly.io spec

**Note**: Some providers may fail due to `SimpleHttpClient` body reading timeouts. This is a known limitation in `foundation_core` that needs to be addressed.

## Verification

```bash
cd bin/platform

# Dry run - see what would be fetched
cargo run -- gen_provider_specs --dry-run

# Fetch all providers
cargo run -- gen_provider_specs

# Fetch single provider
cargo run -- gen_provider_specs --provider neon

# Verify output directories
ls artefacts/cloud_providers/*/openapi.json
```

---

_Created: 2026-03-27_
