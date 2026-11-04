# Discarded ideas

_Part of the [edge data research notes](./README.md). Written in ASD-STE100 Simplified Technical
English (STE). Code and identifiers stay as they are._

**Abbreviations in this file.** Durable Object (DO); key-value (KV); Structured Query Language
(SQL); WebAssembly (wasm); write-ahead log (WAL); software development kit (SDK); Functional
Source License (FSL); AeroSpace and Defence Industries Association of Europe (ASD), the owner of
the STE standard ASD-STE100. D1 and R2 are names of Cloudflare products, not abbreviations.

We reviewed these ideas and threw them away. One line each, with the reason. Source keys are in
[README.md section 3](./README.md#3-sources).

## Storage and tiers

- **KV as the warm tier** (NATS p.104–108): it loses SQL (NATS p.108–109), and KV is eventually consistent across locations.
- **One D1 write and one R2 write for each request** (NATS p.100, p.117): two open promises for each request blow Worker memory under load.
- **"Append" to an R2 object by download and upload again** (NATS p.94; TP Part 3): memory grows with the object.
- **Multipart upload as an append** (NATS p.95–97): parts are invisible until the upload completes.
- **Rewrite the active tail object on each flush** (NATS p.88): R2 allows one concurrent write each second to one key, and each rewrite is a paid write.
- **Fixed records for each segment with `seq mod N` offsets** (NATS p.88–89): records have variable size (issue #155).
- **Arrow IPC streams on R2 as the cold format** (NATS p.91–96): no append on R2 anyway, and it compresses far worse than Parquet.
- **DuckDB in wasm inside a Worker as the default read path** (NATS p.110–111, p.122–123): 200–400 ms cold start and Worker memory limits on large scans; keep it only as a possible cold-query tool.
- **Routing reads by the age of the data** (NATS p.104–107, p.145): breaks when an old key is updated.
- **Time-based purge of OLTP rows** (NATS p.118–119): it deletes live state; prune by terminal state instead.
- **A single global buffer DO** (TP Part 4; GED p.29): one thread, one place, a 128 MB memory ceiling.

## Platform mechanics that do not exist on Workers

- **Page cache, `mmap` indexes, zero-copy buffer pools** (NATS p.10–16, p.24–50): good for a native `foundation_log`, meaningless inside a Worker; issue #155 covers the native side.
- **Raft groups and our own consensus** (NATS p.54–78; TP Part 2): a DO already gives one writer and durable storage.
- **Consistent-hash rings with chained generations of nodes** (NATS p.82–85): we have no nodes; authrim's generations in the ID keep the useful part ([read-scaling-and-replicas.md section 3.3](./read-scaling-and-replicas.md#33-the-shard-in-the-id)).

## DO replicas

- **Building DO read replicas ourselves, as lunora does** (GED p.25–26, p.30–31): owner position, DO replicas "never was possible anyway"; we only keep the follower safety rules.
- **The experimental DO replication surface that emdash feature-detects** (`ctx.storage.primary`, `enableReplicas()`, `waitForBookmark()`, the `replica_routing` flag in emdash `do-sql-class.ts`): not in `@cloudflare/workers-types` or the public DO documentation, and outside the owner position.

## Projects as dependencies

- **lunora** (GED p.26–51): a whole platform that owns the Worker, the DO and the client; one author; alpha; FSL licence; a startup-CPU ceiling (#1166).
- **fokosdb** (GED p.15–19): "Do NOT use this in production, yet"; no benchmarks.
- **authrim** (GED p.24, p.53–54): an identity product of about 900,000 lines; we borrow ideas only.
- **authrim's live binding patcher** (GED p.53–54): our shard list is fixed, so bindings come from the build.
- **cloudflare/vibesdk** (GED p.6–7): binds Containers, Artifacts and Workers for Platforms; costly, and its D1 code is buried.
- **cloudflare/agentic-inbox** (GED p.6, p.14): good shape, but its DO code targets a 2025 compatibility date.
- **tiddlyflare, durable-utils, the original d1 starter** (GED p.9–10): from 2024 and early 2025; we keep their ideas, not their code.
- **Neore and Signalbox** (GED p.43–50): applications, not references; Neore's trap list is kept in [durable-objects-and-workers.md](./durable-objects-and-workers.md#26-traps-found-in-production-use).

## Off topic for edge data

- **lunora's multi-language SDK generator and OpenRPC** (GED p.31–35): about client generation, not data.
- **NATS text protocol against FlatBuffers** (NATS p.7–10): a wire format question.
- **CRC-32 polynomial details** (NATS p.69–75): background only; the checksum choice is an open question in issue #155.
- **Cloudflare Queues against Kafka and NATS throughput** (NATS p.1–3): not a data storage pattern.
