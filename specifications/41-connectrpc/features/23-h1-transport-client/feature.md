---
feature: "HTTP/1.1 Transport impl over foundation_netio (D07/D11)"
description: "Transport::open on the netio client + HttpConnectionPool reuse; round_trip; WASM Fetch capabilities stub"
status: "pending"
priority: "high"
phase: 1
depends_on: ["07-pushable-request-body", "17-transport-seam"]
estimated_effort: "medium"
created: 2026-07-03
---
# Feature 23-h1-transport-client: HTTP/1.1 Transport impl over foundation_netio (D07/D11)

## Description

The first Transport implementation: HTTP/1.1 over the existing netio client and pool, exposing the byte-level TransportStream contract and honest capabilities.

## Normative sources (single source of truth — read before writing code)

- decisions/11-transport-seam.md — §Streaming-capable client Transport (byte-level contract)
- decisions/07-client-architecture.md — §Transport trait, Decided Details (connection reuse — verified netio facts)

## Scope

- Transport impl over netio h1 client: open → TransportStream (byte pipes + awaitable head); PreparedRequest/RequestDescriptor/SimpleResponse types (verified, no new types)
- HttpConnectionPool reuse (checkout/checkin, Connection: close honor) — transparent via the seam
- TransportCapabilities: h1 = request_streaming yes (chunked), full_duplex no; Fetch = request_streaming/full_duplex/h2_trailers false
- round_trip convenience over open

## Out of scope

- HTTP/2/3 transports (30/35)
- WASM Fetch bridge internals (foundation_wasm detail)

## Acceptance criteria

- Streaming request body rides chunked TE through the pushable pipe; response head awaitable
- Half-duplex scheduling: writer starts after request-complete for client/server-streaming
- Pool reuse observed across sequential unary calls (test)
