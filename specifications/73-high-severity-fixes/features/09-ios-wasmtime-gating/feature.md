---
feature: "Gate the wasmtime dependency so iOS targets build"
status: "pending"
priority: "high"
depends_on: []
crates: ["foundation_platform", "foundation_wasmtime"]
issues: [354]
model: "sonnet"
branch: "spec73/f09-ios-wasmtime-gating"
---

# Feature 09: iOS wasmtime gating

_Written in ASD-STE100 Simplified Technical English (STE). Code and identifiers stay as they
are._

**Abbreviations in this file.** WebAssembly (Wasm); iOS (Apple's mobile operating system);
just-in-time compilation (JIT); write XOR execute (W^X, a memory protection rule); Continuous
Integration (CI); AeroSpace and Defence Industries Association of Europe (ASD), the owner of
the STE standard ASD-STE100.

## 1. Goal

Close [#354](https://github.com/ewe-studios/ewe_platform/issues/354): `foundation_platform`
depends on `foundation_wasmtime` without a target gate, so an iOS build fails.

## 2. Problem

**Yes, wasmtime does not support iOS.** The reasons:

1. wasmtime compiles Wasm to machine code with Cranelift. Cranelift is a JIT compiler. It
   writes code pages, then executes them in the same process.
2. iOS does not permit that. The platform enforces W^X for third-party applications and
   rejects runtime allocation of executable memory outside a signed code region. A JIT that
   writes then runs is refused by the operating system.
3. wasmtime does not ship an interpreter backend that would avoid the JIT. So there is no
   wasmtime configuration that runs on iOS.
4. Consequence: the dependency cannot build for an iOS target at all, and the whole crate
   fails, not only a call at runtime.

The live dependency is `backends/foundation_platform/Cargo.toml:50`:
`foundation_wasmtime = { path = "../foundation_wasmtime", version = "0.1.1" }`. It has no
`[target.'cfg(...)'.dependencies]` section. The crate is used in `src/codegen.rs`,
`src/wasmtime_responder.rs`, `src/lib.rs` and `src/session.rs`.

## 3. Requirements

1. An iOS target builds `foundation_platform` without `foundation_wasmtime`.
2. The Wasm feature stays available on macOS, Linux and Windows, unchanged.
3. Code that uses `foundation_wasmtime` is behind the same gate as the dependency. A missing
   feature gives a typed, documented error, not a link error.
4. Choose one gate: a `cfg(target_os = ...)` expression or an optional Cargo feature. State
   the choice and why. An optional feature is preferable: it keeps the Wasm engine out of
   builds that do not need it, and the gate is testable on the host.
5. Record the alternative for readers: an interpreter runtime, for example `wasmi`, is the
   only way to run Wasm on iOS.
6. Apply the version-bump rule for any public API change.

## 4. Design

Complete before handoff:

1. Read the four files that use the crate. Name every public item that changes shape when the
   gate is off.
2. Choose the gate. Write the exact `Cargo.toml` section and the exact `#[cfg]` conditions.
3. State the error a caller gets on iOS: a variant of the existing platform error type, with
   the message naming the missing engine and the reason.
4. State the CI check that proves the fix: `cargo check` for an iOS target on a virtual
   machine, or `cargo metadata` plus a target-specific `cargo check --target
   aarch64-apple-ios` on a macOS host when the SDK is installed.

## 5. Tasks

- [ ] Verify the failure on master: `cargo check -p foundation_platform --target
      aarch64-apple-ios` (record the toolchain and SDK state).
- [ ] Fill section 4.
- [ ] Add the gate.
- [ ] Gate the code paths.
- [ ] Add the typed error.
- [ ] Add the check to CI or the platform VM workflow.
- [ ] Apply the version-bump rule.
- [ ] Run the verify commands. Record the results.
- [ ] Close #354 in the PR body.

## 6. Tests

1. `an_ios_target_builds_without_the_wasm_engine`: a CI step, recorded in the PR.
2. `the_wasm_engine_is_absent_on_ios`: a `cargo metadata` check that the dependency is not in
   the iOS target's graph.
3. Host tests for the typed error: when the feature is off, the Wasm entry point returns the
   error, and the message names the reason.

## 7. Verify

```bash
export CARGO_PROFILE_DEV_DEBUG=0 CARGO_INCREMENTAL=0
cargo fmt -p foundation_platform
cargo clippy --profile uat -p foundation_platform --no-deps --all-targets -- -D warnings
cargo test --profile uat -p foundation_platform
cargo check -p foundation_platform --target aarch64-apple-ios
cargo tree -p foundation_platform --target aarch64-apple-ios | grep -c foundation_wasmtime  # must print 0
```

## 8. Done when

- An iOS target builds `foundation_platform`.
- The dependency is absent from the iOS target's graph.
- Host builds keep the Wasm engine.
- The iOS alternative (an interpreter runtime) is documented.
- #354 is closed by the PR.
