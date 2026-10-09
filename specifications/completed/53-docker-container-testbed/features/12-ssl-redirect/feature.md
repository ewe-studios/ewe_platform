---
feature: "SSL redirect (HTTP→HTTPS)"
description: "When TLS is enabled, a plain-HTTP listener on port 80 answers every request with 301 Moved Permanently to https://{host}{path}"
status: "complete"
priority: "medium"
phase: 4
depends_on: ["18-tls-cert-provisioning"]
estimated_effort: "small"
created: 2026-07-16
---
# Feature 12: SSL Redirect (Decision 25)

## What

`ssl_redirect_loop` in `server.rs` — a raw TCP listener on `0.0.0.0:80` that
reads just the request line + Host header, constructs a `301 Moved Permanently`
redirect to `https://{host}{path}`, writes it, and closes the connection.

Spawned as a dedicated thread when `ProxyServer::start()` detects TLS is
configured (`!matches(config.ssl.provider, SslProvider::None)`).

## Implementation

`server.rs:227-268` — `ssl_redirect_loop(shutdown: &Arc<OnSignal>)`:
- Binds `0.0.0.0:80`, sets non-blocking
- Accept loop checks shutdown signal
- Reads up to 4096 bytes per connection
- Extracts `Host:` header and request path
- Writes `HTTP/1.1 301` with `Location: https://{host}{path}`

## Tests added

`tests/ssl_redirect_tests.rs`:
- `extract_host_header` — parses Host from raw HTTP
- `extract_path_gets_correct_path` — extracts path from request line
- `extract_header_case_insensitive` — handles Host/host/HOST variants
- `generate_redirect_response` — 301 response has correct Location header

## Verification

- `cargo test -p foundation_proxy --test ssl_redirect_tests`
