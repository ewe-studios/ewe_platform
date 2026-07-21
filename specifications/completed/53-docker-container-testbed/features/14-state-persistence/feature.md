---
feature: "State persistence — backend states, TLS certs, config hash"
description: "Wraps foundation_db::FileStateStore to persist backend drain/pause state, TLS certificate data, and configuration hash across proxy restarts"
status: "complete"
priority: "medium"
phase: 4
depends_on: ["21-state-persistence"]
estimated_effort: "small"
created: 2026-07-16
---
# Feature 14: State Persistence (Decision 21)

## What

`persistence.rs` — `ProxyStateStore` wrapping `foundation_db::FileStateStore`:

- `load()` / `save()` — full proxy state as JSON via `store_typed`/`load_typed`
- `save_backend_state(service, url, state)` — per-backend state (Active/Draining/Paused)
- `save_tls_cert(cert_pem, key_pem)` — certificate persistence for TLS auto-renewal
- `compute_config_hash(config)` — deterministic hash of proxy config for change detection between restarts

## Implementation

Uses `foundation_db::core::state::{FileStateStore, StateStore}` — already exists
in the workspace. Single `PersistedProxyState` struct serialized as JSON per
domain. No new dependencies.

## Tests

- `persisted_state_default_is_empty` — default has empty maps
- `persisted_state_serializes_roundtrip` — JSON roundtrip with backend states
- `config_hash_changes_with_different_backend` — different IPs = different hashes
- `config_hash_stable_for_same_config` — identical configs = same hash

## Verification

- `cargo test -p foundation_proxy --lib -- persistence`
