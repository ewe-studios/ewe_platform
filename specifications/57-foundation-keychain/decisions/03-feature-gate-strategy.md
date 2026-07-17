# Decision 03: No Backend Feature Gates — Target-Gated Code

## Problem

The keychain has platform-specific code: Cloudflare Workers needs `#[event(fetch)]`, `worker::Router`, Durable Objects, and Web Crypto; native needs `foundation_http`, `foundation_netio` WebSocket, and valtron. How to gate it?

1. **Mutually exclusive features** (`backend-cloudflare` vs `backend-native`) — explicit but redundant
2. **Target gates** (`cfg(target_family = "wasm")`) — automatic, no user choice needed

## Analysis

`foundation_db` already compiles the right backend based on target + its own features:
- `wasm32-unknown-unknown` + `wasm-bindgen-storage` → `D1WasmStorage`, `KVWasmStorage`, `R2WasmStorage`
- `x86_64`/`aarch64` + `turso` → `TursoStorage`

The keychain doesn't need to re-select the backend. It just uses `StorageProvider::new(StorageBackend::...)` and `foundation_db` does the rest.

The only platform-specific code the keychain owns is the **server bootstrap** (Workers entry point vs. native `main()`) and **notification transport** (Durable Object vs. WebSocket server). These are naturally gated by `cfg(target_family = "wasm")`.

## Decision: Target-gated, no keychain-level backend features

```
foundation_keychain/src/
├── core/                    # Always compiled — portable domain logic
├── server/
│   ├── cloudflare.rs        # #[cfg(target_family = "wasm")]
│   └── native.rs            # #[cfg(not(target_family = "wasm"))]
└── notifications/
    ├── cloudflare.rs        # #[cfg(target_family = "wasm")] — Durable Object
    └── native.rs            # #[cfg(not(target_family = "wasm"))] — WebSocket server
```

Dependencies are also target-gated in Cargo.toml:
```toml
[target.'cfg(target_family = "wasm")'.dependencies]
worker = "0.8"
wasm-bindgen = "0.2"
web-sys = { version = "0.3", features = ["Crypto", "SubtleCrypto"] }
js-sys = "0.3"
wasm-bindgen-futures = "0.4"

[target.'cfg(not(target_family = "wasm"))'.dependencies]
foundation_http = { path = "../foundation_http" }
foundation_netio = { path = "../foundation_netio" }
```

No `backend-*` features. Build for the target, get the right backend.

## Consequences

- Users just `cargo build --target wasm32-unknown-unknown` or `cargo build` — no feature flags
- Same `foundation_keychain::core` on both targets
- CI tests both targets without feature permutations
- Matches how foundation_db and foundation_auth already work
