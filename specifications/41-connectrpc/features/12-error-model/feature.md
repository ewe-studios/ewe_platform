---
feature: "Error model: Code, ConnectError, ErrorTrace, details (D03)"
description: "The full errstacks-based error model: Code enum, ConnectError context, ErrorDetail, wire shapes"
status: "complete"
priority: "high"
phase: 1
depends_on: []
estimated_effort: "medium"
created: 2026-07-03
---
# Feature 12-error-model: Error model: Code, ConnectError, ErrorTrace, details (D03)

## Description

Foundation of the crate: the protocol-agnostic error model on foundation_errstacks — ErrorTrace<ConnectError> everywhere on RPC surfaces, domain errors as custom contexts, google.protobuf.Any-based details that work regardless of codec.

## Normative sources (single source of truth — read before writing code)

- decisions/03-error-model.md — entire doc is normative (mappings, detail layering, ConnectResult scope)

## Scope

- Code (16 variants) + http_status/from_http_status/grpc_code per the tables
- ConnectError (context type for ErrorTrace) + convenience constructors + From<ConnectError> for ErrorTrace
- ErrorDetail (google.protobuf.Any semantics; from_errstacks well-known detail); WireError/WireErrorDetail/EndStreamResponse serde shapes
- not_modified: Code::Unknown + private sentinel (GET-only 304 signaling); wrap_if_* helpers, code_of
- ConnectResult norm scoped to RPC surfaces; CodecError/CompressionError/EnvelopeError/TransportError feed in via change_context

## Out of scope

- ErrorWriter (feature 21 — needs codecs/envelope)

## Acceptance criteria

- Code↔HTTP mapping matches the D03 tables exactly; JSON error serialization matches the Connect spec byte-for-byte in tests
- Domain errors flow via From/change_context; no bare Result<_, ConnectError> in public API
