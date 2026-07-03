---
feature: "HTTP/1.1 per-part response iterators + trailers part (D12 §1–§3)"
description: "Streaming writes: emit head immediately, flush per body chunk, trailers part; atomic path preserved"
status: "pending"
priority: "high"
phase: 1
depends_on: []
estimated_effort: "large"
created: 2026-07-03
---
# Feature 05-http11-part-iterators: HTTP/1.1 per-part response iterators + trailers part (D12 §1–§3)

## Description

Decompose Http11ResponseIterator into per-part iterators (status line / header block / head convenience / body chunk / trailers) with new Http11 variants, a trailers field on SimpleOutgoingResponse, and handler-owned per-frame flush. The existing whole-response path becomes a composition of the same parts — current HTTP/1.1 tests must pass unchanged.

## Normative sources (single source of truth — read before writing code)

- decisions/12-foundation-enablement.md — §1, §2, §3 + Decided Details #1/#2 (normative Http11 variants)
- decisions/05-protocol-wire-formats.md — trailer mechanisms per protocol

## Scope

- New Http11 variants: ResponseStatusLine, ResponseHeaders, ResponseHead(SimpleResponse<()>), ResponseBodyChunk(Http11Chunk), ResponseTrailers
- Chunked wire framing lives in the part iterator — protocols never hand-roll {len:x} framing
- SimpleOutgoingResponse.trailers: SimpleHeaders (empty default); writers emit iff non-empty
- Per-message flush via the handler-owned connection writer (D12 §1); atomic path = composition of the parts

## Out of scope

- HTTP/2 (features 29–30)

## Acceptance criteria

- A streaming handler emits head → N flushed chunks → trailers; interim 1xx = repeated ResponseHead
- All existing HTTP/1.1 tests pass unchanged (guiding constraint of Decision 12)
