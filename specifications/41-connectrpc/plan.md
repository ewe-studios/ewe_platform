# Spec 41: foundation_connectrpc — ConnectRPC for the EWE Platform

## Origin

A friend introduced connectrpc to me and i love this:

The site: /home/darkvoid/Boxxed/@formulas/src.rust/src.RPC/src.connect-protocol/
Conformance: /home/darkvoid/Boxxed/@formulas/src.rust/src.RPC/src.connect-protocol/conformance/
The go implementation: /home/darkvoid/Boxxed/@formulas/src.rust/src.RPC/src.connect-protocol/connect-go/
Examples: /home/darkvoid/Boxxed/@formulas/src.rust/src.RPC/src.connect-protocol/examples-go/
Authn: /home/darkvoid/Boxxed/@formulas/src.rust/src.RPC/src.connect-protocol/authn-go/

## Goal

Port ConnectRPC to Rust as `foundation_connectrpc`, built on top of the platform's existing foundation crates (foundation_core, foundation_http, foundation_netio, foundation_auth, foundation_arrow, foundation_errstacks). The connect-go implementation is the primary design reference.

## Key Decisions

1. **Independent implementation** — not a wrapper around connect-rust. connect-go is the sole design reference.
2. **Port to foundation types** — Replace tower/hyper/tokio with foundation_http handlers, foundation_netio HTTP types, valtron execution.
3. **Abstract codec trait** — Three first-class codecs: buffa (protobuf), serde_json (JSON), foundation_arrow (Arrow IPC).
4. **Auth on foundation_auth** — Port authn-go's middleware pattern, delegate verification to foundation_auth's JWT/OAuth/session infrastructure.

## Design Documents

| # | Decision | Status |
|---|---|---|
| [01](decisions/01-transport-and-runtime.md) | Transport Layer & Runtime Model | draft |
| [02](decisions/02-codec-and-serialization.md) | Codec System & Serialization | draft |
| [03](decisions/03-error-model.md) | Error Model | draft |
| [04](decisions/04-handler-and-interceptor-model.md) | Handler & Interceptor Model | draft |
| [05](decisions/05-protocol-wire-formats.md) | Protocol Wire Formats | draft |
| [06](decisions/06-compression.md) | Compression System | draft |
| [07](decisions/07-client-architecture.md) | Client Architecture | draft |
| [08](decisions/08-router-and-dispatch.md) | Router & Multi-Protocol Dispatch | draft |
| [09](decisions/09-auth-middleware.md) | Authentication Middleware | draft |
| [10](decisions/10-codegen.md) | Code Generation | draft |

## Reference Material

- **Connect protocol spec**: connectrpc.com/docs/protocol.md — full ABNF grammar for wire formats
- **connect-go source**: The mature reference implementation (v1.20.0-dev)
- **buffa**: Pure Rust protobuf with editions, zero-copy views, no_std capable
- **Conformance tests**: 33+ YAML test suites covering all protocols, codecs, compression, streaming, errors, timeouts, TLS

## Crate Structure (Planned)

```
backends/foundation_connectrpc/          # Runtime library
backends/foundation_connectrpc_codegen/  # Code generation library + protoc plugin
backends/foundation_connectrpc_build/    # build.rs helper
```

## Open Questions (Aggregated)

Collected from all design docs — must be resolved before feature specs:

### Transport & Runtime
- Does foundation_netio have HTTP/2 frame-level support, or is it HTTP/1.1 only?
- Does foundation_http handle chunked transfer encoding automatically for streaming responses?
- What backpressure mechanism exists for streaming bodies?
- Does foundation_http's worker model consume the full request body before dispatching?

### Codec & Serialization
- Does buffa's `json` feature produce canonical protobuf JSON (lowerCamelCase, string enums)?
- Can we integrate buffa's zero-copy `MessageView<'a>` into the codec trait?
- What batch size policy for Arrow IPC in streaming RPCs?

### Error Model
- Error details without protobuf: require buffa for details, or support JSON-only as extension?
- Include `"debug"` JSON in error details: always, never, or configurable?

### Handler & Interceptor
- Sync handler traits vs valtron `Stream<D, P>` return for deferred execution?
- How do long-running handlers check for context cancellation?
- How does bidi streaming work on a single worker thread?

### Protocol
- gRPC-Web text mode (base64 body): implement in Phase 1 or defer?
- `google.rpc.Status` protobuf: hand-write, generate, or well-known type?
- Does foundation_netio support HTTP/1.1 trailing headers?

### Client
- What client types does foundation_netio provide for outgoing HTTP requests?
- Connection pooling/reuse for HTTP/1.1?
- Client streaming body accumulation: default buffer limit?

### Router & Dispatch
- Register per-procedure routes or single prefix route in foundation_http?
- Middleware ordering: ConnectRPC interceptors inside or outside foundation_http middleware?

### Auth
- mTLS certificate extraction from foundation_netio TLS connections?
- OAuth token introspection authenticator?
- CORS headers for ConnectRPC-specific header names?

### Codegen
- Unified buffa+connectrpc codegen or separate steps?
- Default `unimplemented` implementations for generated service traits?
- Generate view handler variants for zero-copy deserialization?
