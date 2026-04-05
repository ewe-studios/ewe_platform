---
workspace_name: "ewe_platform"
spec_directory: "specifications/11-foundation-deployment"
feature_directory: "specifications/11-foundation-deployment/features/22-provider-api-feature-flags"
this_file: "specifications/11-foundation-deployment/features/22-provider-api-feature-flags/feature.md"

status: pending
priority: high
created: 2026-04-05
updated: 2026-04-05

depends_on: ["20-gen-resource-types", "10-provider-spec-fetcher-core"]

tasks:
  completed: 0
  uncompleted: 6
  total: 6
  completion_percentage: 0%
---


# Provider-Level Feature Flags

## Overview

Implement provider-level feature flags for the `foundation_deployment` crate to enable users to include only the providers they need, reducing compile times and binary sizes.

**Decision:** Use provider-level flags only (e.g., `gcp`, `cloudflare`, `mongodb`). Per-API flags were considered but rejected because:
- Dependency analysis was overly aggressive, pulling in unrelated APIs
- `cfg(any(...))` patterns defeated the purpose of fine-grained control
- Complexity outweighed benefits for current use cases

## What: Feature Flag Architecture

### Provider-Level Flags

Provider-level flags gate the entire provider module. All resources within a provider are compiled when the provider feature is enabled:

```rust
// backends/foundation_deployment/src/providers/gcp/mod.rs
//! Provider module for GCP.
//!
//! Feature flag: `gcp`

pub mod fetch;
pub mod provider;

#[cfg(feature = "gcp")]
pub mod resources;

#[cfg(feature = "gcp")]
pub use resources::*;
```

**Provider flags:**
- `gcp` - Google Cloud Platform provider
- `cloudflare` - Cloudflare provider
- `mongodb` - MongoDB Atlas provider
- `neon` - Neon provider
- `supabase` - Supabase provider
- `planetscale` - PlanetScale provider
- `fly_io` - Fly.io provider
- `stripe` - Stripe provider
- `aws` - AWS provider (future)
- `azure` - Azure provider (future)

### Generated File Structure

Each generated file within a provider uses the same provider-level flag:

```rust
// backends/foundation_deployment/src/providers/gcp/resources/run.rs
//! Auto-generated resource types for Google Cloud Platform - run.
//!
//! Feature flag: `gcp`

#![cfg(feature = "gcp")]

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Service {
    // ...
}
```

## How: Implementation Details

### 1. Cargo.toml Feature Definitions

Update `backends/foundation_deployment/Cargo.toml` to define provider features:

```toml
[features]
default = []

# Provider-level flags
gcp = []
cloudflare = []
mongodb = []
neon = []
supabase = []
planetscale = []
fly_io = []
stripe = []
aws = []
```

### 2. Generator Changes

The `gen_resource_types` generator adds `#![cfg(feature = "{provider}")]` to all generated files:

```rust
fn generate_file_header(provider: &str, api: Option<&str>) -> String {
    let mut out = String::new();

    writeln!(out, "//! Auto-generated resource types for {provider}.").unwrap();
    writeln!(out, "//!").unwrap();
    writeln!(out, "//! Feature flag: `{provider}`").unwrap();
    writeln!(out).unwrap();
    writeln!(out, "#![cfg(feature = \"{provider}\")]").unwrap();
    writeln!(out).unwrap();
    writeln!(out, "use serde::{{Deserialize, Serialize}};").unwrap();

    out
}
```

### 3. Resources mod.rs Generation

For multi-API providers, generate simple module declarations:

```rust
fn generate_mod_rs(provider: &str, apis: &[String]) -> String {
    let mut out = String::new();

    writeln!(out, "//! Auto-generated resource types for {provider}.").unwrap();
    writeln!(out, "//!").unwrap();
    writeln!(out, "//! Feature flag: `{provider}`").unwrap();
    writeln!(out).unwrap();
    writeln!(out, "#![cfg(feature = \"{provider}\")]").unwrap();
    writeln!(out).unwrap();

    for api in apis {
        writeln!(out, "pub mod {api};").unwrap();
    }
    writeln!(out).unwrap();

    for api in apis {
        writeln!(out, "pub use {api}::*;").unwrap();
    }

    out
}
```

## User Experience

### Enabling Features

Users enable features in their `Cargo.toml`:

```toml
[dependencies]
foundation_deployment = { path = "../backends/foundation_deployment", features = [
    "gcp",           # Enable GCP provider (all APIs)
    "cloudflare",    # Enable Cloudflare provider
] }
```

### Default Features

By default, no providers are enabled:

```toml
[dependencies]
foundation_deployment = { path = "../backends/foundation_deployment" }
# Compiles with zero providers - just core types
```

## Tasks

1. **Update Cargo.toml**
   - [x] Add provider-level feature flags
   - [ ] Remove API-level feature flags (simplify)

2. **Simplify generator**
   - [ ] Remove dependency analysis code
   - [ ] Use provider-level `cfg` only
   - [ ] Generate clean, simple headers

3. **Verification**
   - [ ] Test with `--no-default-features --features "gcp"`
   - [ ] Verify compile time improvements
   - [ ] Verify zero warnings

## Success Criteria

- [ ] Generator produces simple `#![cfg(feature = "provider")]` headers
- [ ] No dependency analysis complexity
- [ ] Users can exclude providers they don't need
- [ ] Clean, maintainable generator code

## Verification Commands

```bash
cd /home/darkvoid/Boxxed/@dev/ewe_platform

# Test with no features
cargo check -p foundation_deployment --no-default-features

# Test with just GCP provider
cargo check -p foundation_deployment --no-default-features --features "gcp"

# Test with multiple providers
cargo check -p foundation_deployment --no-default-features --features "gcp,cloudflare"
```

## Design Decision: Why Provider-Level Only?

We initially designed a two-tier system with per-API flags (e.g., `gcp_compute`, `gcp_run`), but found:

1. **Dependency analysis was too aggressive** - It detected type references across many APIs, creating large `cfg(any(...))` blocks that effectively compiled most of GCP when any API was enabled.

2. **Complexity vs. benefit** - The dependency tracking added significant code complexity but didn't provide meaningful compile-time savings since most APIs ended up being included anyway.

3. **Simpler mental model** - Users either want GCP support or they don't. Fine-grained API selection is a premature optimization for the current use case.

The provider-level approach gives us the main benefit (excluding unused providers like `cloudflare` when only using `gcp`) without the complexity.

---

_Created: 2026-04-05_
_Updated: 2026-04-05 - Simplified to provider-level flags only_
