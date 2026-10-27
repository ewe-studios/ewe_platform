# Change logs, outboxes and the DO-to-D1 flush

_Part of the [edge data research notes](./README.md). Written in ASD-STE100 Simplified Technical
English (STE). Code and identifiers stay as they are. This is research, not a design._

**Abbreviations in this file.** Durable Object (DO); change data capture (CDC); Structured Query
Language (SQL); read-your-writes (RYW); point-in-time recovery (PITR); identifier (ID); command
query responsibility segregation (CQRS); cyclic redundancy check (CRC); write-ahead log (WAL);
deterministic simulation testing (DST); AeroSpace and Defence Industries Association of Europe
(ASD), the owner of the STE standard ASD-STE100. D1 and R2 are names of Cloudflare products, not
abbreviations. NATS and JetStream are product names.

Source keys are in [README.md section 3](./README.md#3-sources).

## 1. Why change logs matter here

Three owner positions need a change log under them:

- The DO flushes to D1 on request. The flush must know what it has not sent yet.
- The tiered store moves data down. Each move must know where it stopped ([tiered-store.md](./tiered-store.md#34-the-watermark-rule)).
- `foundation_log` *is* a change log on object storage ([issue #155](https://github.com/ewe-studios/ewe_platform/issues/155)).

## 2. Insights

### 2.1 The change log is written in the same transaction as the row

- **Source:** lunora `packages/shard-engine/src/ctx-db-cdc.ts` (`appendCdcChange`, the `__cdc_log` table).
- **What it is:** Each committed mutation appends one row to `__cdc_log (seq INTEGER PRIMARY KEY AUTOINCREMENT, ts, table, id, op, doc)` inside the same DO storage transaction as the data write. `doc` is the post-image for insert and update, and `NULL` for delete.
- **Why it is interesting:** "The change is durable if and only if the write is." There is no dual write and no outbox relay to lose an event. One feed then serves every consumer: streaming export, subscription resume, replicas, PITR replay, and the R2 archive.
- **Limits:** Every write pays one more row write ($1.00 for each million rows written). The log grows until something trims it (section 2.4). Only one DO's changes are in it; there is no global order.
- **Where it could apply:** The source of the DO-to-D1 flush. A use case can use a full CDC log, or a smaller outbox table that holds only what D1 needs. Both are written in the same transaction as the hot row.

### 2.2 Commit-ordered sequence, not wall clock

- **Source:** lunora docs `concepts/commit-ordering.mdx`.
- **What it is:** A per-DO integer, bumped inside the transaction that writes the rows. A DO runs one event at a time, so allocation order is commit order. All rows of one mutation share the value.
- **Why it is interesting:** A consumer that pages on `_creationTime` (a wall-clock time from when the handler started) can skip a row forever: a row stamped `t=100` can commit after the consumer passed 100. A commit sequence closes that hole.
- **Limits:** The sequence orders commits, not rows. A bounded page can end in the middle of one commit. Two safe ways to checkpoint:
  - advance only to the last sequence whose rows you saw *completely* (drop the trailing partial group of a full page), or
  - page on the composite cursor `(seq, id)`.
- **Where it could apply:** The flush cursor (`flushed_through`) and the `foundation_log` record sequence. A flush that takes `LIMIT n` rows must not cut a commit in two, or the watermark must be `(seq, id)`.

### 2.3 Indexes that keep one busy table from taxing the rest

- **Source:** lunora `ctx-db-cdc.ts` (`migrateCdcLog`).
- **What it is:** An index on `("table", seq)` so a consumer of one table reads only its own changes. The index build runs in its own `try`, apart from the table creation.
- **Why it is interesting:** The isolation is the lesson. Building an index over a large existing log on the request path can exceed the time budget of the isolate. If that fails the whole migration, every request restarts it and "the shard is bricked rather than slow". Isolated, the read path degrades to a scan, retention shrinks the log, and a later cold start builds the index.
- **Limits:** A degraded read path is slower and costs more rows read until the index exists.
- **Where it could apply:** Migrations that run in a DO constructor (feature 07 DOs, `DoSqlStore`). Separate "must succeed" schema steps from "may be retried later" index steps.

### 2.4 Retention: trim by rows, archive before you trim

- **Source:** lunora `packages/do/src/cdc-retention.ts` and `ctx-db-cdc-archive.ts`.
- **What it is:**
  - The log keeps a row window (`LUNORA_CDC_LOG_RETENTION`, default 1,000,000 rows, about 11 days at one write each second).
  - The sweep runs at most once for each interval, and it rides on a write dispatch, not a timer, so it stays off the per-mutation path.
  - Each sweep deletes a bounded number of rows.
  - It never deletes below the floor of the in-shard consumers.
  - When an R2 bucket is bound, rows go to R2 as segments first (see [tiered-store.md section 6.2](./tiered-store.md#62-key-layout-lunoras-archive-keys)).
  - A consumer below the floor gets `CDC_LOG_TRIMMED`, never a silent gap.
- **Why it is interesting:** It names the failure mode precisely: retention without an archive is a cliff. A connector that was offline over a weekend pays a full re-seed for a few megabytes of deleted log.
- **Limits:** The defaults are lunora's. Ours must be config fields with documented defaults (CLAUDE.md rule 8).
- **Where it could apply:** The retention policy of the T0 change log and of `foundation_log`.

### 2.5 The epoch: detecting a forked timeline

- **Source:** lunora `ctx-db-cdc.ts` (`__cdc_meta`) and `ctx-db-cdc-archive.ts` (`CdcArchiveScope.epoch`); lunora docs `concepts/read-replicas.mdx`.
- **What it is:** A single-row epoch value. Each consumer and each archive key carries it. A changed epoch means "the timeline forked; replay would invent a state that never existed".
- **Why it is interesting:** lunora found a hole in its own design and wrote it down. A PITR restore of a DO also restores the epoch row, so the epoch alone cannot see the fork. The fix needs a witness the restore cannot reach: a consumer's cursor that is above the owner's high-water mark, or archive segments above the restored watermark. The sweep then mints a new epoch before it writes again.
- **Limits:** It is a race, not a guarantee, until a witness notices.
- **Where it could apply:** Any consumer of a DO's log (a D1 flush, an R2 archive, `foundation_log` readers). DO PITR exists for SQLite-backed DOs, so a restore is a real event for us.

### 2.6 CQRS flush-before-read

- **Source:** TP Part 3 ("Model B: The CQRS On-Demand Flush"), TP Part 4 (`EdgeDataEngine::force_immediate_flush`).
- **What it is:** Writes go to the DO for one user. Before a range scan or an aggregate, the repository asks that DO to flush to D1, waits, then runs plain SQL on D1.
- **Why it is interesting:** It matches the owner position "a per-use-case interface asks the DO to flush to D1". D1 then does what it is good at (`SUM`, `ORDER BY ... LIMIT`), and no merge happens in Worker memory.
- **Limits:**
  - The TP code calls the DO with `fetch` and an HTTP path. Cloudflare now advises RPC methods instead (GED p.1; Rules of Durable Objects).
  - The TP code does not carry a D1 bookmark from the flush to the read. Without it a replica can miss the flushed rows ([tiered-store.md section 3.1](./tiered-store.md#31-demotion-t0--t1-the-flush)).
  - One flush for each read of a cold user adds a DO hop and a D1 write before the first byte.
- **Where it could apply:** The flush interface of each use case.

### 2.7 The linear relay: one background flush, not two writes for each request

- **Source:** NATS p.100–117.
- **What it is:** A request writes only to DO SQLite and returns. One background loop reads a bounded page with a cursor, writes it to D1 (`batch()`), then to R2, then deletes it from the DO. Each step waits for the one before it.
- **Why it is interesting:** The PDF shows why the first idea fails: one D1 promise and one R2 promise for each request means 20,000 open promises for 10,000 requests, all in Worker memory (NATS p.117 table). The relay keeps memory flat: the heap holds only one page.
- **Limits:** The PDF's sketch keeps one fixed `LIMIT 5000` and builds a Parquet buffer of the whole page in memory. Page size and part size must be config. Bodies must stream (CLAUDE.md rules 4 and 8).
- **Where it could apply:** The flush and archive loops of the tiered store. The loop runs from a DO alarm.

### 2.8 Core tables and delta tables

- **Source:** NATS p.119–122.
- **What it is:** Core tables hold current truth (users, balances, subscriptions). They stay in D1 for ever. Delta tables hold how the truth changed (invoices, audit trail, order history). Only deltas move to R2, and only after they reach a terminal state (`paid`, `cancelled`, `settled`), and only if no live row still points to them.
- **Why it is interesting:** It replaces the unsafe "delete rows older than 30 days" with a rule an OLTP system can live with.
- **Limits:** "Show my statements from 2024" must then read two tiers and merge them.
- **Where it could apply:** Choosing which tables of a use case are demotable at all.

### 2.9 JetStream's file layout, as a model for a segment

- **Source:** NATS p.24–27, p.47–53, p.65–68, p.78–80; TP Part 1.
- **What it is:** A stream is a chain of fixed-capacity blocks. Writes append to the active block (`WriteAt`) and go to the page cache. A background `fsync` runs on an interval (`sync_interval`, 2 minutes by default), or on every write with `sync: always`. A replica quorum, not the disk, protects acknowledged writes. Each record has a checksum. At boot, a scan stops at the first bad checksum and truncates the tail.
- **Why it is interesting:** It is the shape `foundation_log` takes natively: append only, no B-tree, no LSM compaction, a sync policy in config, and a recovery scan.
- **Limits:** The PDF has errors (issue #155): the checksum is highwayhash, newer NATS has no per-block `.idx` file, there is one Raft group for each stream and each consumer, and "seq mod N" offset math breaks with variable-size records. None of the page-cache tricks exist in a Worker.
- **Where it could apply:** The native tier of `foundation_log`. On Cloudflare the DO SQLite tail replaces the file, and the DO's own durability replaces the quorum.

### 2.10 Migration pages committed with their cursor

- **Source:** fokosdb `docs/agent-plans/2026-09-19-fokos-sharding-runtime.md` section 4.2.12.
- **What it is:** When a partition splits, the target pulls data one page at a time. Each page and its cursor commit in one transaction on the target. `imported` is written before the acknowledgement. Only one page is in flight.
- **Why it is interesting:** It is the watermark rule again, applied to resharding: the cursor and the data can never disagree after a crash.
- **Limits:** Writes to the moving range wait during the copy (reads go through to the source).
- **Where it could apply:** Any copy between tiers or between shards, including moving one country's data to a new D1.

## 3. Per-shard D1 migrations

- **Source:** `Migration Management.md` topics 1 to 3.
- **What it is:** Keep one migration timeline for each database. When there is a shared set and a per-tenant set, keep two pipelines with two tracking tables: the shared pipeline runs once, then the tenant pipeline loops over every tenant.
- **Why it is interesting:** With many named D1 databases for each shard category, this becomes: one "shared" timeline for the global D1, and one "shard" timeline that a runner applies to every database in the category. Each database tracks its own position.
- **Limits:** D1 has no schemas, so "per schema" maps to "per database". A loop over many databases can stop half way, so each database must report its own version, and a deploy must allow mixed versions for a while (backward-compatible changes first, as topic 1 says).
- **Where it could apply:** `foundation_db` migrations (`src/core/schema/migrations.rs`) and the spec 65 feature 07 `migrations.md` work. The list of shard databases is known at build time (GED p.54: the country list is fixed), so the runner can iterate a generated list.
