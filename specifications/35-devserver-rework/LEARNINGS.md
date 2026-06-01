# Learnings

## foundation_netio already covers networking + SSE

**Why:** The initial spec drafted hand-rolling HTTP parsing, raw socket SSE writers, and a poll-layer integration for the proxy. This was unnecessary — `foundation_netio` already provides all of this.

**How to apply:**
- **Proxy networking**: Use `foundation_netio::netcap::connection::{Listener, Connection}` — provides `TcpListener` accept loop + `TcpStream` with `Read + Write`, no async needed.
- **HTTP parsing/responses**: Use `foundation_netio::simple_http::shared::*` — `SimpleIncomingRequest`, `SimpleOutgoingResponse`, `SimpleHeaders`, `Status`, `Proto`. Already handles HTTP/1 parsing.
- **SSE server**: Use `foundation_netio::event_source::*` — `SseEvent` (builder), `EventWriter<W>` (writes to any `Write`), `SseResponse` (builds correct HTTP headers: `text/event-stream`, `no-cache`, `keep-alive`).
- **No need for** `foundation_nativeapis` poll-layer in the proxy — `Connection` already wraps `std::net::TcpStream` with `Read + Write` and timeout support.

**Impact on spec:** Features 06 (Native Proxy) and 07 (SSE Reload) are now `medium` and `small` effort instead of `large` — we're wiring existing APIs, not inventing them.

**Also updated:** Feature 01 (Crate Scaffolding) Cargo.toml — replaced `libc`, `bytes`, `http` with `foundation_netio`. The dependency graph is simpler.

---

_Created: 2026-06-01_
