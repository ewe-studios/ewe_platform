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
  uses `foundation_nativeapis`, so it can use native file I/O. The metadata is an atomically replaced
  file (owner decision 2026-11-27; the workspace has no LMDB dependency). R2/S3 uses
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

## Layering correction (2026-11-27)

- **The client defines its transport trait.** The owner corrected an inversion. The client core
  and `ClientTransport` live in `foundation_core::durable::client`. `foundation_netio` implements
  the trait above core. I had resolved a cycle by moving the wrapper down; the correct fix moves
  the abstraction up.
- **Do not add a second executor to the tests.** `smol` was a shortcut for borrowed futures.
  `block_on_future_scoped` in `foundation_core::valtron::executors::scoped` drives a borrowed,
  non-`Send` future on the caller's thread and parks it. The 13 durable test files use it.
- **Many values become a stream.** A future that yields more than one value returns a
  `futures_core::Stream` or the valtron bridge, not a collected batch.
- **The owner never excluded subscriptions.** I wrote that exclusion into the requirements. It
  was wrong. Feature 10 and issue #1000 put the six skipped subscription tests in scope. The
  gate is not complete until it shows no skip.

## SSE status gap (2026-11-27)

- **The SSE open hides the status.** `send_sse_async` returns only the event stream, so a client
  cannot answer 404 or 410 and cannot read `stream-sse-data-encoding`. Issue #947 records the
  fix: return the response head with the stream. The adapter stays on the long-poll path until
  that lands.

## SSE blocking poll, root cause (2026-11-27)

- **The native SSE stream blocks inside `poll_next`.** A probe against the repository server printed
  the data and control events, then the next poll never returned. A self-waking deadline and a
  2 s socket read timeout both failed to interrupt it. Issue #963 records the fix: the event task
  must return `Stream::Wait` with a wake registration, not block.
- **No client-side timeout can fix this.** The Durable Streams adapter keeps Server-Sent Events
  off until the netio event task stops blocking. The client gate stays at 208 pass, 0 fail.

## SSE block location (2026-11-27)

- **`EventSourceState::ReadingStream` polls a blocking iterator.** `event_source/native/task.rs`
  holds a boxed `Iterator`. The task's `next()` calls it, and `into_pure_future_stream` calls
  `next()` from `poll_next`, so the poll parks in the socket read. A readiness-aware state that
  returns `TaskStatus::Wait` is the fix. Issue #963.

## SSE watchdog experiment (2026-11-27)

- **A watchdog removes the stall but not the gap.** Answering a parked SSE read from a timer gives
  212 pass and 46 fail: the real read never returns, so every data test sees an empty result.
- **No adapter-side workaround delivers SSE data.** The readiness-aware `ReadingStream` state in
  issue #963 is the only fix. The gate stays at 208 pass, 0 fail with SSE off.

## SSE readiness attempt (2026-11-27)

- **A readiness check before `iterator.next()` is not enough.** The socket can be readable with a
  partial event buffered, so the read still blocks. The body iterator must parse incrementally and
  return no event until the buffer holds a complete one. Issue #963.
- **Every server-side and adapter-side workaround is now measured.** None delivers SSE data. The
  fix is in the netio SSE body iterator. The gate stays at 208 pass, 0 fail.

## Producer property oracle (2026-11-27)

- **A retried producer sequence stores one copy.** The new property test appends a random byte
  vector per sequence and retries each sequence. The read equals the concatenation, for every
  generated input.
- **A text stream read returns one chunk.** The byte log does not keep per-append boundaries.
  Only a JSON stream returns one chunk per message. An oracle that counts chunks on a text
  stream is wrong.

## Stream-Seq ordering (2026-11-27)

- **`StreamSeq` compares bytes.** A producer that uses decimal numbers must zero-pad them
  (`0009` before `0010`). Plain `9` sorts after `10`, so the store refuses `10`. The property
  test uses four-digit values and pins the rule.

## Bounded read oracle (2026-11-27)

- **The page byte limit is enforced by the service.** With a 4-byte page, a 10-byte stream
  takes at least three reads and no chunk exceeds 4 bytes. The test walks the cursor to the tail
  and joins the bytes. This pins the "no VFS materialises a whole stream" rule.

## File VFS (2026-11-27)

- **The first implementation did not compile.** `StoredFork` derived `Default`, but
  `StreamName` has no `Default`. Remove the derive. `Option<StoredFork>` needs no `Default` for
  the parent struct.
- **A fork length is relative to its base.** The data log holds only the bytes the stream wrote.
  Add the fork base to the log length. A raw log length made a fork report too small a tail after
  an append.
- **A missing fork source is `NotFound`, not `Io`.** Map `io::ErrorKind::NotFound` in the
  metadata read. The protocol maps `NotFound` to the right status.
- **The metadata version must not change on touch.** The store reads a version, then appends with
  that expected version. A touch bump causes false conflicts. Only append, close and trim bump
  the version.
- **Cascade needs a loop.** Removing a fork can drop its source to zero references. A single scan
  leaves that source on disk. Repeat until no candidate remains.
- **Append must notify.** The notifier is the long-poll wake path. Call it after a successful
  append, append-with-meta, close and soft delete.

## Valtron wrappers in the durable tree (2026-11-27)

- **The owner put both wrappers under `durable`.** The new path is
  `foundation_core::durable::valtron`. The compatibility module is removed now. Keep one
  copy only.
- **A module-level re-export did not cycle.** The wrapper imports `crate::valtron` items, and a
  compatibility module re-exported the wrapper. Rust resolved this because the items are not
  recursive. The compatibility module is removed now, so only the new path remains.
- **The client wrapper imports its sibling core by full path.** `super::client` inside
  `valtron/client.rs` names the wrapper's own module, not the client core. Use
  `crate::durable::shared::client`.

## Scoped block_on (2026-11-27)

- **`mpsc::channel` needs its type at the call.** Write `mpsc::channel::<Waker>()`. The
  receiver's later use does not infer it.
- **A poll count proves park, not spin.** The cross-thread test records two polls: one `Pending`,
  one after `unpark`. A busy loop would record many.

## 405 flake, root cause (2026-11-27)

- **The request builder rewrote parsed headers.** `SimpleIncomingRequestBuilder::build` removed
  `Content-Length` and added `Transfer-Encoding: chunked` for a streamed body. That is correct
  for a client that renders a request, but wrong for a server that parses one. The server parser
  reuses the builder, so every streamed `Content-Length` body lost its length.
- **The lost length hid the oversized body.** The adapter reads at most 8 MiB. The conformance
  large-payload test sends a 10 MiB body of zeros. The adapter read 8 MiB, saw no declared
  length, answered 204 and kept the connection open. The 2 MiB tail stayed on the socket.
- **The tail became the next request line.** The next keep-alive GET read 2 MiB of zeros first,
  then its own line. The method token parsed as `Custom("\0...\0GET")`, so the adapter answered
  405 `method not allowed`. It happened once in two to five runs, when the suite reused the
  poisoned connection.
- **The fix has two parts.** A `with_preserved_headers` builder flag keeps the wire headers of a
  parsed request. The durable adapter answers 413 and closes when the declared body exceeds its
  cap. A regression test sends a 9 MiB declaration and asserts 413.
- **The fallback routing broke 405 too.** `Router::dispatch_result` swallowed
  `RouteOp::MethodNotAllowed` and turned it into a fallback miss, so a known path with a wrong
  method answered 404. Propagating the variant restored the 405 with `Allow`. This regression
  came from the durable adapter commit, and the full crate test gate found it.
- **`DURABLE_LOG` turns on tracing in the example server.** The value names the log level. The
  connection loop logs the request line and method when the method is unknown.

## Subscriptions, feature 10 (2026-11-27)

- **Re-export the module, not its items.** `pub use foundation_core::durable;` makes
  `foundation_event_stream::durable::StreamName` the same type as the core type. A list of
  `pub use` items would create a second path and break the "one type" rule.
- **Wrap the VFS, do not change the core.** The engine installs an `ObservingVfs` over the raw
  VFS and reports each append. The engine itself writes state and wake events through the raw
  VFS, so its own writes do not wake the subscription again. The core stays transport-neutral.
- **Keep the hot state in memory and snapshot it to the VFS.** Each change appends one JSON
  snapshot line to a control stream. The loader reads the last line. This is enough for the
  protocol's `wake_id`, generation, lease and retry metadata to survive a restart.
- **A delete needs a tombstone.** Removing the record from the map alone lets the last snapshot
  resurrect the subscription on the next start. The delete appends `{"deleted":true}` and the
  loader treats it as an absent record.
- **Validate the generation before the token.** The reference callback token carries its epoch.
  A random token does not. To answer `409 FENCED` for a stale wake, check the request generation
  against the record first, then the token, then the wake identifier.
- **The conformance server test gate is one option.** `runConformanceTests` runs the
  subscription block only when `subscriptions: true`. `ci/durable_conformance.sh` sets it in the
  generated vitest file.
- **A stale server on the gate port hides every failure.** The script starts its server and waits
  for `__conformance_probe`, but it does not check that its own server bound the port. A
  `durable_server` left from an earlier run answers the probe. The suite then runs against the old
  binary: the stream tests pass and all six subscription tests fail. Find the listener with
  `lsof -nP -iTCP:4437 -sTCP:LISTEN` and stop that exact PID before a gate run.
- **Webhook delivery runs in the append path.** The observer awaits the delivery, because the
  core has no task spawner and a host supplies the transport. A slow receiver delays the append
  by up to the read timeout, and a webhook `lease_ttl_ms` can expire while the delivery is still
  in flight. The reference server uses a fire-and-forget send. This is a throughput limitation,
  not a protocol error. A host that needs it can supply a sender that enqueues the call. Tracked
  as issue #1028.
- **The large-payload test still fails once in a while.** One gate run failed
  `should handle large payload appropriately` with a client-side `ECONNRESET`; the next run
  passed 338 of 338. The earlier 405 flake has a recorded root cause and fix. This run has no
  server panic and no log line that names the cause. Tracked as issue #1029.

## Wasm body length, issue #1019 (2026-11-27)

- **A parsed request must keep its wire framing headers.** The native reader already sets
  `with_preserved_headers()` (issue #926). Both wasm bridges missed it. A fetch request from the
  host is already framed. The bridge only wraps its body in a `Stream`.
- **The default builder derives framing, and that is correct for a fresh request.** A client that
  builds a request must advertise the framing of the body it renders. Do not change that default.
  Set the flag only where the host framed the request.
- **The bug was silent.** The builder removed `Content-Length` and added
  `Transfer-Encoding: chunked`. A handler that read the body still worked. Only a handler that
  used the declared length saw the loss.
- **A host Request keeps a set `Content-Length`.** Deno, Chromium and workerd keep the header in
  the `Headers` object. A wasm-bindgen case can therefore prove the bridge outcome. Both cases
  failed on the old code and pass with the flag.
- **Run the wasm cases on the Deno host.** The command is
  `cargo run -p foundation_testbed --no-default-features --features wasm --profile uat --bin
  wasm-testbed -- test bindgen-deno backends/foundation_http/integration/wasm --filter <name>`.
  The `foundation_http` wasm test crate sits outside the root workspace, so a root `cargo test`
  does not compile it.
- **The `--features multi` wasm check fails in `foundation_core`.** On wasm32 the `multi`
  executor refers to the native-only `idle_wake` module. The CI wasm run uses
  `--no-default-features`, so this is a known native-only feature, not a new bug. Use
  `--no-default-features --features wasm-fetch` for a wasm check of `foundation_netio`.
- **Building the wasm test crate updates its `Cargo.lock`.** The package has its own lock file.
  New `foundation_http` dependencies (`time`, `base64`) must reach it, or the next build rewrites
  it again.

## Default wasm build with `multi`, issue #1030 (2026-11-27)

- **Cargo merges features before it picks a target.** One dependency that asks for
  `foundation_core/multi` turns it on for a wasm build of the graph. A `compile_error!` in the
  crate cannot stop that. The feature must compile.
- **Gate the native-only call, not the feature.** The `multi` executor called
  `idle_wake::park_idle` without a target gate, and the module is native-only. The call now
  has `cfg(not(target_family = "wasm"))`; wasm runs the plain wait. `cargo check -p
  foundation_core --target wasm32-unknown-unknown --features multi` passes.
- **Follow the feature, not the target, for the `Send` bound.** netio's
  `ExchangeContinuation` added `+ Send` only on native. valtron adds the bound when `multi` is
  on. The two conditions disagreed on wasm. The alias now follows `feature = "multi"`.
- **A default-feature wasm run is the guard.** `ci/packages.toml` now runs the
  `foundation_netio` wasm check with its default features, and `foundation_core` with
  `--features std,multi`. The old run used `--no-default-features`, so it never saw this
  failure.

## Clock parameter guard, issue #1034 (2026-11-27)

- **`DurableStream` takes the context bundle.** It held a bare clock and built a
  `MonotonicClock` by default. `new` and `with_ctx` now take `&Ctx`, and the store reads the
  clock with `ctx.clock()`. The host builds the real clock in the context.
- **A stored clock field still trips the guard.** The guard scans every source line, not only
  parameters. A field of type `Arc<dyn Clock>` fails it. Store the `Ctx` instead.
- **Tests build a context.** A test with a hand-moved clock uses `Ctx::fixed(clock)`. A test
  that does not check time uses `Ctx::host()`.
- **The event-stream store lost its clock binding.** `SubscriptionStore::new` passes `ctx` to
  `DurableStream::new` and keeps no `Arc<dyn Clock>` local.


## SSE close-out (2026-11-27)

- **The reference server chunks the SSE body.** `@durable-streams/server` sends
  `Transfer-Encoding: chunked` for a live SSE response. The client body extractor checked the
  content type first and fed the raw chunked bytes to the event parser, so the chunk-size lines
  were read as event fields and every event lost its `event:` type. The fix de-chunks with a
  `Read` adapter over the chunk iterator before the event parser runs.
- **The SSE open must return the head.** `ClientTransport::open_sse` now returns `SseOpen`
  (status, headers, stream). The client answers 404 and 410 from the head and reads
  `stream-sse-data-encoding` there. The HEAD probe workaround is gone.
- **One future yields many values as a stream.** `ReadBuilder::open_sse` returns an `SseRead`
  stream. `ReadBuilder::send` rejects `ReadMode::Sse` and points to `open_sse`. A data event
  waits for its following control event, so a chunk carries the control's metadata (offset,
  cursor, closed).
- **The event task parks, it does not block.** `EventSourceTask::ReadingStream` sets the socket
  non-blocking and returns `TaskStatus::Depends` on the connection readiness source when the body
  iterator reports `WouldBlock`. A test that drives `next_status` directly must park on that
  status, not spin; a consumer should drive the task with `execute`.
- **Escape U+2028 and U+2029 in adapter output.** Node's readline treats both as line
  terminators, so a raw separator inside a JSON result splits the line. The adapter escapes them,
  as the reference client does.
- **Strip `\n` and `\r` from base64 before decoding.** A server may split one base64 payload
  across `data:` lines. The decoder removes the joined newlines first (protocol section 5.8).
- **The suite forces two skips.** `sse-base64-data-integrity-multiline` and
  `sse-base64-large-payload-4kb` are skipped by the suite itself with
  "Swift implementation broken". No client can run them. Our own client-core tests cover the
  same rules.

## VFS testkit and oracles (2026-11-27)

- **A differential oracle finds faults a conformance test misses.** The shared kit ran the same
  scenario on the memory and file VFS. The file VFS passed its own direct tests, which always
  passed a finite read limit. The oracle used the core default (`ReadLimit::max_bytes = None`),
  which becomes `usize::MAX`. The file VFS then sized a buffer with `vec![0; usize::MAX]` and
  panicked. Issue #1041 records it; the fix bounds the buffer by the bytes that remain in the
  own log. A VFS must accept `usize::MAX` as a limit.
- **The core turns "no limit" into `usize::MAX`.** Every VFS must treat that value as "read to the
  end of the data it holds", not as an allocation size. Bound the read by the stored length.
- **A factory per scenario keeps runs independent.** `run_all` builds a fresh store and clock for
  each scenario. A shared store would carry metadata from one scenario into the next and hide a
  fault.
- **The store clock is the wall clock.** `DurableStream` reads `ctx.clock().wall_now()`. A test
  that moves `SimulatedClock` must call `advance_wall`, not `advance`; the monotonic offset does
  not change the time-to-live (TTL) window.
- **A direct `read_at` of a soft-deleted entry is `Gone` on every VFS.** The memory VFS used to
  read through a soft-deleted entry; the file VFS returned `Gone`. Both now refuse the direct
  read, and the memory check sits at `read_at` before the chain. The internal fork chain still
  reads a soft-deleted source on purpose, because a living fork depends on that data. Issue
  #1042.
- **The kit lives behind a feature, not in a test module.** `vfs-testkit` enables `std` and
  `test-clock`, so a VFS crate adds one dev-dependency line. The module is `pub` under that
  feature and is absent from a normal build.
- **A coverage test can record what it does not reach.** The fork-depth limits, the I/O error
  branches and the malformed-metadata parser need a fault injection or a deep chain. Naming them
  in `progress.md` is better than a silent gap.

## Conformance CI wiring (2026-11-27)

- **A job cannot filter its own pull request paths.** `paths` sits on the workflow trigger, so it
  would skip every other check. Use a first step that diffs the base and writes an output, then
  put `if:` on the later steps. The job stays green when no durable path changes.
- **The server count line is the gate.** Vitest prints `Tests  338 passed (338)` only when the
  run has no skip. A skip changes the line to `338 passed | 1 skipped (339)`. Match the full
  `338 passed (338)` text; do not parse the number alone.
- **The client runner prints a summary and one line per skip.** The summary is `Total:`,
  `Passed:`, `Failed:` and `Skipped:`. Each skip prints `(skipped: <reason>)`. Gate on the
  summary and on the reason of every skip line.
- **Do not end a pipeline with `grep -q` under `pipefail`.** When grep exits early, the writer
  takes `SIGPIPE` and the pipeline status becomes nonzero even after a match. Read every line
  into a variable with `grep -v` instead.
- **The VFS conformance file is behind a feature.** `tests/durable/vfs_conformance.rs` compiles
  only with `vfs-testkit`. Add the feature to `runs` and `clippy-runs`; no other tree compiles
  the kit file.
- **A fork-safe job needs no CI image.** A runner with the pinned Rust toolchain and Node 20
  runs the gate. The image path needs registry credentials, so a fork takes the artifact path.
- **The wasm checks from #1030 were already wired.** `foundation_netio` checks its default set
  and `wasm-fetch`; `foundation_core` checks `std,multi`. `foundation-ci plan` accepts both.

## Clippy gate for the durable module (2026-11-27)

- **A long function often hides two rules.** `create` mixed create with fork resolution, and
  `append` mixed append with the closed-stream answer. Each block now sits in a helper with its
  own doc comment. Both functions fit the 100-line limit, and the behavior did not change.
- **One pattern form can satisfy two lints.** `Ok(true | false) | Err(VfsError::NotFound |
  VfsError::Gone)` is nested and merged. It clears `unnested_or_patterns` and `match_same_arms`
  in one arm.
- **A remainder check replaces two length checks.** `as_chunks::<2>()` returns the pairs and the
  remainder. The remainder check rejects an odd length, so the modulo test is gone.
- **A private module rename keeps the public path.** Only `client/mod.rs` re-exports the client
  names. Renaming the inner `client` module to `core` clears `module_inception` and changes no
  public path.
- **A test clock cannot build a monotonic `Instant` without `Instant::now()`.** The TTL test
  reads only wall time. One `#[expect]` with a reason is the clean answer. The durable code
  never reads the monotonic clock.
- **The `clippy.toml` path needs a real dependency.** `foundation_core` has no `rand`
  dependency, so the `rand::thread_rng` entry warns as unreachable. The warning is not a lint,
  so `-D warnings` does not fail. A later change can remove the entry.

## SQL VFS and the two executors (2026-11-27)

- **A vacuumed read needs a block index.** The table design lists only
  `stream_meta` and `pending_data`. After a vacuum removes the delivered rows,
  a read cannot derive the block key or its byte offset from the offset alone.
  The engine therefore keeps a `blocks` text column on `stream_meta`. It lists
  each delivered block as `first`, `last` and `key`. The two-table design stays
  true; the index lives in the metadata row.
- **The batch age needs an append time.** `pending_data` lists only stream,
  sequence, bytes and state. An injected-clock batcher cannot measure an age
  without a timestamp, so the engine adds `appended_ms`. Both extra columns are
  documented in the module and in `progress.md`.
- **Advance the metadata first and guard the row update.** The first design
  marked rows delivered and then advanced `delivered_seq`. A lost race left
  rows delivered with no block index entry, so a vacuum could never reach them.
  The transaction now updates `stream_meta.delivered_seq` first, guarded by the
  old value. The row update follows, guarded by `EXISTS (... delivered_seq =
  next)`. A losing batcher leaves its rows pending, and a later flush retries
  the same deterministic key.
- **A conditional insert tells the writer what happened.** `INSERT ... ON
  CONFLICT(name) DO NOTHING` followed by `UPDATE stream_meta SET ref_count =
  ref_count + changes()` keeps the fork reference count correct under a create
  race. `changes()` is the insert count from the previous statement.
- **A read fetches three bounded ranges per block.** The reader gets 12 header
  bytes, then `count * 24` index bytes, then only the payload range it needs.
  The chunk count is capped by the block byte limit, so a corrupt count cannot
  force a large read. A missing block is an `Io` error, never zero bytes.
- **The clock guard rejects a `dyn Clock` parameter.** `check-clock-params`
  scans `backends/*/src` for `dyn Clock`. The engine stores the injected `Ctx`
  and reads `ctx.clock()` at the call site. The guard then reports no hit.
- **libsql is already in the workspace.** `foundation_db` and
  `foundation_vectors` build libsql 0.6 for native targets. The SQL VFS tests
  use it as a dev-dependency, open an in-memory database, and run the real
  engine. No fake SQL executor exists.
- **The Workers Rust library binds `exec`, not `transactionSync`.**
  `worker` 0.8.5 and 0.8.7 wrap `SqlStorage::exec` (`worker-0.8.7/src/sql.rs`,
  around line 201) but not `DurableObjectStorage::transactionSync`.
  `worker-sys` 0.8.5 binds only `sql_storage.rs` `exec` and `databaseSize`.
  Version 0.8 has no `durable_object` feature; `durable` and `sql` compile
  unconditionally. The JavaScript API does have `transactionSync`:
  `@cloudflare/workers-types` declares
  `transactionSync<T>(closure: () => T): T` on `DurableObjectStorage`. The fix
  is a small wasm-bindgen binding in
  `foundation_db/src/wasm/bindgen/cf/do_sql.rs`, not a stub. The handle comes
  from `worker::durable::State::_inner()`, then `.storage()`. `State` is not
  cloneable, so the constructor consumes it; the normal entry point is
  `DoSqlExecutor::from_state`.
- **A `transactionSync` rollback is a JavaScript throw.** The closure commits
  when it returns and rolls back when it throws. Rust must throw from the
  closure with `wasm_bindgen::throw_str`. That call does not run destructors, so
  it leaks the closure's allocations on the error path only. The executor
  stores the `SqlError` in a shared `Rc<RefCell<...>>` before the throw, so the
  caller gets the real error, not the throw text. The fixture's `/rollback`
  route proves the rollback on the real runtime: a failing second statement
  leaves zero rows from the first insert.
- **`rowsWritten` is not the changed-row count.** The Durable Object cursor's
  `rowsWritten` includes index rows: an insert into a table with a primary key
  index reports two. It is also filled only after the cursor reaches its end.
  The first conformance run failed scenario `create` with this counter.
  `changes()` is correct for a write but stale after a query. The executor
  measures the change in SQLite `total_changes()` before and after each
  statement. That delta matches the D1 `meta.changes` count, counts one row per
  inserted, updated or deleted row, and is zero for a query.
- **The `workers-rs` feature needed the wasm storage types.** Enabling
  `workers-rs` alone failed: its D1 interop module names `crate::D1Database`,
  which was gated behind `wasm-bindgen-storage`. The feature now includes
  `wasm-bindgen-storage`, so one feature is self-consistent.
- **A wasm check of `foundation_db` must disable the default features.** The
  default set is `turso`, `d1` and `r2`, and `turso` turns on `multi`.
  `foundation_core/multi` makes `TaskIterator` require a `Send` future. The
  crate keeps its state store future `!Send` on wasm by design
  (`store_state_task.rs`), so the two rules conflict. The error is
  `next_status ... TaskIterator` not satisfied. The same failure happens with
  no feature flag at all, so it predates the Durable Object (DO) work. Use
  `--no-default-features --features do-sql` (or another wasm set). Issue #1056
  records it. `ci/packages.toml` `wasm-runs` uses `--no-default-features` and
  now runs `do-sql`.
- **A local Durable Object needs `new_sqlite_classes`.** The migration must
  declare `new_sqlite_classes = ["DoSqlObject"]`. `new_classes` creates a
  key-value Durable Object with no `storage.sql`. The fixture in
  `foundation_db/integration/do_sql/` runs `worker-build` and
  `wrangler dev --local`; both the DO route and the D1 route return all nine
  kit scenarios with `{"ok":true,...}`. The built worker is outside the
  workspace, so the native `tests/` never compile for wasm32.
- **A test seed comes from a local RNG, not the host.** The valtron tests need
  one `u64` seed for the executor. A local `ChaCha8Rng::seed_from_u64` with a
  named constant gives it. The global `foundation_compact::rng::rng` call then
  goes, so the clippy ban covers the whole crate and a failed run replays from
  the constant. A host-ABI test still reads the global RNG, because it proves
  the host boundary, and it lives in a standalone crate.

## ConnectRPC adapter (2026-11-27)

- **Generated services name the crate, so the crate aliases itself.** The
  codegen emits `foundation_connectrpc::…` paths because it targets a consumer
  crate. Inside `foundation_connectrpc` that path does not resolve. `extern
  crate self as foundation_connectrpc;` in `lib.rs` fixes it with one line.
- **An `async fn` in a trait is not `Send` by default.** A generic adapter over
  `D: DurableDriver` cannot prove that `store().read(..)` is `Send`, so the
  generated `+ Send` handler signature fails to compile. The adapter is
  concrete over `DurableStream`, the same choice as the HTTP adapter. The host
  swaps the VFS, not the adapter.
- **The first stream page opens the RPC stream.** The generated
  `server_stream_handler` sends the response head together with the first
  message. A live read that waits for data before its first message blocks the
  generated client in `subscribe().await`. `Subscribe` sends one page at open,
  even when it is empty. The page carries the current cursor and
  `up_to_date = true`. The test then appends and receives the next page.
- **Register the wake before the read.** An append that lands between the read
  and the wait is otherwise lost until the next append. The stream arms the
  notifier first, reads second, and arms again after each emitted page.
- **The status table is one row per `StoreError`.** `Gone` maps to `NotFound`
  because Connect has no "gone" code. `Trimmed` maps to `OutOfRange`.
  `ConfigConflict` maps to `AlreadyExists`. A pure test loops over every
  variant, so a new variant fails the test until the table names it.
- **`block_on_future_scoped` can drive a real wire test.** The client transport
  needs the process-wide valtron pool, but the test thread can block in
  `block_on_future_scoped` while pool workers pump the call. Start one pool per
  test with `initialize_pool(seed, Some(8))` and run the tests with
  `serial_test`, because the pool is process-wide. The server can serve Connect
  over HTTP/2 cleartext, which avoids the HTTP/1.1 worker-starvation problem.
- **`requirements.md` 3.9 and the T9 brief differ on `Read`.** The section table
  lists `Read` as a server stream. The brief makes `Read` one bounded page and
  `Subscribe` the live stream. The brief wins for this task, so the proto has
  seven unary calls and one streaming call.

## Configurable body cap, feature 06 T12 (2026-11-27)

- **The adapter held the cap as a constant.** `MAX_BODY_BYTES` lived in
  `native/durable.rs`, so a host could not change the largest body. The cap is
  now `DurableLimits::max_body_bytes`, with `with_max_body_bytes` and an 8 MiB
  default. The adapter reads `service.limits().max_body_bytes` once per request.
- **The declared length and the read share one value.** A declared
  `Content-Length` over the cap answers `413` and closes the connection, so a
  partial body cannot leave a tail for the next request. The bounded read uses
  the same value, so the two checks cannot drift apart.
- **A small cap makes the test cheap and proves the source.** The test helper
  puts `DurableLimits` in the `Ctx`. A 64-byte cap makes a 65-byte declared body
  answer `413`, and a 64-byte JSON body append with `204`. The old test used a
  9 MiB body and the fixed default, so it could not tell the limit from a
  constant.
- **The suite's 5 s limit measures the host, not the server.** On a shared host
  with load 300 to 800 from other worktrees, 3 to 8 heavy tests hit the Vitest
  `Test timed out in 5000ms` limit on each run. No test failed an assertion or
  returned a wrong status. With `--testTimeout=60000` the suite passed 337 of
  338, and the one failure was also a timeout. A live probe of a 10 MiB declared
  body answered `413` in 11 ms. The body cap was not the cause. Run the gate on
  a quiet host.

## S3/R2 object-store VFS, feature 09 (2026-11-27)

- **A fork must compare the source's combined length.** The source head holds
  only published bytes. A source with 10 acknowledged spool bytes has head
  `len` 0. The first fork create failed with `Conflict` until the check used
  the combined length (head plus spool). The memory and file VFS never show
  this gap because their append is the only write.
- **Trim needs the combined length too.** Capping `trim_from` at the published
  head `len` silently refused a trim over pending bytes. The cap now uses the
  combined metadata length, so a read below it answers `Trimmed`.
- **A model of a lost CAS is not a model of a crash.** A test fault that
  returns a backend error from `If-Match` models a crash after the segment PUT
  but cannot exercise the retry. The retry path needs a `PreconditionFailed`
  fault. Two separate switches keep the two tests honest.
- **The segment target is a cap, not a batch size.** Compaction joins adjacent
  segments only while the sum stays under the target. A test that writes one
  32-byte batch at an 8-byte target gets 8-byte segments that cannot merge.
  Small appends with a flush between them give mergeable segments.
- **Count the blocks from the byte count.** A 26-byte batch at an 8-byte target
  is four objects, not three. The retention test had the wrong expectation.
- **A canonical URI must not be encoded twice.** The object URL already
  percent-encodes each key segment. Encoding the path again in the SigV4
  canonical request changes the signed path and breaks the signature. The
  canonical URI now takes the path as sent.
- **Known vectors are cheap.** The empty SHA-256, the SHA-256 of `head` and the
  CRC32 of `abc` (`NSRBwg==`) check the hash and checksum code without a
  service. The AWS SigV4 example suite was not used.
- **The head is revalidated after the body.** A cached head can describe a
  segment list that a writer replaced during the GET. The reader compares the
  head ETag after the read, refreshes and repeats within the configured bound.
  If the bound is reached it returns the bytes it read; a busy writer cannot
  stall a reader forever.
- **No local service on this host.** `which minio localstack` found nothing.
  The stub transport and the in-memory object store are the test doubles. A
  real-service result needs a quiet host with MinIO.

## Features 02 to 04: oracles and DST (2026-11-27)

- **A DST needs a clock the scenario moves.** `SimulatedClock::advance_wall` moves the wall
  clock and wakes parked executors. The DST runs no timed task, so the wake is harmless. A fixed
  seed in `Ctx::with_seed` gives the scenario its draws, so a second run repeats every logged
  `Debug` value.
- **Bind the `Arc` before you lock it.** `ctx.rng()` returns a fresh `Arc`. `ctx.rng().lock()`
  drops that `Arc` while the guard still borrows it. The test helper `draw` binds the `Arc`
  first. Keep the guard out of an `await`.
- **The TTL table is data.** The nine rows of feature 04 section 2.1 run as one loop. Each row
  builds a fresh store and clock. A new row is one line.
- **Expiry with a fork soft-deletes.** The first access after the deadline reports `NotFound`.
  The second reports `Gone`, because the entry is soft-deleted. The fork still reads. Deleting
  the last fork cascades the cleanup.
- **`HEAD` never slides the window.** The test reads the head at 9 s and 9.999 s. At 10 s the
  stream is gone. A read or an append at the same times keeps it alive.
- **The core has no valtron feature to disable.** The crate keeps `durable::valtron` in the
  same module tree, and the owner requires `block_on_future_scoped` in tests. A new architecture
  guard proves the core reader and producer tests never name the wrapper. This is the closest
  available form of the T9 and T7 rule.
- **The differential oracle uses the two core entry paths.** The crate has one store
  implementation, so `drivers_agree` compares the store-trait path with the business-API path on
  two streams. A memory/SQL differential belongs to the feature 08 matrix.
- **The producer model copies the ladder, not the data.** The model tracks `(epoch, last_seq)`
  and returns the typed decision. The test compares decisions, then the stored byte count.
- **One durable run failed one test under host load.** Thirty later runs passed and the name was
  not captured. The likely cause is a `valtron_test` 5 s timeout under load, not an assertion.
  A failure capture on the next run will name it.

## Feature 08 T3: cross-implementation runs (2026-11-27)

- **The reference Rust client is not on crates.io.** The published `durable-streams` crate is
  the Rust *server* (`description = "High-performance Durable Streams server (Rust)"`). The
  client package in `packages/client-rust` also has the name `durable-streams`, but it is
  unpublished. A registry dependency would fetch the wrong crate. The honest form is a public
  git dependency pinned by commit.
- **A git dependency resolves a package below the repository root.** The reference repository
  has no root `Cargo.toml`, but Cargo still found `packages/client-rust/Cargo.toml` by package
  name. The pin is `461b40267aabd644558f9b19dbb9507dd5f691cf`, the checkout HEAD on 2026-11-27.
- **The dependency adds no second version.** `reqwest 0.12.28`, `tokio 1.53.1`, `bytes`,
  `base64 0.22`, `parking_lot`, `serde_json` and `thiserror 2` were already in the workspace
  lock. Cargo unified every requirement except `wasm-streams`, which is new.
- **The two clients need different sync boundaries.** Our client runs under
  `block_on_future_scoped` with a valtron pool for the netio tasks. The reference client needs a
  tokio runtime with I/O and timers, because it uses reqwest. Two runtimes in one test process
  do not interfere.
- **The server address differs by client.** Our netio client can use `resolve_to` and a fake
  host (`http://testserver`). Reqwest resolves domain names itself, so the reference client must
  use the real `http://127.0.0.1:<port>` base URL. The Durable handler reads only the path, so
  this changes nothing else.
- **The SSE body type is chosen by an exact content-type match.** `foundation_netio` builds
  `SendSafeBody::SseStream` only when a response content-type equals `text/event-stream` after
  lowercasing. The server sends exactly that value, so `send_async` returns the event iterator.
- **Long-poll tests need a second writer.** Both test files start the read first, then sleep
  100 ms in another thread (or tokio task) and append. This proves the server wakes the read.
  The read data is asserted, not only the status.
- **The cross-run found a wire defect (issue #1065).** The SSE control event writes
  `streamCursor` as a JSON number. The protocol and the reference server use a string, and the
  reference client reads `as_str()`, so it silently ignores our cursor. Both of our sides read a
  number, so self-tests stay green. This is why the cross-implementation run is worth its cost.
- **`close()` with an empty body still sends a content type.** The reference client always adds
  `Content-Type`; our server ignores it when the body is empty and treats the request as a
  close. The scenario passes.
- **The SSE cursor is a string and needs a fallback on read (#1065).** `sse_control_json`
  inserted a `u64` with `cursor.into()`, so the wire form was a number. The reference client
  reads `as_str`, so it ignored the cursor. Our client read `as_u64`, so self-tests stayed
  green. The fix writes `cursor.to_string()` and reads `as_str` first, then `as_u64` for an old
  peer. A data event arrives before its control event, so the first reference
  `next_chunk` call has no cursor. The new test reads the second chunk (the control event) and
  asserts `chunk.cursor` and `reader.cursor()`.
- **A path filter can hide a protocol mismatch.** The conformance job first ran only when a
  pull request touched a durable path. A change outside that list could break the protocol and
  stay green. Owner rule 2026-11-27 removes the filter: the gate runs on every CI run. The job
  keeps its timeout, caches and pull request concurrency group, and `checks-passed` still
  requires it. This is a deliberate, documented exception to the "affected packages only" CI
  rule (AGENTS.md rule 16); the rule stays in force for every other job.
- **The foundation_http durable files carried pedantic clippy debt.** The crate denies warnings,
  so `cargo clippy --all-targets` failed before this change. The example put its `use` items
  after statements, the test header helper took a `SimpleHeader` by value, and the route test
  needed the owned request that its macro requires. Clear the debt before you trust the gate.

## Feature 01 T4: valtron as a Cargo feature (2026-11-27)

- **A `std` gate cannot prove a valtron-free core.** The module gate was
  `#[cfg(feature = "std")]`, so every build with `std` compiled valtron. A test crate could not
  turn it off. The fix adds a separate `valtron` feature. It implies `std` and stays on by
  default.
- **A feature that needs valtron must imply it.** `multi`, `test-clock`, `vfs-testkit`,
  `js-wasmbindgen` and `js-foundation-wasm` all use valtron code. Each now implies `valtron`.
  Without this, the CI trees for `multi` and `test-clock` would silently stop compiling the
  thread pool and `SimulatedClock` that those trees exist to test.
- **`vfs-testkit` needs `SimulatedClock`.** The clock lives in `valtron::ctx`, and its `advance`
  method wakes parked executors. Making `vfs-testkit` imply `valtron` is smaller and safer than
  moving the clock.
- **`synca::mpp` and valtron form a cycle.** The channels park on `ReadinessNotifier`, and the
  valtron executors use `mpp`. Both modules need the same feature gate.
- **The proof needs a package boundary.** A workspace-wide build merges the features of every
  member, so a `default-features = false` member still sees valtron when another member turns it
  on. The proof holds in `cargo test -p durable_core_no_valtron`, which is how CI runs one
  package.
- **A new feature tree is a new lint tree.** Clippy with `std,alloc` found two old doc warnings
  in `trace/mod.rs`: a bare URL and an empty line after a doc comment. The `standard` tree hid
  them, because the item after the comment was disabled. An inner module doc comment and a
  wrapped URL keep both trees clean.
- **A gate is a schema entry, not a workflow filter.** The Durable Streams conformance gate
  first ran on every change, then through a job-local path filter. Both broke rule 16. It is now
  `[gates.durable-conformance]` in `ci/packages.toml`: `foundation-ci affected` decides from the
  changed files, and `foundation-ci run` executes the recipe. A gate has no crate, so `plan`
  lists it and keeps it out of the package matrices, and `owner` never maps a file to it.
- **Select a gate from the changed files, not from the affected set.** The planner builds its
  reverse graph from the resolver closure, which includes dev-dependencies. A change to
  `foundation_wasm_ui` therefore marked `foundation_core` as a dependent and selected the gate
  through an unrelated crate. Reading the file owners of the change keeps the gate on the
  packages that the owner named.

## Feature 08 runtime matrix and skip register (2026-11-27)

- **The DO fixture is also the Worker proof.** `backends/foundation_db/integration/do_sql/`
  serves `GET /run-d1` from its top-level `#[event(fetch)]` handler. That route runs the VFS kit
  over `D1Executor` in the Worker context. The `GET /run` route forwards to `DoSqlObject` and
  runs the same kit over DO SQLite. One fixture proves both rows. A DO-only fixture would not
  prove the Worker row.
- **A wasm build is not a wasm run.** The CI wasm gate catches feature-gate drift. It found
  #1030 and #1056. It cannot find an event-loop fault. The `integration/wasm_tests` crate runs
  valtron executor cases on Deno, not the durable core. Issue #1080 records the missing
  roundtrip. Do not call the build a runtime test.
- **No WebSocket durable driver exists.** `foundation_netio::websocket` is a frame transport.
  `foundation_http::native::upgrade::accept_websocket` completes the upgrade. Neither maps a
  message to `DurableService` or to the client transport. The spec names a driver that no code
  implements. Issue #1077 records the missing piece.
- **`AgentSession` is not backed by a stream.** It uses `foundation_db::traits::DocumentStore`
  and `agentic::memory_store::MemoryStore`. No implementation of either trait names a durable
  stream. `SessionRecord` is the log element, but the log storage is not a stream. Issue #1079
  records the missing driver.
- **The conformance gate compiles the valtron feature.** `ci/durable_conformance.sh` builds
  `foundation_http --features multi`. `multi` implies `foundation_core/multi`, which implies
  `valtron`. The example server also calls `initialize_pool` and mounts the subscription
  handler. The protocol path uses `DurableService`, not the valtron reader or writer. Requirement
  8 says the gate does not enable the valtron layer. The gate does. Issue #1081 records the
  variance. The separate `durable_core_no_valtron` package is the real no-valtron proof.
- **The npm package does not carry its git commit.** The two skip lines are in
  `read-sse-base64.yaml`. To name the author and commit, fetch the file from the upstream
  repository at the pinned commit and search its history. Both skips were present in
  `82a566ace620b1b8d0d43cdf181356e6a6f6f4aa` by Valter Balegas. The pinned checkout
  `461b40267aabd644558f9b19dbb9507dd5f691cf` has the same two lines at lines 55 and 367.
- **Spec test names can be aspirational.** A search for `websocket_core_roundtrip`,
  `session_uses_core` and `wasm_core_roundtrip` found no test. A name in a task list is not
  evidence. The report marks each missing name as a gap with an issue.
- **The embedded row has two forms.** `durable_core_no_valtron` runs with `futures-lite` and no
  valtron feature. `durable::runtime_matrix` runs under the valtron test harness, because the
  rest of the crate tests do. Both use direct async calls and no transport. The second form
  proves the business API and `DurableService`; the first proves the core compiles and runs
  without valtron.
