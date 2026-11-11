# Edge data patterns: research notes

_Written in ASD-STE100 Simplified Technical English (STE). Code and identifiers stay as they are._

**Abbreviations in this file.** Durable Object (DO); Structured Query Language (SQL); change data
capture (CDC); deterministic simulation testing (DST); read-your-writes (RYW); identifier (ID);
Amazon Simple Storage Service (S3); portable document format (PDF); uniform resource locator
(URL); AeroSpace and Defence Industries Association of Europe (ASD), the owner of the STE standard
ASD-STE100. D1 and R2 are names of Cloudflare products, not abbreviations.

## 1. Purpose and framing

These notes explore other projects and pull out interesting ideas. The owner's framing:

> These are meant for us to explore and pull out interesting ideas from those projects: how they
> uniquely use Durable Objects and Workers; how they scale reads and replicas; how they do change
> logs. None of these is informing designs. We take the insights and create markdown from them to
> learn from, and the useless we throw away.

So nothing here is a decision. Each insight gives its source, what it is, why it is interesting,
its limits, and where it could apply in our crates.

## 2. Owner positions (the lens)

The notes use these positions as given. They do not argue with them.

1. **Many D1 databases for each shard category** (for example country, tenant). Each entity picks the right database. Naming the databases gives the placement benefit; use whatever placement Cloudflare offers.
2. **The R2 layout is the business of each use case.**
3. **Hot operations live in a DO** that writes to its SQLite storage. A per-use-case interface asks the DO to flush to D1. Reads come from D1 first, then from the DO. Global data goes straight to a shared D1.
4. **DO replicas "never was possible anyway".**
5. **Each spec has its own DST layer.**
6. **High interest:** the tiered store, and `foundation_log` (an append-only log) on S3 and R2, made very fast. These two get the deepest treatment ([tiered-store.md](./tiered-store.md)). Issue [#155](https://github.com/ewe-studios/ewe_platform/issues/155) holds the research brief for `foundation_log` on S3.

## 3. Sources

| Key | Source |
|---|---|
| TP | [`Cloudflare, DOs and D1s Architecture Thought process.md`](../../65-agent-rpc-serving-system/resources/Cloudflare,%20DOs%20and%20D1s%20Architecture%20Thought%20process.md) |
| GED | [`Ged Scalable Dos and D1s Discussions.pdf`](../../65-agent-rpc-serving-system/resources/Ged%20Scalable%20Dos%20and%20D1s%20Discussions.pdf), 54 pages, cited as "GED p.N" |
| NATS | [`Nats and Cloudflare.pdf`](../../65-agent-rpc-serving-system/resources/Nats%20and%20Cloudflare.pdf), 148 pages, cited as "NATS p.N" |
| MM | [`Migration Management.md`](../../65-agent-rpc-serving-system/resources/Migration%20Management.md), for per-shard D1 migrations only |
| lunora | `github.com/anolilab/lunora`, branch `alpha`: docs `concepts/read-replicas.mdx`, `sharding.mdx`, `commit-ordering.mdx`, `caching.mdx`; code `packages/shard-engine/src/ctx-db-cdc.ts`, `ctx-db-cdc-archive.ts`, `packages/do/src/cdc-retention.ts` |
| fokosdb | `github.com/lambrospetrou/fokosdb`: `README.md`, `docs/agent-plans/2026-09-19-fokos-sharding-runtime.md` |
| authrim | `github.com/sgrastar/authrim`: `private/docs/implementation/tenant-d1-control-plane/README.md`, `packages/ar-lib-core/src/utils/region-sharding.ts` |
| emdash | `github.com/emdash-cms/emdash`: `packages/cloudflare/src/db/` (`d1.ts`, `d1-session-guard.ts`, `coalescing-d1.ts`, `coalescing-do-sql.ts`, `do-sql-class.ts`) |
| Cloudflare | `cloudflare/cloudflare-docs`: D1 limits, pricing and read replication; DO limits, pricing and "Rules of Durable Objects"; R2 limits, pricing and consistency |
| DS | [`durable-streams`](https://github.com/durable-streams/durable-streams): `PROTOCOL.md`, `docs/concepts.md`, `docs/fork.md`, `docs/durable-state.md`, `docs/stream-db.md`, `docs/building-a-server.md`, `docs/deployment.md` |

The project repositories and the Cloudflare documentation were read at their default branches on
2026-10-27. The PDFs are browser print-outs: wide code blocks and diagrams are cut at the right
edge in the PDF itself, so some code lines are incomplete.

## 4. Index

| File | Contents |
|---|---|
| [durable-objects-and-workers.md](./durable-objects-and-workers.md) | Patterns for DOs and Workers: parent and child DOs, the DO as a write buffer, one alarm for many jobs, self-splitting DOs, relay DOs, production traps, the 100-parameter cap, migrations in the constructor |
| [read-scaling-and-replicas.md](./read-scaling-and-replicas.md) | D1 Sessions API and bookmarks, consistency classes for each query, coalescing, many named D1 databases, the shard in the ID, caching, what lunora and fokosdb do |
| [change-logs.md](./change-logs.md) | CDC in the same transaction, commit-ordered sequences, retention and archive, epochs, the CQRS flush, the linear relay, core and delta tables, JetStream's layout, per-shard migrations |
| [tiered-store.md](./tiered-store.md) | **Deep.** DO SQLite → D1 → R2: the tiers, demotion and promotion, the watermark rule, reads across tiers, crash windows for DST, costs, the mapping onto `foundation_db`, and `foundation_log` on R2 and S3 |
| [durable-streams.md](./durable-streams.md) | **Deep.** Durable Streams: the resumable, offset-addressable log over HTTP. Offset-based resumption, idempotent producers and epoch fencing, live tail, fork without copy, the immutable prefix, and the mapping onto valtron, `foundation_log` and spec 65 |
| [discarded.md](./discarded.md) | Ideas reviewed and thrown away, one line each |

## 5. Top insights

1. **Write below, record the watermark, then destroy above.** Every safe move between tiers uses this order. The watermark lives in the DO, not in a listing of the bucket ([tiered-store.md 3.4](./tiered-store.md#34-the-watermark-rule)).
2. **A flush to D1 is not read-your-writes without the D1 bookmark.** D1 replicas are asynchronous. The flush must return its session bookmark, and the read must start from it ([tiered-store.md 3.1](./tiered-store.md#31-demotion-t0--t1-the-flush)).
3. **The change log is written in the same DO transaction as the row, with a commit sequence, not a clock.** One feed then serves the D1 flush, the archive and every consumer ([change-logs.md 2.1–2.2](./change-logs.md#21-the-change-log-is-written-in-the-same-transaction-as-the-row)).
4. **Key segments by their last sequence, zero-padded, under an epoch prefix.** One `list(startAfter)` finds every segment a reader needs, with no manifest. A restored timeline writes under a new prefix. `If-None-Match: *` fences a second writer. This is the core of `foundation_log` on R2 and S3 ([tiered-store.md 6.2](./tiered-store.md#62-key-layout-lunoras-archive-keys)).
5. **Never append to an object.** No rewrite, no multipart-as-append. Acknowledge at the DO tail, seal on size, count or age (all config), upload once ([tiered-store.md 6.3–6.4](./tiered-store.md#63-never-append-to-an-object)).
6. **Give each D1 query a consistency class.** Replica-eligible, primary-required, read-after-write, and "not found on a replica means check the primary once" ([read-scaling-and-replicas.md 2.2](./read-scaling-and-replicas.md#22-consistency-classes-for-each-query)).
7. **Put the route in the ID, with a generation.** Any Worker routes with no lookup, and a new layout does not break old IDs ([read-scaling-and-replicas.md 3.3](./read-scaling-and-replicas.md#33-the-shard-in-the-id)).
8. **D1 is the most expensive tier per byte; row writes are the expensive operation.** R2 storage is 50 times cheaper than D1. Each tier hop costs row writes that batching does not remove ([tiered-store.md 7](./tiered-store.md#7-costs)).
9. **Prune by terminal state, not by age.** Core tables stay in D1; only finished deltas move to R2 ([change-logs.md 2.8](./change-logs.md#28-core-tables-and-delta-tables)).
10. **Coalesce the reads of one request into one `batch()`.** One round trip instead of 5 to 17 ([read-scaling-and-replicas.md 2.4](./read-scaling-and-replicas.md#24-coalescing-reads-into-one-batch)).
11. **Make the cursor the state, not the connection.** A reader stores `Stream-Next-Offset` and resumes from it, so a drop mid-stream reconnects with no skip and no duplicate ([durable-streams.md 3.1](./durable-streams.md#31-offset-based-resumption-is-the-whole-client-contract)).
12. **Fence a writer with an epoch, deduplicate with a sequence.** `(producer-id, epoch, seq)` gives exactly-once writes: a stale epoch is fenced, a retried sequence is acknowledged without a duplicate ([durable-streams.md 3.2](./durable-streams.md#32-exactly-once-writes-from-an-idempotent-producer)).
13. **Branch history without copying it.** A fork shares the source prefix and owns only its tail; reads span the boundary transparently ([durable-streams.md 3.4](./durable-streams.md#34-fork-branch-a-stream-without-copying-the-prefix)).

## 6. Where the insights could apply

| Crate or spec | Insights | Notes |
|---|---|---|
| `foundation_db` | 1, 2, 3, 6, 9, 10 | `DoSqlStore` (feature 07 task T20) is the hot tier. `D1WasmStorage` needs a session constraint. `QueryStore` needs a parameterised batch, and `BlobStore` needs streaming, range reads, conditional put and an ordered listing. The gaps are on the bug backlog, section 11. |
| `foundation_log` (issue #155) | 1, 4, 5, 11, 12 | Key layout, fencing, acknowledgement levels, group commit as config, the recovery steps for objects. Durable Streams adds the *reader* contract: an opaque, lexicographically sortable cursor, and a trimmed segment is a refusal, never a gap. |
| `foundation_core` (valtron) | 11, 12, 13 | A `DurableStream` reader/writer and `DurableStreamStore` trait live in valtron, so every backend spreads the capability. This is spec 71. |
| `foundation_http`, `foundation_netio`, `foundation_connectrpc` | 11, 13 | The six HTTP verbs, the cursor headers, the immutable-prefix caching and the live tail map onto the existing HTTP and RPC surfaces. |
| Spec 65 feature 07 (`registry-and-models.md`, `hosts.md`) | 1, 3, 7 | The R2 history archive already follows insight 1. The D1 instance index is the "registry of DOs" pattern. `do_name` already puts the route in the name. |
| Spec 65 streaming operations (`01-agent-rpc`, `05-cloudflare`) | 11, 12, 13 | The agent-rpc turn and progress streams become resumable and replayable; a forked conversation is a fork of the turn log; the Cloudflare SSE tail gains the cursor contract. |
| `foundation_deployment_cloudflare` | 6, 7 | Create named D1 databases with `primary_location_hint` or `jurisdiction`, and turn on `read_replication`. The update body for `read_replication` is untyped (bug backlog, section 8). |
| Migrations for each shard | — | One timeline for each database; a shared pipeline and a shard pipeline with separate tracking ([change-logs.md 3](./change-logs.md#3-per-shard-d1-migrations)) |
| Each spec's DST layer | 1, 2, 4 | The crash windows in [tiered-store.md 5.1](./tiered-store.md#51-crash-windows-inputs-for-the-dst-layer) and the traps in [durable-objects-and-workers.md 2.6](./durable-objects-and-workers.md#26-traps-found-in-production-use) |
