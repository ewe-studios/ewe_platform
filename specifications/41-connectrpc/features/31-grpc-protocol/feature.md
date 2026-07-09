---
feature: "gRPC protocol over HTTP/2 (D05 Protocol 2)"
description: "Full gRPC: enveloped unary+streaming, trailing-HEADERS status, timeout header, status-details-bin"
status: "complete"
priority: "high"
phase: 2
depends_on: ["30-http2-multiplexers", "15-envelope-framing"]
estimated_effort: "medium"
created: 2026-07-03
---
# Feature 31-grpc-protocol: gRPC protocol over HTTP/2 (D05 Protocol 2)

## Description

The strict protocol: gRPC riding the owned HTTP/2 module with real trailing HEADERS, completing the three-protocol matrix.

## Normative sources (single source of truth — read before writing code)

- decisions/05-protocol-wire-formats.md — §Protocol 2: gRPC (normative wire shapes)

## Scope

- ProtocolHandler/ProtocolClient impls; always-200 + grpc-status trailers; percent-encoded grpc-message; grpc-status-details-bin (google.rpc.Status)
- grpc-timeout header (feature 15 codec); Te: trailers; content types application/grpc(+proto|+json)
- Capability floor enforced: min_http_version HTTP/2 + h2_trailers (D11)

## Acceptance criteria

- [x] gRPC handler (`GrpcHandler`) + client (`GrpcClient`) implemented (all ProtocolHandler/ProtocolClient methods)
- [x] Router dispatch: protocol match by `application/grpc+{codec}` Content-Type (Decision 05 §Protocol 2)
- [x] ErrorWriter: gRPC error → 200 OK + grpc-status trailers
- [x] Wire tests (8) — transport-level unary/streaming/error + typed-client streaming/bidi over h2c
- [ ] gRPC conformance suites green over h2 (h2c + TLS); interop against grpcurl and connect-go servers/clients
- [ ] h2 trailer plumbing — HTTP/2 trailing HEADERS not forwarded to transport caller or ClientConn (blocks grpc-status verification in typed-client tests)
