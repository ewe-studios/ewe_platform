# 02 — Proc Macro Location

**Date:** 2026-07-08
**Status:** Resolved

## Decision

The `#[docker_container]` proc macro lives in `foundation_macros` (alongside all
other workspace proc macros). The runtime types (`ContainerHandle`,
`ContainerConfig`, `WaitFor`, `DockerError`) live in
`foundation_deployment_platform::docker`. Users import everything from one
crate — `foundation_deployment_platform` — since the macro is re-exported
from there.

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

An alternative would be to put the macro in `foundation_deployment_platform`
itself (making it a dual proc-macro + library crate). This is not the workspace
convention and would require `foundation_deployment_platform` to have
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
pub fn foundation_deployment_platform_path() -> proc_macro2::TokenStream {
    resolve_crate("foundation_deployment_platform")
}
```

The macro's generated code uses this path for all references to
`ContainerHandle`, `ContainerConfig`, `WaitFor`, and `DockerError`:

```rust
let __path = foundation_deployment_platform_path();
quote! {
    let __cfg = #__path::docker::ContainerConfig::new("redis:7")
        .port(6379)
        .wait(#__path::docker::WaitFor::Port { port: 6379, timeout: ... });
    let __guard = #__path::docker::ContainerHandle::start(__cfg).await;
}
```

---

## Re-export pattern

`foundation_deployment_platform/src/lib.rs` re-exports the macro:

```rust
pub use foundation_macros::docker_container;
```

Users write:

```rust
use foundation_deployment_platform::docker_container;

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
    port_mapped = (6379, 3030),     // (repeatable) (container_port, host_port)
    port_udp = 5353,                // (repeatable) UDP container port → auto host port
    env = [("KEY", "VALUE"), ...],  // (optional) environment variables
    network = "testbed-net",        // (optional) Docker network name
    volume = ("/host/path", "/container/path"),  // (repeatable) bind mount
    wait_port = 6379,               // wait for TCP port open (container-side port)
    wait_stdout = "Ready",          // wait for log message
    wait_http = "http://localhost:8080/health",  // wait for HTTP 200
    wait_timeout = 60,              // max wait seconds (default 30)
    memory = "512m",                // memory limit
    cpus = 2,                       // CPU limit
    always_pull = true,             // force image pull
    required = true,                // no graceful skip when Docker is absent
    stop_timeout = 10,              // graceful stop timeout seconds (default 10)
)]
```

`wait_port` names the **container-side** port, not the host port: the wait
resolves it through the container's port map, so a host port that was never
mapped to a container port fails with "port N was not mapped".

`memory` takes the `docker run --memory` spellings — a plain byte count, or a
decimal with a `b`/`k`/`m`/`g`/`t` suffix (binary multipliers, so `1k` = 1024).
An unparseable limit is a hard `InvalidConfig` error rather than a silent
"no limit".

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
| No wait attr, but `port`/`port_mapped` present | `WaitFor::Port { port: first_port, timeout }` (default) |
| No wait attr, no port | `WaitFor::None` |

`wait_timeout` applies to every strategy in the set (each variant carries its own
`timeout` field), defaulting to 30s. Composite order is `wait_port`, then
`wait_http`, then `wait_stdout`.

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

    let __cfg = ::foundation_deployment_platform::docker::ContainerConfig::new("redis:7")
        .port(6379)
        .wait(::foundation_deployment_platform::docker::WaitFor::Port {
            port: 6379,
            timeout: ::core::time::Duration::from_secs(30),
        });

    // Sync fn: the async Docker work is driven through valtron. `Option` carries
    // the skip decision out of the async block — a `return` inside it would only
    // return from the block, and the handle has no `Default`.
    let __docker_guard = ::foundation_core::valtron::block_on_future(async move {
        match ::foundation_deployment_platform::docker::ContainerHandle::start_async(__cfg).await {
            Ok(handle) => Some(handle),
            Err(e) if e.current_context().is_connection_error() => {
                ::tracing::warn!("SKIP: Docker not available ({e})");
                None
            }
            Err(e) => ::std::panic!("Failed to start Docker container: {e}"),
        }
    });
    let __docker_guard = match __docker_guard {
        Some(handle) => handle,
        None => return,  // test passes, main returns early
    };

    let __result = __docker_body_test_redis();
    ::core::mem::drop(__docker_guard);  // explicit drop after body returns
    __result
}
```

The sync arm needs a live valtron pool, since the Docker client is valtron-based
— hence the documented pairing with `#[valtron_test]`. With `required = true`
the skip arm is omitted entirely, so a connection error falls through to the
panic.

### Design notes

1. **Inner fn for sync functions** — The original body moves into
   `fn __docker_body_{name}()`. This preserves `return` semantics (a closure
   would silently capture `return`). Same pattern as `valtron_entry.rs`.

2. **Explicit `drop(__docker_guard)` after body** — The container is alive
   during the entire body execution. On panic, unwinding triggers Drop
   automatically. On normal return, the explicit drop provides deterministic
   cleanup order. Same pattern as `valtron_entry.rs`.

3. **`.await` for Docker ops** — Docker setup/teardown `.await`s directly.
   The whole thing runs inside valtron's `block_on_future` async block, so
   bollard futures compose naturally. The user's body can be sync or async.

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
    let __docker_guard = match ContainerHandle::start_async(__cfg).await {
        Ok(handle) => handle,
        Err(e) if e.current_context().is_connection_error() => {
            ::tracing::warn!("SKIP: Docker not available ({e})");
            return;
        }
        Err(e) => ::std::panic!("Failed to start Docker container: {e}"),
    };
    let __result = { /* original async body */ };
    ::core::mem::drop(__docker_guard);
    __result
}
```

The body is spliced in as a plain block, **not** wrapped in `async { … }.await`:
the enclosing fn is already `async`, so `.await` inside the body compiles as-is,
and `return` keeps meaning "return from this fn" (an `async` block would capture
it). No `block_on_future` here — the caller's executor drives the Docker work.
An early `return` from the body still stops the container, via the guard's Drop.

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
order**. All containers are alive when the body runs, which is the property that
matters — startup order only affects readiness, not the body.

The nesting runs opposite to how the attributes read. Rust expands the *topmost*
attribute first, and it re-emits the remaining attributes onto its generated fn;
so the **lowest** `#[docker_container]` ends up outermost at runtime and starts
its container **first**, while the topmost one starts last and stops first.
Measured on the stacked test (`docker create`/`start` events): the bottom
attribute's container is created and started before the top one's. Nothing
should depend on this order — declare an explicit readiness `wait_*` instead.

A future enhancement could support a single `#[docker_container]` with
comma-separated specs, but stacking is simpler and already works with Rust's
attribute model.
