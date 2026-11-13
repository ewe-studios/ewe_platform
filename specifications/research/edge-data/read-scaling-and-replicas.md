# Read scaling and replicas

_Part of the [edge data research notes](./README.md). Written in ASD-STE100 Simplified Technical
English (STE). Code and identifiers stay as they are. This is research, not a design._

**Abbreviations in this file.** Durable Object (DO); Structured Query Language (SQL);
read-your-writes (RYW); change data capture (CDC); uniform resource locator (URL); application
programming interface (API); identifier (ID); personally identifiable information (PII);
Representational State Transfer (REST); Hypertext Transfer Protocol (HTTP); round trip time
(RTT); central processing unit (CPU); AeroSpace and Defence Industries Association of Europe
(ASD), the owner of the STE standard ASD-STE100. D1 and R2 are names of Cloudflare products, not
abbreviations.

Source keys are in [README.md section 3](./README.md#3-sources).

## 1. The platform facts these notes rest on

- A Worker runs near every user already. The latency is the hop from the Worker to the data (GED p.19).
- A DO lives in one data centre, near its first request or near a location hint. It does not move after it is created (GED p.19).
- D1 has read replicas. Without the Sessions API, every query still goes to the primary (GED p.2).
- Nothing is narrower than a region. The only hard residency guarantees are jurisdictions (EU, FedRAMP, and US for D1 since October 2026) (GED p.23).
- A D1 database runs one query at a time. Throughput is the inverse of query time, so indexes matter more than anything else (GED p.2).

Owner position: DO replicas "never was possible anyway". This file records how lunora builds them
only because the brief asks what lunora does (section 5). It is not an input to our work.

## 2. D1 read replicas and the Sessions API

### 2.1 Sessions and bookmarks

- **Source:** GED p.2, p.11; Cloudflare D1 "Global read replication" documentation; `cloudflare/templates` `d1-starter-sessions-api` (GED p.7).
- **What it is:** `db.withSession(constraint)` starts a session. The constraint is `"first-unconstrained"` (any replica), `"first-primary"` (the latest version), or a bookmark from an earlier session. `session.getBookmark()` returns the bookmark to pass on. All queries in a session are sequentially consistent. Each result has `meta.served_by_region` and `meta.served_by_primary`.
- **Why it is interesting:** Replicas are free; you pay only rows read. Turning replication on is one REST call on the database (`read_replication.mode = auto`), with no deploy. A database with replication off still works through the same session code, so small and large deployments share one code path (authrim, section 4.2).
- **Limits:** Writes always go to the primary. Replica lag has no stated bound. Turning replication off can take up to 24 hours to stop replica traffic (authrim plan). Read replicas of a database with a jurisdiction stay in that jurisdiction.
- **Where it could apply:** `D1WasmStorage` has no session today. A session constraint is the missing parameter on the T1 read path ([tiered-store.md section 8](./tiered-store.md#8-how-it-maps-onto-foundation_db)). Our generated D1 types already have `read_replication` on create and details; the update body is untyped (bug backlog, section 8).

### 2.2 Consistency classes for each query

- **Source:** authrim `private/docs/implementation/tenant-d1-control-plane/README.md`, section "D1 Read Replication and Sessions API".
- **What it is:** Each query names one of four classes, and the class picks the session start:

| Class | Session start | Example |
|---|---|---|
| `replica_eligible` | `first-unconstrained` | Display settings, public client metadata, policy that rarely changes |
| `primary_required` | `first-primary` | Credential checks, revocation, account status, authorization policy |
| `read_after_write` | the earlier bookmark, else `first-primary` | Reading back an account just created |
| `replica_not_found_recheck` | `first-unconstrained`, then `first-primary` once | A route or record that may not have replicated yet |

- **Why it is interesting:** It turns "use replicas" into a per-query decision with a name. Two details are sharp:
  - A "not found" from a replica is never final. Check the primary once. Never cache the negative answer across requests.
  - Security reads never accept replica lag. They start at the primary.
- **Limits:** Every query site must choose a class. A wrong class is a silent stale read.
- **Where it could apply:** A `ReadConsistency` value on the T1 read path. The trust store and revocation reads of spec 65 feature 07 (`SqlTrustStore<D1WasmStorage>`) are `primary_required`.

### 2.3 Carrying the bookmark, and guarding the session

- **Source:** emdash `packages/cloudflare/src/db/d1.ts`, `d1-session-guard.ts`; GED p.14, p.21.
- **What it is:**
  - emdash keeps the bookmark in a cookie (`__em_d1_bookmark`) for signed-in users only. Anonymous reads start `first-unconstrained` and get no cookie.
  - It treats the cookie as untrusted input: it caps the length and rejects control characters, but it does not parse the opaque value.
  - A guard races session queries against a 5-second timeout until the first query in the isolate settles. With the `global_fetch_strictly_public` compatibility flag, a session query can hang with no error. On a hang the guard marks the isolate broken, re-runs plain `SELECT`s on the direct binding, and fails writes with a clear error.
- **Why it is interesting:** These are the failure modes that only a production user finds. The rule "re-run reads, never re-run writes" is the right one for any timeout.
- **Limits:** The cookie couples the database's bookmark to the browser. An API client needs a header (`x-d1-bookmark` in the Cloudflare example).
- **Where it could apply:** The Worker host of feature 07, if it reads D1 through sessions.

### 2.4 Coalescing reads into one batch

- **Source:** emdash `packages/cloudflare/src/db/coalescing-d1.ts`, `coalescing-do-sql.ts`; GED p.11.
- **What it is:** `SELECT`s issued in the same event-loop turn of one request are sent as one D1 `batch()`, so one round trip replaces N. emdash reports pages with 5 to 7 serial queries at 15 to 40 ms each. The same trick over a DO collapses about 17 reads to one RPC.
- **Why it is interesting:** It is the "batch your queries" rule done for you. It is safe only on a per-request connection: a shared connection would batch one request's queries into another's round trip (and into its bookmark).
- **Limits:** Only plain `SELECT`. `WITH` is excluded because SQLite allows a CTE in front of a write.
- **Where it could apply:** A batching `QueryStore` wrapper for D1 in `foundation_db`.

## 3. Many named D1 databases

Owner position: several D1 databases for each shard category (for example country or tenant).
Each entity picks its database. Naming the databases gives the placement benefit; use whatever
placement Cloudflare offers.

### 3.1 What the platform allows

- **Source:** Cloudflare D1 limits; GED p.20–23, p.53–54.
- **What it is:** Up to 50,000 databases for each account by default, more on request. About 5,000 D1 bindings for each Worker script. 10 GB for each database. A database gets a location hint (`apac`, `weur`, `enam`, ...) or a jurisdiction **only when it is created**.
- **Why it is interesting:** It makes "one database for each country, or each tenant" practical. It also splits the single-query-at-a-time limit across databases.
- **Limits:** A location hint is a preference, not a guarantee. A jurisdiction cannot be added or changed later; changing it means a new database and a data move. One Worker deployment has one placement setting; per-region compute means several Workers (GED p.23).
- **Where it could apply:** `foundation_deployment_cloudflare` creates databases with the generated `D1CreateDatabaseRequest` (`primary_location_hint`, `jurisdiction`).

### 3.2 Authrim's provisioning and routing

- **Source:** GED p.22–24, p.52–54; authrim tenant D1 control plane plan.
- **What it is:**
  - A control plane creates each D1 through the API with a location hint or a jurisdiction, and turns on read replication.
  - It records each database in a registry, then patches the bindings onto the running Workers, deploys, runs a signed smoke test, and rolls back on failure.
  - Creation is a state machine. It sorts Cloudflare errors into permanent and retryable, retries with back-off for up to two hours, and checkpoints so a crash never creates two databases.
  - Each database has a `data_role` (`lookup`, `tenant_core`, `tenant_pii`, `control`), a `residency_policy_id`, a `jurisdiction`, a `location_hint` and a `policy_generation`. Core data and PII live in different databases, so residency rules can treat them differently.
  - Routing fails closed. An unknown or wrong route is an error, never a fall back to a shared database.
- **Why it is interesting:** It is the only public code that creates per-region databases end to end (GED p.22). The data-role split and fail-closed routing are useful even with a fixed country list.
- **Limits:** About 900,000 lines of TypeScript; the binding patcher alone is 944 lines (GED p.54). We can skip the patcher: our country list is fixed, so bindings can be generated at build time (GED p.54).
- **Where it could apply:** A database resolver in `foundation_db` that maps a shard category value to a binding name, generated from the list, and refuses unknown values.

### 3.3 The shard in the ID

- **Source:** authrim `packages/ar-lib-core/src/utils/region-sharding.ts`; GED p.53–54.
- **What it is:** IDs carry their placement: `g1:apac:3:ses_<uuid>` is generation 1, region `apac`, shard 3. Any Worker reads the ID and goes straight to the right DO or database, with no lookup. Shard counts and the region mix live in configuration. A change starts a new generation; old IDs keep their old route (authrim keeps up to five generations).
- **Why it is interesting:** It is online resharding without moving data: new IDs go to the new layout, old IDs stay valid. For countries: `th:venue-123` routes to the Thai database with no lookup (GED p.54).
- **Limits:** An ID that carries its route leaks that route. Moving one existing entity still needs a data move.
- **Where it could apply:** The naming scheme for entities that live in a per-category D1 or DO. Feature 07's `do_name(scope, instance)` already derives the DO name from data in the request.

## 4. Caching

- **Source:** GED p.20–21 (page caching with purge on write, cf-do-locator issue #5); lunora docs `concepts/caching.mdx`; fokosdb README (topology in Workers Cache).
- **What it is:** Cache whole responses in the Workers Cache with `Cache-Tag`; purge by tag on write. fokosdb caches its partition topology in the Workers Cache with stale-while-revalidate.
- **Why it is interesting:** For shared data that many people read and few write, a tag purge is cheaper than any replica.
- **Limits:** In lunora only the Worker sees the cache binding; code that runs inside a DO cannot purge. Cached routing data must be a hint only (fokosdb: "Caches are hints. A miss, a full cache, a stale entry, or a disabled cache changes latency only.").
- **Where it could apply:** Read-mostly global data in the shared D1.

## 5. What lunora and fokosdb do

### 5.1 lunora: tables with three homes

- **Source:** GED p.29–31, p.43–44; lunora docs `concepts/sharding.mdx`, `concepts/read-replicas.mdx`.
- **What it is:**
  - A table is root (one DO), `.shardBy(field)` (one DO for each value), or `.global()` (D1 with replicas and an `x-d1-bookmark` header).
  - A `ShardRegistryDO` records which shards exist, because a DO namespace cannot be listed. A cross-shard read is a fan-out through a coordinator.
  - `shardRegion(shardKey)` returns a region hint, used only when the DO is created. A jurisdiction turns every region hint off.
- **Why it is interesting:** The split "per-entity DO, global data in D1 with replicas" is the same split as the owner positions. The registry DO solves the same problem as feature 07's instance index in D1.
- **Limits:** Neore, the author's own app, says `ctx.db` does not route by `.shardBy()`; it had to build per-user sharding by hand (GED p.43–44). Changing a table's sharding is offline: stop writes, export, import (GED p.30).
- **Where it could apply:** Confirms the index-in-D1 pattern of feature 07 (`agent_rpc_instances`).

### 5.2 lunora: DO read replicas (recorded, not adopted)

- **Source:** GED p.25–26, p.30–31; lunora docs `concepts/read-replicas.mdx`.
- **What it is:** A replica is a second DO named `<shard>::replica::<region>`. It replays the owner's CDC log (1,000 changes for each pull) into its own SQLite. Only one-shot queries go to it. A read without a cursor may be up to `LUNORA_REPLICA_MAX_STALENESS_MS` (1,000 ms) behind. A read with a cursor waits for the replica to reach it, or goes to the owner.
- **Why it is interesting:** The safety rules transfer to any follower, including a D1 table fed by a DO flush: a follower that cannot prove it is current sends the read to the owner; a forked or trimmed log makes the follower refuse, never guess.
- **Limits:** By lunora's own docs, below about one read each second for each region a replica is slower than the owner. A diverged replica stays diverged for the life of the deployment. Owner position: DO replicas are not for us.
- **Where it could apply:** Only the safety rules, through the D1 flush in [tiered-store.md](./tiered-store.md).

### 5.3 fokosdb: sharding by size, not by place

- **Source:** GED p.14–19; fokosdb `docs/agent-plans/2026-09-19-fokos-sharding-runtime.md`.
- **What it is:** A DO splits itself when it gets too big or too busy. Many keys in one DO: a hash split into children, as a tree. One huge key: the key is "promoted" to its own DO, then split by sort-key range. Data moves in pages; reads go through to the source during the move; writes retry. Every answer tells the caller where the data lives now, so the next call goes direct. A Bloom filter remembers promoted keys.
- **Why it is interesting:** It is the answer to "one entity outgrew one DO", which per-entity design does not solve. The route caches change latency only, never correctness.
- **Limits:** It has nothing to do with geography: one location hint for the whole table, no read replicas (GED p.19). The shape of the tree is fixed once data exists. Not ready for production; no benchmarks (GED p.16).
- **Where it could apply:** Not now. It is the reference if one country's hot DO, or one D1, outgrows its limits (GED p.21).
