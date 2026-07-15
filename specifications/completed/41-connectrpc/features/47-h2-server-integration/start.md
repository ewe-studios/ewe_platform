---
spec_directory: "specifications/41-connectrpc"
feature: "47-h2-server-integration"
created: 2026-07-09
---

# Start: H2ConnectionHandler, ServeH2, h2c detect, ConnectRpcServeH2

1. Read `plan.md`, then Decision 12 (§5, §6, §Decided Details #3/#4) —
   the decisions are normative; this feature file is the work unit.

2. Read `.agents/skills/rust-clean-code/` — tests in `tests/`.

3. Implement in order:
   a. `ServeH2` trait in `foundation_http`
   b. `H2ConnectionHandler` (valtron TaskIterator) in `foundation_http`
   c. h2c detection branch in `HttpServer::serve_loop()`
   d. `ConnectRpcServeH2` in `foundation_connectrpc`
   e. `encode_h2_response()` helper
   f. `h2_echo` example

4. Verify: `cargo check` all crates, `cargo test`, run example.
