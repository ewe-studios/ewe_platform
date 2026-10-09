---
feature: "Writer affinity — sticky sessions via cookie"
description: "Client affinity via __proxy_sticky cookie: requests from the same client route to the same backend, falling back to weighted round-robin on miss/unhealthy"
status: "complete"
priority: "medium"
phase: 4
depends_on: ["10-networking-model", "11-wait-strategies"]
estimated_effort: "small"
created: 2026-07-16
---
# Feature 13: Writer Affinity — Sticky Sessions (Decision 23)

## What

When a service has multiple backends, the proxy sets a `__proxy_sticky=<idx>`
cookie on every response. On subsequent requests, the same client is routed
to the same backend (if it's still eligible). Single-backend services don't
emit the cookie.

## Implementation

### runtime.rs — `pick_sticky()`

`ServiceRuntime::pick_sticky(Option<usize>)` — if given an index, tries that
backend first (eligibility-gated). Falls back to smooth weighted round-robin.
Returns `(BackendLease, usize)` — the index the caller should set in the cookie.

### handler.rs — cookie extraction + injection

- `extract_sticky_cookie(req)` — parses `Cookie: __proxy_sticky=<idx>` from
  incoming request headers
- `serve()` calls `pick_sticky(sticky_idx)` instead of `pick()`
- Builds `Set-Cookie: __proxy_sticky=<idx>; Path=/; HttpOnly` when multi-backend
- Passes sticky header through relay to `forward_http_with_headers`

### forward.rs — `forward_http_with_headers`

New function that accepts `extra_headers: Option<&SimpleHeaders>`. `write_response`
injects them after stripping hop-by-hop headers. Original `forward_http` delegates
to the new function with `extra_headers: None`.

## Tests added

`runtime.rs` (inline mod tests):
- `sticky_pick_routes_to_specific_backend` — supplies cookie index, gets that backend
- `sticky_falls_back_on_ineligible` — sticky target unhealthy → round-robin
- `single_backend_no_loop` — one-backend service, sticky index works

`forward.rs` (inline mod tests):
- `extra_headers_injected_into_response` — Set-Cookie appears in output

## Verification

- `cargo test -p foundation_proxy --lib`
