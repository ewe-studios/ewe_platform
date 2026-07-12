---
feature: "Client<Req,Res>, options, per-call Ctx, stream facades (D07)"
description: "The typed client: unary/server/client/bidi surfaces, ClientOptions, per-call derived Ctx, GET support"
status: "complete"
priority: "high"
phase: 1
depends_on: ["19-connect-protocol", "23-h1-transport-client"]
estimated_effort: "large"
created: 2026-07-03
---
# Feature 24-client-core: Client<Req,Res>, options, per-call Ctx, stream facades (D07)

## Description

The client half: typed per-procedure clients over the streaming Transport with the propagation-first Ctx contract, split-half stream facades, and idempotent GET.

## Normative sources (single source of truth — read before writing code)

- decisions/07-client-architecture.md — entire doc is normative (contract table, facades, GET, options)

## Scope

- Client<Req,Res> { transport, config, protocol, codecs } + the four async methods
- Per-call Ctx derivation per the contract table (min-deadline merge; CancelSignal::linked; spec/peer overwritten; extensions cloned)
- ClientOptions/ClientConfig incl. codec_name selection, get_url_max_bytes/get_use_fallback/preferred_http_version/with_pipe_depth; Clone impl (R19)
- ServerStream/ClientStream/BidiStream facades over split halves + BidiSender/BidiReceiver; C1 simple variants; C3 spec()/peer(); C4 close
- Unary GET (idempotency-gated) + fallback per the decided rules

## Out of scope

- Generated typed clients (26)

## Acceptance criteria

- Caller cancel stops the call; call deadline never cancels the root ctx (linked one-way test)
- BidiStream::split enables concurrent send/receive (deadlock stress test)
- Unary/streaming round-trips against our own server (feature 22) for Connect + gRPC-Web

## Completion (2026-07-08)

All scope bullets delivered:

- `Client<Req, Res>` with four async methods (`unary`, `server_stream`, `client_stream`, `bidi_stream`)
- `ClientOptions` builder (22 methods) + `ClientConfig` (frozen, validated at `Client::new`)
- `ProtocolSelection` (Connect/Grpc/GrpcWeb) with capability matching
- Per-call `Ctx` derivation: deadline merge (min), `CancelSignal::linked`, spec/peer/extension copy
- `ServerStream<Res>`, `ClientStream<Req, Res>`, `BidiStream<Req, Res>` facades over split `ClientConn` halves
- `BidiSender<Req>` / `BidiReceiver<Res>` for concurrent send/receive (`split()`)
- Unary GET with idempotency gating + POST fallback (two deterministic cases: URL too long, 405/415)
- `ProtocolClient::new_conn` extended with `cancel: CancelSignal` parameter (cancel-composed conn halves)
- `encode_get_query` helper in `protocol::connect` for GET query encoding
- `do_unary_round_trip` internal helper (transport.open + push + close + collect)

### Verification

- **Check:** `cargo check` — zero warnings (lib + tests)
- **Tests:** 83/83 pass (10 new client_core_tests, 73 existing — zero regressions)
- **Files:** `src/client.rs` (~1310 lines), `tests/client_core_tests.rs` (383 lines)
- **Modified:** `lib.rs`, `protocol/mod.rs`, `protocol/connect.rs`, `protocol/grpc_web.rs`, `tests/connect_protocol_tests.rs`, `tests/grpc_web_tests.rs`

### Known limitation

- gRPC protocol selection returns `unimplemented` (requires HTTP/2 transport — Phase 2).
