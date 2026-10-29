# Durable Streams: the resumable, offset-addressable log over HTTP

_Part of the [edge data research notes](./README.md). Written in ASD-STE100 Simplified Technical
English (STE). Code and identifiers stay as they are. This is research, not a design._

**Abbreviations in this file.** Hypertext Transfer Protocol (HTTP); Server-Sent Events (SSE);
Uniform Resource Locator (URL); Content Delivery Network (CDN); time to live (TTL); end of file
(EOF); JavaScript Object Notation (JSON); Newline-Delimited JSON (NDJSON); identifier (ID);
Lightning Memory-Mapped Database (LMDB); Durable Object (DO); Structured Query Language (SQL);
Amazon Simple Storage Service (S3); deterministic simulation testing (DST); AeroSpace and Defence
Industries Association of Europe (ASD), the owner of the STE standard ASD-STE100. Durable Streams,
Caddy, Electric Cloud, Durable State, Durable Proxy, StreamDB, TanStack DB and Cloudflare are
product names, not abbreviations. R2 and D1 are names of Cloudflare products, not abbreviations.

**Source key.** `DS` = `github.com/durable-streams/durable-streams`, read at commit `461b4026`
(2026-10-30). Files: `PROTOCOL.md`, `README.md`, `docs/concepts.md`, `docs/fork.md`,
`docs/durable-state.md`, `docs/stream-db.md`, `docs/building-a-server.md`, `docs/deployment.md`.
The repository is checked out at `/Users/darkvoid/GitSources/src.Electric/durable-streams`.

## 1. What Durable Streams is

A Durable Stream is a **URL-addressable, append-only, durable sequence of bytes**. Each stream
lives at its own URL and has one content type set at creation. Data written at a position never
changes. New data only appends to the end. Readers read from any offset and resume exactly where
they stopped, across page refreshes, network flaps and device switches.

It is a protocol first, with many implementations. The protocol uses plain HTTP. The server side
has two reference implementations: a Node.js development server (`@durable-streams/server`, log
files plus LMDB) and a production Go server as a Caddy plugin (`caddy-plugin`, LMDB). A
Cloudflare Workers plus DO server exists (`@durable-streams/server-cloudflare`). Client libraries
exist for ten languages, including Rust (`client-rust`).

## 2. The protocol in one page

| Operation | HTTP | Meaning |
|---|---|---|
| Create | `PUT` | Idempotent. `200` on a matching re-create, `409` on mismatch. |
| Append | `POST` | Add bytes at the tail. `Stream-Closed: true` on the same call closes atomically. |
| Read | `GET ?offset=` | Catch-up, or live with `live=long-poll` / `live=sse`. |
| Metadata | `HEAD` | Tail offset, TTL, closure, content type. No body. |
| Close | `POST` + `Stream-Closed: true` | Durable EOF. No more appends. Existing data stays readable. |
| Delete | `DELETE` | Remove the stream and its data. |

The key ideas:

- **Offsets are opaque and lexicographically sortable.** A client never parses or builds an
  offset. It compares two offsets from one stream as strings. Two sentinels exist: `"-1"` is the
  start, `"now"` is the current tail. Reading at `"now"` skips existing data and watches only new
  data.
- **`Stream-Next-Offset` is the resume cursor.** Every read returns it. The client stores it and
  passes it back on the next read. This is the whole resumption mechanism.
- **Two flags end a read loop.** `Stream-Up-To-Date: true` means "you saw everything available
  now, more may come". `Stream-Closed: true` means "no more will ever come" (EOF).
- **`410 Gone` when the offset is trimmed.** If retention dropped the requested offset, the
  server refuses, never skips. The client resets to `"-1"` or `"now"`.

## 3. Insights

### 3.1 Offset-based resumption is the whole client contract

- **What it is:** The client is a loop. It reads from an offset, consumes the body, stores
  `Stream-Next-Offset`, and reads again from that offset. Byte-exact resumption is a conformance
  requirement: reading in chunks through the next-offset cursor must rebuild the same bytes as one
  full read, with no skip and no duplicate.
- **Why it is interesting:** WebSocket and SSE connections are easy to start and fragile to keep.
  A tab suspension or a network flap drops in-flight data. Durable Streams makes the *cursor* the
  state, not the connection. The connection can die at any time; the client reconnects and resumes
  from the cursor. Token streaming is the UI for chat and copilots, and agentic apps stream tool
  outputs and progress over long sessions — when the stream fails, the product fails even if the
  model was right.
- **Limits:** Offsets are per-stream, not global. A client must keep one cursor per stream. The
  server defines the offset format; a client must not assume it.
- **Where it could apply:** Every valtron `StreamIterator` we expose over HTTP or RPC. The resume
  cursor is the missing contract that turns `Stream::Next(chunk)` into "reconnect and continue".
  This is the core of spec 71 (durable streams in valtron).

### 3.2 Exactly-once writes from an idempotent producer

- **What it is:** A writer identifies itself with three headers: `Producer-Id` (stable name),
  `Producer-Epoch` (bumped on restart), and `Producer-Seq` (monotonic within the epoch). All three
  come together or not at all. The server stores the last accepted `(stream, producer, epoch, seq)`.
  A retried sequence returns a deduplicated `204` success instead of a duplicate write. A new epoch
  starts at `seq=0`. A stale epoch is **fenced**: requests from an old epoch get `403 Forbidden`.
  A sequence gap gets `409 Conflict`.
- **Why it is interesting:** This is the *same* fencing rule the edge notes already use elsewhere.
  The producer epoch fences a zombie writer exactly as lunora's epoch detects a forked timeline
  ([change-logs.md section 2.5](./change-logs.md#25-the-epoch-detecting-a-forked-timeline)). The
  deduplicated `204` is "the write is durable if and only if it is acknowledged once", the same
  idea as the change-log-is-in-the-same-transaction rule ([change-logs.md section 2.1](./change-logs.md#21-the-change-log-is-written-in-the-same-transaction-as-the-row)).
- **Limits:** Producer state and the log append must commit atomically or a crash between them can
  replay or drop. The Caddy file store does not do this atomically today; its failure mode is a
  false `409`, not a duplicate (the durable-streams repo issue #143).
- **Where it could apply:** Any place we write to `foundation_log`, a DO tail, or a D1 flush from
  a retryable path. `foundation_log` already plans fencing via `If-None-Match: *` on objects
  ([tiered-store.md section 6.4](./tiered-store.md#64-making-it-fast)). The producer triple gives
  the same guarantee at the *record* level, not the object level.

### 3.3 One interface, two live modes, CDN fan-out for free

- **What it is:** The same `GET` serves catch-up and live reads. `live=long-poll` holds the
  connection until data arrives (200 with data) or a timeout expires (204, retry with the same
  offset). `live=sse` streams `data` events (payload) and `control` events (next offset, up-to-date,
  closed). The server closes SSE connections roughly every 60 seconds and hands back a
  `Stream-Cursor` so a CDN edge can **collapse** many waiting clients at the same offset into one
  upstream connection. Historical catch-up reads are immutable, so the CDN caches them with
  `Cache-Control` and `ETag`.
- **Why it is interesting:** It separates "read history" (cacheable, immutable, cheap) from "watch
  the tail" (collapsible fan-out). One origin can serve millions of concurrent viewers because the
  CDN absorbs the read path. This is the fan-out answer that lunora's relay DOs reach for by hand
  ([durable-objects-and-workers.md section 2.5](./durable-objects-and-workers.md#25-sockets-terminate-near-the-client-through-relay-dos)).
- **Limits:** The CDN collapse depends on a cursor the edge nodes understand. Auth is out of the
  protocol scope; Caddy or the Worker adds it.
- **Where it could apply:** The `Stream::Wait` + `WakeableStream` shape valtron already has is the
  long-poll. The SSE `control` event is the `Stream-Next-Offset` cursor surfaced in-band. The
  cursor-based collapsing is a deployment property, not a code property.

### 3.4 Fork: branch a stream without copying the prefix

- **What it is:** A `PUT` with `Stream-Forked-From: <source>` and an optional
  `Stream-Fork-Offset` creates a new stream that shares the source's bytes up to the fork offset
  and has its own tail after it. Reads span the boundary transparently. The fork has its own URL,
  TTL, closure and deletion, and can outlive the source. Deleting a source with live forks
  soft-deletes it (`410` for the URL, `409` on re-create) and its data stays readable through the
  forks; when the last fork is deleted the source is garbage-collected, cascading up the chain.
- **Why it is interesting:** Fork is "speculative or exploratory writes" and "writer handoff" done
  cheaply. An AI conversation can branch at a message, a replay can run against a fixed history, a
  new producer can take over at a known offset under a new URL — all without copying the shared
  prefix.
- **Limits:** The reference count and soft-delete are server bookkeeping. A fork chain that is
  read must resolve every hop.
- **Where it could apply:** The agent-rpc turn/event stream. A forked agent session (one
  conversation that branches into two) is a fork of the turn log. In valtron it is a combinator:
  chain the source's prefix iterator with an owned tail iterator.

### 3.5 An immutable prefix makes the read path cacheable and trimmable

- **What it is:** Bytes at an offset never change. So a catch-up response for a given offset range
  is immutable, and the CDN caches it. Retention may drop old data; a read of a dropped offset
  returns `410 Gone` (a refusal), never a silent gap. A reader that cannot reach its offset resets
  to `"-1"` or `"now"`.
- **Why it is interesting:** "A refusal, never a gap" is the same rule lunora states for its
  archive ([tiered-store.md section 6.2](./tiered-store.md#62-key-layout-lunoras-archive-keys)).
  Durable Streams makes it a protocol error code.
- **Limits:** Retention is server policy; the protocol only says how to signal it.
- **Where it could apply:** `foundation_log` segments are already immutable. The `410`-style
  refusal is the reader-side contract to add: a trimmed segment is an error, not a skipped range.

### 3.6 JSON mode: message boundaries and array flattening

- **What it is:** A stream created with `Content-Type: application/json` preserves message
  boundaries (each `POST` stores its payload as one message), flattens a `POST` of an array `[a, b,
  c]` into three messages, and returns a `GET` as a JSON array of the messages in range. An empty
  array `[]` is rejected with `400`. Byte streams use a self-delimiting format such as NDJSON
  instead.
- **Why it is interesting:** Batching many messages in one `POST` is the exact shape of
  `Stream::Spread(Vec<StreamSpread<D, P>>)` in valtron: one delivery that carries several values.
- **Limits:** JSON mode is a convenience, not a requirement; the same effect comes from NDJSON
  over a byte stream.
- **Where it could apply:** The mapping from a `Stream::Spread` batch to one wire append, and back.

### 3.7 State protocol and Durable Sessions layered on top

- **What it is:** Three layers stack on Durable Streams. (1) The **State protocol** defines typed
  `insert`/`update`/`delete` change messages and `snapshot-start`/`snapshot-end`/`reset` control
  messages over a JSON stream. (2) **StreamDB** builds a reactive, schema-typed database on that
  (`@durable-streams/state/db` with TanStack DB). (3) **Durable Sessions** multiplex an AI token
  stream with structured state so several users and agents join the same session and replay it.
  A **Durable Proxy** (`@durable-streams/proxy`) puts resumable streaming in front of an existing
  SSE AI endpoint without changing the upstream.
- **Why it is interesting:** It shows the layering order that works: durable byte delivery, then
  structured CRUD, then application protocols. The `snapshot-start/end` control events are the same
  boundary the edge notes need for a full state dump versus incremental changes.
- **Limits:** `MaterializedState` is a plain in-memory map; the reactive query engine (TanStack DB)
  is an application dependency, not part of the stream protocol.
- **Where it could apply:** Spec 65's agent-rpc state (a turn log plus a materialised instance
  state) and the "snapshot boundary" idea for any state feed we ship.

### 3.8 The storage contract is small and any backend fits

- **What it is:** A conforming server needs only: durable append, offset generation (opaque,
  lexicographically sortable, strictly increasing, unique within a stream), read-from-offset in a
  bounded chunk, stream metadata (content type, tail, TTL, closed), and delete. The reference
  servers use in-memory and file-plus-LMDB stores. The docs name Postgres, SQLite and S3 as
  valid backends.
- **Why it is interesting:** The contract is the *minimum* a durable log needs. Every tier the
  edge notes already map (`foundation_db` `BlobStore`/`QueryStore`, DO SQLite, D1, R2/S3) can
  satisfy it. This is the trait boundary spec 71 turns into a valtron `DurableStreamStore`.
- **Limits:** The contract says nothing about consistency across streams, auth, or multi-region.
- **Where it could apply:** The storage trait injected through valtron's `Ctx` (rule 11: side
  effects are injected, never global). The same reader/writer types run on in-memory (DST), a
  native file, R2/S3 segments, DO SQLite (T0) or D1 (T1).

## 4. How the ideas map onto our crates

| Idea | Our home | Notes |
|---|---|---|
| Offset + `Stream-Next-Offset` cursor | `foundation_core::valtron` | A `DurableStream` reader type; the cursor threads through `Stream::Next`. |
| Idempotent producer (`producer-id`/`epoch`/`seq`) | `foundation_core::valtron` + `foundation_log` | A `#[task]` producer state machine; fencing = a distinct error state. |
| Live tail (`Wait` + wake) | `foundation_core::valtron` | `Stream::Wait` + `WakeableStream` already is the long-poll. |
| `Stream::Spread` batching | `foundation_core::valtron` | Array flattening and batch append map onto `Spread`. |
| Fork without copy | `foundation_core::valtron` | A prefix-chain combinator; soft-delete is backend bookkeeping. |
| `410` = a refusal, never a gap | `foundation_log` | A trimmed segment is an error, not a skip. |
| Immutable prefix + ETag/cache | `foundation_http`, `foundation_netio` | Catch-up responses are cacheable; live responses are not. |
| State protocol (insert/update/delete + snapshot) | `foundation_db`, spec 65 feature 07 | Structured CRUD over the stream; the snapshot boundary. |
| Storage contract | `foundation_db` `BlobStore`/`QueryStore` | The `DurableStreamStore` trait; every tier implements it. |
| Six HTTP verbs + headers | `foundation_http`, `foundation_connectrpc` | The wire surface that "spreads the capability around". |

## 5. What we throw away

- **The CDN/edge emphasis as a design goal.** We keep the immutable-prefix and cursor properties
  (they make the read path cacheable), but our first target is a native log and a Cloudflare
  Worker, not a public CDN product.
- **The ten client libraries.** We need one Rust implementation in valtron, not ten language
  clients.
- **JSON mode as a wire format choice.** We use the `Spread` idea, not the JSON-array wire rule.
  Byte streams with NDJSON or typed messages cover the same ground.
- **TanStack DB / StreamDB as a dependency.** The reactive query engine is application glue. The
  stream + state protocol is the part we want.
- **The Durable Proxy as a product.** Its lesson (a resumable read URL in front of a streaming
  API) applies to spec 65's Cloudflare target, not as a separate service.
