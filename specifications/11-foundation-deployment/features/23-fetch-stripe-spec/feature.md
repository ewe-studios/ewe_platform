---
workspace_name: "ewe_platform"
spec_directory: "specifications/11-foundation-deployment"
feature_directory: "specifications/11-foundation-deployment/features/19-fetch-stripe-spec"
this_file: "specifications/11-foundation-deployment/features/19-fetch-stripe-spec/feature.md"

status: completed
priority: medium
created: 2026-03-27

depends_on: ["10-provider-spec-fetcher-core"]

tasks:
  completed: 5
  uncompleted: 0
  total: 5
  completion_percentage: 100%
---


# Fetch Stripe OpenAPI Spec

## Iron Law: Zero Warnings

> **All code must compile with zero warnings and pass all lints. No suppression. No exceptions.**

## Overview

Implement the Stripe OpenAPI spec fetcher. Stripe provides their API spec via their GitHub OpenAPI repo. Stripe's API is large and well-documented with extensive OpenAPI coverage.

## Stripe Spec Details

| Property | Value |
|----------|-------|
| URL | `https://raw.githubusercontent.com/stripe/openapi/master/openapi/spec3.json` |
| Format | OpenAPI JSON |
| Auth Required | No |
| Rate Limits | Standard HTTP rate limits |
| Notes | Large spec (~10MB+) with many resource types |

## Architecture

Provider implementations live in `backends/foundation_deployment/src/providers/`, **not** in `bin/platform/`. Each provider gets its own directory:

```
backends/foundation_deployment/src/providers/
├── stripe/
│   ├── mod.rs       # Module declaration (pub mod fetch;)
│   ├── fetch.rs     # Fetch logic, constants, process_spec
│   └── resources/
│       └── mod.rs   # (future) Auto-generated resource types
├── openapi.rs       # Shared OpenAPI 3.x extraction utilities
└── standard/
    └── fetch.rs     # Generic HTTP fetch (used by all standard providers)
```

### Stripe-Specific Fetcher

```rust
// backends/foundation_deployment/src/providers/stripe/fetch.rs

pub const SPEC_URL: &str = "https://raw.githubusercontent.com/stripe/openapi/master/openapi/spec3.json";
pub const PROVIDER_NAME: &str = "stripe";

pub fn fetch_stripe_specs(output_dir: PathBuf) -> Result<impl StreamIterator<...>, DeploymentError> {
    standard::fetch::fetch_standard_spec(PROVIDER_NAME, SPEC_URL, output_dir)
}

/// Process a fetched Stripe spec.
/// Stripe's spec is large (~10MB+), so content hash computation may take longer.
pub fn process_spec(spec: &serde_json::Value) -> ProcessedSpec {
    openapi::process_spec(spec)
}
```

## Tasks

1. **Create Stripe provider module**
   - [x] Create `backends/foundation_deployment/src/providers/stripe/mod.rs`
   - [x] Create `backends/foundation_deployment/src/providers/stripe/fetch.rs`
   - [x] Implement fetch and process_spec functions

2. **Register in module tree**
   - [x] Add `pub mod stripe;` to `providers/mod.rs`
   - [x] Wire into fetcher

3. **Write unit tests**
   - [x] Test constants
   - [x] Test endpoint extraction (Stripe has many endpoints)

4. **Integration test**
   - [x] Verify `cargo run -- gen_provider_specs --provider stripe` works
   - [x] Verify large spec is handled correctly

5. **Handle edge cases**
   - [x] Large response bodies handled via curl in `standard::fetch`
   - [x] GitHub raw redirects followed by curl `-L` flag

## Success Criteria

- [x] All 5 tasks completed
- [x] Zero warnings
- [x] Stripe spec fetches successfully to `artefacts/cloud_providers/stripe/`
- [x] All endpoints correctly extracted

## Verification

```bash
cargo test -p foundation_deployment -- resources::stripe
cargo run -- gen_provider_specs --provider stripe
```

---

_Created: 2026-03-27_
_Updated: 2026-04-04 - Corrected directory structure to backends/foundation_deployment_
