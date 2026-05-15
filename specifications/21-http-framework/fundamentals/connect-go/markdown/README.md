# connect-go — Documentation Index

The reference Go implementation of ConnectRPC — a protocol-agnostic RPC framework supporting Connect, gRPC over HTTP/2, and gRPC-Web over HTTP/1.1.

## Core Documents

| # | Document | Description |
|---|----------|-------------|
| 00 | [Overview](00-overview.md) | Architecture overview, protocol comparison, error codes, key files |
| 01 | [Protocol Abstraction](01-protocol-abstraction.md) | The `protocol` interface, dual dispatch, handler/client interfaces |
| 02 | [Envelope Framing](02-envelope-framing.md) | 5-byte framing, flags, `envelopeWriter`/`envelopeReader`, discard limits |
| 03 | [Codec System](03-codec-system.md) | `Codec` interface, `marshalAppender`, `stableCodec`, proto/JSON codecs |
| 04 | [Error Handling](04-error-handling.md) | Error wrapping chain, RST_STREAM mapping, wire errors, protobuf Any details |
| 05 | [Connect Protocol](05-connect-protocol.md) | Unary GET support, end-stream envelopes, JSON error bodies, code-to-HTTP mapping |
| 06 | [gRPC Protocols](06-grpc-protocols.md) | gRPC over HTTP/2 and gRPC-Web over HTTP/1.1, trailers, timeout encoding, percent encoding |
| 07 | [Interceptor Architecture](07-interceptor-architecture.md) | `Interceptor` interface, reverse-order chain, sentinel context checks |
| 08 | [Compression & Buffers](08-compression-buffers.md) | `sync.Pool` for compression and buffers, asymmetric negotiation, 8MiB recycle limit |
| 09 | [Handler Lifecycle](09-handler-lifecycle.md) | `Handler` struct, `ServeHTTP` dispatch, constructor variants, header merging |
| 10 | [Client Lifecycle](10-client-lifecycle.md) | `Client[Req, Res]` generic, `duplexHTTPCall` transport, call methods |

## Implementation Guides

| Guide | Description |
|-------|-------------|
| [Implementing ConnectRPC](implementing-connectrpc-protocol.md) | Complete blueprint for implementing the Connect protocol from scratch |
| [Implementing gRPC](implementing-grpc-protocol.md) | Complete blueprint for implementing gRPC over HTTP/2 natively |
| [Implementing gRPC-Web](implementing-grpcweb-protocol.md) | Complete blueprint for implementing gRPC-Web over HTTP/1.1 natively |

## Source

All documents reference the Go source at `/home/darkvoid/Boxxed/@formulas/src.rust/src.http-nu/src.connect-protocol/connect-go/`.

## Build

```bash
python3 build.py
```

Generates HTML in `html/` from markdown in `markdown/`.
