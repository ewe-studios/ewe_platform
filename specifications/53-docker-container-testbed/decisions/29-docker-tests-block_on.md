# 29 — Docker test `block_on` runtime fix

**Date:** 2026-07-10
**Status:** Resolved

## Decision

Fix the tokio runtime ("no reactor running") panic that blocks
`foundation_proxy`'s Docker-backed integration tests under the `docker-tests`
feature. The root cause: `futures_lite::block_on` (used by
`ContainerHandle::start()`'s sync wrapper via `crate::block_on`) doesn't create
a tokio reactor, but bollard requires one.

## Problem

```
thread panicked at bollard/src/docker.rs:1703:
there is no reactor running, must be called from the context of a Tokio 1.x runtime
```

Stack trace:
1. Test calls `RT.block_on(async { ContainerHandle::start_async(...) })`
2. `ContainerHandle::start_async()` calls `bollard::Docker::connect_with_local_defaults()`
3. Bollard internally calls `tokio::time::timeout(...).await`
4. Tokio checks `Handle::try_current()` → no reactor in scope → panic

The test has a `LazyLock<tokio::runtime::Runtime>` and calls `RT.block_on(...)`.
But bollard's internal `tokio::time::timeout` needs `Handle::current()` to
resolve, and `Runtime::block_on` doesn't always enter the context correctly for
lazy-initialized runtimes.

## Solution

### Option A: Explicit `Runtime::new()` + `enter()` guard

```rust
fn docker_tests() {
    let rt = tokio::runtime::Runtime::new().expect("rt");
    let _guard = rt.enter();  // makes Handle::current() work
    rt.block_on(async { ... });
}
```

This is the simplest fix. `rt.enter()` sets the runtime handle as the current
context, so bollard can find it.

### Option B: Helper function

```rust
/// Run an async block with a guaranteed tokio context.
/// Unlike `LazyLock<Runtime>::block_on`, this sets the handle
/// so that `Handle::current()` works inside the block.
fn with_tokio<F: Future<Output = T>, T>(f: impl FnOnce() -> F) -> T {
    let rt = RT.enter(); // enters the lazy runtime context
    RT.block_on(f())
}
```

But `Runtime::enter()` returns a guard that must live for the duration. So:

```rust
fn block_with_reactor<F: Future<Output = T>, T>(f: F) -> T {
    let _guard = RT.enter();
    RT.block_on(f)
}
```

### Recommended: make `docker-tests` use `block_with_reactor`

Replace all `RT.block_on(async { ... })` with `block_with_reactor(async { ... })`.
The `_guard` ensures the reactor context is set before bollard tries to use it.

## Alternative

Make `ContainerHandle::start()` use `tokio::runtime::Runtime::new()` instead of
`futures_lite::block_on`:

```rust
pub fn start(config: ContainerConfig) -> DockerResult<Self> {
    let rt = tokio::runtime::Runtime::new()
        .map_err(|e| docker_err(DockerError::Connection(format!("tokio: {e}"))))?;
    rt.block_on(Self::start_async(config))
}
```

This works because `Runtime::new()` enters its own context automatically inside
`block_on()`. But it creates a fresh runtime per call — heavier than reusing a
static one.

## Decision

Use **Option A variant**: add a `block_with_reactor` helper in the test file that
enters the lazy runtime context before blocking. This fixes the immediate problem
without changing production code (`ContainerHandle` is fine — it's the test
setup that's missing the context entry).

Change `proxy_integration_tests.rs`:
```rust
fn block_with_reactor<F: std::future::Future>(f: F) -> F::Output {
    let _guard = RT.enter();
    RT.block_on(f)
}

// Then replace all RT.block_on(...) with block_with_reactor(...)
```

## Verification

1. `cargo test -p foundation_proxy --features docker-tests` — all 9 Docker
   tests pass, no "no reactor running" panic.
2. Test runtime unchanged for non-Docker tests (they don't use `RT` at all).
