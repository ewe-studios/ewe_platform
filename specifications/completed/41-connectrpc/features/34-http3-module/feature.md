---
feature: "HTTP/3 module: framing + QPACK over the QUIC traits (D01 roadmap)"
description: "h3-design replicated tokio-free: frame codec, QPACK, connection/stream mapping over QuicConnection"
status: "complete"
priority: "medium"
phase: 3
depends_on: ["33-quic-backend", "29-http2-substrate"]
estimated_effort: "large"
created: 2026-07-03
---
# Feature 34-http3-module: HTTP/3 module: framing + QPACK over the QUIC traits (D01 roadmap)

## Description

The third transport: HTTP/3 framing replicated from h3's design over our valtron-native QUIC traits — same Router, same handler types.

## Normative sources (single source of truth — read before writing code)

- decisions/01-transport-and-runtime.md — §Future-Phase Transport Roadmap (HTTP/3) — normative deviations list

## Scope

- http3/ module: frame codec (IncrementalDecoder), QPACK, request/response mapping to Simple types; quic.rs backend-agnostic trait boundary

## Moved to feature 35 (2026-07-10)

- ~~Alt-Svc advertisement (T2); ConnectionContext population at connection setup~~

Both are server-integration concerns. `Alt-Svc: h3=...` is a header an **HTTP/1.1 or
HTTP/2 response** carries to advertise that the origin also speaks HTTP/3 — it is emitted by
the TCP server, not by the HTTP/3 module, and there is no TCP server to emit it from until
F35 routes `HttpServer` through netcap's `Listener`. `ConnectionContext` is populated by the
front end at accept/handshake time (D12 §13); `H3Connection` already threads an
`Arc<ConnectionContext>` through `request_from_fields`, so F35 has only to fill it in.

## Acceptance criteria

- HTTP/3 request/response round-trip to the shared Router; produces/consumes the universal Simple types

## Completed 2026-07-10

### What shipped

- **`http3/varint.rs`** — QUIC variable-length integers (RFC 9000 §16). Every frame type,
  length, setting, stream type and error code in HTTP/3 is one of these. Over-long
  (non-canonical) encodings are accepted, as §16 requires; a short read is `Ok(None)`, never
  an error, because the frame decoder resumes on it.
- **`http3/frame.rs`** — the frames of RFC 9114 §7 and `FrameDecoder`, an `IncrementalDecoder`
  (D12 §11). A QUIC stream hands a frame's bytes over in arbitrary chunks, so the codec has to
  be resumable. Unknown frame types decode as `Frame::Unknown` and are **ignored** (§9), which
  is what lets a newer peer's grease frames pass; a decoder that rejected them could not talk
  to any later draft. Oversized frames are refused on the announced length alone, before
  anything is buffered.
- **`http3/qpack/`** — RFC 9204, with the dynamic table disabled
  (`QPACK_MAX_TABLE_CAPACITY = 0`, `QPACK_BLOCKED_STREAMS = 0`). §3.2.3 permits this
  explicitly; it is a conforming QPACK, not a subset. It removes the encoder and decoder
  streams, insert-count accounting and dynamic-table head-of-line blocking — roughly two thirds
  of an implementation, and the two thirds where the subtle bugs live. The five representations
  that can only reference the dynamic table are rejected loudly rather than misparsed. Huffman
  is shared with HPACK (§5).
- **`http3/types.rs`** — the mapping onto the universal Simple types. HTTP/3 and HTTP/2 share
  their pseudo-headers verbatim (§4.3 defers to RFC 9113 §8.3) and both compressions decode to
  `(name, value)` pairs, so this **reuses** `http2::types::header_from_hpack` rather than
  duplicating the folding, URI composition and routing-key derivation. It adds only what HTTP/3
  forbids: §4.2's connection-specific fields, `TE` other than `trailers`, uppercase field names
  (§4.1.1), and pseudo-headers after a regular field (§4.3).
- **`http3/stream.rs`** — the bridge from `QuicRecvStream`'s `Stream<Result<Option<Bytes>>>` to
  the decoder's `io::Read`. A FIN with bytes still in the decoder is a **truncated frame**, not
  a clean close; `has_partial()` is what tells them apart.
- **`http3/connection.rs`** — `H3Connection` and `H3Request`. Opens the control stream, sends
  SETTINGS first (§6.2.1), reads the peer's, and accepts request streams. Everything is
  progress-returning like the QUIC traits beneath it: no `Poll`, no `Waker`.

### Bugs found while building it

- **HTTP/2, not HTTP/3.** `header_from_hpack` mapped every method that was not GET onto POST,
  so PUT, DELETE and PATCH over h2 were silently rewritten. gRPC only ever sends POST, which is
  why it went unnoticed. Caught by reusing the code for HTTP/3 and testing every method.
- **`FramedRecv` stranded frames.** It skipped stepping the decoder when its own byte buffer
  was empty — but `FrameDecoder` keeps an accumulating buffer of its own, and a previous step
  routinely leaves whole frames in it. The DATA frame after HEADERS was therefore never
  decoded, and the subsequent FIN looked like a truncated frame. Found by the round-trip test.
- **`compression = ["dep:brotli"]`** suppressed the implicit `brotli` feature that the code's
  `cfg(feature = "brotli")` guards check, so every brotli path had been compiling out silently.

### Acceptance

`tests/http3/` — 48 tests.

- `varint_tests` — RFC 9000's own worked examples, the length-class boundaries, the
  non-canonical encodings a receiver must accept, the partial-input contract.
- `frame_tests` — round-trips, a frame split across **every** byte boundary, grease frames,
  SETTINGS with a repeated identifier, GOAWAY with trailing bytes, an oversized frame.
- `qpack_tests` — RFC 9204's Appendix B.1 field section verbatim, every representation, the
  static table's boundaries, all five dynamic-table rejections, and a decompression bomb.
- `types_tests` — the pseudo-header contract, §4.2's forbidden fields, `:status` as a bare
  code, and `every_method_survives_the_mapping` (verified by mutation to catch the h2 bug).
- `connection_tests` — **the acceptance criterion**: a real HTTP/3 request and response
  round-tripping over live QUIC on loopback, arriving as a `SimpleIncomingRequestHeader` and
  leaving as a `SimpleOutgoingResponse`. Plus `H3_FRAME_UNEXPECTED` for a request stream that
  opens with DATA, and `H3_MISSING_SETTINGS` for a control stream whose first frame is not
  SETTINGS.

netio: 1190 tests pass, crate is warning-free.
