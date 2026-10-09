# foundation_http — HTTP framework and middleware

## What it is
HTTP server framework with middleware pipeline, routing, and response generation.

## Key modules
- **`middleware/`** — Pluggable middleware: auth, CORS, compression, logging,
  body size limits, rate limiting.
- **`routing/`** — URL-based routing with pattern matching.
- **`response/`** — Typed response builders (JSON, HTML, binary, streaming).

## Middleware chain
```
Request → Auth → CORS → Compression → Logger → Handler → Response
```

Each middleware can short-circuit (return early) or pass through. Body limit
middleware rejects oversized payloads before they reach the handler.

## Integration
- Works with `foundation_netio`'s HTTP server as the transport layer.
- Middleware can use `SessionAccessProvider` for auth checks.
