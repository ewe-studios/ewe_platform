---
feature: "Cloudflare crates build for wasm32 with default features; clippy warnings fail CI for crates at zero; faster genapi"
status: "pending"
lane: "B"
model: "sonnet"
branch: "fix/io-51-60-lane-b"
crates: ["foundation_codegentools", "foundation_openapi", "foundation_deployment_cloudflare", "foundation_cloudflare_*", "scripts/ci", "ci/packages.toml"]
catalogue_rows: [64, 65, 66, 67]
---

# Feature 71: Cloudflare wasm build, CI clippy, genapi speed

_Written in ASD-STE100 Simplified Technical English (STE). Code and identifiers stay as they are. Follow [requirements.md](../../requirements.md) sections 3, 4 and 8._

Found in feature 55 (backlog §8, §21).

## E1. The default-feature wasm32 build of the Cloudflare crates fails (§21) — DONE by the main session

Done on branch `fix/io-71-cloudflare-wasm` (commit `a7148a870`, from the lane B branch), at
the owner's request on 2026-11-03: the generator now emits the group crates and the types crate
as native-only umbrella dependencies; `chrono` and `foundation_core` moved native-only; a
`compile_error!` in `foundation_core` names the cause if `multi` reaches wasm32 again; CI wasm
runs added for the crate. **Merge `origin/fix/io-71-cloudflare-wasm` into the lane B branch
before starting E2.** The text below is the original task, kept for the record.



`cargo check -p foundation_deployment_cloudflare --target wasm32-unknown-unknown` fails in
`foundation_core` (valtron `multi` executors use `idle_wake`, native-only). Cause: the generated
group crates depend on `foundation_netio`/`foundation_deployment` with `multi` (and other
native-only) features on every target. Fix it in the generator (lane B): emit those dependencies
in a `[target.'cfg(not(target_family = "wasm"))'.dependencies]` section, and the wasm entries
without native features; regenerate (generator commit and generated-output commit separate). If
the cause is a feature line inside a lane A crate's manifest (`foundation_deployment`,
`foundation_netio`), stop and report the exact line instead of editing it.
Verify: the wasm32 check above exits 0 with default features; add it to the crate's
`wasm-runs` in `ci/packages.toml` so CI keeps it.

## E2. Six dependencies unused on wasm32 when `workers` is off (§21)

`chrono` and `foundation_core` are done (E1). Left: `derive_more`, `foundation_macros`,
`js-sys`, `serde`, `serde_json`, `tracing` — on wasm32 only the `workers` module uses them.
Make each wasm32 use optional and enabled by `workers`, keeping the native entries.

Target-gate them so `cargo check -p foundation_deployment_cloudflare --target
wasm32-unknown-unknown --no-default-features` has no unused-dependency warnings (nightly lint).

## E3. Clippy warnings do not fail CI (§8)

The CI clippy step reports warnings but passes. For every crate that is now at zero clippy
warnings (foundation_auth, foundation_openapi, foundation_codegentools, foundation_db and any
other the lane B and lane A features brought to zero), make CI fail on a clippy warning: add a
per-package setting (for example `clippy-deny-warnings = true` in `ci/packages.toml`, passing
`-D warnings` to that package's clippy runs). Keep it off for crates that still have debt, and
list both sets in the PR. Lane A crates: set the flag only for the ones already at zero on the
lane B branch; feature 49 sets the rest.

## E4. `genapi generate` runs `cargo fix` over the whole generated crate (~5 min) (§8)

Find why it runs `cargo fix` (unused imports in generated code?). Fix the generator so the code
is clean as emitted, and drop the `cargo fix` step (or limit it to the files it changed). Target:
a full `genapi generate cloudflare` under 1 min in dev. Report before/after times.

## Verify

```bash
export CARGO_PROFILE_DEV_DEBUG=0 CARGO_INCREMENTAL=0
cargo check -p foundation_deployment_cloudflare --target wasm32-unknown-unknown --profile dev
python3 scripts/ci/ci.py run foundation_deployment_cloudflare --profile dev
python3 scripts/ci/ci.py run foundation_codegentools --profile dev
```

## Done when

- [ ] wasm32 default build green and in CI; clippy warnings fail CI for zero-debt crates;
      genapi faster; the §8 and §21 entries deleted.
