# Build Optimization - Start Here

## Quick Start

This specification addresses build times of 2+ hours for `ewe_deployables` by:
1. Wiring 306 feature flags to module compilation
2. Enabling incremental compilation + Cranelift backend
3. Splitting `foundation_deployment` into per-provider crates
4. Updating generators to produce feature-guarded code

## Current State

- **512,000 lines** of GCP code compiled on every build
- **306 API modules** - all compile when `gcp` feature enabled
- **`incremental = false`** - full recompilation every time
- **Cranelift unused** - LLVM does all codegen

## First Steps

1. Read `requirements.md` for full specification
2. Start with Phase 1 (feature flag wiring) - biggest impact
3. Each feature has its own `features/NN-feature-name/feature.md`

## Key Files

- `backends/foundation_deployment/src/providers/gcp/resources/mod.rs` - 306 modules
- `backends/foundation_deployment/src/providers/gcp/clients/mod.rs` - 306 modules  
- `backends/foundation_deployment/src/providers/gcp/api/mod.rs` - 306 modules
- `backends/foundation_deployment/Cargo.toml` - 306 feature flags defined
- `Cargo.toml` (workspace) - profile configuration

## Expected Impact

- Clean build: **2+ hours → 2-5 minutes**
- Incremental: **5 min → 15-30 seconds**
- **95%+ build time reduction**
