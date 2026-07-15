# 13 — Tri-Config & `wireguard!` Macro (PROPOSED)

**Date:** 2026-07-12
**Status:** **Proposed** (mirrors `foundation_proxy`; owner review pending)

## Decision (proposed)

Expose three converging configuration paths, mirroring `foundation_proxy`:

1. **`wireguard!` macro** — compile-time, type-checked, single-binary network declaration.
2. **Programmatic builder** — runtime, dynamic, full Rust.
3. **`wireguard.toml` file** — reloadable, `clap --config`.

All three converge on one `WgConfig`. The macro lives in `foundation_macros` (per
`feedback_macros_location` — never a companion `*_macros` crate), re-exported from
`foundation_wireguard`.

## Why

- `foundation_proxy` already establishes this exact tri-config pattern in the codebase; matching it
  gives users one mental model across foundation services (`feedback` on retrieval-led conventions).
- The owner explicitly asked for "easy to instantiate either via a configuration file or in code like
  we do with `foundation_proxy` via functions we can call and a macro."

## What we do (proposed)

### `WgConfig` (the convergence type, serde)

```rust
pub struct WgConfig {
    pub network: NetworkConfig,     // network_id, overlay CIDR, seed/token or seed-from-env
    pub node: NodeConfig,           // identity persistence path, advertised endpoints, caps (relay?)
    pub dataplane: DataPlane,       // Smoltcp (default) | KernelTun { name, mtu }
    pub bootstrap: BootstrapConfig, // seeds/endpoints, admission mode, seed TTL
    pub security: SecurityConfig,   // optional app-layer mTLS toggle
    pub relay: RelayConfig,         // advertise relay?, max sessions
}
```

- A bare backend/endpoint string must deserialize into a full struct (as `foundation_proxy` does for
  `BackendTarget`) — forgiving TOML, ergonomic Rust.
- Secrets: `seed` may be inline (dev), or `seed_env = "WG_SECRET"` / `seed_file = "..."` (prod), never
  a silent default (`feedback_no_silent_defaults`).

### `wireguard!` macro (sketch)

```rust
let config = wireguard! {
    network: "prod-mesh",
    overlay: "fd00:ewe::/32",
    seed_env: "WG_SECRET",
    endpoints: ["wg.example.com:51820"],
    dataplane: smoltcp,          // or: tun { name: "ewe0", mtu: 1420 }
    relay: advertise,
    security: { mtls: off },
};
```

### Entry ergonomics

- A `#[wireguard_main]` attribute (like the platform's other `*_main` entries) that initializes a
  valtron pool, builds the node from config/env, joins the network, and hands the user a `WgHandle`
  exposing `tcp_connect`/`tcp_listen`/`udp_bind` over the overlay.
- Programmatic: `WgNode::builder().network(...).seed(...).build()?.join().await`.

## Runtime requirement

Like `foundation_proxy`, the node serves via valtron; callers must initialize a pool
(`foundation_core::valtron::initialize_pool`) and hold its guard for the node's lifetime (documented
in the crate docs and the `#[wireguard_main]` expansion).

## Open questions for review

- Whether `wireguard.toml` should support **hot reload** of membership/relay knobs (proxy reloads
  routes) — proposed **yes** for relay/security toggles, **no** for identity/seed (restart).

## Related decisions

- [04 — Bootstrap token envelope](04-bootstrap-token-envelope.md)
- [12 — IPAM addressing](12-ipam-addressing.md)
- Feature [09 — Config, macro & builder](../features/09-config-macro-and-builder/feature.md)
