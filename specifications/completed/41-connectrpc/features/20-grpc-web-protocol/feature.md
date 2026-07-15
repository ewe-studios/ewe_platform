---
feature: "gRPC-Web protocol handler + client incl. text mode (D05)"
description: "gRPC-Web over HTTP/1.1: in-body 0x80 trailers, trailers-only responses, whole-body base64 text mode"
status: "complete"
priority: "high"
phase: 1
depends_on: ["15-envelope-framing", "17-transport-seam"]
estimated_effort: "medium"
created: 2026-07-03
---
# Feature 20-grpc-web-protocol: gRPC-Web protocol handler + client incl. text mode (D05)

## Description

Browser-reach protocol on HTTP/1.1: gRPC framing with in-body trailer frames, plus the whole-body base64 text mode with a correct streaming base64 adapter.

## Normative sources (single source of truth — read before writing code)

- decisions/05-protocol-wire-formats.md — §Protocol 3: gRPC-Web + Decided Details #1 (text mode is Phase 1)

## Scope

- ProtocolHandler/ProtocolClient impls; trailer frame (0x80) as final body chunk; trailers-only-as-headers (P8)
- Text mode: stateful base64 stream adapter OUTERMOST on the body (partial 3-byte groups across chunk boundaries)
- grpc-status/message/status-details-bin encoding incl. percent-encoding + P9 details preference; X-User-Agent (P12)

## Out of scope

- Real HTTP/2 trailers (feature 31)

## Acceptance criteria

- Conformance vectors for binary and text modes pass; fragmented base64 boundaries decode correctly
- Status precedence: Grpc-Status-Details-Bin over Grpc-Status/Message when both present
