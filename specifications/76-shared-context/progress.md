# Spec 76 progress

_Written in ASD-STE100 Simplified Technical English (STE). Code and identifiers stay as they
are._

**Abbreviations in this file.** pull request (PR); AeroSpace and Defence Industries
Association of Europe (ASD), the owner of the STE standard ASD-STE100.

## Landed

- 2026-11-09: the spec, with six features. The owner chose a new crate (`foundation_context`),
  the name `RpcCtx`, and one PR per feature.

- 2026-11-09: feature 01. The new crate `backends/foundation_context` owns `Clock`,
  `MonotonicClock`, `Config`, `MinimumElement` and `Ctx`. `foundation_core::valtron`
  re-exports them. `SimulatedClock` stays in `foundation_core`. The crate is in
  `ci/packages.toml`, and the time-source allowlist points at the new path.

- 2026-11-09: feature 02. `foundation_http::ContextBag` is gone. Every user takes
  `foundation_context::Ctx`, which `foundation_http::shared::context::Ctx` re-exports. `Ctx` has the
  new methods `build`, `store`, `get_cloned`, `with_empty_map`, `rebase`, `with_seed` and
  `try_host`. The map lock is an `RwLock`. The connectrpc `Ctx` is now `RpcCtx { ctx, request }`.
  Both generators emit `RpcCtx`. `Ctx::host()` seeds its RNG from the entropy backend of
  `foundation_compact` (issue #448). The owner closes #448.

- 2026-11-10: feature 02 is in PR #467. Review follow-up: `Ctx::new` is renamed `Ctx::fixed`,
  and the valtron pool constructors and macros seed the root RNG from the pool seed
  (`Ctx::with_seed`).

- 2026-11-10: the migration order for features 03 to 05 is top-down waves (requirements
  section 3.1). Wave 1 starts: three lanes on haiku.

- 2026-11-10: wave 1, small crates lane (branch `spec76/w1-small-crates`). The five crates
  `foundation_repl`, `foundation_testing`, `foundation_toolings`, `foundation_testbed` and
  `foundation_cronjobs` take `Ctx` instead of `Arc<dyn Clock>`. Each crate reads its clock
  from the `Ctx`. Callers in `bin/platform` and `apps/answerme-agent` pass `Ctx::host()`.
  Test status: repl and testing unit and integration tests pass. Toolings, testbed and
  cronjobs test suites pass. Clippy is not run yet for testing, toolings, testbed and cronjobs.
  Testing doctests failed with a full disk, so they are not re-run yet. `ewe_platform` and
  `answerme-agent` do not build here, because the llama.cpp submodule is not checked out.

- 2026-11-10: wave 1, foundation_ai lane: 124 `dyn Clock` sites in `src` now take `Ctx`.
  answerme-agent, bin/platform, cf-agent-loop, cf-cloud-provider and the foundation_toolings
  cf_examples test follow. 501 tests pass (493 unit and integration, 8 doctests). clippy
  `-D warnings` is clean.
- 2026-11-10: wave 1, small-crates lane: foundation_repl, foundation_testing,
  foundation_toolings, foundation_testbed and foundation_cronjobs take `Ctx`.

- 2026-11-10: wave 1, platform, proxy and keychain lane (branch
  `spec76/w1-platform-proxy-keychain`). Feature 04 recipe, applied to three crates:
  - `foundation_proxy`: 32 `dyn Clock` sites in `src` are now 0. The state, the server,
    the state store, ACME, TLS, health probes, UDP passthrough and the H3 relay take `Ctx`.
    Tests pass: 35 test binaries. clippy `-D warnings` is clean. A new test,
    `the_stored_cert_time_follows_the_ctx_clock`, passes. Commit `8e9d62b4f`.
  - `foundation_keychain`: 8 sites are now 0. `KeychainContext` holds a `Ctx`, and its
    accessor is `ctx()`. Tests pass: 23 test binaries. clippy `-D warnings` over all
    targets is clean. A new test, `the_keychain_context_reads_its_clock_through_the_ctx_it_holds`,
    passes. Commit `f470b3e78`.
  - `foundation_platform`: 21 sites are now 0. `PlatformBuilder::ctx(Ctx)` replaces
    `clock(..)`. `PlatformSession::new` and `new_test` take a `Ctx`. The session no
    longer keeps a second clock. The `platform_test` macro in `foundation_macros` passes
    `Ctx::fixed`. Commit `7e70ac4af`. The crate does not compile here (see In progress).
  - Issue #469 is fixed. The three platform examples call `.ctx(host_ctx())`, and the
    `platform_run!` doc example calls `.ctx(Ctx::host())`. Commit `73f2616fc`.
  - The CI scripts `check_time_sources.py` and `check_executor_wake.py` pass.

- 2026-11-10: wave 2, lane `spec76/w2` (local branch, not pushed). Five crates take `Ctx`
  instead of `dyn Clock`:
  - `foundation_deployment_platform`: commits `af53fb46e`, merge `dd974b4f8`, `d1ac0d926`
    and `765042e1e` (new test `winrm_clock_tests`). The `dyn Clock` sites in `src` drop
    from 81 to 2. The two kept sites are the `clock()` getters of `WinRM` and
    `BootstrapLogger`. Tests pass with default features and with `vms`. clippy
    `-D warnings` is clean for both.
  - `foundation_macros`: commit `9f07eeeba`. The `proxy!` doc example passes
    `Ctx::host()` to the async `start` call.
  - `foundation_browser`: commit `6ce32a37f`. The `dyn Clock` sites drop from 19 to 0.
    `LaunchConfig::ctx` replaces `clock`. A launch without a context returns
    `BrowserError::Launch` and no longer panics. `foundation_testbed` passes its `Ctx`
    directly. Tests pass. clippy is clean. The `browser-tests` feature compiles.
  - `foundation_deployment_cloudflare`: commit `6e23326f2`. The `dyn Clock` sites drop
    from 5 to 0. `CloudflareClient::clock()` is now `ctx()`. Default tests pass. clippy is
    clean. The wasm32 checks pass for the default and `workers` feature sets.
  - `foundation_wasmtime`: commit `5b825884d`. The WASI wall and monotonic clock wrappers
    hold a `Ctx`. The `dyn Clock` sites drop from 2 to 0. Tests pass. clippy is clean.
    CI has no entry for this crate, so only default features were run.
  - The CI scripts `check_time_sources.py` and `check_executor_wake.py` pass.
  - Nothing is pushed. The owner reviews the lane before any push.

- 2026-11-10: wave 3, lane `spec76/w3` (local branch). Four crates take `Ctx` instead of
  `dyn Clock`. Nothing was pushed by this lane:
  - `foundation_wireguard`: commit `2731a10a6`. The 15 `dyn Clock` sites in `src` drop to 0.
    `WgNode`, `TunnelDriver`, `RelayServer`, `MeshDataPlane`, `serve` and `serve_until` take
    `Ctx`. The mesh task now uses the node's `Ctx` (it used to reset the RNG to seed 0).
    Tests pass. The `spec55-complete` tests compile. clippy `-D warnings` is clean.
  - `foundation_connectrpc`: commit `bbd91ef7f`. `RequestContext` holds `pub ctx: Ctx`
    instead of a clock copy. `empty` and `for_dispatch` take a `Ctx`. Client streams, the
    dispatch path, the HTTP/1.1 server and the h2 transport store a `Ctx`. The only kept
    `dyn Clock` is the `RpcCtx::clock()` getter. Tests pass except
    `a_blackholed_dial_does_not_block_the_worker`, which also fails on master (#466).
    The `h3` set compiles. The wasm check passes with `-D warnings`.
  - `foundation_deployment`: commit `39c8c7a9f`. The 11 sites in `src` drop to 0 except
    the `ProviderClient::clock()` getter, which returns `self.ctx.clock()`. `ProviderClient`
    adds `ctx()`. The callers in `foundation_deployment_hetzner` and
    `foundation_deployment_docker` follow in the same commit. Hetzner's `HetznerClient`
    stores a `Ctx`. Its `from_env` takes `&Ctx`. Tests pass for `foundation_deployment` (lib,
    tests and doctests), for hetzner (all) and for docker (default and `docker` features, and
    `docker,integration-tests` compiles). clippy `-D warnings` is clean for
    `foundation_deployment` and docker. Docker `--all-features` cannot build here:
    `protoc` is missing for the `buildkit` build script.
  - `foundation_sshkit`: commit `c114f6b24`. `ConnectionPool` stores a `Ctx`. Its
    `clock()` getter returns `self.ctx.clock()`. `ctx()` returns `&Ctx`. Tests pass.
  - The CI scripts `check_time_sources.py` and `check_executor_wake.py` pass.
  - Allowed kept sites are the getters that return `self.ctx.clock()`, plus the
    `store_typed` call sites that pass `clock.as_ref()` to `foundation_db` (a later wave).

- Wave 4: `foundation_auth`: commit `6a2bc40ff`. `src` has 0 `dyn Clock` sites (it had 80).
  Every struct field now holds a `Ctx`. The helpers in `shared/clock.rs` take `&Ctx`.
  `AuthManager`, `JwtManager`, `JwksManager`, `SessionManager`, `IdpServer` and
  `IdpHandlerCore` take a `Ctx`. The callers in `foundation_keychain`,
  `foundation_connectrpc` (tests) and `examples/cf-login-app` follow in the same commit.
  Results:
  - Tests pass for the default set (lib and all test targets) and for `server-test`
    (245 integration tests).
  - clippy `-D warnings` is clean for the default set and for `server-test`.
  - `cargo check` passes for `server-native`, for wasm (`wasm` and `wasm-bindgen-session`),
    for `foundation_ai` (`testing`, and default backends) and for the `cf-login-app` wasm build.
  - `foundation_keychain` passes `cargo check --all-targets` and the wasm check.
  - `foundation_connectrpc --features auth` passes `cargo check --all-targets`.
  - The CI guards `check_time_sources.py` and `check_executor_wake.py` pass.

## In progress

- Wave 2: the `WinRM` and `BootstrapLogger` `clock()` getters still return `Arc<dyn Clock>`.
  Decide whether they should expose the `Ctx` only.
- Wave 2: the browser supervisor waits and the testbed browser runner sleep with
  `std::thread::sleep`. The deadline reads the `Ctx`, but the sleep does not. See learnings.
- Wave 1 small-crates lane: clippy, the remaining doctests, and the GitHub issues for the
  findings listed in the report.
- Wave 1 platform lane: `foundation_platform` does not compile in this environment. The
  `gdk-sys` build script needs the system library `gdk-3.0`, which is missing. The platform
  edits and the platform test edits are checked by reading only. Next: compile on a machine
  with GTK 3 installed, then run clippy and the platform tests, and fix any error.

## Next

- Wave 4 checks not run yet, because the disk filled: `cargo test -p foundation_keychain`
  (its `native_e2e` and `sends` targets failed to link), `cargo test -p foundation_connectrpc
  --features auth`, and clippy for `foundation_keychain` and `foundation_connectrpc`.
  The disk had 3.3 GB free after the routine. Free more space, then run them.

- Features 03, 04 and 05 after feature 02 is reviewed. They can run in parallel.
  Feature 06 is last.
