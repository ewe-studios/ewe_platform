# 02 — Proc Macro Location

**Date:** 2026-07-08
**Status:** Resolved

## Decision

The `#[docker_container]` proc macro lives in `foundation_macros` (alongside all
other workspace proc macros), with a re-export from `foundation_deployment_docker`
so users have a single import path. The runtime types (`ContainerHandle`,
`ContainerConfig`, `WaitFor`, `DockerError`) live in `foundation_deployment_docker`.

## Table of Contents

1. [Why foundation_macros](#why-foundation_macros)
2. [Cross-crate path resolution](#cross-crate-path-resolution)
3. [Re-export pattern](#re-export-pattern)
4. [Macro attribute grammar](#macro-attribute-grammar)
5. [Generated code shape](#generated-code-shape)
6. [Sync and async function support](#sync-and-async-function-support)
7. [Docker-unavailable handling](#docker-unavailable-handling)
8. [Multi-container composition](#multi-container-composition)

---

## Why foundation_macros

All workspace proc macros live in `foundation_macros`:

- `#[valtron]`, `#[valtron_test]` — valtron runtime initialization
- `#[wasm_ui_server]` — browser test server lifecycle
- `#[wasm_test]` — wasm test case discovery
- `#[serial_test]` — serialized test execution
- `#[service]`, `generate!` — ConnectRPC code generation
- `html!`, `theme!` — UI macros
- `#[timeout]`, `#[wasm_entrypoint]`, `#[scaffold_impl]`, etc.

The pattern is: **proc macros in `foundation_macros`, runtime types in their
respective crates**. `#[docker_container]` follows this pattern exactly.

An alternative would be to put the macro in `foundation_deployment_docker`
itself (making it a dual proc-macro + library crate). This is not the workspace
convention and would require `foundation_deployment_docker` to have
`proc-macro = true` in its Cargo.toml, which blocks it from being used as a
normal library dependency by downstream crates (proc-macro crates can only
export proc macros).

---

## Cross-crate path resolution

`foundation_macros/src/crate_paths.rs` resolves crate paths so macros can
reference types from other workspace crates even if the downstream crate
renames the dependency:

```rust
// In foundation_macros/src/crate_paths.rs:
pub fn foundation_deployment_docker_path() -> proc_macro2::TokenStream {
    resolve_crate("foundation_deployment_docker")
}
```

The macro's generated code uses this path for all references to
`ContainerHandle`, `ContainerConfig`, `WaitFor`, `DockerError`, and `block_on`:

```rust
let __path = foundation_deployment_docker_path();
quote! {
    let __cfg = #__path::ContainerConfig::new("redis:7")
        .port(6379)
        .wait(#__path::WaitFor::Port { port: 6379, timeout: ... });
    let __guard = #__path::block_on(#__path::ContainerHandle::start(__cfg));
}
```

---

## Re-export pattern

`foundation_deployment_docker/src/lib.rs` re-exports the macro:

```rust
pub use foundation_macros::docker_container;
```

Users write:

```rust
use foundation_deployment_docker::docker_container;

#[docker_container(image = "redis:7", port = 6379)]
#[valtron_test]
fn test_redis() { ... }
```

A single import path, no need to know about `foundation_macros`.

---

## Macro attribute grammar

```
#[docker_container(
    image = "redis:7",              // (required) Docker image name[:tag]
    name = "my-redis",              // (optional) container name; auto-generated if absent
    port = 6379,                    // (repeatable) container port → auto host port
    port_mapped = (6379, 3030),     // (repeatable) explicit host:container mapping
    env = [("KEY", "VALUE"), ...],  // (optional) environment variables
    network = "testbed-net",        // (optional) Docker network name
    volume = ("/host/path", "/container/path"),  // (repeatable) bind mount
    wait_port = 6379,               // wait for TCP port open
    wait_stdout = "Ready",          // wait for log message
    wait_http = "http://localhost:8080/health",  // wait for HTTP 200
    wait_timeout = 60,              // max wait seconds (default 30)
    memory = "512m",                // memory limit
    cpus = 2,                       // CPU limit
    always_pull = true,             // force image pull
    stop_timeout = 10,              // graceful stop timeout seconds (default 10)
)]
```

### Attribute parsing

Following the `wasm_ui_server_entry.rs` pattern: parse the attribute tokens as
`syn::punctuated::Punctuated<syn::Meta, Token![,]>` — comma-separated
`key = value` pairs. Each key maps to a `ContainerConfig` builder method.
Unknown keys produce a compile error with a spanned message.

The proc macro builds a `ContainerConfig` expression at compile time (builder
chain), so the user's attribute values become a constructed config with zero
runtime overhead for parsing.

### Wait strategy resolution

| Attribute(s) | Maps to |
|-------------|---------|
| `wait_port = 6379` | `WaitFor::Port { port: 6379, timeout }` |
| `wait_stdout = "Ready"` | `WaitFor::Stdout { message: "Ready", timeout }` |
| `wait_http = "http://..."` | `WaitFor::Http { url, expected_status: 200, timeout }` |
| Multiple wait attrs | `WaitFor::Composite { strategies }` (AND semantic, in order) |
| No wait attr, but `port` present | `WaitFor::Port { port: first_port, timeout }` (default) |
| No wait attr, no port | `WaitFor::None` |

---

## Generated code shape

For a sync function:

```rust
// User writes:
#[docker_container(image = "redis:7", port = 6379)]
fn test_redis() { /* body */ }

// Expands to (simplified):
fn test_redis() {
    fn __docker_body_test_redis() { /* original body */ }

    let __docker_guard = {
        let __cfg = ::foundation_deployment_docker::ContainerConfig::new("redis:7")
            .port(6379)
            .wait(::foundation_deployment_docker::WaitFor::Port {
                port: 6379,
                timeout: ::core::time::Duration::from_secs(30),
            });
        match ::foundation_deployment_docker::block_on(
            ::foundation_deployment_docker::ContainerHandle::start(__cfg)
        ) {
            Ok(handle) => handle,
            Err(e) if e.is_connection_error() => {
                ::tracing::warn!("SKIP: Docker not available ({e})");
                return;  // test passes, main returns early
            }
            Err(e) => ::std::panic!("Failed to start Docker container: {e}"),
        }
    };

    let __result = __docker_body_test_redis();
    ::core::mem::drop(__docker_guard);  // explicit drop after body returns
    __result
}
```

### Design notes

1. **Inner fn for sync functions** — The original body moves into
   `fn __docker_body_{name}()`. This preserves `return` semantics (a closure
   would silently capture `return`). Same pattern as `valtron_entry.rs`.

2. **Explicit `drop(__docker_guard)` after body** — The container is alive
   during the entire body execution. On panic, unwinding triggers Drop
   automatically. On normal return, the explicit drop provides deterministic
   cleanup order. Same pattern as `valtron_entry.rs`.

3. **`block_on` for Docker ops** — Docker setup/teardown uses the internal
   tokio runtime's `block_on`. The user's body is unchanged (sync or async,
   depending on their function signature).

4. **`is_connection_error()` guard** — Distinguishes "Docker not running" from
   actual failures. On connection error, the test returns early (passes). This
   matches testcontainers' CI behavior.

---

## Sync and async function support

### Sync functions (default)

Body is wrapped in an inner `fn`. Works with `#[test]`, `#[valtron_test]`,
`main()`, or any regular function.

### Async functions

For `async fn`, the body cannot be moved into an inner `fn` (async blocks
capture differently, and `async fn` return types involve opaque `impl Future`).
Instead, the body runs inline:

```rust
// User writes:
#[docker_container(image = "redis:7", port = 6379)]
async fn test_redis() { /* body with .await */ }

// Expands to (simplified):
async fn test_redis() {
    let __docker_guard = { /* same setup as sync */ };
    let __result = { /* original async body */ }.await;
    ::core::mem::drop(__docker_guard);
    __result
}
```

The async case uses an inline `async { ... }.await` block. This preserves the
function's async signature and `.await` semantics.

---

## Docker-unavailable handling

The `DockerError::is_connection_error()` method returns `true` for:

- Docker socket not found (`ENOENT` on `/var/run/docker.sock`)
- Permission denied (user not in `docker` group)
- Docker daemon not running (connection refused)
- Timeout connecting to Docker socket

On connection error, the macro-generated code:
1. Logs a warning via `tracing::warn!`
2. Returns early — for `#[test]` functions this means the test passes
3. For `main()` functions, returns early (or panics, configurable)

This behavior can be overridden with a `required = true` attribute:

```rust
#[docker_container(image = "redis:7", port = 6379, required = true)]
fn must_have_docker() { ... }  // panics if Docker is unavailable
```

---

## Multi-container composition

Multiple containers are composed by stacking `#[docker_container]` attributes:

```rust
#[docker_container(image = "redis:7", port = 6379)]
#[docker_container(image = "postgres:16", port = 5432, env = [("POSTGRES_PASSWORD", "test")])]
#[valtron_test]
fn test_with_db_and_cache() {
    // Both redis (outer) and postgres (inner) are running
}
```

Each macro wraps the next, so containers are started/stopped in **nested LIFO
order** (outermost starts first, outermost stops last). All containers are
alive when the body runs. This is correct for testcontainers semantics — order
of startup only matters for readiness, not for the body itself.

A future enhancement could support a single `#[docker_container]` with
comma-separated specs, but stacking is simpler and already works with Rust's
attribute model.
