---
feature: "Generators and Cloudflare crates: rustfmt-clean shims, openapi/codegentools clippy debt, wasm unused dependencies"
status: "pending"
lane: "B"
model: "haiku"
branch: "fix/io-51-60-lane-b"
crates: ["foundation_codegentools", "foundation_openapi", "foundation_deployment_cloudflare", "foundation_cloudflare_*"]
catalogue_rows: [22, 23, 24]
---

# Feature 55: generator hygiene

_Written in ASD-STE100 Simplified Technical English (STE). Code and identifiers stay as they are. Follow [requirements.md](../../requirements.md) sections 3, 4 and 8._

Do feature 55 after feature 52 (both change the Cloudflare generator).

## E1. Per-group shims are not rustfmt-clean (backlog §8)

The 43 `backends/foundation_cloudflare_*/src/lib.rs` shims have one long `#![allow(...)]`
and unsorted `pub use` lists. `rustfmt.toml` ignores them (feature 39). Make the generator
emit rustfmt-clean output (simplest robust way: run the same `rustfmt` edition on the
generated string before writing, or emit already-wrapped text), regenerate, remove the
`ignore` entries, and confirm `cargo fmt --all --check` passes and the build-script
byte-compare stays clean.

## E2. Clippy debt (backlog §8)

`foundation_openapi` (~112) and `foundation_codegentools` (~67): fix all; mechanical only.
Add their clippy runs to CI if missing. (The docker part of the same entry is lane A, feature 49.)

## E3. `foundation_deployment_cloudflare` wasm unused dependencies (backlog §21)

Move `foundation_errstacks` and `foundation_netio` to the native target section; drop the
wasm-only `foundation_netio` entry. Verify:
`cargo check -p foundation_deployment_cloudflare --target wasm32-unknown-unknown` with no
unused-dependency warnings (nightly lint if available).

## Verify

```bash
export CARGO_PROFILE_DEV_DEBUG=0 CARGO_INCREMENTAL=0
cargo fmt --all --check
python3 scripts/ci/ci.py run foundation_codegentools --profile dev
python3 scripts/ci/ci.py run foundation_openapi --profile dev
python3 scripts/ci/ci.py run foundation_deployment_cloudflare --profile dev
```

## Done when

- [ ] No rustfmt ignore for generated shims; clippy at zero for both crates; §8 and §21
      entries deleted (coordinate the §8 docker part with feature 49). CI green.
