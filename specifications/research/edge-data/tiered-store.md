# Tiered store and `foundation_log` on R2 and S3

_Part of the [edge data research notes](./README.md). Written in ASD-STE100 Simplified Technical
English (STE). Code and identifiers stay as they are. This is research, not a design._

**Abbreviations in this file.** Durable Object (DO); Structured Query Language (SQL); online
transaction processing (OLTP); read-your-writes (RYW); change data capture (CDC); deterministic
simulation testing (DST); application programming interface (API); point-in-time recovery (PITR);
gigabyte (GB); gigabyte-month (GB-mo); mebibyte (MiB); identifier (ID); Hypertext Transfer
Protocol (HTTP); JavaScript Object Notation Lines (JSONL); Amazon Simple Storage Service (S3);
time to live (TTL); AeroSpace and Defence Industries Association of Europe (ASD), the owner of the
STE standard ASD-STE100. D1 and R2 are names of Cloudflare products, not abbreviations.

Source keys are in [README.md section 3](./README.md#3-sources). "NATS p.97" is page 97 of
`Nats and Cloudflare.pdf`. "GED p.25" is page 25 of `Ged Scalable Dos and D1s Discussions.pdf`.
"TP" is `Cloudflare, DOs and D1s Architecture Thought process.md`.

## 1. The idea in one paragraph

A tiered store keeps each record in the cheapest place that still gives the reads it needs. On
Cloudflare there are three tiers:

| Tier | Product | Role | Time to read | Scope of one query |
|---|---|---|---|---|
| T0, hot | DO SQLite storage | All writes land here. Read-your-writes for its own entity. | microseconds inside the DO; one hop from a Worker | one entity (one DO) |
| T1, warm | D1 | Queryable state across entities, read replicas near readers | tens of milliseconds, less on a near replica | one database (one shard category value) |
| T2, cold | R2 | Immutable segments, history, archive | one range `GET` | one object |

Data moves **down** (T0 → T1 → T2) by explicit steps. Each step writes the next tier first, then
records a watermark, then deletes from the tier above. That one ordering rule is the core of the
idea. Every source agrees on it (NATS p.99, p.113–116; TP Part 3; the lunora changelog archive).

## 2. The tiers in detail

### 2.1 T0: DO SQLite storage (hot)

- **Source:** GED p.2–3; TP Part 3 and Part 4; NATS p.86–87, p.101–103; Cloudflare "Rules of Durable Objects".
- **What it is:** The SQLite database inside a DO. `ctx.storage.sql.exec()` is synchronous and runs in the same thread as the DO code (GED p.2).
- **Why it is interesting:** A write commits locally and is durable when the call returns. Many queries inside one DO call cost no network. The DO is single-threaded, so the sequence of commits is the sequence of calls. A per-DO counter in the same transaction gives a gap-free commit order (lunora `_commitSeq`, see [change-logs.md](./change-logs.md#22-commit-ordered-sequence-not-wall-clock)).
- **Limits:** 10 GB for each DO. About 500 to 1,000 simple requests each second for each DO (Rules of Durable Objects, "Message throughput limits"). 100 bound parameters for each statement. A query cannot cross DOs. Storage costs $0.20/GB-mo, rows written cost $1.00 for each million.
- **Where it could apply:** `DoSqlStore` (spec 65 feature 07, task T20) gives `QueryStore` and `KeyValueStore` over `worker::Storage::sql()`. That is T0 for us.

### 2.2 T1: D1 (warm)

- **Source:** GED p.2, p.19–21; NATS p.109–116, p.127–131; Cloudflare D1 read replication documentation.
- **What it is:** A SQLite database behind a network API. One query runs at a time for each database. Read replicas exist in several regions. The Sessions API with bookmarks gives sequential consistency.
- **Why it is interesting:** It is the only tier where one query sees many entities. Joins, aggregates and pagination work natively. Read replicas cost nothing extra; you pay only rows read (GED p.2).
- **Limits:** 10 GB for each database. 50,000 databases for each account by default (raise on request). About 5,000 D1 bindings for each Worker script. Storage costs $0.75/GB-mo, which is the highest of the three tiers. Writes always go to the primary.
- **Where it could apply:** `D1WasmStorage` is T1 for us. Owner position: many D1 databases for each shard category, picked for each entity ([read-scaling-and-replicas.md](./read-scaling-and-replicas.md#3-many-named-d1-databases)).

### 2.3 T2: R2 (cold)

- **Source:** NATS p.87–99; Cloudflare R2 limits, pricing and consistency documentation; lunora `ctx-db-cdc-archive.ts`.
- **What it is:** Object storage with strong read-after-write and strong listing consistency. It has conditional writes (`If-None-Match`, `If-Match`, `onlyIf`) and range reads.
- **Why it is interesting:** Storage is $0.015/GB-mo, 50 times cheaper than D1. Egress is free. Objects are immutable from the reader's view, so a reader can cache them forever.
- **Limits:** No append (NATS p.94). The parts of a multipart upload are invisible until the upload completes (NATS p.97). At most one concurrent write each second to the same key. Each `PUT` is a Class A operation ($4.50 for each million). A listing compares keys as strings.
- **Where it could apply:** `BlobStore` (`R2WasmStorage`, `R2Store`). Owner position: the R2 layout is the business of each use case. So T2 here is a contract (immutable segments, ordered keys, a watermark), not a file format.

## 3. Moving data between tiers

### 3.1 Demotion T0 → T1: the flush

Owner position: the hot operations live in a DO that writes to its SQLite storage. A
per-use-case interface asks the DO to flush to D1.

The sources give one safe shape for the flush:

1. The DO reads a bounded page of un-flushed rows with a cursor, for example `WHERE seq > ? ORDER BY seq LIMIT ?` (NATS p.102–103, p.114). It never loads the whole backlog.
2. The DO writes the page to D1 in one `batch()` (NATS p.115). A D1 `batch()` runs as one transaction.
3. When D1 confirms, the DO writes `flushed_through = <last seq>` in its own storage.
4. Then the DO deletes the flushed rows, or keeps them for a configured window.
5. The DO returns the D1 bookmark of its flush session (`session.getBookmark()`) to the caller.

Step 5 is the insight that the sources do not state. After a flush, a read through a D1 replica
can still miss the flushed rows, because replication is asynchronous. The reader must start its
D1 session from the bookmark that the flush returned. Then the replica waits until it has the
flushed rows. Without the bookmark, "flush, then read D1" is not read-your-writes.

To make a repeated flush harmless, write D1 rows as an upsert that carries the DO sequence:

```sql
INSERT INTO items (id, payload, seq, deleted) VALUES (?, ?, ?, ?)
ON CONFLICT (id) DO UPDATE SET payload = excluded.payload, seq = excluded.seq,
    deleted = excluded.deleted
WHERE excluded.seq > items.seq;
```

What the code does: a replayed or out-of-order flush never moves a row back to an older version.
A delete travels as a tombstone row (`deleted = 1`), as NATS p.143 suggests for the overlay.

### 3.2 Demotion T1 → T2: the archive

- **Source:** NATS p.119–122 (core tables and delta tables); lunora `cdc-retention.ts` and `ctx-db-cdc-archive.ts`; spec 65 feature 07 `hosts.md` section 5.12.
- **What it is:** Rows that reached a terminal state (an invoice that is `paid`, an audit entry, a closed conversation) move to R2. Core state (users, balances, settings) stays in D1 for ever. Prune by state, not by age (NATS p.119–120).
- **Why it is interesting:** It keeps D1, the most expensive tier per byte, small. lunora calls the alternative a "cliff": without an archive, a reader that falls behind the retention window must re-seed from a snapshot. With an archive, the cliff becomes a tier.
- **Limits:** A user who asks for old data needs a second read path (R2), and the two results must be merged (NATS p.121).
- **Where it could apply:** Feature 07 already archives a destroyed instance's history to R2 before it calls `delete_all()`. That is the same rule at the scale of one DO.

### 3.3 Promotion T2 → T1 or T0

The sources do not move cold data back up during normal reads. A cold read is served from R2
directly: a range `GET`, or a query engine over Parquet (NATS p.110–111, p.122–123). Promotion
happens only on restore or re-seed. Keep it that way: promotion on read would make the cold tier
a cache with an invalidation problem.

### 3.4 The watermark rule

Every source that survives a crash uses the same three steps: **write below, record the
watermark, then destroy above.** lunora states it most clearly in `writeCdcArchivedThrough`:
"Must be called only after the `put` resolves, and before anything destructive runs."

lunora also keeps the watermark as DO state, not as something it reads back from the bucket. R2
lists only in ascending order, so "what is the newest segment" is a walk over the whole archive.
The DO already knows the answer.

## 4. Reading across the tiers

Owner position: reads come from D1 first, then from the DO. Global data goes straight to a shared
D1. The sources show three ways to do "D1 first, then the DO":

| Method | Source | Correct for | Breaks for |
|---|---|---|---|
| Flush first, then read D1 only (CQRS) | TP Part 3, Model B; TP Part 4 | Aggregates, pagination, joins: D1 does all the work | Adds one DO call and a D1 write before the read. Must carry the flush bookmark (section 3.1). |
| Overlay: D1 gives the base rows, the DO gives the un-flushed rows on top | NATS p.140–145 | One entity's bounded list | Large results (memory), `SUM`/`COUNT`, `LIMIT`/`OFFSET`, multi-row transactions across the two tiers (NATS p.146–148) |
| Point fallback: one key, one tier at a time | NATS p.137–139 | Point reads | Lists |

Both the overlay and the fallback must check the DO for a newer version of a key that D1 has.
A D1 hit alone is not proof that the DO has nothing newer.

The sources agree on where each method fits (NATS p.148): use the overlay or the fallback only
for data that belongs to one entity and is naturally small. Use flush-first for everything that
D1 must aggregate.

## 5. Consistency, step by step

| Moment | What a reader sees | Why |
|---|---|---|
| The DO commits a write | The DO sees it. D1 does not. | The flush has not run. |
| The flush `batch()` commits on the D1 primary | A D1 session that starts `first-primary`, or from the flush bookmark, sees it. Another session may not. | Replicas are asynchronous. |
| The DO writes `flushed_through` | A retry of the flush starts after this point. | The watermark is the resume cursor. |
| The DO deletes the flushed rows | Only D1 has the rows. | The watermark was durable first. |
| The archive `PUT` to R2 resolves | The segment is readable everywhere at once. | R2 read-after-write is strong. |
| D1 deletes the archived rows | Only R2 has the rows. | Same rule one tier down. |

### 5.1 Crash windows (inputs for the DST layer)

Owner position: each spec has its own DST layer. These are the faults a tiered store must
survive. Each row is one test.

| Crash or fault | Effect | What makes it safe |
|---|---|---|
| After the D1 `batch()`, before `flushed_through` | The next flush sends the same rows again | The `WHERE excluded.seq > items.seq` upsert |
| After `flushed_through`, before the delete | Rows stay in the DO longer | The next flush deletes at or below the watermark |
| After the R2 `PUT`, before the archive watermark | The next sweep writes the same range again | The key is derived from the range, so the second `PUT` writes the same bytes. Add `If-None-Match: *` and treat `412` as "already done". |
| The D1 `batch()` fails | Nothing changed in D1 | The DO keeps its rows and retries. Never advance the watermark on failure. |
| The reader has a stale bookmark | The read waits on a replica, or goes to the primary | Use the D1 constraints, not your own timers |
| A PITR restore of the DO | The DO rewinds its watermark and its sequence. Old R2 segments describe a different timeline. | Put an epoch in the R2 key prefix and re-mint it on restore (lunora, section 6.2) |

## 6. `foundation_log` on R2 and S3

The owner asked: "how do we also apply it to S3, and make it super performant". Issue
[#155](https://github.com/ewe-studios/ewe_platform/issues/155) holds the full research brief
(prior art, primitives, checksums, fencing). This section adds only what these sources show.

### 6.1 The log is the tiered store with one table

A segmented append log fits the three tiers exactly:

- T0 is the hot tail. On Cloudflare it is DO SQLite (NATS p.86–87). Natively it is a local file (NATS p.25–27). For plain S3 with no DO it is an in-memory group-commit buffer.
- T1 is optional. A log does not need D1 unless something must query records across streams.
- T2 is sealed segments as objects (NATS p.87–89).

### 6.2 Key layout: lunora's archive keys

- **Source:** lunora `packages/shard-engine/src/ctx-db-cdc-archive.ts`.
- **What it is:** `cdc/<shard>/<epoch>/<pad16(last seq)>.json`. The key is the segment's **last** sequence, zero-padded to 16 digits.
- **Why it is interesting:**
  - A reader at cursor `c` finds every segment it still needs with one `list({ startAfter: prefix + pad16(c) + ".json" })`. There is no manifest object to keep consistent with the segments.
  - The padding makes string order equal number order.
  - The epoch is part of the prefix. A forked timeline (after a restore) writes under a new prefix, so old segments are unreachable by construction, not by a check the reader can forget.
  - The writer keeps "archived through" in its own storage. It never derives the newest segment from a listing.
- **Limits:** A listing is paged, and lunora caps one read-back at 32 segments. Beyond that the consumer re-seeds. An R2 lifecycle rule can delete old segments. The reader then must refuse, never skip (lunora: "a refusal, never a gap").
- **Where it could apply:** The key scheme of `foundation_log` on R2 and S3. S3 `ListObjectsV2` with `start-after` gives the same "one call from a cursor".

### 6.3 Never append to an object

| Approach | Why it fails | Source |
|---|---|---|
| Download, append, upload again | Memory grows with the object; Worker memory is small | NATS p.94; TP Part 3 |
| Multipart upload as an append | Parts are invisible until `CompleteMultipartUpload`; minimum part is 5 MiB | NATS p.95, p.97 |
| Rewrite the active tail object on each flush | R2 allows one concurrent write each second to one key; each rewrite is a Class A operation | NATS p.88; R2 limits |

So: a segment is written once, complete, under a key it never shares with another segment. The
hot tail stays in T0 until the segment is full.

### 6.4 Making it fast

| Lever | What to do | Source |
|---|---|---|
| Acknowledge at the tail | A DO SQLite commit is durable. Acknowledge the producer there (1–2 ms), and upload later. Offer two acknowledgement levels: "accepted" (tail) and "sealed" (object). | NATS p.86–87, p.117 |
| Group commit | Seal on bytes, records or age, whichever comes first. All three are config fields. | NATS p.88, p.99; CLAUDE.md rule 8 |
| Amortise the `PUT` | One Class A operation for each segment. At 5,000 records for each segment, `PUT` cost is about $0.0009 for each million records. | R2 pricing |
| Bounded memory while sealing | Read the tail with a cursor in pages, stream the encoded bytes into the upload. Never hold the segment as objects in memory. | NATS p.102–103 |
| Conditional create | `PUT` with `If-None-Match: *`. A second writer of the same segment gets `412`. That is fencing for a single writer, and it makes a retried upload harmless. | R2 consistency documentation |
| Immutable reads | A sealed segment never changes. Cache it with no TTL (Workers Cache, or a local disk cache natively). Use range `GET`s for the index and the records you need. | NATS p.89; R2 range reads |
| Prefetch | A sequential reader fetches segment n+1 while it streams segment n. | NATS p.89 |
| Recovery | A native tail file needs the scan that cuts off a half-written record (NATS p.67–68). An object is all or nothing, so on R2/S3 recovery is: list after the watermark, check each found segment, adopt it or re-upload. | NATS p.67–68 |

### 6.5 Corrections to the source PDF

Issue #155 already lists them: NATS uses highwayhash, not CRC-32; newer NATS has no per-block
`.idx` files; JetStream runs one Raft group for each stream and each consumer; "seq mod N" offsets
break with variable-size records (NATS p.88–89).

## 7. Costs

Workers Paid list prices from the Cloudflare pricing pages, read in October 2026:

| Item | DO SQLite (T0) | D1 (T1) | R2 Standard (T2) |
|---|---|---|---|
| Storage | $0.20/GB-mo | $0.75/GB-mo | $0.015/GB-mo |
| Writes | $1.00 for each million rows written | $1.00 for each million rows written | $4.50 for each million `PUT`s (Class A) |
| Reads | $0.001 for each million rows read | $0.001 for each million rows read | $0.36 for each million `GET`s (Class B) |
| Included each month | 50 M rows written, 25 B rows read, 5 GB | 50 M rows written, 25 B rows read, 5 GB | 1 M Class A, 10 M Class B, 10 GB |

What the numbers say:

- **Writes cost more than storage.** One row that goes T0 → T1 → T2 pays at least four row writes: insert and delete in the DO, insert and delete in D1. Indexes add more. Batching cuts the R2 cost to almost nothing, but it does not cut row writes.
- **D1 is the most expensive place to keep bytes.** Keep only queryable core state and recent deltas there (NATS p.119–122).
- **Skipping T1 saves two row writes.** Append-only history that nobody queries across entities can go T0 → T2 directly (NATS p.118). The cost is that SQL over it needs a query engine on R2.

## 8. How it maps onto `foundation_db`

| Tier | Trait | Type today | Gap |
|---|---|---|---|
| T0 | `QueryStore`, `KeyValueStore` | `DoSqlStore` (planned, feature 07 task T20) | None for the tier itself |
| T1 | `QueryStore`, `KeyValueStore` | `D1WasmStorage` | No session or consistency parameter (no `withSession`, no bookmark). `execute_batch_async` takes one raw SQL string with no parameters, so an atomic multi-row flush through `batch()` is not possible. |
| T2 | `BlobStore` | `R2WasmStorage`, `R2Store` | Whole objects in memory (`Vec<u8>`), no range read, no conditional put, no ordered listing from a start key |

The gaps are on the [bug backlog](../../../docs/bug-backlog.md), section 11.

A tiered store is then a composition over the three traits, plus a policy object (thresholds,
batch sizes, retention) whose fields are config with documented defaults. The flush interface of
each use case owns the row shape. This note does not design the composition.

## 9. Open questions

1. Must every use case use the same flush shape, or only the same watermark rule?
2. Is an outbox table enough as the T0 change feed, or does each use case need a full CDC log ([change-logs.md](./change-logs.md))?
3. Where does the D1 bookmark of a flush travel: the response of the flush call, a header, or a cookie (emdash uses a cookie)?
4. Which cold format for which use case: JSONL plus gzip (feature 07 archive), Parquet (NATS p.96), or `foundation_log` segments?
