---
feature: "HTTP/3 frame cap and streaming, WebTransport session lifetime, wasm SSE read errors"
status: "pending"
priority: "medium"
depends_on: ["02-h2-h3-server-readiness"]
branch: "fix/io-07-h3-wt-sse"
crates: ["foundation_netio"]
backlog: "docs/bug-backlog.md §2 FramedRecv buffers whole DATA frames; poll_accept_webtransport finishes the CONNECT stream after the 200; A wasm SSE read error ends the stream as if it finished cleanly"
---

# Feature 07: HTTP/3 frame cap and streaming, WebTransport session, wasm SSE errors

_Written in ASD-STE100 Simplified Technical English (STE). Code and identifiers stay as they
are._

**Abbreviations in this file.** Hypertext Transfer Protocol version 3 (HTTP/3); WebTransport
over HTTP/3 (WebTransport); Server-Sent Events (SSE); JavaScript (JS); Internet Engineering
Task Force (IETF); pull request (PR); AeroSpace and Defence Industries Association of Europe
(ASD), the owner of the STE standard ASD-STE100.

Three independent fixes in `foundation_netio`. Do them as three commits in one PR.

## 1. HTTP/3 DATA frames: configurable cap and streaming within a frame

**Now.** `http3/stream.rs` `FramedRecv::with_max_frame_size` exists, but `H3Connection` /
`H3Request` never expose it, so every connection uses `DEFAULT_MAX_FRAME_SIZE` (16 MiB,
`http3/frame.rs:233`). A DATA frame is buffered whole before it is yielded, so one 16 MiB frame
costs 16 MiB of memory per stream.

**Do.**
- Add `max_frame_size` to the HTTP/3 connection config with a builder method. Default: keep
  16 MiB (the current constant). Find the default that the `h3` crate (hyperium) or quinn's
  examples use for the same limit and write it in the doc comment; if you find none, write
  "kept from the previous constant" and add the question to the PR body.
- Pass it to every `FramedRecv` the connection creates.
- **Stream within a frame:** after the DATA frame header (type + length) is read, yield the
  payload in pieces as they arrive (up to a read-chunk size from config, default 16 KiB as in
  Go `net/http`'s HTTP/2 default read size), instead of waiting for `length` bytes. Non-DATA
  frames (HEADERS, SETTINGS) stay whole, capped by the limit.

**Tests** (`foundation_netio/tests/http3/`):
- `data_frame_is_streamed_before_it_is_complete`: a peer sends a DATA frame header for 1 MiB
  and then 64 KiB of payload and pauses. Assert the receiver yields at least 64 KiB before the
  rest arrives.
- `frame_over_configured_cap_is_rejected`: cap 1 KiB, a 2 KiB HEADERS frame; assert the
  connection error `H3_FRAME_ERROR` (or the code the module uses), not an allocation.

## 2. WebTransport CONNECT stream lifetime

**Now.** `poll_accept_webtransport` (`http3/connection.rs`) sends the `200` and then finishes
(FIN) the CONNECT stream. In WebTransport over HTTP/3 the session lives as long as its CONNECT
stream: closing it ends the session.

**Do.**
- Check the current IETF draft (`draft-ietf-webtrans-http3`, section "Session Termination":
  closing the CONNECT stream terminates the session; `CLOSE_WEBTRANSPORT_SESSION` capsule).
  Quote the section number in the PR body.
- Keep the CONNECT stream open after the `200`. Hand it to `WtSession`. Closing the session
  sends the close capsule (if implemented) and then FIN. Reading FIN or a reset on the CONNECT
  stream ends the session.

**Tests** (`foundation_netio/tests/http3/` or the WebTransport test file):
- `webtransport_session_stays_open_after_accept`: after accept, open a bidirectional stream on
  the session 200 ms later; assert it works.
- `closing_connect_stream_ends_session`: the client closes the CONNECT stream; assert the server
  session ends.

## 3. wasm SSE read error ends the stream cleanly

**Now.** `wasm/client/stream.rs` `spawn_stream_reader` (around line 38) sets `done` on a JS
`ReadableStream` read error, so the consumer sees a normal end.

**Do.** Record the error next to `done`. `QueueSseEvents` then ends with
`Stream::Pending(SseProgress::Failed(msg))` and then `None`, the same as a fetch failure
(spec 65 feature 06 already uses `SseProgress::Failed`).

**Test** (`foundation_netio/tests/wasm/…`, `wasm-bindgen-test`, run under Node as the other wasm
tests): a `ReadableStream` that errors after one event; assert the consumer gets the event, then
`SseProgress::Failed`, then the end.

## 4. Tasks

- [x] T1. Branch from the base after feature 02.
- [x] T2. §1 frame cap and streaming, with tests. Commit.
- [x] T3. §2 WebTransport lifetime, with tests. Commit.
- [x] T4. §3 wasm SSE error, with the test. Commit.
- [x] T5. Delete the three backlog entries. Update `progress.md`, `learnings.md`.

## 5. Verification (run exactly)

```bash
export CARGO_PROFILE_DEV_DEBUG=0 CARGO_INCREMENTAL=0
python3 scripts/ci/ci.py run foundation_netio --profile dev   # includes the wasm step
python3 scripts/ci/ci.py run foundation_connectrpc --profile dev
cargo check --workspace --all-targets --profile dev
# 20-run flake check for each new native test.
```

## 6. Done when

- [x] All §5 commands pass; new tests passed 20 runs in a row (native).
- [x] The draft section is quoted for §2.
- [x] CI green.

## 7. Review checklist (main session)

- [ ] Partial DATA yields keep frame boundaries correct for the next frame.
- [ ] The cap applies before allocation.
- [ ] WebTransport streams opened after accept use the session id of the open CONNECT stream.
