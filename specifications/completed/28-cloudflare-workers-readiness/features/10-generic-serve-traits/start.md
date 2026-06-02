---
feature: "Generic Serve Traits"
feature_directory: "specifications/28-cloudflare-workers-readiness/features/10-generic-serve-traits"
this_file: "specifications/28-cloudflare-workers-readiness/features/10-generic-serve-traits/start.md"
---

# Start: Generic Serve Traits

## Workflow

1. Read `feature.md` for full architecture details
2. Implement environment-specific connection types (`CfConn`, `WebConn`)
3. Define `CfServe` and `WebServe` traits
4. Make `RouteMethod<S>`, `RouteSegment<S>`, `Router<S>` generic — remove `Server` enum
5. Make `HttpApp<S>` generic — add `route_writer_cf` / `route_writer_web`
6. Update wasm dispatch to use `CfConn` / `WebConn` instead of `WasmStream` + `parse_raw`
7. Update bridge modules (`cf.rs`, `web.rs`) to use new connection types
8. Write tests for all new types
9. Verify both native and wasm builds compile cleanly
