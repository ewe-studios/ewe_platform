# foundation_toolings

Development tooling server: reverse proxy, file watcher, hot reload — built on
valtron iterators and native APIs with **zero async runtime dependencies**.

## Architecture

- **Valtron TaskIterators** for file watching, building, and binary management
- **foundation_http** (`HttpServer`, `HttpApp`, `Serve`) for HTTP proxy and SSE
- **TunnelProxy** for raw TCP tunneling on the same port
- **BackgroundJobRegistry** for blocking build operations (cargo, wasm-pack)
- **QueueReadiness** for zero-CPU-spinning event-driven task parking

## Zero async runtime deps

No tokio, no axum, no hyper, no tower, no async-trait. All I/O uses
`std::io::{Read, Write}` and `std::net`, coordinated by valtron's synchronous
executor.
