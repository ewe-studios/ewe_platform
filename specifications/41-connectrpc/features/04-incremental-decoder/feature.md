---
feature: "IncrementalDecoder primitive (D12 §11)"
description: "Resumable frame decoding over non-blocking sources; accumulating BytesMut core; zero-copy Bytes out"
status: "pending"
priority: "high"
phase: 1
depends_on: []
estimated_effort: "medium"
created: 2026-07-03
---
# Feature 04-incremental-decoder: IncrementalDecoder primitive (D12 §11)

## Description

The shared IncrementalDecoder seam: frame decoders that retain partial state across reads and yield Pending on short reads, never errors. One accumulating-buffer core (task-owned BytesMut — it must survive parks, so never thread-local). Adopted by the envelope reader, the http2 frame codec, HTTP/3 framing and the WebSocket decoder.

## Normative sources (single source of truth — read before writing code)

- decisions/12-foundation-enablement.md — §11 (normative trait + rules)
- decisions/06-compression.md — §Buffer Pool (read path is task-owned, NOT pooled)

## Scope

- trait IncrementalDecoder { type Frame; fn step(&mut self, src) -> Result<DecodeStep<Frame>, DecodeError> }
- Accumulating buffer core (leftover carry, contiguous view, completed payloads out as Bytes via split/freeze)
- Composes with Decision 00 parking: Pending on empty socket → Depends via reactor, timeout-poll fallback
- Blocking callers get loop{step}-until-Frame wrappers (backward compatible)

## Out of scope

- Per-protocol codecs (their own features: envelope 16, http2 29, WS 36)

## Acceptance criteria

- A frame spanning N reads decodes without error; a short read yields Pending with state retained
- Completed payloads are Bytes slices — no memcpy on the identity path
