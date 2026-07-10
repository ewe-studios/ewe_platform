# Spec 41 Progress

Last updated: 2026-07-10

## Completed features (committed + tested)

| # | Feature | Tests | Key decision |
|---|---------|-------|-------------|
| 01-03 | Valtron core | — | F02 Pipe<T> is the seam primitive |
| 04-11 | Netio/HTTP enablers | — | IncrementalDecoder, reactor parking |
| 12-28 | ConnectRPC crate | 167 | Unary bypasses seam; streaming uses join! |
| 29-30 | HTTP/2 substrate + multiplexers | — | H2Channel, H2Transport, h2c detection |
| **31** | **gRPC protocol over h2c** | **11** | **h2 trailer plumbing: H2StreamEvent, TransportStream.trailers** |
| 32 | h2 flow tuning | — | |
| 44-47 | Walking skeleton, backpressure, verb builders, h2 server | — | |
| **36** | **WS resumable frame decoder** | **8** | **IncrementalDecoder impl, byte-level state machine** |
| **37** | **WS server task (v1 — committed, needs Pipe migration)** | **4** | **auto-Pong, Close handshake, assembler** |
| **38** | **WS Depends read model (v1 — committed, needs caller-injected redesign)** | **inherited** | **Delayed fallback** |
| **39** | **WS transport (v1 — WsBytePump committed, needs redesign)** | **2 unit** | **To be replaced with ReconnectingWebSocketTask + Pipe** |

## In-progress / redesigned

| # | Feature | Status |
|---|---------|--------|
| 37 | WS server task + Pipe migration | Spec revised 2026-07-10, awaiting review |
| 38 | Caller-injected EventReadiness | Spec revised 2026-07-10, awaiting review |
| 39 | WsTransport via ReconnectingWebSocketTask | Spec revised 2026-07-10, awaiting review |

## Pending

| # | Feature | Depends on | Effort |
|---|---------|-----------|--------|
| 33 | QUIC backend | 10 | large |
| 34 | HTTP/3 module | 33, 29 | large |
| 35 | HTTP/3 transport | 34, 22 | large |
| 40 | Shared reactor | 10 | medium |
| 41 | uring selector | 40 | large |
| 42 | uring selection probe | 41 | medium |
| 43 | uring completion mode | 41, 42 | large |

## Current goal

1. ~~F31~~ ✅
2. F40-43 (pending — blocked on F37-F39 redesign approval)
3. F33 (pending)
4. F34-35 (pending)
5. F36 ~~(completed)~~ ✅
6. F37 ~~(v1 committed)~~ → redesign in review
7. F38 ~~(v1 committed)~~ → redesign in review
8. F39 ~~(v1 committed)~~ → redesign in review
