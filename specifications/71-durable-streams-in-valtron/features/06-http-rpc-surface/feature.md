---
feature: "HTTP and RPC surface"
status: "planned"
priority: "high"
depends_on: ["02-resumable-reader", "03-idempotent-producer"]
crates: ["foundation_http", "foundation_connectrpc", "foundation_netio"]
---

# Feature 06: HTTP and RPC surface

_Written in ASD-STE100 Simplified Technical English (STE). Code and identifiers stay as they
are._

**Abbreviations in this file.** Hypertext Transfer Protocol (HTTP); Server-Sent Events (SSE);
Content Delivery Network (CDN); AeroSpace and Defence Industries Association of Europe (ASD), the
owner of the STE standard ASD-STE100.

## 1. Goal

Map the durable contract onto the existing HTTP and RPC seams, so any `foundation_http` handler or
`foundation_connectrpc` service exposes a resumable, replayable stream with no new protocol.

## 2. Requirements

1. **The six verbs.** `PUT` create (idempotent, `200` on a match, `409` on a mismatch), `POST`
   append/close, `GET` read, `HEAD` metadata, `DELETE`. Map each onto the store methods.
2. **The cursor headers.** `Stream-Next-Offset`, `Stream-Up-To-Date`, `Stream-Closed` map to
   `StreamCursor` fields. A response carries them; the client echoes the next offset back.
3. **Live modes.** `?live=long-poll` parks on `Stream::Wait` and returns `200` with data or `204`
   on timeout. `?live=sse` streams the reader through `into_next_stream()`; `data` events carry
   chunks, `control` events carry the cursor. A closed stream ends the SSE and the client does not
   reconnect.
4. **JSON mode.** A JSON stream preserves message boundaries and flattens a `POST` array into
   several messages. This maps to `Stream::Spread` on read and to a batch append on write. An
   empty array is a `400`.
5. **Caching.** Catch-up responses are immutable and cacheable (`Cache-Control`, `ETag`,
   `If-None-Match`/`304`). The `ETag` varies with closure so a stale `304` never hides EOF. `HEAD`
   returns `Cache-Control: no-store`.
6. **Cursor for collapsing.** A live response carries a `Stream-Cursor` so an edge can collapse
   waiting clients at the same offset. This is a header on the existing path, not new code.
7. **Park, never poll.** The SSE and long-poll paths use `Stream::Wait` + `WakeableStream` (feature
   02), reusing the wasm SSE parking work from spec 65 feature 06. No timer-based polling.

## 3. Design

A thin adapter sits between a `DurableStreamStore` and the HTTP/RPC handlers. It is not a new
server:

```rust
pub struct DurableHandler<Store> { store: Store, limits: DurableLimits }

impl<Store: DurableStreamStore> DurableHandler<Store> {
    pub fn handle(&self, req: HttpRequest) -> HttpResponse { /* six verbs, headers, live modes */ }
}
```

`DurableLimits` is configuration: page size, long-poll timeout, SSE reconnect interval, and the
cache TTL, each with a documented default (nginx, Go `net/http`, Caddy, the Durable Streams
conformance suite).

## 4. Tasks

- [ ] **T1 — verb mapping.** `foundation_http`. The six verbs, idempotent create, content-type
      preservation. Tests: `create_is_idempotent`, `append_after_close_is_rejected`,
      `head_is_no_store`.
- [ ] **T2 — cursor headers.** The three headers on every read. Test: `cursor_round_trips`.
- [ ] **T3 — live modes.** Long-poll parks on `Wait` (no timer) and SSE streams `data`/`control`.
      Test: `long_poll_parks_then_wakes`, `sse_ends_on_closed_stream`.
- [ ] **T4 — JSON mode and caching.** `Stream::Spread` flattening, empty-array `400`, `ETag`
      varies with closure. Test: `json_array_flattens`, `etag_varies_with_closure`.
- [ ] **T5 — RPC surface.** `foundation_connectrpc`: a streaming RPC whose response is the
      reader's `into_next_stream()`, resumable by a client-supplied offset. Test:
      `rpc_stream_resumes_from_offset`.

## 5. Success criteria

- [ ] `cargo test -p foundation_http` and `cargo test -p foundation_connectrpc` green.
- [ ] The long-poll path parks on `Stream::Wait` (no polling); verified by a poll-count test like
      spec 70's readiness tests.
- [ ] Byte-exact resumption over HTTP (feature 02's test driven through the handler).
- [ ] `cargo check --workspace --all-targets --profile dev` green.
