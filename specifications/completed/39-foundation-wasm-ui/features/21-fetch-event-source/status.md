# Feature 21 — Status: COMPLETE (2026-06-12)

## What shipped (foundation-wasm-ui.js)

- **`SseParser`** — incremental, chunk-boundary-safe SSE parser mirroring the
  Rust `foundation_netio::event_source::shared::sse::SseParser` semantics
  exactly (id rejects NUL, exactly-one leading value space, multi-line data
  joined with `\n`, comments surfaced immediately, empty line dispatches only
  with accumulated data, EOF flushes, no-colon lines ignored — the Rust
  impl's reading, kept for cross-side consistency, noted as a W3C deviation),
  PLUS the byte-stream concerns: streaming `TextDecoder` for split UTF-8
  runes, CRLF/LF/CR endings including a CRLF split across chunks, partial
  trailing lines buffered.
- **`FetchEventSource`** — SSE over `fetch()` with ANY method/headers/body
  (the browser `EventSource` is GET-only — the reason this feature exists,
  per the Datastar approach): streams `response.body` through the parser,
  reconnects on stream end/error with the F06 backoff (server `retry:`
  overrides the delay; backoff resets on successful open), carries
  `Last-Event-ID` on reconnects, `close()` aborts the in-flight fetch and
  cancels retries.
- **`SSETransport`** reworked onto the owned stack: no browser `EventSource`
  anywhere; `config.method` defaults to POST when `data` is given (JSON
  body + content-type), GET otherwise; the G46 `event:`-field demux
  (html/arrow/json, unnamed ⇒ html) unchanged.

## Verification

20 tests: the full parser battery mirroring the Rust unit tests
(simple/id-persistence/event-type/multiline/comment/empty-lines/retry-
validation/EOF-flush/NUL-id/unknown+no-colon/CRLF+lone-CR/space-stripping)
plus chunk-boundary torture (mid-line split, CRLF split across chunks,
4-byte emoji split across chunks, one-byte-at-a-time feed), and the
transport layer (POST body/headers/accept reach fetch; reconnect carries
Last-Event-ID and honors `retry:`; `close()` stops reconnection;
SSETransport method default + event-name demux). Full wasm_ui JS suite
87/87.
