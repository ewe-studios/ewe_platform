# 23 — Writer affinity (session stickiness)

**Date:** 2026-07-10
**Status:** Resolved

## Decision

`foundation_proxy` supports cookie-based session stickiness ("writer affinity"):
a client that performs a write (POST/PUT/PATCH/DELETE) is routed to the same
backend for subsequent requests within the session TTL. This is the same
mechanism kamal-proxy uses for stateful applications.

## Why

Stateless round-robin works for most services. But stateful apps — those with
in-memory session state, WebSocket connections, or filesystem writes — need a
guarantee that a client keeps hitting the same backend for the duration of its
session. Without stickiness:

1. **File uploads chunk across backends** — part 1 on backend A, part 2 on B.
2. **WebSocket drops on reconnect** — reconnects to a different backend that
   has no session state.
3. **CSRF token mismatch** — token generated on A, validated on B.

## Mechanism

When writer affinity is enabled for a service:

1. **First request** (any method): proxy picks a backend normally (WRB).
   Response carries `Set-Cookie: {cookie_name}={backend_id}; Path=/; Max-Age={ttl}`.
2. **Read request** (GET/HEAD/OPTIONS): if cookie is present and backend is
   healthy+active, route to the named backend. If cookie is absent or the
   named backend is unhealthy, fall through to normal WRB.
3. **Write request** (POST/PUT/PATCH/DELETE): same as read, but also **refreshes**
   the cookie's Max-Age (extending the session).

```rust
pub struct WriterAffinityConfig {
    /// Cookie name. Default: "foundation_writer".
    pub cookie_name: String,
    /// Session TTL. Default: 300s (5 minutes).
    pub ttl: Duration,
    /// Only apply stickiness after a write. If false, the first request
    /// (even a GET) sets the cookie. Default: true.
    pub require_write: bool,
}
```

## Routing logic (in handler)

```rust
fn resolve_backend(
    service: &ServiceRuntime,
    req: &SimpleIncomingRequest,
) -> Option<BackendLease> {
    let affinity = service.config().writer_affinity.as_ref();

    // Try cookie-based routing first.
    if let Some(cfg) = affinity {
        if let Some(backend_id) = extract_cookie(req, &cfg.cookie_name) {
            if let Some(backend) = service.find_healthy_backend(&backend_id) {
                if backend.is_eligible() {
                    // Refresh cookie on writes.
                    if is_write(&req.method) {
                        set_cookie = Some((cfg.cookie_name.clone(), backend_id, cfg.ttl));
                    }
                    return backend.try_reserve();
                }
            }
        }
    }

    // Fall through to normal weighted round-robin.
    let lease = service.pick()?;

    // Set initial cookie on first request.
    if affinity.is_some() {
        let id = hash_backend_url(lease.backend().url());
        set_cookie = Some((cfg.cookie_name.clone(), id, cfg.ttl));
    }

    Some(lease)
}
```

## Cookie format

```
Set-Cookie: foundation_writer=a3f2b1c; Path=/; Max-Age=300; HttpOnly; SameSite=Lax
```

The cookie value is a short hash of the backend URL — not the full URL (privacy).
Collisions are astronomically unlikely with 6 hex chars from a 64-bit hash (16M
combinations).

## Trade-offs

| Pro | Con |
|-----|-----|
| Works with any HTTP client (browser, curl, SDK) | Adds latency: cookie parse + hash lookup |
| No server-side session store needed | Backend must be tolerant of sticky sessions |
| Cookie is HttpOnly + SameSite=Lax by default | Not suitable for cross-origin API calls |
| Falls through to WRB if backend is unhealthy | Adds ~10 lines to the hot path |

## Integration

- `ServiceConfig` gains an `Option<WriterAffinityConfig>` field
- `ProxyHandler::serve()` calls `resolve_backend()` instead of `service.pick()` directly
- `ConnectionResult` gains an optional `Set-Cookie` header that the HTTP framework appends
- Feature-gated behind `default` (no feature flag — it's cheap)

## Verification

1. Service with `writer_affinity` enabled, 2 backends.
2. Client sends POST → gets cookie → subsequent GETs route to same backend.
3. Stop the sticky backend → next request falls through to the other backend,
   gets a new cookie.
4. Wait past TTL → cookie expires → new request gets fresh WRB pick.
5. Service without `writer_affinity` → no cookie set, normal WRB.
