---
feature: "Transport seam: Frame, split conns, facades, capability matching (D11)"
description: "HandlerConn/ClientConn + halves, MessageSink/Source, Frame pipes, Transport trait, TransportError, check_compatible"
status: "pending"
priority: "high"
phase: 1
depends_on: ["02-pipe-primitive", "13-codec-system", "16-ctx-request-context"]
estimated_effort: "large"
created: 2026-07-03
---
# Feature 17-transport-seam: Transport seam: Frame, split conns, facades, capability matching (D11)

## Description

The centerpiece: the byte/frame seam that decouples the three RPC protocols from every transport — split conn halves for concurrent read/write, typed facades holding the codec, capability matching as data, transport-level errors mapped to codes.

## Normative sources (single source of truth — read before writing code)

- decisions/11-transport-seam.md — entire doc is normative (layering, split rationale, EOS semantics, capabilities)

## Scope

- Frame enum (Message(Bytes) | EndStream{error, trailers}); FramePipe usage per §Core primitive
- HandlerConn (spec/peer/request_headers/split) → ConnReceiver + ConnSender (send_headers/send/close(error,trailers))
- ClientConn (spec/request_headers_mut/split) → ClientSender (request_headers_mut/flush_headers/send/close_send) + ClientReceiver (receive with Err-on-terminal-EOS, response_headers/trailers)
- MessageSink<T>/MessageSource<T> typed facades (codec + message-middleware host); async fns; flow control awaited never errored
- Transport trait (capabilities + open→TransportStream byte pipes) + round_trip convenience + TransportError with Code mapping
- CallRequirements/TransportCapabilities/check_compatible per (protocol × stream type); cancellation composed into pipe parking (AnyReadiness)

## Out of scope

- Protocol handler impls (19/20/31)
- h1 transport client (23)

## Acceptance criteria

- Bidi over one conn: concurrent send/receive via split halves — deadlock stress test passes at depth 4
- check_compatible reproduces the Fetch matrix (unary/server-stream allowed; client/bidi rejected) and the 505/gRPC rules
- receive() returns Err on terminal EndStream error, Ok(None) on clean EOS; a parked receive wakes on cancel
