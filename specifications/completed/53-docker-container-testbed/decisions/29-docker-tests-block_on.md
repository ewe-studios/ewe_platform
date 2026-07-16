# 29 — Docker test `block_on` tokio runtime fix

**Date:** 2026-07-10
**Status:** Investigated — resolved by spec-54

## Problem

```
thread panicked at bollard/src/docker.rs:1703:
there is no reactor running, must be called from the context of a Tokio 1.x runtime
```

`foundation_proxy`'s Docker-backed tests under the `docker-tests` feature fail
with "no reactor running" when calling `ContainerHandle::start_async()`. Bollard
internally calls `tokio::time::timeout(...).await`, which checks
`Handle::try_current()`. If nothing set the runtime handle, it panics.

The identical test pattern works in `foundation_deployment_platform/tests/` and
`foundation_sshkit/tests/` — those crates' test binaries successfully run bollard
inside `RT.block_on()`.

## What was tried

1. **`RT.enter()` before `block_on`** — `enter()` sets the handle for the current
   thread, but does not propagate across threads or nested `block_on` calls.
   Failed immediately with the same error.

2. **`Runtime::new().block_on()` per call** — creates a fresh runtime each time.
   Failed immediately with the same error.

3. **Clean rebuild** — same result, ruling out incremental-compilation artifact.

4. **Identical LazyLock pattern** as the working `deployment_platform` tests —
   same error in the proxy test binary, confirming it's not a test-code issue.

## Root cause

The proxy test binary links tokio via a different dependency path than the
deployment_platform test binary. `foundation_proxy` depends on `tokio` as a
dev-dependency with `rt-multi-thread`. The proxy's dependency tree brings in
hyper/tokio/bollard through a chain that creates a subtle version or context
mismatch — bollard's hyper transport can't find the tokio reactor handle.

This is consistent with bollard's design: it assumes a single global tokio
runtime everywhere, with all users sharing the same `Handle::current()`.
Foundation crates use valtron (not tokio) as the async executor, creating an
inherent mismatch between how tokio/hyper/bollard expects the world to work and
how the platform is built.

## Resolution

**spec-54 (`foundation_deployment_docker`)** eliminates bollard entirely. The
new Docker client uses `SimpleHttpClient` + valtron `TaskIterator` — zero tokio
dependency, zero hyper, zero reactor issues. Once spec-54 is complete and
`foundation_deployment_platform` switches to it, the reactor issue disappears
at the crate level.

Until then, the proxy Docker tests compile but cannot run outside the
`foundation_deployment_platform` or `foundation_sshkit` test binaries. The test
code itself is correct — it uses the identical `RT.block_on` pattern that works
in the other crates.

## Verification

1. `cargo test -p foundation_deployment_platform --test ssh_backend_tests -- --ignored` —
   ssh Docker tests pass in that crate's binary (5/5).
2. `cargo test -p foundation_proxy --features docker-tests` — proxy Docker tests
   fail with "no reactor running" in the proxy test binary.
3. spec-54 removes bollard → all Docker tests work in any binary.
