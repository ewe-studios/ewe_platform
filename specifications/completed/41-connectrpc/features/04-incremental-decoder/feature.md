---
feature: "IncrementalDecoder primitive (D12 §11)"
description: "Resumable frame decoding over non-blocking sources; accumulating BytesMut core; zero-copy Bytes out"
status: "complete"
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

## Verification (complete)

Implemented in `backends/foundation_core/src/io/incremental_decoder.rs` (exported from
`foundation_core::io`, alongside the `ioutils` IO utilities per D12 §11):
- `trait IncrementalDecoder { type Frame; fn step(&mut self, src: &mut impl Read) -> Result<DecodeStep<Frame>, DecodeError>; fn has_partial(&self) -> bool; }`
  and `enum DecodeStep<F> { Pending, Frame(F) }`. `Pending` is a short read (state retained),
  never an error; `DecodeError { Io, Protocol }` is reserved for genuine violations.
- `AccumulatingBuffer`: the reusable core — a task-owned `BytesMut` (survives parks, never
  thread-local, never pooled — D06 read path). `fill_from` reads one chunk directly into the
  buffer's zero-extended tail (no temp-buffer second copy); `view` gives a contiguous slice;
  `split_to(n)` hands the front out as a **zero-copy** `Bytes` (`BytesMut::split_to().freeze()`),
  `advance`/`clear` manage leftovers. `has_partial` distinguishes clean EOS from truncation.
- `read_frame_blocking(decoder, src)`: the backward-compatible `loop { step }`-until-frame
  wrapper for blocking callers — `Ok(Some(frame))`, `Ok(None)` on clean EOS at a boundary, or a
  truncated-frame `Protocol` error (via an EOF-tracking reader wrapper).
- Per-protocol codecs are out of scope (envelope F16, http2 F29, WS F36 adopt this trait).

Tests (`tests/io/incremental_decoder_tests.rs`) with a sample `[u32 len][payload]` decoder:
frame spanning 1-byte-per-read reads decodes with Pending+retained-state each read; partial
payload retains state and resumes; two frames from one buffer come out as zero-copy `Bytes`
(clone shares the same `as_ptr`); blocking wrapper reads frames then `Ok(None)` on clean EOS
and errors on truncation; a non-`WouldBlock` I/O error surfaces as `DecodeError::Io`;
`WouldBlock` yields `Pending`. `7 passed`; lib clean on default + `wasm32-unknown-unknown`.
