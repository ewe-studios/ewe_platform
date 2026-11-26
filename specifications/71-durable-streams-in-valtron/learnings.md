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
- **The valtron layer adds only scheduling.** `Stream::Wait`, `Stream::Spread`,
  `Stream::Pending`, `WakeableStream`, `#[task]` and the bridges sit above the core. They are
  the right tools for the job. They are not the contract.
