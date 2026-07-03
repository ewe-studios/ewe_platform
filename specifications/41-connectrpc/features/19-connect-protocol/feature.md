---
feature: "Connect protocol handler + client (D05)"
description: "Connect unary POST/GET + streaming: content-type mechanics, exchanges, EndStreamResponse, GET query encoding"
status: "pending"
priority: "high"
phase: 1
depends_on: ["15-envelope-framing", "17-transport-seam"]
estimated_effort: "large"
created: 2026-07-03
---
# Feature 19-connect-protocol: Connect protocol handler + client (D05)

## Description

The flagship protocol on HTTP/1.1 from day one: full Connect unary (POST + idempotent GET) and streaming with the normalized Frame layering, protocol-owned enveloping tasks, and exact wire conformance.

## Normative sources (single source of truth — read before writing code)

- decisions/05-protocol-wire-formats.md — §Protocol 1: Connect (wire shapes are normative), §Protocol Implementation Trait, Decided Details
- decisions/07-client-architecture.md — §Unary Call Flow, §HTTP GET, §GET Fallback

## Scope

- ProtocolHandler/ProtocolClient impls for Connect; new_conn → HandlerExchange/ClientExchange (reader/writer tasks + conn)
- Unary POST (bare body) + unary error (always application/json, code→HTTP status) + Trailer- prefixed metadata
- Streaming (connect+{codec}; Connect-Content/Accept-Encoding; always 200; EndStreamResponse flags=0x02)
- Unary GET: stable-codec query encoding (compression before base64url), Vary/cacheable (P11/P14), 405/415/505 + Accept-Post, RequireConnectProtocolHeader (P7)
- Content-type canonicalization (P6/charset) + protocol constants

## Out of scope

- gRPC-Web (20)
- gRPC (31 — needs HTTP/2)

## Acceptance criteria

- Wire shapes byte-match the doc examples in tests; error JSON matches the Connect spec
- GET→POST fallback fires only on URL-length precheck and 405/415 (never transport errors)
- Streaming end-of-stream normalizes to Frame::EndStream and back
