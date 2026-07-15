---
feature: "HTTP/1.1 per-part response iterators + trailers part (D12 §1–§3)"
description: "Streaming writes: emit head immediately, flush per body chunk, trailers part; atomic path preserved"
status: "complete"
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

## Verification (complete)

In `backends/foundation_netio/src/simple_http/shared/impls.rs`:
- `SimpleOutgoingResponse` gains a `trailers: SimpleHeaders` field (empty default; builder
  `with_trailers`/`add_trailer`). Wire-indistinguishable "no trailers" vs "empty" → plain
  `SimpleHeaders`, not `Option` (Decided Details #2).
- Shared byte renderers (`render_status_line`, `render_header_block`, `render_trailers`) and
  `Http11Chunk { Chunked, Raw }` with `render()` — chunked `{len:x}\r\n…\r\n` framing lives in
  the part, not any protocol (§2 / B5c#3). The atomic `Http11ResponseIterator`'s Intro/Headers
  states now call the same helpers, so the whole-response path is a **composition of the parts**
  (byte-identical — the 557-test simple_http suite passes unchanged).
- New `Http11` variants + constructors + `RenderHttp` wiring: `ResponseStatusLine(Status)`,
  `ResponseHeaders(SimpleHeaders)`, `ResponseHead(SimpleResponse<()>)`,
  `ResponseBodyChunk(Http11Chunk)`, `ResponseTrailers(SimpleHeaders)`, each backed by a
  one-/two-shot part iterator. `ResponseTrailers` with empty trailers = the bare chunked
  terminator `0\r\n\r\n`; interim 1xx = a header-less `ResponseHead` emitted before the final.
- Per-message flush needs no new mechanism (§1): the `Serve` handler owns the connection writer.

Cross-crate: added the `trailers` field to 5 struct literals in `foundation_http` tests
(non-behavioral).

Tests (`tests/simple_http/http11_response_parts_tests.rs`, custom lowercase headers so they're
independent of feature 06): each part's exact wire bytes; chunked hex framing; raw passthrough;
trailers terminator+block; the streaming composition head→2 chunks→trailers; interim-1xx head;
and the `trailers` field carrying. `9 passed`; existing simple_http `557 passed`, http_stream +
event_source `67 passed`, 0 failed; netio lib clean.
