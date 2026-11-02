# Feature 22: learnings

## E1. Per-worktree target directories

- `scripts/ci/ci.py` already spread a package environment over every step in
  `run_package`. Setting `CARGO_TARGET_DIR` there is the one place that covers
  every step and every worktree.
- The worktree name is the basename of the repository root (`ROOT.name`). A
  worktree checkout has its own directory name, so the target directory stays
  separate.
- Do not use `RUSTFLAGS=-Cmetadata=<id>`. That rebuilds all dependencies.

## E2. Delete `crates/ewe_deployables`

- The decision check is `rg -n "use ewe_deployables|ewe_deployables::" --glob
  '*.rs'` outside the crate. It returns zero hits. No `Cargo.toml` depends on
  it. The only references are specification documents and the backlog entry.
- The per-provider deployment crates (`foundation_deployment_cloudflare`,
  `foundation_deployment_docker`) cover the old crate, so deletion is correct.
- The `.agents/AGENTS.md` doc note for E1 is left uncommitted in the `.agents`
  submodule. That submodule shares its working tree with rules 11 and 13 that
  the main session is still drafting, so this pass does not commit there.

## E5. Gate the `Bindgen` command binding

- The warning came from binding `args` in the match arm and using it only in
  the `#[cfg(feature = "wasm-bindgen-test")]` branch. A `run_bindgen_command`
  helper, one per cfg arm, removes the warning: the `wasm-bindgen-test` arm
  uses `args`, the other arm takes `_args`.
- cargo nightly 1.100.0 surfaces a new `unused_dependencies` manifest lint.
  `foundation_testbed` declares `deno_v8` only to enable its V8 backend for
  `deno_core` (feature unification), so the lint flags it. This is not the E5
  warning; it is recorded in the backlog.

## E6. A separate mise environment for the wasm tools

- mise environment files overlay the base `mise.toml`; they do not replace it.
  Pinning only deno and wasm-bindgen in `mise.wasm.toml` is not enough, because
  the base `[tools]` list is still merged in. The isolation comes from
  `[settings] auto_install = false` plus an explicit `mise install deno
  wasm-bindgen` in the task body.
- Verified: `MISE_ENV=wasm mise config get settings.auto_install` returns
  `false`; the two wasm tasks appear only under `MISE_ENV=wasm`; the base
  `mise tasks` no longer lists them.

## E7. Portable SignalR framing in `core::notifications`

- The four frame helpers lived in `server/signalr_do.rs`, a
  `#[cfg(target_family = "wasm")]` module, so their inline tests never ran on a
  native host. Rule 7 forces a move to `tests/`, and the helpers must become
  public in the portable `core::notifications` module to be reachable.
- The backlog said `rmpv` is unused on wasm. That is stale: the crate depends on
  `rmp-serde`, and `core::notifications` uses it on wasm too. No MessagePack
  dependency moves.
- Moving the test exposed a real bug: `decode_signalr_frame` called
  `rmp_serde::from_slice::<u8>(payload)`, but the payload is a MessagePack array
  `[msg_type, ..]`, not a scalar, so the decode always failed. Fixed by decoding
  the payload as `serde_json::Value` and reading the first array element. The
  old test had been wasm-gated and never ran, so the bug shipped silently.
- The move also made the helpers reachable by the native clippy gate for the
  first time. That surfaced 13 pedantic warnings in the new code (`doc_markdown`
  backticks, `must_use_candidate`, `cast_possible_truncation`). Fixed with a
  follow-up commit: backticks in the doc comments, `#[must_use]` on the three
  pure helpers, and `try_from` in place of the three truncating `as` casts. The
  wasm CI step runs `cargo check`, not clippy, so the old wasm-gated copy was
  never linted.

## E8. Drop the unused `worker::Error` import

- The inline test in `durable_object.rs` stays in place. It constructs a
  `worker::WebSocket` with `std::mem::zeroed()`, which only works inside the
  Workers runtime, so it cannot move to a native `tests/` file.
- The wasm check also surfaces five `unused_dependencies` manifest lints in the
  same crate (`base85rs`, `sha2`, `foundation_errstacks`, `foundation_netio`
  twice). They are not the `worker::Error` warning: `base85rs` and `sha2` are
  used nowhere, and the other three are used only in native-gated modules.
  Recorded in the backlog; fixing them is out of E8 scope.

## E4. Wasm runtime tests in CI

- The library-only wasm gate (`cargo check --target wasm32 -p <crate>`) never
  compiled the `wasm_bindgen_test` suites. They live in wasm32-only test targets
  and in a standalone crate outside the root workspace. `cargo check --tests
  --target wasm32-unknown-unknown` is the check that compiles them.
- The wasm test code rotted under that gate:
  - foundation_netio: `WasmSseIterator` moved to `wasm::client::stream`,
    `error_with_e` now returns `()`, and the `Stream` match is non-exhaustive.
  - foundation_wireguard: `JsDevice::poll` was removed (the smoltcp `Device`
    trait's `receive` replaces it), and `valtron::is_pool_initialized` is
    `valtron::single::is_initialized` on wasm.
  - foundation_testbed forced `foundation_core/multi` on every target. `multi`
    is native-only (its `idle_wake` module is `cfg(not(target_family = "wasm"))`),
    so it broke every wasm test consumer. Moving it to a
    `cfg(not(target_family = "wasm"))` target table fixes it.
- boringtun 0.7.1 upstream has no wasm32 sleepyinstant backend (only unix and
  windows), so foundation_wireguard's wasm tests did not compile. The Cargo.toml
  comment claimed a patch that did not exist. A vendored patch
  (`patches/boringtun`) adds a wall-clock `Instant` on `js_sys::Date::now()`.
  boringtun uses `Instant` only for handshake timers, RTT and rate limiting, so
  a wall clock is sound for the tests; the wrapped `duration_since` clamps a
  negative difference to zero. The patch also silences boringtun's eight
  upstream warnings (deprecated ring/base64 calls, an unused `KeyBytes`, an
  elided-lifetime nit).
- The E6 image regression: the CI image's `version_checks` reads the main
  `mise.toml [tools]` and fails when `wasm-bindgen` is absent. E6 moved the tool
  to `mise.wasm.toml` only, so the image build broke. The pin is restored to
  `mise.toml [tools]`; the wasm env still overlays its own copy, and
  `auto_install = false` still isolates the wasm tasks.
- foundation_core's `stream_future` test module is native-only (its three inner
  modules are all `cfg(not(target_family = "wasm"))`), but `tests/valtron/mod.rs`
  did not gate the module itself, so its top-level imports and waker helpers
  became dead code on wasm. Gating the module removes four warnings.
- foundation_reactor's `Poll::rearm_readable` is dead code on wasm (its only
  call site is a native poll/kqueue path). Recorded in the backlog; not fixed
  here because foundation_reactor is outside feature 22's crates.

## E3. Split the Cloudflare groups into per-group crates

- The split keeps every caller's path. The umbrella shim emits
  `pub mod <group> { pub use foundation_cloudflare_<group>::<group>::*; }`, not
  a flat `pub use foundation_cloudflare_<group> as <group>`. A flat alias would
  turn `generated::zones::Zones0GetArgs` into `generated::zones::zones::Zones0GetArgs`.
- Each group crate has a local `shared` module in `src/lib.rs` that re-exports
  `foundation_cloudflare_types::generated::*` and
  `foundation_deployment::providers::common::*`. The generated group body is
  unchanged: its `super::shared` resolves to the crate root, so the shared
  content does not move into every group file.
- The `JsonHash` derive expands to `use sha2::{Digest, Sha256}` and
  `use base85rs::encode`. Any crate that derives it needs `sha2`, `base85rs`
  and `serde_json` as dependencies. After the split, only the types crate and
  the group crates derive it, so the umbrella drops those two deps.
- The types crate must stay a leaf. `foundation_deployment` depends on
  `foundation_db`, which depends on `foundation_cloudflare_types`; making the
  types crate depend on `foundation_deployment` would cycle. The group crates
  sit above both, so their local `shared` module is the seam.
- The umbrella depends on the types crate as an optional dependency, and the
  base `cloudflare` feature lists `dep:foundation_cloudflare_types`. A wasm
  `workers`-only build never compiles the shim, so a non-optional dep would
  surface as an `unused_dependencies` lint there.
- `check()` stages split output in a scratch dir (`groups/`, `shared_crate/`)
  and merges prefixed trees against the committed tree. The committed group
  scan must exclude the shared types crate by path, because its
  `foundation_cloudflare_` prefix collides with the group-crate prefix.
- A broken generated manifest is a chicken-and-egg: cargo cannot build the
  generator to fix a manifest it cannot parse. One manual repair is enough;
  after it, the generator reproduces the correct manifest idempotently
  (`genapi generate cloudflare --check` reports "output matches").
- The build script renderer must emit `.split_groups(..)` before `.crate_dir(..)`
  with no trailing semicolon. Emitting it after a terminated chain produces a
  parse error (`expected expression, found .`).

