# connect-go — Spec

## Source

- **Path:** `/home/darkvoid/Boxxed/@formulas/src.rust/src.http-nu/src.connect-protocol/connect-go/`
- **Language:** Go
- **Package:** `connect` — connectrpc.com/connect
- **License:** Apache 2.0
- **Core source files:** 20 `.go` files (excluding tests, internal examples, and generated testdata)

## What It Is

The reference Go implementation of ConnectRPC — a protocol-agnostic RPC framework supporting Connect, gRPC over HTTP/2, and gRPC-Web over HTTP/1.1. Uses a clean `protocol` interface abstraction where each protocol is a separate type, with typed generic handlers, envelope-based streaming, and an interceptor middleware architecture.

## Documentation Goal

1. Understand the `protocol` interface and dual dispatch strategy (handler/client)
2. Understand the 5-byte envelope framing for streaming
3. Understand the codec system (Codec, marshalAppender, stableCodec)
4. Understand error handling (wrapping chain, RST_STREAM mapping, wire errors, error details)
5. Understand the Connect protocol (unary GET support, end-stream envelopes, JSON error bodies)
6. Understand the gRPC protocol (HTTP/2 trailers, gRPC-Web body trailers, timeout encoding, percent encoding)
7. Understand the interceptor architecture (interface, chain composition, sentinel context checks)
8. Understand compression negotiation, buffer pooling, and performance design

## Structure

```
connect-go/
├── spec.md
├── markdown/
│   ├── README.md
│   ├── 00-overview.md
│   ├── 01-protocol-abstraction.md
│   ├── 02-envelope-framing.md
│   ├── 03-codec-system.md
│   ├── 04-error-handling.md
│   ├── 05-connect-protocol.md
│   ├── 06-grpc-protocols.md
│   ├── 07-interceptor-architecture.md
│   ├── 08-compression-buffers.md
│   ├── 09-handler-lifecycle.md
│   └── 10-client-lifecycle.md
├── html/
│   └── ... (generated)
```

## Tasks

| Task | Status |
|------|--------|
| Read all 20 core source files | DONE |
| Write spec.md | DONE |
| Write 00-overview.md | DONE |
| Write 01-protocol-abstraction.md | DONE |
| Write 02-envelope-framing.md | DONE |
| Write 03-codec-system.md | DONE |
| Write 04-error-handling.md | DONE |
| Write 05-connect-protocol.md | DONE |
| Write 06-grpc-protocols.md | DONE |
| Write 07-interceptor-architecture.md | DONE |
| Write 08-compression-buffers.md | DONE |
| Write 09-handler-lifecycle.md | DONE |
| Write 10-client-lifecycle.md | DONE |
| Write implementing-connectrpc-protocol.md | DONE |
| Write implementing-grpc-protocol.md | DONE |
| Write implementing-grpcweb-protocol.md | DONE |
| Write README.md | DONE |
| Generate HTML via build.py | DONE |
| Grandfather review | TODO |

## Build System

```bash
python3 build.py connect-go
```

## Quality Requirements

Per documentation_directive.md — minimum 2 mermaid diagrams, 3 code snippets with file paths, 1 Aha moment per document. All names/numbers/flows verified against source.

## Expected Outcome

A reader can implement all three ConnectRPC protocols (Connect, gRPC, gRPC-Web) from scratch, understanding every envelope flag, header, timeout encoding, error mapping, and interceptor pattern.

## Resume Point

All 14 markdown docs written (11 numbered + README + 3 implementation guides). HTML generated (15 files + index + styles.css). Awaiting grandfather review.
