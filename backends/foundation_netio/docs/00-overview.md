# foundation_netio — Network I/O abstractions

## What it is
Cross-platform networking layer: HTTP client/server, WebSockets, SSE, TLS
backends, and connection management.

## Key modules
- **`simple_http/`** — Lightweight HTTP client and server. Features: request
  pipelining, connection pooling, redirect following, cookie management,
  latency/load tracking, timeout handling.
- **`netcap/`** — Network capability layer: TLS verification, connection
  lifecycle, SSL backend selection (rustls/native-tls).
- **`event_source/`** — Server-Sent Events (SSE) client with reconnection
  and progress tracking.
- **`websocket/`** — WebSocket client with framing and message handling.

## Design principles
- **Pluggable HTTP client**: `HttpClient` trait allows native or wasm (`FetchHttpClient`)
  implementations behind the same interface.
- **Target-gated**: TLS backends, raw sockets, and epoll/kqueue are native-only.
  Wasm uses `fetch` API via `web_sys`.
- **Streaming**: Large responses are handled via `SendSafeBody` streams, not
  buffered entirely in memory.

## Feature flags
- `multi` — Multi-threaded executor (native only)
- `ssl-rustls` — Rustls TLS backend (default)
- `compression` — gzip/deflate support
- `wire-native` — Native networking (sockets, epoll, kqueue)
