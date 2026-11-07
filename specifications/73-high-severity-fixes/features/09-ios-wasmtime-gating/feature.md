---
feature: "Gate the wasmtime dependency so iOS targets build"
status: "completed"
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
3. Code that uses `foundation_wasmtime` is behind the same gate as the dependency. On an
   unsupported target the Wasm items do not exist, so a caller gets a compile error that names
   them, not a link error. The crate documents this rule and the alternative.
4. Record the alternative for readers: an interpreter runtime, for example `wasmi`, is the
   only way to run Wasm on iOS.
5. No version bump unless an item changes on a target that builds today.

## 4. Design (completed 2026-11-07)

### 4.1 The gate

Use both an optional Cargo feature and a target condition. The feature keeps the engine out
of a build that does not want it. The target condition keeps it out of an iOS build even when
the feature is on, because a default feature cannot depend on the target.

`backends/foundation_platform/Cargo.toml`:

```toml
[features]
default = ["wasmtime"]
wasmtime = ["dep:foundation_wasmtime"]

[target.'cfg(not(any(target_os = "ios", target_os = "tvos", target_os = "watchos")))'.dependencies]
foundation_wasmtime = { path = "../foundation_wasmtime", version = "0.1.1", optional = true }
```

Delete the plain `foundation_wasmtime` line at `Cargo.toml:50`.

The one condition to repeat in code:

```rust
#[cfg(all(feature = "wasmtime", not(any(target_os = "ios", target_os = "tvos", target_os = "watchos"))))]
```

Why tvOS and watchOS: they are the same class of target. They also forbid executable memory
after signing, so wasmtime cannot build for them for the same reason. Measure each with
`cargo metadata --filter-platform <target>`. If a target builds today, keep it out of the
gate and state that.

### 4.2 The gated code

1. `src/lib.rs:29` `mod wasmtime_responder;` and `src/lib.rs:60`
   `pub use wasmtime_responder::{wasmtime_app, WasmtimeResponder, WasmtimeShell};` go under
   the condition.
2. `src/session.rs:128` field `wasmtime_shell_`, `src/session.rs:171` in the constructor,
   `PlatformSession::register_wasmtime` and `PlatformSession::wasmtime_shell` go under the
   same condition.
3. `src/codegen.rs` stays unchanged. It only checks the app manifest for the string
   `foundation_wasmtime` and writes generated code as text. It never names a wasmtime type.
4. `src/wasmtime_responder.rs` stays as it is, behind the gate.

### 4.3 The error a caller gets

On iOS the items `WasmtimeResponder`, `WasmtimeShell`, `wasmtime_app`,
`PlatformSession::register_wasmtime` and `PlatformSession::wasmtime_shell` do not exist. An
app that names one gets a compile error at build time. This is better than a link error and
it costs no new API. Document the rule in the `lib.rs` module doc:

> The Wasm engine (wasmtime) compiles with a just-in-time compiler, which iOS, tvOS and
> watchOS refuse. `foundation_platform` keeps the engine out of those targets. An app that
> names the engine items does not compile for them. To run Wasm on iOS, use an interpreter
> runtime such as `wasmi`.

Do not add a runtime error type. No code path can produce it, because the entry points that
would return it cannot be called on those targets.

### 4.4 Version

No public item changes on macOS, Linux or Windows. The items are absent only on targets that
cannot build today. No version bump.

### 4.5 Proof

1. `cargo tree -p foundation_platform --target aarch64-apple-ios | grep -c foundation_wasmtime`
   prints 0.
2. `cargo check -p foundation_platform --target aarch64-apple-ios` passes when the iPhoneOS
   SDK is installed on the host. If it is not, record that fact and leave the CI proof to
   feature 10.
3. `cargo check -p foundation_platform --no-default-features` passes on the host and shows the
   gate works without the engine.
4. The host suite still passes with the engine.

### 4.6 Known limitation, not in scope

`WasmtimeResponder::respond` builds the instance but does not call the module's
`handle_request` export (the body comment says so). Filed as #371, severity medium. Do not fix
it here.

### 4.7 Result (2026-11-07)

Commit `94c4da99b`. Cargo section and cfg conditions as in 4.1 and 4.2. Verify: fmt 0, clippy
`-D warnings` 0, 194 tests passed and 6 ignored, `--no-default-features` check 0, and
`cargo tree --target aarch64-apple-ios | grep -c foundation_wasmtime` prints 0. The local iOS
`cargo check` stops at `objc2-exception-helper` because this Mac has no iPhoneOS SDK; the
failure moved past wasmtime, which is the proof the gate removed it. Feature 10 owns the iOS
build proof.

The work found #372 (high): `PlatformSession::get_responder` returns a `&dyn RouteResponder`
after its read guard drops, so a replacement responder frees the `Box` under it. Fixed on the
same branch in commit `db320c0fe`: the registry holds `Arc<dyn RouteResponder>` and
`get_responder` returns the `Arc`. `register_responder` keeps its `Box` signature and converts
with `Arc::from`. Test `a_replaced_responder_stays_alive_through_an_old_handle`. The crate is
0.0.4. The full suite passes 220 tests (6 ignored). The branch also adds the previously
untracked `examples/platform_ios/src-tauri/Cargo.lock`, to match the tracked Android example
lock.

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

1. `an_ios_target_builds_without_the_wasm_engine`: the `cargo tree` and `cargo check` commands
   in 4.5, recorded in the PR with their exact output.
2. `the_gate_works_without_the_engine_on_the_host`: `cargo check -p foundation_platform
   --no-default-features` passes on macOS. This pins the feature side of the gate on any host.
3. The host test suite stays green with the engine on. No new unit test is needed: the gate
   removes code, it does not add behaviour.

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

- An iOS target builds `foundation_platform`, or the missing SDK is recorded with the CI proof
  left to feature 10.
- `cargo tree --target aarch64-apple-ios` shows no `foundation_wasmtime`.
- Host builds keep the Wasm engine, and `--no-default-features` removes it.
- The iOS alternative (an interpreter runtime) is documented in the crate and the feature.
- #354 is closed by the PR.
