# How the projects use Durable Objects and Workers

_Part of the [edge data research notes](./README.md). Written in ASD-STE100 Simplified Technical
English (STE). Code and identifiers stay as they are. This is research, not a design._

**Abbreviations in this file.** Durable Object (DO); remote procedure call (RPC); Structured
Query Language (SQL); central processing unit (CPU); identifier (ID); input/output (I/O);
mebibyte (MiB); megabyte (MB); deterministic simulation testing (DST); AeroSpace and Defence
Industries Association of Europe (ASD), the owner of the STE standard ASD-STE100. D1 and R2 are
names of Cloudflare products, not abbreviations.

Source keys are in [README.md section 3](./README.md#3-sources).

## 1. Ground rules the projects agree on

These come from Cloudflare's "Rules of Durable Objects" (GED p.1–2), and every project graded in
GED p.14 follows them. They are short here because they are not new; the insights in section 2
build on them.

| Rule | Why | Source |
|---|---|---|
| One DO for each "thing" (user, room, mailbox, instance), never one global DO | One DO is one thread and one place. About 500 to 1,000 simple requests each second. | GED p.1, p.11; Rules of DOs |
| SQLite storage and RPC methods, not key-value storage and `fetch()` | Best performance and typed calls | GED p.1 |
| Put the logic inside the DO, not in the Worker that calls it query by query | One hop to the DO, then local SQL in microseconds, against one network round trip for each D1 query | GED p.2–3 |
| Know the input and output gates | The runtime prevents most races. `blockConcurrencyWhile()` is for migrations only. | GED p.1, p.14 |
| Hibernatable WebSockets with `serializeAttachment()` | An idle socket costs almost nothing | GED p.1, p.11 |
| `deleteAll()` also deletes the alarm (compatibility date 2026-02-24 or later) | Surprise in clean-up code | GED p.2 |

Feature 07 of spec 65 already follows the first three: one DO for each agent instance, SQLite
storage through `DoSqlStore`, the session runs inside the DO.

## 2. Insights

### 2.1 A parent DO to coordinate, a child DO for each item

- **Source:** GED p.9 (`lambrospetrou/tiddlyflare`); Rules of DOs ("Use parent-child relationships").
- **What it is:** Each tenant has one DO that only creates and deletes that tenant's wikis. Each wiki has its own DO. Traffic to a wiki never queues behind the tenant DO.
- **Why it is interesting:** It keeps a control path (rare, needs ordering) apart from the data path (frequent, needs parallelism).
- **Limits:** A listing across children needs an index somewhere, because a DO namespace cannot be listed.
- **Where it could apply:** Feature 07 uses D1 for that index (`agent_rpc_instances`). lunora uses a `ShardRegistryDO` for the same job ([read-scaling-and-replicas.md section 5.1](./read-scaling-and-replicas.md#51-lunora-tables-with-three-homes)).

### 2.2 The DO as a single-writer write buffer in front of D1 and R2

- **Source:** TP Part 3; NATS p.85–87, p.97–104.
- **What it is:** A Worker sends each write to the entity's DO. The DO commits it to SQLite and returns in 1 to 2 ms. A background loop moves the rows on.
- **Why it is interesting:** It is the owner's hot path. The single thread gives commit order for free, and the DO's durable storage lets it acknowledge before any network write.
- **Limits:** A single global buffer DO fails: one thread, 128 MB of memory, and all traffic goes to one place (TP Part 4). It must be one DO for each entity or each partition.
- **Where it could apply:** T0 of the [tiered store](./tiered-store.md).

### 2.3 One alarm drives many jobs

- **Source:** GED p.18; fokosdb `docs/agent-plans/2026-09-19-fokos-sharding-runtime.md` section 4.2.13.
- **What it is:** A DO has one alarm. fokosdb runs splits, data copies, clean-up and the host's own jobs (for example expiry) from it, with a durable table of due jobs and a fallback alarm time.
- **Why it is interesting:** A flush to D1, an archive to R2, retention and an idle timeout all want the alarm. They need one scheduler, not four `setAlarm` calls that overwrite each other.
- **Limits:** Alarm handlers must be idempotent; the platform retries them (Rules of DOs).
- **Where it could apply:** Feature 07 uses the alarm for `instance_idle_ttl`. A flush loop in the same DO would need to share it.

### 2.4 A DO that splits itself (fokosdb)

- **Source:** GED p.14–19; fokosdb sharding runtime plan, sections 4.2.10 to 4.2.12.
- **What it is:** A runtime that a DO class creates in its constructor. The class keeps its own tables and methods; the runtime owns routing, splits and migration. A split is a durable state machine (`queued → planned → cutover → completed → cleaned`). It survives a crash in the middle: the demo kills DOs during splits.
- **Why it is interesting:**
  - The routing caches are hints. "Ownership is decided by the route override table and the durable topology, never by a cache."
  - Every response carries route evidence, so callers learn the new layout without a directory service.
  - The sharding part imports nothing from the database part. A build guard enforces it, so other DO classes (a transaction coordinator, a search index) reuse it.
- **Limits:** Not production-ready. The tree shape is fixed once data exists. It solves size, not distance.
- **Where it could apply:** Reference only, for the day one entity outgrows one DO.

### 2.5 Sockets terminate near the client through relay DOs

- **Source:** lunora docs `concepts/read-replicas.mdx` ("Sockets near the client"); GED p.30.
- **What it is:** Live subscriptions belong to the owner DO. Once a shard is busy, connections spread across relay DOs. Each relay is created in the region of the first client that reaches it. Only the owner-to-relay hop crosses the world, once for each relay.
- **Why it is interesting:** It moves fan-out cost off the owner without any replica of the data. The spread is random on purpose: one relay for a whole region would recreate the fan-out wall.
- **Limits:** One socket for each shard; lunora does not merge them.
- **Where it could apply:** Streaming turn events to many watchers of one agent instance, if that ever becomes a need.

### 2.6 Traps found in production use

- **Source:** GED p.40–44 (lunora issues #1166 and #1310, Neore `AGENTS.md`); GED p.48 (dotflowy).
- **What it is:**

| Trap | What happened | Lesson |
|---|---|---|
| Startup CPU | A Worker has 1 second of CPU to start. A generated registry that imports every function at startup failed to deploy at about 1,000 functions and an 11 MB bundle. | Load lazily; measure startup in CI. |
| Lazy table creation | The first global-table access in each new isolate ran 368 statements: about 3 seconds, a full minute from a deployed Worker until batched. | Run schema work at deploy time, or batch it. |
| One outbound call without a deadline | A single slow `fetch` in the root DO blocked every request, because a DO handles one request at a time. | Every outbound call from a DO needs a deadline. |
| Concurrent first loads on one DO | 5 queries took 4.9 s, 10 took 8.8 s, then the backend died. | Budget the queries of a first paint, and test the budget. |
| Cron never ran | The Worker entry did not forward `scheduled`. | Test every entry point once deployed. |
| Lookup by ID alone | An ID that does not name its table probed every table: 70 to 125 ms for each call on D1. | Put the table, or the route, in the ID (see [read-scaling-and-replicas.md section 3.3](./read-scaling-and-replicas.md#33-the-shard-in-the-id)). |
| Row vanishes after a confirmed write | dotflowy saw a row missing for one moment after a confirmed write. | A read path across tiers must prove it saw the write ([tiered-store.md section 4](./tiered-store.md#4-reading-across-the-tiers)). |

- **Why it is interesting:** Each one type-checked clean and failed on the first real request (GED p.44).
- **Limits:** These are one project's experiences, with one author.
- **Where it could apply:** Checklist items for the Worker host of feature 07 and for any DO we write. Several are DST faults: a slow outbound call, a burst of first requests, an isolate restart.

### 2.7 Workerd's 100 bound parameters

- **Source:** lunora `ctx-db-cdc.ts` (`CDC_TABLE_FILTER_CHUNK`); Cloudflare D1 and DO SQL limits.
- **What it is:** A statement can bind at most 100 parameters, in D1 and in DO SQLite. lunora splits an unbounded `IN (...)` list into chunks below the cap, and keeps the chunking at the one call site that needs it.
- **Why it is interesting:** A multi-row flush hits this at once: an `INSERT` of 20 rows with 5 columns is the limit. Rows for each statement = `floor(100 / columns)`; more rows go into more statements of one `batch()`.
- **Limits:** Each statement in a batch also has the 100 KB statement-length limit.
- **Where it could apply:** The D1 flush, and any `foundation_db` helper that builds multi-row SQL.

### 2.8 Migrations inside the DO constructor

- **Source:** GED p.6, p.9, p.14 (agentic-inbox, durable-utils, fokosdb); lunora `ctx-db-cdc.ts`.
- **What it is:** Each DO migrates its own schema when it starts, so every mailbox or partition upgrades itself. `blockConcurrencyWhile()` wraps only this step. lunora runs expensive index builds in their own `try`, so a failed build degrades a read path and does not stop the DO from starting.
- **Why it is interesting:** With millions of DOs there is no "run the migration" step; each object must do it on wake. One expensive step must not brick an object.
- **Limits:** The first request after a deploy pays the migration. A long migration can exceed the CPU budget of that request.
- **Where it could apply:** `DoSqlStore` and the DOs of feature 07; see also [change-logs.md section 2.3](./change-logs.md#23-indexes-that-keep-one-busy-table-from-taxing-the-rest).

### 2.9 Retry only what Cloudflare marks retryable

- **Source:** GED p.9, p.14 (durable-utils, fokosdb).
- **What it is:** Retry a DO call only when the error is marked retryable and not overloaded, with back-off.
- **Why it is interesting:** Retrying an overloaded DO makes the overload worse. Most code gets this wrong (GED p.9).
- **Limits:** A retried call must be idempotent.
- **Where it could apply:** `DoStubTransport` in feature 07.
