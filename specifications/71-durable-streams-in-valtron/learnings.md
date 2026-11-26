# LEARNINGS — Durable streams in valtron

_No implementation yet. This file is seeded at spec creation and updated after each milestone.
See `.agents/skills/specifications-management/skill.md`._

## Key finding (creation, 2026-10-30)

Valtron already owns every piece a durable stream needs, but they are not named as one thing:

- `Stream::Wait` + `WakeableStream` is the long-poll tail.
- `Stream::Spread` is the batch append (the protocol's JSON-array flattening).
- `Stream::Pending(P)` is the pending-progress context.
- `into_next_stream` / `into_future_stream` are the async consumption seam.
- `#[task]` is the idempotent-producer state machine.
- `Ctx` (rule 11) is the storage injection point.

The spec does not invent a new runtime. It names the contract and binds those pieces to a small
`DurableStreamStore` trait.

## Load-bearing decisions (creation, 2026-10-30)

- **Offsets are opaque and ordered, never parsed.** Only the store builds offsets. This is what
  lets a file backend, an R2 segment key and a DO SQLite sequence share one reader.
- **A refusal, never a gap.** `StoreError::Trimmed` is the only answer to a dropped offset. This
  mirrors the "a refusal, never a gap" rule the edge notes already record for `foundation_log`.
- **Acknowledgement has two levels.** `Accepted` (durable at the tail) and `Sealed` (durable in
  the sealed object) are both configuration, matching the edge-data "acknowledge at the tail"
  lever.
- **Storage is injected, never global.** The DST layer swaps an in-memory store with a simulated
  clock and a seeded RNG, so any run replays from its seed.

## Review correction (2026-11-27)

- **`Stream-Seq` was missing.** The first version had only the producer triple. The protocol has
  two layers. The producer triple gives retry safety. `Stream-Seq` gives writer ordering.
- **Producer headers come first.** The store validates the producer triple before `Stream-Seq`.
  A retry with an older `Stream-Seq` is deduplicated, not refused.
- **Compare as bytes.** `Stream-Seq` is an opaque string. `"2"` then `"10"` is a regression.
  The comparison is byte-lexicographic, not numeric.
- **Scope is per stream.** A fork starts with fresh writer state.
- **The store trait stays generic.** Native `async fn` is not object-safe. `Ctx` holds an
  `Arc<DurableStream>` enum, not `Arc<dyn DurableStreamStore>`. Each enum variant holds one
  backend, and the enum implements the trait by delegation.
- **Errors carry the cause.** `StoreError` variants carry the current epoch, the sequence gap,
  the tail offset or the content types. The HTTP layer maps variants to statuses. It does not
  parse error strings. `Gone` is distinct from `NotFound`.
- **Conformance is the gate.** The server suite and the client suite must both pass. A skip needs
  a recorded reason and owner approval. The spec is not done without green suites.

## Architecture correction (2026-11-27)

- **Conformance first, valtron second.** The protocol core must pass both suites without valtron.
  The valtron layer wraps the core. It must not change protocol outcomes.
- **The core has no transport or runtime.** It has no HTTP type, status code, header, ETag, SSE
  frame, CDN cursor or valtron type. The adapters own the wire rules.
- **One live state machine, three transports.** HTTP long-poll, SSE and a ConnectRPC server stream
  all drive the same core reader. Do not make one transport the core model.
- **The client core is independent too.** The Rust client core is plain async. A valtron wrapper
  turns it into a `StreamIterator`. The conformance adapter drives the core, not the wrapper.
- **The valtron layer is required.** `Stream::Wait`, `Stream::Spread`, `Stream::Pending`,
  `WakeableStream`, `#[task]` and the bridges sit above the core. They make the core usable in
  valtron tasks and streams. They are not optional. They do not change the contract.
- **Storage is a boxed VFS.** `DurableStream` is a struct over `Arc<dyn DurableStreamVfs>`. The
  VFS trait is object-safe via boxed futures. The memory VFS lives in `foundation_core`. The
  file VFS lives in `foundation_nativeapis`. The protocol core and `Ctx` do not change when the
  VFS changes. `Ctx` holds `Arc<DurableStream>`.
- **Follow `shared/`, `wasm/`, `cf/` and `native/`.** The durable core and memory VFS live in
  `foundation_core/src/durable/shared/`. Platform VFS code lives in the `native/`, `wasm/` or
  `cf/` module of its home crate. Do not put platform code in `shared/`.
- **Each VFS lives in its native crate.** The memory VFS uses `foundation_core`. The file VFS
  uses `foundation_nativeapis`, so it can use native file I/O and LMDB. R2/S3 uses
  `foundation_nativeapis` too. DO SQLite and D1 use their spec 83 crates. The VFS uses the full
  capability of its environment.

## Rivet object storage (2026-11-27)

- **Current Rivet open source has no S3/R2 cold tier.** It moved to Enterprise. The last
  open-source implementation is commit `d51993c0`.
- **Rivet's cold tier is an archive, not a VFS.** It writes one about 256 KiB object per shard.
  Keys are deterministic. It sends one PUT per activity. It reads the full object. A
  transactional database (FoundationDB) is the publication and fencing authority.
- **Rivet has no S3 CAS, multipart, range GET or local spool.** Our VFS adds them because it has
  no database authority in the publication path.
- **Borrow:** deterministic keys, publish-after-PUT, post-read revalidation, hot-first read,
  background cache fill and an object lifecycle with a grace window.
- **Layout and batch sizes are configuration.** The default is segmented objects with a 256 KiB
  target. An append-only file layout is supported. Batching is local: one batch becomes one
  object PUT. Do not hard-code the layout or the segment size.
- **Avoid:** unchecked per-key `DeleteObjects` errors, no fsync in the filesystem tier, and
  implicit R2 configuration. Configure R2 region `auto`, path-style and CRC32.
- **HTTP is one surface, not the server.** The core is runtime-neutral. The same core runs in a
  local embedded process, a DO, a Worker, a WebSocket server and the `foundation_ai` Session.
  Tests prove each target. A socket, a global runtime or an HTTP type in the core is a defect.

## HTTP adapter (2026-11-27)

- **Offsets need a wire codec.** The core offset is an opaque byte string (8 bytes, big endian,
  for the memory and file VFS). It is not printable text. The adapter renders it as lowercase
  hexadecimal, two characters per byte. The fixed width keeps the byte order, so the text sorts
  like the bytes. The first draft passed the raw bytes to the header and to the query parser,
  and the offset echoed back did not match. The fix is one encode and one decode function in
  `foundation_http::shared::durable::parse`.
- **The adapter is testable without a socket.** `dispatch` takes a `DurableRequest` and returns
  a `DurableResponse`. `serve` only reads the socket body, builds the request and writes the
  response. Every wire rule is tested from `tests/`.
- **SSE needs the connection.** A Server-Sent Events (SSE) response is a loop of writes, not one
  `DurableResponse`. `serve` checks the live mode first and sends SSE to `serve_sse`. The event
  format, the control JSON and the head renderer are pure functions with their own tests.
- **Wait on the notifier, time out on the injected clock.** Register the notifier callback
  before the first read, so an append between the read and the wait cannot be lost. Race the
  callback against `foundation_core::valtron::timer::sleep_for`, which reads the `Ctx` clock.
  A store without a notifier fires at once, so a live read answers without data instead of
  hanging.
- **A streaming head must not declare a content length.** `render_head` always adds
  `Content-Length`. An SSE head uses `SendSafeBody::SseStream(None)` and `render_message`, so the
  body runs to connection close. The response adds `Connection: close`.
- **Limits are configuration.** The SSE connection lifetime is `DurableLimits::sse_lifetime_ms`
  (default 60 s, the protocol guidance), next to the long-poll timeout.

## Server conformance run (2026-11-27)

- **Run the reference suite early.** The first run failed 63 of 338 tests. The failures clustered
  into a few root causes, not 63 bugs. Run the suite as soon as the adapter answers one request.
- **Offset formats differ.** The suite hard-codes the reference zero offset
  `0000000000000000_0000000000000000` and a beyond-length offset in the same
  `<sequence>_<byte>` form. The decoder accepts the composite form and uses the byte part. Our
  own offsets stay lowercase hexadecimal.
- **Producer state decides before closure.** An append to a closed stream must first check the
  producer tuple. A duplicate answers `204`; anything else answers `409` with
  `Stream-Closed: true`. `close` is a producer write: it opens the writer state, slides the
  time-to-live (TTL) window and deduplicates.
- **Error responses carry protocol headers.** A `403` carries `Producer-Epoch`. A `409`
  sequence gap carries `Producer-Expected-Seq` and `Producer-Received-Seq`. A closed `409` carries
  `Stream-Closed` and `Stream-Next-Offset`.
- **A soft-deleted fork keeps its source reference.** Deleting a fork must not decrement the
  source count until the fork itself is collected. Otherwise a deeper fork loses its chain.
- **SSE framing uses `data:` with no space.** The suite matches `data:{json}`. A control event
  follows each data event; an idle open stream sends one control on connect. Every carriage
  return and line feed in a payload starts a new `data:` line, so a payload cannot inject an
  event.
- **`offset=now` is not cacheable.** The response uses `Cache-Control: no-store`. For long-poll,
  `now` becomes the tail and then the read waits.
- **A soft-deleted path answers `409` on a create and on a fork.** A read still answers `410`.
  An expired stream answers `404`.
- **The suite waits inside a 5 s test limit.** Set the long-poll timeout to 3 s in the
  conformance server. Keep the 30 s production default.

## Client core (2026-11-27)

- **One offset codec for both sides.** The server adapter and the client must agree on the wire
  text. The codec lives in `foundation_core::durable::wire`. A second copy in the client would
  drift.
- **Inject the transport, not the runtime.** `ClientTransport` takes one request and returns one
  response. A mock transport tests every verb with no socket; a native host wraps the
  `foundation_netio` client; a wasm host wraps `fetch`.
- **The client core builds for wasm32.** It uses `shared/` only, with no native or valtron type.

## Client conformance run (2026-11-27)

- **Initialize the valtron pool in the adapter.** The native client spawns tasks. Without the
  pool, every request fails as `NETWORK_ERROR`.
- **Map standard header names to their enum variant.** A custom `content-type` key sits beside
  the builder's own `Content-Type`, so the request carries two values and the server picks the
  wrong one.
- **Send `offset=-1` when the caller gives no offset.** The reference server reads from the tail
  for an omitted offset.
- **A JSON read is one body.** The client returns the array body as one chunk. Per-message
  chunks fail the `[{"message":"hello"}]` comparison.
- **Track the producer sequence per `(path, producer)`.** A retried close must reuse the last
  sequence. Append and close share the counter.
- **Auto-claim jumps to `Producer-Epoch + 1`.** Incrementing by one walks into the fenced epoch
  and then deduplicates, so the retry writes no data.
- **Announce every unsupported feature as false.** The runner skips its tests, so the pass
  result states exactly what the adapter supports.

## Client Server-Sent Events (SSE) (2026-11-27)

- **A background read blocks a serial adapter.** The runner marks a read as background and
  continues. A single-threaded adapter that blocks on that read stops answering. An end-to-end
  SSE adapter needs a timeout per read, and it should serve commands while a stream is open.
- **Keep the announced features honest.** When the end-to-end path is not green, announce the
  feature as unsupported and keep the gate at zero failures. A partial feature that hangs the
  adapter fails more tests than an absent feature.

## SSE deadline finding (2026-11-27)

- **An event-loop deadline does not cover the open.** `open_sse` waits for the response
  headers. When that wait blocks, a deadline inside the event loop never runs. The adapter
  needs a timeout around both the open and the read.
- **A self-waking poll loop spins.** It needs a wall clock and a free thread. A better fix is a
  timer that the valtron reactor drives, so the adapter parks instead of spinning.
- **The adapter keeps SSE off until both waits are bounded.** The client gate stays at 208
  pass, 0 fail.

## SSE second finding (2026-11-27)

- **The open phase needs its own bound.** `NetioTransport` races `send_sse_async` against
  `valtron::timer::sleep_for`. The timeout is configuration, default 30 s. Some SSE reads now
  finish in 23 ms.
- **The adapter still stalls on other SSE reads.** Empty-stream and reconnect cases time out
  the next command. A control event without an `event:` type, or a data event with an empty
  payload, is not handled. The serial adapter also cannot serve commands while a read is open.
- **Keep the adapter honest.** SSE stays announced as unsupported until the whole suite is
  green. The client gate stays at 208 pass, 0 fail.

## Concurrent adapter experiment (2026-11-27)

- **Unordered worker results break the runner.** The runner keeps one pending response. A worker
  pool that writes each result when it finishes mixed the answers to four streaming-equivalence
  reads. The failure count rose from 0 to 4.
- **The adapter needs an ordered result queue.** The main thread reads every command at once and
  assigns a sequence number. Workers run in parallel, and one writer thread emits results in
  sequence order. A later result waits for an earlier one. The runner never sees an out-of-order
  line.
- **The adapter stays serial until the queue exists.** The client gate stays at 208 pass, 0 fail.

## Ordered adapter results (2026-11-27)

- **The ordered result queue works.** The main thread numbers every command. A read runs on its
  own thread. One sink writes results in command order and holds a later result until every
  earlier one is out. The client gate stays at 208 pass, 0 fail.
- **The worker path needs the same feature gate.** The worker read bypassed the adapter's
  "SSE is not enabled" answer and opened a stream, which stalled the command. The gate must sit
  in the shared read helper, not only in the serial path.

## SSE open deadline, second attempt (2026-11-27)

- **The self-waking poll deadline compiles and bounds the open future.** The native transport no
  longer races a valtron timer. It polls the open future, wakes itself and stops at a wall-clock
  deadline.
- **SSE still stalls the suite when enabled.** The adapter keeps SSE off. The remaining suspects
  are the event stream's own progress under the blocking executor, and the runner's background
  read pattern. The client gate stays at 208 pass, 0 fail.
