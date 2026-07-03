---
feature: "HTTP/3 ConnectRPC transport + conformance (D01 phase 3)"
description: "Transport impl + ConnectionHandler third branch; gRPC/Connect over h3"
status: "pending"
priority: "medium"
phase: 3
depends_on: ["34-http3-module", "22-router-dispatch"]
estimated_effort: "medium"
created: 2026-07-03
---
# Feature 35-http3-transport-integration: HTTP/3 ConnectRPC transport + conformance (D01 phase 3)

## Description

Wiring HTTP/3 into the seam: the third ConnectionHandler branch and client transport, closing the any-protocol-on-any-transport matrix for QUIC.

## Normative sources (single source of truth — read before writing code)

- decisions/01-transport-and-runtime.md — phasing; decisions/11-transport-seam.md — capabilities

## Scope

- Transport impl (full_duplex, multiplexed, HTTP30); server ConnectionHandler third branch; capability matrix entries

## Acceptance criteria

- Connect + gRPC suites pass over HTTP/3; bidi full-duplex verified
