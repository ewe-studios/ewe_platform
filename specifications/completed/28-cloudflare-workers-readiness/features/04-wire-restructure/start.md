---
feature: "Wire Module Restructure"
depends_on: ["01-core-wasm-compat"]
---

# Start: Wire Module Restructure

## Pre-requisites

- Feature 01 (Core Wasm Compat) completed — `foundation_core` compiles on wasm32
- `wire-native` feature added to `Cargo.toml` (already done)
- `wire/mod.rs` gates already in place (already done)

## Workflow

1. Read `feature.md` for full classification and design
2. Move `ContentLengthEnforcingIterator` and `Extensions` from `simple_http/client/` to `simple_http/shared.rs`
3. Update `impls.rs` imports to use shared types
4. Gate `simple_http/client/` behind `wire-native` cfg in `simple_http/mod.rs`
5. Gate `event_source/task.rs` and `event_source/reconnecting_task.rs` behind `wire-native` cfg in `event_source/mod.rs`
6. Gate `websocket/connection.rs`, `task.rs`, `reconnecting_task.rs`, `server.rs` behind `wire-native` cfg in `websocket/mod.rs`
7. Verify native build: `cargo check -p foundation_core`
8. Verify wasm32 with std only: `cargo check -p foundation_core --target wasm32-unknown-unknown --no-default-features --features std`
9. Run clippy: `cargo clippy -p foundation_core -- -D warnings`
10. Commit changes

---

_Created: 2026-05-16_
