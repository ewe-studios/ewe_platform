# Learnings — crates.io Publishing

## Crate Name Availability (2026-06-16)

- All 38 crate names confirmed available on crates.io
- Initial scan flagged 4 names as "TAKEN" (`foundation_core`, `foundation_macros`, `foundation_nostd`, `foundation_wasm`) — but direct verification via WebFetch confirmed they are all available (likely yanked previously or stale cache from the agent)
- The crates.io API (`https://crates.io/api/v1/crates/<name>`) returned 404 for all 4, confirming availability

## Metadata Requirements

- 4 crates were missing `description` fields: `foundation_auth`, `foundation_cedar`, `foundation_logging`, `foundation_netio`
- Descriptions were added during feature 00 — all 38 crates now have descriptions
- No crates have `readme` fields yet — this is tracked in feature 01
- `categories` field is optional but recommended for crates.io discoverability

## Infrastructure Crates

- `infrastructure_llama_bindings` and `infrastructure_llama_cpp` are FFI bindings that require C compilation (llama.cpp)
- These may have longer build times and platform-specific issues during `cargo publish`
- User confirmed both should be published to crates.io
