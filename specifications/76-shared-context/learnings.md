# Spec 76 learnings

_Written in ASD-STE100 Simplified Technical English (STE). Code and identifiers stay as they
are._

**Abbreviations in this file.** random number generator (RNG); Hypertext Transfer Protocol
(HTTP); AeroSpace and Defence Industries Association of Europe (ASD), the owner of the STE
standard ASD-STE100.

- valtron `Ctx` and `foundation_http::ContextBag` are the same idea: a type map, a clock and a
  shared `ChaCha8Rng`. Only `Ctx` has a config, and only the bag uses an `RwLock`. The bag
  also has an `unsafe impl Send/Sync` that the compiler does not need.
- `SimulatedClock` cannot move to `foundation_context`. Its `advance` must wake the parked
  executors (#305), and that wake is crate-private in `foundation_core`. A listener hook would
  let a caller build a simulated clock that does not wake them.
- `foundation_connectrpc::Ctx` is emitted by two generators (`foundation_macros` and
  `foundation_connectrpc_codegen`). The rename to `RpcCtx` must change both and regenerate.
- About 600 `dyn Clock` sites exist outside tests (2026-11-09). `foundation_ai` (124),
  `foundation_deployment_platform` (81), `foundation_auth` (80) and `foundation_nativeapis`
  (73) are the largest.
- The master review on 2026-11-09 found more parallel time traits: `TimeSource` in
  `foundation_nostd` (`primitives/wait_duration.rs`), `foundation_wasm` (`wasi_host.rs`) and
  `foundation_compact` (`ids/generator.rs`, with `RandSource`), `DeadlineTimer` in connectrpc
  and `ClockSource` in foundation_browser. The crates below `foundation_context` cannot take a
  `Ctx`, so this spec does not remove them. Issue #459 tracks the `std::time::Instant` use in
  two of them.
- `Ctx::host()` seeds its RNG with `0` (#448). Feature 02 fixes it, because it changes the
  `Ctx` constructors.
- About 27 direct randomness reads bypass `Ctx` (#453). That is a separate change: this
  spec moves the clock, and `ctx.rng()` is already the place those reads must use.
- Feature 01: `SimulatedClock::new` reads `Instant::now()` for its base. That read stayed in
  `foundation_core/src/valtron/ctx.rs`, so the time-source allowlist needs an entry for that
  file, next to the new entry for `foundation_context/src/lib.rs`.
- Feature 01: `cargo fmt --all` rewrites files in other crates (`foundation_connectrpc_codegen`
  `generator.rs`, `foundation_openapi` `unified/emit.rs`). Those files were already not
  formatted. Revert them, or format only the crate you changed.
- Feature 01: `foundation_context` has no default features. Its wasm build compiles with no
  time backend chosen, so each consumer must forward `wasm-bindgen` or `host-abi`.
- Feature 02: a text replace of `Ctx` is not safe in the connectrpc, proxy and docker crates.
  They name both the RPC `Ctx` and the valtron `Ctx` (for example `use
  foundation_core::valtron::{self, Ctx, ..}` in `transport/h2.rs` and `h3.rs`). Rename the RPC
  type first, then check each valtron import by hand.
- Feature 02: `cargo fix` removes `use std::sync::Arc;` when no code uses it in the checked
  feature set. `foundation_http/src/native/serve.rs` needs `Arc` only with the `quic` feature, so
  that import now has a `cfg`. Check each crate with `--all-features` after `cargo fix`.
- Feature 02: `cargo clippy -p <crate> -- -D warnings` also lints workspace dependencies. Today
  `foundation_reactor` fails it, so read only the warnings that name the changed crate.
- Feature 02: `Ctx::host()` now reads entropy. It panics if the entropy source fails. A host that
  must not panic calls `Ctx::try_host()`. `PlatformSession` does this and keeps its warning.
- Feature 02: `Ctx` has no pointer-equality method. A test that proved "the app stores the
  caller's context" with `Arc::ptr_eq` now stores a value through one handle and reads it from the
  other, because a `Ctx` clone shares the map.
- Review on PR #467 (owner): `Ctx::new(clock)` hid a fixed seed `0` behind a neutral name, so a
  production caller got a stream that every host shares. It is now `Ctx::fixed(clock)`, so the
  name says what it does. `Ctx::with_seed` takes a chosen seed, and `Ctx::host` seeds from
  entropy.
- The valtron pool constructors take a seed (`initialize_pool(seed, ..)`,
  `#[valtron_test(seed = ..)]`), but their default root used `Ctx::host()`, so `ctx.rng()`
  ignored that seed. The default root is now `Ctx::with_seed(real clock, seed)` (the
  crate-private `seeded_root`), in the executors and in the macro expansion. The macro binds the
  seed once, because its default seed expression draws a new value each time it runs.
- Restoring files with `tar xf` keeps their old modification times, so cargo does not rebuild
  them. Touch the files after a restore.
- Wave 1 small crates: a worktree `target` grows by about 12 GB during one lane. The disk
  filled to 100% during `cargo test` doctests (the `foundation_testing` doc link failed with
  "No space left on device"). Run `df -h /` before each cargo command. Remove the build output of
  finished owned crates in `target/uat/build/` first.
- Wave 1: `DurationSleeper`, `BinaryRunnerTask::new`, the harness constructors and `DevService`
  take `Ctx` by value. Private `wait_for_*` helpers still take `&dyn Clock`, because they only
  read the clock.
- Wave 1: `foundation_toolings`'s `tests/sse_reload_tests.rs` uses the `foundation_http` `Ctx`,
  not the `foundation_context` one. Its `MonotonicClock` use is a test-only host clock, so it was
  left unchanged.
- Wave 1: `ewe_platform` and `answerme-agent` depend on `infrastructure_llama_bindings`. That build
  script needs the `tools/llama.cpp` submodule. The submodule is not checked out in this
  worktree, so these crates cannot be checked here.
- Feature 03: `cargo check --all-targets` is the fastest check of the test build. Cargo stops at
  the first failing test target, so re-run it after each batch of fixes.
- Feature 03: `examples/*` is excluded from the workspace. Check each one by name. The two cf
  examples are wasm crates. Check them with `--target wasm32-unknown-unknown`.
- Feature 03: a full `cargo test -p foundation_ai` run used about 4 GB of the shared target and
  filled the disk. Check `df -h /` before each run.

- Wave 1 platform, proxy and keychain lane: a full disk fails a link step with
  `rustc-LLVM ERROR: IO failure on output stream: No space left on device`. The cargo
  summary shows it as `linking with clang failed, signal`. Read the full log, not the summary,
  when a test link fails.
- Wave 1 platform, proxy and keychain lane: `foundation_http::shared::context::Ctx` is a
  re-export of `foundation_context::Ctx`. The proxy handlers and the H3 relay already held the
  right type, so they needed no type change. Only the clock reads changed.
- Wave 1 platform, proxy and keychain lane: `foundation_platform` cannot compile in this
  environment. Tauri is a hard dependency, and the `gdk-sys` build script fails without the
  system library `gdk-3.0`. `cargo check --lib` stops in that build script. Check platform
  changes on a machine with GTK 3 installed. Read the diff until then.
- Wave 1 platform, proxy and keychain lane: `PlatformSession` kept a second clock field
  beside its `Ctx`. Removing it made the session read the clock from one place only. The
  session needed no other change.
- Wave 1 platform, proxy and keychain lane: the entropy fallback in `PlatformSession`
  (warn, then use a fixed RNG) depended on a clock that the library may not build. The
  fallback moved to the host entry points (the three examples), as issue #469 asked. The
  library now takes a `Ctx` only.
- Wave 1 platform, proxy and keychain lane: tests that used `new_test` with a real clock
  now pass `Ctx::fixed`. `Ctx::fixed` keeps the seed-0 RNG that `new_test` used before.
  Tests that build a service with a real clock pass `Ctx::host()`.
- Wave 1 platform, proxy and keychain lane: the shell guard in this worktree refuses one
  compound command that contains `cd` and a heredoc. Split such commands into single calls.
- Wave 1 platform, proxy and keychain lane: `foundation_macros/src/platform_test.rs`
  generates calls to `PlatformSession::new_test`. A signature change in `foundation_platform`
  also changes the generated code. Search the macro crates for generated calls, not only
  for direct calls.

- Wave 2: a `SimulatedClock` that never advances makes a deadline loop run forever. The
  browser wait helpers (`wait_for_port`, `discover_ws_url` and `connect_with_retry`) read
  their deadline from the `Ctx`, but they pause with `std::thread::sleep`. A test with a frozen
  simulated clock would hang. The fix is to pause through the injected sleeper. Severity
  medium. Not filed as an issue, per the coordinator.
- Wave 2: `foundation_testbed` calls `BrowserDriver::launch_borrowed`. An API change in one
  crate can break a caller outside it. Run `cargo check --all-targets` on each caller.
- Wave 2: `BootstrapLogger::new` writes into the current directory (`.testbed/`). A test that
  builds one writes files. The deployment platform clock test uses `WinRM` instead, because
  `WinRM::new` touches no file.
- Wave 2: clippy `doc_markdown` rejects a bare type name such as `WinRM` in a doc comment.
  Put type names in backticks.
- Wave 2: clippy `must_use_candidate` flags a new `pub fn new` that returns `Self`. Add
  `#[must_use]`.
- Wave 2: rustfmt reflows lines after an argument rename. Run rustfmt on the changed files
  only, then check that the diff stays small.
- Wave 2: the shell guard refused several compound commands in this worktree. One refused
  `sed` chain did not run, so an edit was missing. Check each edit with `grep` after a
  refused command.
- Wave 2: `foundation_deployment_cloudflare` takes about 5 minutes for a native check at
  `-j 1`. Run it in the background with a log file, not in the foreground.
- Wave 2: the disk filled again during a `vms` link step. Removing `uat/build/<crate>` for each
  finished crate freed enough space.

- Wave 3: the grep `dyn Clock` misses the full path `dyn foundation_core::valtron::Clock`.
  Use `grep -rn "dyn .*Clock"` to find every site. connectrpc had 8 short sites and more
  full-path sites.
- Wave 3: the connectrpc `Ctx` name is not a clash. `foundation_http::shared::context::Ctx`
  re-exports `foundation_context::Ctx`, so it is the same type.
- Wave 3: clippy `needless_pass_by_value` rejects a `Ctx` parameter that a function only
  clones. Take `&Ctx` (hetzner `from_env`).
- Wave 3: `foundation_wireguard::WgNode::join` built `Ctx::fixed(clock)` for the mesh task.
  That reset the RNG to seed 0 in library code. It now uses the node's `Ctx`. Severity
  medium. Fixed in `2731a10a6`.
- Wave 3: `foundation_deployment_docker` with `--all-features` needs `protoc` for its
  `buildkit` build script. The sandbox has no `protoc`, so that clippy set cannot run here.
  Run it on a machine with `protoc`, or install `protoc` first.
- Wave 3: `foundation_deployment_hetzner` tests pass `&MonotonicClock` to `store_typed`.
  That is a `foundation_db` API (wave 7) and still takes `&dyn Clock`, so the call stays.
- Wave 3: connectrpc `h2_dial_tests::a_blackholed_dial_does_not_block_the_worker` fails on
  master in this sandbox (#466). It is not caused by the `Ctx` change.
- Wave 3: `foundation_wireguard` completeness check built a `RelayServer` with a real
  clock in library code. It now checks the constructor signature with a typed function
  pointer, which needs no clock.
- Wave 3: the shell guard refuses one command that combines `cd` with a heredoc or a
  redirect. Use plain commands, or a script file run by a plain command.

- Wave 4: `require_auth(ctx: AuthContext, clock)` already had a parameter named `ctx`. A
  mechanical rename to `ctx: &Ctx` would give two parameters one name. The auth context
  parameter is now `auth`. Before a rename, check the original file with `git grep -w ctx`.
- Wave 4: `IdpServer::http_app`, `serve_adapter::run_sync` and the `Serve` path built
  `Ctx::fixed(clock)` inside library code. That reset the RNG to seed 0 and broke the rule
  that only tests use `Ctx::fixed`. Severity medium. Both now reuse the `Ctx` that the
  server stores. The dispatch reads no map entries, so the behavior does not change.
- Wave 4: `cargo fmt` from the workspace formats the crate with the workspace style. Running
  `rustfmt` on `examples/cf-login-app/src/lib.rs` reordered its imports and reflowed code
  outside the change. Revert that noise. Edit the example by hand, or format it with its own
  toolchain setup.
- Wave 4: `examples/cf-login-app` is a separate workspace. It does not define the `uat` profile.
  Check it with the default dev profile and a shared `CARGO_TARGET_DIR`.
- Wave 4: the Bash tool writes its output under `/tmp`. When the root volume is full, the tool
  refuses to run and reports "Disk full", even for a plain `df`. The linker reported a bus
  error first, which was a symptom of the same full disk. Check `df -h /` before each build.
- Wave 4: removing the llama build objects (`uat/build/infrastructure_llama_bindings`) and the
  finished auth and connectrpc build outputs freed only about 3 GB. The routine's 4 GB floor
  was not reached. The `uat/deps` directory holds most of the space. The routine does not
  allow removing it, so the remaining checks wait for the owner.
