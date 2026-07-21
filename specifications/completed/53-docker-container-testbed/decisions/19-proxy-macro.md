# 19 — `proxy!` macro (Stage 2)

**Date:** 2026-07-10
**Status:** Resolved

## Decision

Provide a `proxy!` macro that compiles proxy configuration at build time.
Desugars to the builder pattern. Type-checked by the compiler — invalid service
names, missing fields, wrong types are caught before the binary runs. This is
the "baked-in" config path from Decision 14.

## Why

Three config paths were designed (Decision 14):
1. `proxy!` macro — compile-time, single binary
2. Programmatic builder — runtime, dynamic
3. `proxy.toml` file — reloadable

Paths 2 and 3 exist. Path 1 is missing. It's the right choice for single-purpose
deploys where the proxy config is known at build time and should never be wrong.

## Syntax

```rust
use foundation_proxy::proxy;

let config = proxy! {
    domain: "example.com",
    public_ip: "1.2.3.4",
    ssl: lets_encrypt { email: "admin@example.com" },

    services: {
        app: {
            host: "app.example.com",
            backends: ["http://localhost:3000"],
            health_check: {
                path: "/up",
                interval: 5,
                timeout: 2,
            },
        },
        windows: {
            host: "windows.example.com",
            backends: ["tcp://localhost:8006"],
        },
        dns: {
            host: "dns.example.com",
            backends: ["udp://localhost:53"],
        },
    },
};

config.start()?;
```

## Desugaring

```rust
// proxy! { domain: "example.com", public_ip: "1.2.3.4", ssl: ..., services: { app: { ... } } }
// ↓
ProxyConfig::new("example.com", "1.2.3.4")
    .ssl(SslConfig::lets_encrypt("admin@example.com"))
    .service(
        ServiceConfig::new("app", "app.example.com")
            .backend("http://localhost:3000")
            .health_check_config(HealthCheckConfig {
                path: "/up".into(),
                interval: Duration::from_secs(5),
                timeout: Duration::from_secs(2),
                ..HealthCheckConfig::default()
            })
    )
    .service(
        ServiceConfig::new("windows", "windows.example.com")
            .backend("tcp://localhost:8006")
    )
    .service(
        ServiceConfig::new("dns", "dns.example.com")
            .backend("udp://localhost:53")
    )
    .build()
```

## Implementation

A `proc_macro` in `foundation_macros` (where `#[docker_container]` already lives).

```rust
// foundation_macros/src/proxy.rs
#[proc_macro]
pub fn proxy(input: TokenStream) -> TokenStream {
    // Parse the custom syntax via syn
    // Emit builder chain
}
```

The macro:
1. Parses key-value pairs with nested blocks (`services: { name: { ... } }`)
2. Validates field names at compile time (unknown fields → compiler error)
3. Calls the same builder methods that the programmatic API uses
4. Protocol is inferred from URL scheme (no `proto` field needed)

## Trade-offs

| Pro | Con |
|-----|-----|
| Compile-time type checking | Must rebuild to change config |
| Zero external files | Not reloadable at runtime |
| Single binary deploy | Less flexible than TOML |
| Same `ProxyConfig` output as other paths | Macro syntax is custom, not standard TOML |

## Verification

1. `proxy!` with valid config compiles and produces correct `ProxyConfig`
2. `proxy!` with invalid service name → compile error
3. `proxy!` with wrong SSL variant → compile error
4. Round-trip: macro output matches equivalent builder chain
