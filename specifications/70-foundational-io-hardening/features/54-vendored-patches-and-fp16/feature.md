---
feature: "Vendored crate patches reach published users; stable fp16 target feature"
status: "pending"
lane: "B"
model: "sonnet"
branch: "fix/io-51-60-lane-b"
crates: ["workspace", "foundation_wireguard (manifest only)", "foundation_ai", "patches/*"]
catalogue_rows: [20, 21]
---

# Feature 54: `[patch.crates-io]` and `fullfp16`

_Written in ASD-STE100 Simplified Technical English (STE). Code and identifiers stay as they are. Follow [requirements.md](../../requirements.md) sections 3, 4 and 8._

Note: `foundation_wireguard` is a lane A crate; this feature may change **only its
`Cargo.toml` dependency line** for boringtun. Tell agent A (through the main session) when it
lands.

## Problem

The root `Cargo.toml` has `[patch.crates-io]` for `gemm-common` (AVX2 check, CI-only
crash under Docker Desktop emulation) and `boringtun` (adds a wasm32 clock so
`foundation_wireguard` compiles for wasm). A patch section applies only inside this
workspace. A user of the **published** `foundation_wireguard` gets upstream boringtun and
cannot build it for wasm32; a user of `foundation_ai` gets the unpatched gemm.

## Design

1. **boringtun**: publishable fork. Rename the vendored crate to `foundation_boringtun`
   (package name; keep the lib name `boringtun` with `[lib] name = "boringtun"` so source
   code does not change), move it to `backends/foundation_boringtun` (keep the BSD-3 licence
   file and add a NOTICE that says what changed: the wasm sleepyinstant backend), add it to the
   workspace, and make `foundation_wireguard` depend on
   `foundation_boringtun = { path, version }`. Remove its `[patch]` line. Keep upstream
   tests that the vendored crate had; do not add `#[ignore]`s (the boringtun copy has bare
   `#[ignore]`s on integration tests from upstream — leave them, but list them in the PR).
2. **gemm-common**: it is a transitive dependency of candle, so a rename is not possible
   without forking candle. Keep the patch for the workspace. Add a backlog entry: published
   `foundation_ai` on a CPU with FMA but without AVX2 (emulation) panics in gemm; upstream
   fix needed (write the one-line upstream patch in the entry). Write the patch as
   `patches/gemm-common/UPSTREAM.md` ready to send.
3. **fp16** (backlog §10): `.cargo/config.toml` sets `-C target-feature=+fullfp16` for
   `aarch64-apple-darwin`. Check whether `+fp16` gives the same instructions
   (`vfmaq_laneq_f16`): on aarch64, `fullfp16` is the LLVM name and rustc's stable name is
   `fp16`, which implies full FP16 arithmetic. Change to `+fp16` if the macOS CI build of
   foundation_ai candle tests still assembles; otherwise keep and document. This needs the macOS
   job; if it cannot run, change it and report that only the macOS CI can confirm.

## Verify

```bash
export CARGO_PROFILE_DEV_DEBUG=0 CARGO_INCREMENTAL=0
cargo package -p foundation_boringtun --list --allow-dirty | head
cargo check -p foundation_wireguard --target wasm32-unknown-unknown --profile dev
python3 scripts/ci/ci.py run foundation_wireguard --profile dev
grep -n 'boringtun' Cargo.toml   # no [patch] line
```

## Done when

- [ ] boringtun fork is a workspace crate that publishes; wireguard uses it; gemm entry and
      upstream note written; fp16 decided. CI green.
