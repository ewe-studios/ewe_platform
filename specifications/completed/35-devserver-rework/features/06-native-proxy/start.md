---
feature: "Native Proxy"
description: "Start here — use foundation_http HttpServer + TunnelProxy instead of hyper/axum"
status: "pending"
created: "2026-06-01"
---

# Start: Feature 06 — Native Proxy

## Workflow

1. Read `feature.md` for full details
2. Read `.agents/skills/rust-clean-code/skill.md`
3. Read `crates/devserver/src/proxy.rs` and `crates/devserver/src/streams.rs` (current proxy)
4. Read `backends/foundation_http/src/shared/serve/mod.rs` for `Serve`, `ConnectionResult`
5. Read `backends/foundation_http/src/shared/app/mod.rs` for `HttpApp` builder
6. Read `backends/foundation_http/src/native/server/mod.rs` for `HttpServer`
7. Read `backends/foundation_http/src/native/server/connection.rs` for `ConnectionHandler` (valtron TaskIterator)
8. Read `backends/foundation_http/src/native/handlers/static_asset.rs` for `StaticAssetHandler`
9. Read `backends/foundation_netio/src/netcap/connection/` for `Connection`, `Listener` (tunnel fallback)
10. Read `backends/foundation_netio/src/simple_http/` for HTTP types (proxy forwarding)
11. Implement per feature.md
12. Update `LEARNINGS.md`

---

_Created: 2026-06-01_
