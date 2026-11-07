---
feature: "Docker ContainerConfig requires its clock and does not fall back to fork_ctx"
status: "in-review"
priority: "medium"
depends_on: []
crates: ["foundation_deployment_platform", "foundation_macros"]
issues: [220]
model: "sonnet"
branch: "spec72/f04-docker-container-clock"
---

# Feature 04: Docker container clock

_Written in ASD-STE100 Simplified Technical English (STE). Code and identifiers stay as they
are._

**Abbreviations in this file.** AeroSpace and Defence Industries Association of Europe (ASD),
the owner of the STE standard ASD-STE100.

## 1. Goal

Remove the `fork_ctx().clock()` fallback from `ContainerConfig` and let the
`#[docker_container]` macro pass a clock. Close issue
[#220](https://github.com/ewe-studios/ewe_platform/issues/220).

## 2. Problem

`backends/foundation_deployment_platform/src/docker/container.rs`: a `ContainerConfig` built
without an explicit clock falls back to `fork_ctx().clock()`. That panics when no multi pool is
initialized. The `#[docker_container]` macro cannot pass a clock. A test that uses the macro
outside a pool panics instead of failing with a typed error.

## 3. Requirements

1. A missing clock is a typed error (root rule 11). Never a silent fallback to the real clock
   and never a panic.
2. `ContainerConfig` takes its clock from the `Ctx` bundle of the caller.
3. `#[docker_container]` accepts an optional `clock = <expr>` argument, in the same style as
   the `#[platform_test]` item.
4. The macro without the argument behaves as today for host code, but it resolves the clock
   from the pool context it already receives. If it has no context, it returns the typed
   error.
5. This is a breaking change for callers that relied on the fallback. Record it and update
   every caller in the workspace.

## 4. Design

- `ContainerConfig` keeps `clock: Option<Arc<dyn Clock>>` with its existing `.clock()` builder
  and gains `resolve_clock()`. A required constructor parameter would remove the only path
  that produces the typed missing-clock error.
- `start_async` resolves the clock before any I/O and returns `DockerError::MissingClock` when
  it is absent. There is no `Env` bundle and no `ContainerConfigError`; `Ctx` is the bundle.
- The macro generates `clock = <expr>` when the attribute gives it, and `pool_clock()`
  otherwise. A function that returns `DockerResult<()>` receives the typed error; a plain
  `()` function logs a skip and returns, so neither path panics.

## 5. Tasks

- [x] Read `container.rs`, the macro crate code, and every `ContainerConfig` caller.
- [x] Make the clock required. Remove the `fork_ctx` fallback.
- [x] Add the macro argument.
- [x] Update the callers and the macro tests.
- [x] Add the regression tests.
- [x] Run the verify commands. Record the results.
- [x] Close #220 in the PR body.

## 6. Tests

In `backends/foundation_deployment_platform/tests/` and the macro crate's tests:

1. `a_container_config_without_a_clock_is_a_typed_error`: run the macro outside a pool and
   assert the error. On the old code the call panics.
2. `a_container_config_uses_the_injected_clock`: build with a simulated clock and assert the
   timestamps come from it.
3. `the_macro_accepts_a_clock_argument`: compile and run a macro with `clock = expr`.

## 7. Verify

```bash
export CARGO_PROFILE_DEV_DEBUG=0 CARGO_INCREMENTAL=0
cargo fmt -p foundation_deployment_platform -p foundation_macros
cargo clippy --profile uat -p foundation_deployment_platform -p foundation_macros --all-targets -- -D warnings
cargo test --profile uat -p foundation_deployment_platform
cargo test --profile uat -p foundation_macros
```

## 8. Done when

- No `fork_ctx` clock fallback remains in `container.rs`.
- The macro accepts a clock and the no-pool path is a typed error.
- Every workspace caller compiles and passes.
- Issue #220 is closed by the PR.
- `progress.md` and `learnings.md` record the change and the breaking change.
