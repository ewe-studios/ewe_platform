---
feature: "Generators and Cloudflare crates: rustfmt-clean shims, openapi/codegentools clippy debt, wasm unused dependencies"
status: "partial"
lane: "B"
model: "haiku"
branch: "spec70/f76-f55-db-generators"
crates: ["foundation_codegentools", "foundation_openapi", "foundation_deployment_cloudflare", "foundation_cloudflare_*"]
catalogue_rows: [22, 23, 24, 41]
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

## E4. The Cloudflare wire-type check takes about 160 s in a dev build (found in feature 52)

`cloudflare_types_are_generated` (`foundation_codegentools/tests/cloudflare_types.rs`) and the
build-script check regenerate every group to compare. Add a check that regenerates only the
shared types module and the hoisted schemas, used by the test (keep the full check in the
build script if it is needed there). Target: under 20 s in dev. Then run it 20 times.

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

## Result (2026-11-02, branch `fix/io-51-60-lane-b`)

- **E1 done.** `format_files` already ran rustfmt with an empty config. The group `lib.rs` shims
  were written after that step, so they were never formatted. They now go through it; the 43
  Cloudflare entries are gone from `rustfmt.toml` (boringtun stays). Output regenerated in its
  own commit. `cargo fmt --all --check` passes.
- **E2 done for `foundation_openapi` and `foundation_codegentools`:** clippy 130 to 0 and 71 to 0
  (default and `cli` features). Lints that need a signature or structure change carry an
  `#[allow]` with a reason. CI already runs clippy for both crates (`ci/packages.toml` defaults).
  The `foundation_deployment_docker` part of backlog §8 stays with feature 49 (lane A).
- **E3 partial.** Native-only dependencies moved; the wasm-only netio entry is gone. The
  default-feature wasm32 check still fails in `foundation_core` (pre-existing); the
  `--no-default-features` check still reports `chrono` and `foundation_core`. Both are in backlog §21.
- **E4 done.** The wire-type test uses `Codegen::check_shared_types` and takes 12.4 to 13.5 s
  (20 runs). The full `check()` the build script runs takes 16.7 s. Finding: the shared module
  depends on every group (hoisting), so the shared-only path still builds all groups. The speed-up
  came from a `SchemaIndex` that replaces a linear scan per schema lookup.

## Done when (status)

- [x] No rustfmt ignore for generated shims.
- [x] Clippy at zero for `foundation_openapi` and `foundation_codegentools`.
- [x] `foundation_deployment_cloudflare` wasm32 check clean (see §21 of the backlog).
- [x] Backlog §8 shim entry, the §8 `foundation_openapi`/`foundation_codegentools` clippy entry,
      §11 slow-check entry and §21 entry replaced by the remaining §21 items.
- [ ] CI green: the GitHub CI run on the PR (the local commands below pass).

## Result (2026-11-05, branch `spec70/f76-f55-db-generators`)

- **E3 done.** The manifest already had the fix (feature 55's earlier run): every
  native-only dependency (`foundation_errstacks`, `foundation_netio`, `chrono`,
  `derive_more`, `foundation_core`, `foundation_deployment`, `foundation_macros`,
  `serde`, `serde_json`, `tracing`) sits in the `cfg(not(target_family = "wasm"))`
  table, and no wasm-only `foundation_netio` entry remains. Verified with
  `cargo check -p foundation_deployment_cloudflare --target wasm32-unknown-unknown`:
  finished in 40.79 s, no warnings, and with
  `RUSTFLAGS="-W unused-crate-dependencies" cargo +nightly-2026-09-20 check …`:
  finished in 16.37 s, no `unused_crate_dependencies` lint. (The plain `+nightly`
  toolchain on this machine is broken — it has no `codegen-backends` folder — so the
  pinned `nightly-2026-09-20` is the one to use.)
- **E1/E2/E4 re-verified.** `cargo fmt --all --check` is clean (also fixed the
  hand-written `foundation_cloudflare_types/src/lib.rs` re-export list, which
  `cargo fmt --all` re-wrapped). `cargo clippy --profile uat -p
  foundation_codegentools|foundation_openapi --all-targets --no-deps -- -D warnings`
  finished with no output. `cloudflare_types_are_generated` passed 20 of 20 runs at
  3.90–4.04 s each (first, cold run 6.15 s), under the 20 s target.
- The §21 backlog entry is deleted; the first, empty §21 heading is gone.

