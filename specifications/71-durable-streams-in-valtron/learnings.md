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
