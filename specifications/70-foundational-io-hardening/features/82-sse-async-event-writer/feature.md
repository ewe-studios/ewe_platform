---
feature: "SSE event writes go through the async ResponseWriter; a partial write cannot truncate an event"
status: "pending"
lane: "next (priority) — starts after feature 67 merges"
model: "sonnet"
branch: "spec70/f82-sse-async-event-writer"
crates: ["foundation_netio", "foundation_http", "foundation_toolings"]
catalogue_rows: []  # from docs/bug-backlog.md §3; not a §8 catalogue row
---

# Feature 82: async SSE event writes

_Written in ASD-STE100 Simplified Technical English (STE). Code and identifiers stay as they are. Follow [requirements.md](../../requirements.md) sections 3, 4 and 8._

Priority: this is a data-loss bug. It runs before the remaining lower-priority features of
spec 70.

## Problem (measured 2026-11-05)

`SseStream::new` writes the response head through the async `ResponseWriter` (feature 63).
Every event after the head still takes the synchronous path:

- `EventWriter::send`, `comment` and `message`
  (`backends/foundation_netio/src/event_source/shared/writer.rs:53`) use `writeln!` and
  `flush` on a `W: Write` stream. Only `write_all` is reached, and it is synchronous.
- `SseStream` (`backends/foundation_http/src/native/upgrade/mod.rs:127`) wraps the
  `SharedByteBufferStream<RawStream>` in `EventWriter` and calls it from `send`,
  `message` and `comment`, which are synchronous too.
- `foundation_toolings` `src/proxy/handlers.rs:54` creates `EventWriter::new(&mut conn)` on the
  same non-blocking connection. Its response head also uses the synchronous
  `http_render_to_writer` (`foundation_netio/src/shared/http/impls.rs:515`), which calls
  `write_all` and `flush` on the `SharedByteBufferStream`. Feature 63 migrated
  `SseStream::new`; it did not migrate this handler.

The socket is non-blocking. A slow client fills the send buffer. `writeln!` then returns
`WouldBlock` (or `Interrupted`) after it wrote part of one line. The caller gets an error and
the rest of the event is never sent. The client sees a partial event, and the SSE stream
breaks. Data is lost.

Feature 63 removed the same class of bug from every response path. This is the one remaining
synchronous writer on a non-blocking native socket.

## Design

Split formatting from writing, then make the native writer async.

1. `foundation_netio`: add `SseEvent::render(&self, out: &mut Vec<u8>)` (and a
   `rendered(&self) -> Vec<u8>` helper). It writes exactly the bytes the current
   `EventWriter::send` writes: `event:`, `id:`, `retry:`, one `data:` line per data line, then
   the empty line. `EventWriter` keeps its `W: Write` API for in-memory writers, and its
   `send`/`comment`/`message` use the renderer. This keeps one source of truth for the wire
   format.
2. `foundation_http`: add `SseWriter` in `native/upgrade/mod.rs` (or `native/writer.rs`). It
   holds a `ResponseWriter` and provides:
   - `async fn send(&mut self, event: &SseEvent) -> Result<(), UpgradeError>`,
   - `async fn comment(&mut self, text: &str)`,
   - `async fn message(&mut self, data: impl Into<String>)`.
   Each renders the event to a `Vec<u8>`, calls `ResponseWriter::write_all`, then `flush`.
   `ResponseWriter` parks on write readiness and restarts its idle timer, so a partial socket
   write resumes instead of dropping bytes.
3. `SseStream` holds `SseWriter`, not `EventWriter<SharedByteBufferStream<RawStream>>`. Its
   `send`/`message`/`comment` become `async`. `SseStream::new` creates one `ResponseWriter` and
   keeps it: the head write and the event writes share it.
4. `foundation_toolings` proxy SSE handler: write the head with `ResponseWriter` (the same
   path feature 63 gave `SseStream::new`), then build a `SseWriter` instead of
   `EventWriter::new(&mut conn)`. The handler's `Serve::serve` is synchronous in this crate;
   drive the async writes through the executor the way the other `serve` implementations do
   (or make this handler's writes run in a `FutureTask`). Do not leave a synchronous socket
   write behind.
5. Keep the synchronous `EventWriter` public for in-memory writers and tests. Document that it
   is not for a non-blocking socket.

Public API change: `SseStream::send`, `message` and `comment` become `async`. Bump
`foundation_http` (0.0.4 → 0.0.5) and `foundation_netio` (0.0.5 → 0.0.6) when their public API
changes, and update every requirement in the workspace and in `bin/platform/templates/*`.

## Tests

- `backends/foundation_http/tests/sse_writer_tests.rs` (new): drive `SseWriter` over the
  deterministic fake in-memory `OverlayReadWrite` transport from
  `response_writer_timer_tests.rs`. Fill the transport, send an event, read slowly. Assert the
  client receives the complete event bytes. On the old synchronous path the test must fail with
  `WouldBlock`/truncation. Record the before/after output.
- `foundation_netio` `tests/event_source/writer_tests.rs`: keep every existing formatting test.
  Add a test that `SseEvent::render` and `EventWriter::send` produce identical bytes.
- `foundation_toolings`: one proxy SSE test that a client reading at one byte per poll receives
  every event whole.
- If `SseStream` has an existing test with a `Vec<u8>` writer, keep it and adapt the calls to
  `.await`.

## Verify

```bash
cargo fmt --all --check
cargo clippy --profile uat -p foundation_netio -p foundation_http -p foundation_toolings \
  --all-targets --no-deps -- -D warnings
cargo test --profile uat -p foundation_netio
cargo test --profile uat -p foundation_http --features multi
cargo test --profile uat -p foundation_toolings
cargo check --workspace --all-targets --profile uat
python3 scripts/ci/ci.py run foundation_http --profile uat   # in the Linux CI image
```

## Done when

- [ ] No native SSE write goes through a synchronous `Write`.
- [ ] The regression test fails on the old code and passes on the new code.
- [ ] The backlog entry "SSE event writes can truncate on a non-blocking socket" is deleted.
- [ ] Versions and every consumer requirement are updated.
- [ ] CI green.
