---
feature: "Envelope framing + gRPC timeout codec (D05 shared layer)"
description: "5-byte envelope encode/decode (zero-copy Bytes), EnvelopeReader/Writer over IncrementalDecoder, grpc-timeout"
status: "complete"
priority: "high"
phase: 1
depends_on: ["04-incremental-decoder", "13-codec-system", "14-compression-buffers"]
estimated_effort: "medium"
created: 2026-07-03
---
# Feature 15-envelope-framing: Envelope framing + gRPC timeout codec (D05 shared layer)

## Description

The shared wire-framing layer all three protocols use: zero-copy envelope decode, pooled envelope encode, and the reader/writer built on the shared IncrementalDecoder primitive.

## Normative sources (single source of truth — read before writing code)

- decisions/05-protocol-wire-formats.md — §Envelope Framing, §EnvelopeReader/Writer (normative)
- decisions/12-foundation-enablement.md — §11 (the decoder seam it must sit on)

## Scope

- Envelope { flags, data: Bytes } — decode slices zero-copy; encode_into pooled scratch (RS8)
- EnvelopeReader (frame-level only, never decodes messages) as an IncrementalDecoder impl; blocking read = step-loop wrapper
- EnvelopeWriter: write / write_end_stream / write_trailer_frame; per-envelope compression flags
- encode_grpc_timeout/decode_grpc_timeout (8-digit cap, P13)

## Out of scope

- Protocol handlers (features 19/20/31)

## Acceptance criteria

- Envelope decode of a frame spanning N reads succeeds; payload Bytes share the arrival allocation (no memcpy)
- Round-trip property tests for envelopes and grpc-timeout units
