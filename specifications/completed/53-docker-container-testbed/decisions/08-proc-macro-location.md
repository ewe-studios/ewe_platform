# 02 — Proc Macro Location

**Date:** 2026-07-08
**Updated:** 2026-07-17 — handle exposure (see [Reaching the containers](#reaching-the-containers))
**Status:** Resolved

## Decision

The `#[docker_container]` proc macro lives in `foundation_macros` (alongside all
other workspace proc macros). The runtime types (`ContainerHandle`,
`ContainerConfig`, `WaitFor`, `DockerError`) live in
`foundation_deployment_platform::docker`. Users import everything from one
crate — `foundation_deployment_platform` — since the macro is re-exported
from there.

The annotated function **must** take exactly one parameter, which receives a
`ContainerGroup` of every started container; containers are addressed by a
logical key (`as = "..."`) and reached via `handle.address(port)`.

## Table of Contents

1. [Why foundation_macros](#why-foundation_macros)
2. [Cross-crate path resolution](#cross-crate-path-resolution)
3. [Re-export pattern](#re-export-pattern)
4. [Reaching the containers](#reaching-the-containers)
5. [Macro attribute grammar](#macro-attribute-grammar)
6. [Generated code shape](#generated-code-shape)
7. [Sync and async function support](#sync-and-async-function-support)
8. [Docker-unavailable handling](#docker-unavailable-handling)
9. [Multi-container composition](#multi-container-composition)

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

## Reaching the containers

**Decided 2026-07-17.** The original design kept the `ContainerHandle` in a
hidden local, so a body could only reach its container by pinning a host port
with `port_mapped`. That is backwards: it forces every test to hard-code a host
port, and two tests doing so in parallel fail the second container's start with
a port conflict. The handles are now handed to the body.

### The function must take the group

The annotated fn declares exactly one parameter; the macro passes it a
`ContainerGroup` holding every container the invocation started, in start
order. A fn without a parameter is a **compile error**:

```
error: #[docker_container] requires exactly one parameter to receive the started
       containers, but `no_group_param` declares 0. Add `containers: ContainerGroup`
       (or `_: ContainerGroup` if the body does not need the handles).
```

Required rather than optional: a container a body cannot address is of no use to
it, and one shape to document beats two. A body that genuinely ignores its
containers writes `_: ContainerGroup`.

The group **owns** the handles, so teardown is the group's Drop at the end of the
body — there is no separate per-container guard. `ContainerGroup` is reused
rather than a new type: it already modelled "several containers, cleaned up
together in reverse start order", and only needed incremental building
(`empty()` + `insert()`) so the macro can fill it one container at a time.

### Lookup key: `as`, not the Docker name

Containers are looked up by a **logical key** set with `as = "cache"`, which
never reaches the Docker API:

```rust
let cache = containers.container("cache").expect("cache should be in the group");
```

`name` was rejected as the lookup key because Docker container names are
**global to the daemon**. Making a handle findable would mean pinning a name,
and two parallel tests (or one leftover container) would then collide with a
409 — the same failure mode as the pinned host ports this change removes. A
logical key is local to the group, so the daemon keeps auto-naming uniquely and
any number of runs can use the key `"cache"` simultaneously.

`name` keeps its meaning (the real container name) and stays optional.
`ContainerGroup::container(key)` matches the logical key first, then falls back
to the real Docker name, so groups built directly via `ContainerGroup::start_async`
still resolve by name. Two containers sharing one logical key panic on insert
(naming the duplicate) rather than letting `container(key)` return an arbitrary
one. Unkeyed containers remain reachable positionally via `get(index)`.

### Ports: let Docker assign, ask the handle

With handles exposed, `port = 6379` (Docker assigns the host port) is the
preferred form, and the body asks where it landed:

```rust
let addr: SocketAddr = cache.address(6379).expect("6379 should be mapped");
```

`ContainerHandle::address(container_port)` returns the host-side `SocketAddr`
(`udp_address` for UDP). `port_mapped = (container, host)` remains for cases that
truly need a fixed host port, with the collision caveat that implies.

---

## Macro attribute grammar

Each container is one `{ ... }` block; a bare list is shorthand for a single
container. The keys within a block:

```
#[docker_container(
    image = "redis:7",              // (required) Docker image name[:tag]
    as = "cache",                   // (optional) logical lookup key for ContainerGroup::container()
    name = "my-redis",              // (optional) real container name; auto-generated if absent
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

Comma-separated `key = value` pairs, each mapping to a `ContainerConfig` builder
method. Unknown keys produce a compile error spanned on the key itself.

The pairs are parsed **by hand**, not as
`syn::punctuated::Punctuated<syn::Meta, Token![,]>` (the `wasm_ui_server_entry.rs`
pattern). `as` is a Rust keyword, so `Meta`'s path parser rejects `as = "cache"`
with *"expected identifier, found keyword `as`"*; peeking for `Token![as]`
accepts it. The keyword spelling was kept because it reads naturally at the call
site, and hand-rolling also improves the spans on unknown keys.

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
#[docker_container(image = "redis:7", as = "cache", port = 6379)]
fn test_redis(containers: ContainerGroup) { /* body */ }

// Expands to (simplified):
fn test_redis() {
    fn __docker_body_test_redis(containers: ContainerGroup) { /* original body */ }

    let __cfg = ::foundation_deployment_platform::docker::ContainerConfig::new("redis:7")
        .port(6379)
        .wait(::foundation_deployment_platform::docker::WaitFor::Port {
            port: 6379,
            timeout: ::core::time::Duration::from_secs(30),
        });

    // Sync fn: the async Docker work is driven through valtron. One async block
    // fills the group with every declared container, in order. `Option` carries
    // the skip decision out — a `return` inside it would only leave the block.
    let __docker_group = ::foundation_core::valtron::block_on_future(async move {
        let mut __docker_group = ContainerGroup::empty();
        {
            let __cfg = /* this container's config chain */;
            match ::foundation_deployment_platform::docker::ContainerHandle::start_async(__cfg).await {
                Ok(h) => __docker_group.insert(Some(String::from("cache")), h),
                Err(e) if e.current_context().is_connection_error() => {
                    ::tracing::warn!("SKIP: Docker not available ({e})");
                    return None;  // drops the group -> stops anything already started
                }
                Err(e) => ::std::panic!("Failed to start Docker container: {e}"),
            }
        }
        // ... one such block per declared container ...
        Some(__docker_group)
    });

    let __docker_group = match __docker_group {
        Some(g) => g,
        None => return,  // test passes, main returns early
    };

    // The group owns the handles: the containers are stopped and removed when it
    // drops at the end of the body, so there is no separate guard to drop here.
    __docker_body_test_redis(__docker_group)
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

2. **The group owns the handles** — The container is alive for the whole body;
   teardown is the `ContainerGroup`'s Drop when the body's parameter goes out of
   scope. On panic, unwinding triggers it just the same. (Before handles were
   exposed, each expansion held its own guard and dropped it explicitly after the
   body; the group replaces that.)

3. **`.await` for Docker ops** — Docker setup/teardown `.await`s directly.
   The whole thing runs inside valtron's `block_on_future` async block, so the
   Docker client's futures compose naturally. The user's body can be sync or
   async.

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
    // The same startup block as the sync arm — only the driver differs.
    let __docker_group = match async move { /* fill the group; None if absent */ }.await {
        Some(g) => g,
        None => return,
    };

    let containers = __docker_group;  // bind to the parameter's pattern

    /* original async body */
}
```

The body is spliced in as a plain block, **not** wrapped in `async { … }.await`:
the enclosing fn is already `async`, so `.await` inside the body compiles as-is,
and `return` keeps meaning "return from this fn" (an `async` block would capture
it). No `block_on_future` here — the caller's executor drives the Docker work.
An early `return` from the body still stops the container, via the group's Drop.

Note the declared `ContainerGroup` parameter is consumed by the macro, so the
callable fn takes no arguments — an `async fn redis_body(containers: ContainerGroup)`
is invoked as `redis_body()`.

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

**Decided 2026-07-17.** Every container goes in **one** invocation, as a
comma-separated list of `{ ... }` blocks:

```rust
#[docker_container(
    { image = "redis:7", as = "cache", port = 6379 },
    { image = "postgres:16", as = "db", port = 5432,
      env = [("POSTGRES_PASSWORD", "test")] }
)]
#[valtron_test]
fn test_with_db_and_cache(containers: ContainerGroup) {
    // Both redis and postgres are running, and both are in the group.
    let cache_addr = containers.container("cache").unwrap().address(6379).unwrap();
    let db_addr = containers.container("db").unwrap().address(5432).unwrap();
}
```

The bare `key = value` list stays as shorthand for a single container:
`#[docker_container(image = "redis:7", as = "cache", port = 6379)]`. The parser
picks the shape by peeking for a leading `{`.

Containers **start in the order written** and are torn down in reverse (the
group's `Vec` drop order). All of them are alive when the body runs, which is the
property that matters — start order only affects readiness, so a container that
depends on another should declare an explicit `wait_*` rather than lean on order.

### Why one invocation instead of stacked attributes

Stacking (the original design) made each attribute an independent expansion that
could not see its siblings, which cost far more than it bought:

- **The group had to be threaded through the nest.** Each expansion inspected the
  attributes it re-emitted to decide whether it was the outermost one (seed the
  group, emit a zero-arg fn) or a middle one (accept the group as a parameter,
  emit `fn f(mut __docker_group_in: ContainerGroup)`), and the async arm had to
  bind the group to the parameter's pattern *before* a same-named binding could
  shadow it. All of that is deleted.
- **Start order was inverted.** Rust expands the topmost attribute first, and it
  re-emits the ones below onto its generated fn, so the *lowest* attribute ended
  up outermost at runtime and started **first** — measured via `docker events`.
  The single-invocation form starts containers in the order written.
- **Duplicate lookup keys could only be caught at runtime.** One invocation sees
  the whole set, so `as = "cache"` twice is now a compile error instead of a
  panic once containers are already running.

Stacking is therefore rejected outright, rather than left to fail confusingly
(the second attribute would receive an already-expanded zero-arg fn and complain
that it "requires exactly one parameter"):

```
error: #[docker_container] cannot be stacked: declare every container in one
       invocation, one `{ ... }` block each —
       #[docker_container({ image = "redis:7", as = "cache" }, { image = "postgres:16", as = "db" })]
```

### Generated shape

One `ContainerGroup::empty()` is filled in declaration order inside a single
async block that yields `Option<ContainerGroup>` — `None` meaning "Docker absent,
skip". The `Option` (rather than a `return`) is what carries the skip out, since
a `return` inside an async block only leaves the block. On `None` the partially
filled group drops right there, stopping any container that had already started.
The two arms differ only in what drives that block: `.await` for an `async fn`,
`block_on_future` for a sync one.
