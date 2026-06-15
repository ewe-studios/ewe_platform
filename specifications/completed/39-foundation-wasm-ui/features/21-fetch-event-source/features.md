# Feature 21: Fetch-based EventSource (owned SSE parser)

**Origin:** post-F06 review (2026-06-12) — the browser `EventSource` API only
supports GET, no request bodies, no custom headers. Datastar's approach:
`fetch()` the stream with ANY method and parse the SSE protocol ourselves
from the response `ReadableStream`. The Rust side already owns this parser
(`foundation_netio::event_source::shared::sse`); the JS side must mirror its
semantics so both ends of the wire agree.

## 1. Components (foundation-wasm-ui.js)

### SseParser — incremental, chunk-boundary-safe

Push-based: `push(Uint8Array) -> Event[]`, `end() -> Event[]` (EOF flush).
Mirrors the Rust `SseParser` semantics exactly:

| Rule | Behavior (both sides) |
|------|----------------------|
| fields | `id` (rejected if it contains `\0`), `event`, `data` (multi-line, joined `\n`), `retry` (integer ms) — unknown fields ignored |
| value | exactly ONE leading space stripped |
| `: comment` | surfaced immediately as a comment event |
| empty line | dispatch IF data accumulated (no data ⇒ reset, no event) |
| EOF | dispatch accumulated data |
| no-colon line | ignored (matches the Rust impl; deviation from W3C's empty-value reading, kept for cross-side consistency) |
| `last_event_id` | persists across events; attached to every result |

JS-side additions the Rust reader doesn't need:
- streaming `TextDecoder` (UTF-8 sequences split across chunks),
- CRLF/LF/CR line endings INCLUDING a CRLF split across two chunks,
- partial trailing lines buffered until their terminator arrives.

### FetchEventSource — the transport

`new FetchEventSource(url, options)` with `method` (ANY verb), `headers`,
`body`, injectable `fetchFn`, `onOpen/onEvent/onComment/onError`. Behavior:
- streams `response.body` through `SseParser`;
- auto-reconnects with the F06 backoff (`reconnectDelay`) when the stream
  ends or errors, unless `close()`d; server `retry:` overrides the delay;
- sends `Last-Event-ID` on reconnects;
- `close()` aborts the in-flight fetch (AbortController) and stops retries.

### SSETransport rework

`SSETransport.connect(url, data, onResult)` now rides `FetchEventSource`
(no browser `EventSource`): `config.method` (default `POST` when `data` is
given, else `GET`), body JSON-encoded, `event:` names html/arrow/json demux
exactly as before (G46), unnamed events default to html.

## 2. Testing

Parser battery mirroring the Rust unit tests (simple/id/event-type/multiline/
comment/empty-line-skip/retry/EOF-flush/null-id/unknown-field/CRLF/no-colon)
PLUS chunk-boundary torture: mid-line splits, CRLF split across chunks,
multi-byte UTF-8 split across chunks, one-byte-at-a-time feed. Transport:
scripted `ReadableStream` via injected fetch — method/headers/body assertions,
POST-with-stream-response e2e, `Last-Event-ID` on reconnect, `retry:` honored,
`close()` stops reconnection.
