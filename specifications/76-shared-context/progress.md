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

## In progress

- Wave 1 small-crates lane: clippy, the remaining doctests, and the GitHub issues for the
  findings listed in the report.
- Wave 1 platform lane: `foundation_platform` does not compile in this environment. The
  `gdk-sys` build script needs the system library `gdk-3.0`, which is missing. The platform
  edits and the platform test edits are checked by reading only. Next: compile on a machine
  with GTK 3 installed, then run clippy and the platform tests, and fix any error.

## Next

- Features 03, 04 and 05 after feature 02 is reviewed. They can run in parallel.
  Feature 06 is last.
