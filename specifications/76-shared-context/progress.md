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

## In progress

- Wave 1 small-crates lane: clippy, the remaining doctests, and the GitHub issues for the
  findings listed in the report.

## Next

- Features 03, 04 and 05 after feature 02 is reviewed. They can run in parallel.
  Feature 06 is last.
