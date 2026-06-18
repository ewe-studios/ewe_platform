---
feature: "MemoryStore — fast latest-memory retrieval per session"
description: "A MemoryStore cache (over KeyValueStore + optional fjall) that stores the latest memory SessionRecord per tier per SessionId (one key/session, O(1) hydrate) — no Snapshot structs; a MemoryCoordinator facade (held by AgentSession) owns MemoryStore + DocumentStore and does dual-write + cache->audit fallback"
status: "in-progress"
priority: "high"
depends_on: ["01-message-model", "06-documentstore-trait-sql-memory"]
estimated_effort: "medium"
created: 2026-06-14
last_updated: 2026-06-18
author: "Main Agent"
tasks:
  completed: 6
  uncompleted: 3
  total: 9
  completion_percentage: 67%
---

# Feature 07: MemoryStore

> **Implementation status (2026-06-18).** **Landed in `foundation_ai::agentic`:** `MemoryStore` trait +
> `MemoryTier` + `SessionMemory` (`memory_store.rs`); `KvMemoryStore<K: KeyValueStore>` (single key
> `memory:{session}` → `SessionMemory`, so `hydrate` is one get); the `MemoryCoordinator`
> (`memory_coordinator.rs`) owning MemoryStore + `DocumentStore` with **audit-first dual-write** and the
> **cache-miss fallback** (scan the audit log, rebuild the latest of each missing tier, re-populate); and
> `impl PromotableDocument for SessionRecord` (the F06 deferral — now that `foundation_ai` depends on
> `foundation_db`, the `record_type`/`summary` promoted columns are populated). Tests: 4 unit
> (`agentic::memory_store`) + 4 coordinator integration (`tests/memory_coordinator_tests.rs`, incl.
> dual-write, latest-wins, fallback-rebuild, session isolation) — all green.
>
> **Remaining:** the optional native `FjallMemoryStore` in `foundation_nativeapis` (OD-07-2 — perf opt-in;
> the KV path is the universal one) and the `fundamentals/` doc. The async surface is the established
> `#[async_trait]` (`Send`) form — the unified-`Send`/`SendWrapper` adapter is F00e's scope (OD-07-5).

> **Standing directive (user, 2026-06-15):** don't let an existing API bound the design — understand the
> limitation, redesign, and **create new purpose-fit traits where the existing ones are too bounded**.
> This applies to **every** feature/decision in this spec; each is updated toward the desired end goal,
> with genuine holes surfaced to the user rather than papered over. F07's application of this is resolved
> below.

> **RESOLVED (user, 2026-06-15; Item #5) — store the `SessionRecord` directly; `MemoryStore` is a
> purpose-built cache, not bounded by `KeyValueStore`.** It stores the **latest memory `SessionRecord` per
> tier** (no `*Snapshot` structs, no `MemoryBundle`, no serde-flatten — we store the record we already
> have). Key design:
> - **One key per session** `memory:{session_id}` → `SessionMemory { working/observation/reflection:
>   Option<SessionRecord> }` → **`hydrate` is ONE get** (hot resume path). The live session caches it, so
>   `set` mutates the cached copy + writes once (no RMW). A `BulkKeyValueStore::get_many` extension exists
>   if a backend ever needs per-tier keys — never N blind round-trips. (OD-07-8.)
> - **`set_async` takes `&SessionRecord`** — our trait, not bounded by `KeyValueStore`'s by-value `set`. (OD-07-9.)
> - **Async surface:** one unified **`Send`** async trait (Item #1, §A1) — **no `?Send` mirror**;
>   single-threaded wasm uses the `SendWrapper`-style adapter. See OD-07-5.
> - **MemoryStore does NOT wrap DocumentStore** — a `MemoryCoordinator` facade (held by `AgentSession`)
>   owns both + dual-write + fallback (Item #5 / OD-07-3).

> **Crate placement (resolved).** `MemoryStore` + `MemoryCoordinator` live in **`foundation_ai::agentic`**
> (they name `SessionId`/`SessionRecord`; `foundation_db` can't depend on `foundation_ai`). `KvMemoryStore`
> serializes over the `&str`-keyed `KeyValueStore` from `foundation_db`; `FjallMemoryStore` lives in
> **`foundation_nativeapis`** (where `fjall` is).

> Implements the user's MemoryStore idea: "a new MemoryStore that works on top of fjall, the usual
> `KeyValueStore` trait and its implementers, storing the latest/last Memories per `SessionId` for
> fast retrieval." Complements the append-only `DocumentStore` (the full audit log) with a
> **latest-snapshot** cache the resume path reads directly.

## WHY: Problem Statement

On resume (Decision 01), the agent must hydrate Working / Observation / Reflection memory **fast** —
it should not scan the whole message log to find the latest record of each tier. The append-only
`DocumentStore` is the source of truth (every memory `SessionRecord` is also appended there, Decision 03),
but finding "the newest reflection for this session" via the log is a scan. A `MemoryStore` keeps a
**direct, overwriting pointer** to the latest memory `SessionRecord` of each tier per session — an O(1)
get on resume. It works over the platform's existing `KeyValueStore` (every backend) and, on native,
optionally a dedicated fjall keyspace. The `MemoryCoordinator` (not MemoryStore) bridges it to the
`DocumentStore` audit log (dual-write + fallback).

## WHAT: Solution

> **RESOLVED (user, 2026-06-15) — Item #5 / discussion §A3.** MemoryStore stores the **`SessionRecord`
> directly** (no `*Snapshot` structs, no `MemoryBundle`); it does **not** wrap DocumentStore; a dedicated
> **`MemoryCoordinator`** facade (held by `AgentSession`, F20) owns both stores + dual-write + fallback;
> keyed by the **`SessionId`** struct.

### `MemoryStore` trait — stores the latest memory `SessionRecord` per tier

```rust
/// Latest-memory cache: the newest memory SessionRecord of each tier, per session.
/// Overwriting (not append-only): set() replaces the prior record of that tier.
/// One unified Send async surface (Item #1 / F00e); sync wrapper via valtron.
#[foundation_compact::send_async_trait]
pub trait MemoryStore {
    /// Latest memory record of a tier (None if never written).
    async fn get_async(&self, session: &SessionId, tier: MemoryTier)
        -> StorageResult<Option<SessionRecord>>;
    /// Overwrite the latest record for that session+tier. `record` MUST be a memory variant
    /// (WorkingMemory/Observation/Reflection); the tier is read from the variant.
    async fn set_async(&self, session: &SessionId, record: &SessionRecord) -> StorageResult<()>;
    /// Load all tiers at once (resume fast-path) — one get on the KV backend.
    async fn hydrate_async(&self, session: &SessionId) -> StorageResult<SessionMemory>;
    /// Clear a session's cached memory.
    async fn clear_async(&self, session: &SessionId) -> StorageResult<()>;
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum MemoryTier { Working, Observation, Reflection }

/// The three latest memory records (just Option<SessionRecord> per tier — NOT a snapshot factoring).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SessionMemory {
    pub working:     Option<SessionRecord>,   // SessionRecord::WorkingMemory
    pub observation: Option<SessionRecord>,   // SessionRecord::Observation
    pub reflection:  Option<SessionRecord>,   // SessionRecord::Reflection
}
```

We store the **exact `SessionRecord`** the loop already produced — no separate `WorkingMemorySnapshot`/
`ObservationSnapshot`/`ReflectionSnapshot` types and **no `MemoryBundle`/serde-flatten** (dissolves the
old OD-07-1 hole). `set_async` matches the variant to route to a tier; a non-memory variant is rejected.

### Backing: `KvMemoryStore` over `KeyValueStore` — one key per session

Wraps any `KeyValueStore` (works on every backend: SQLite, Turso/D1, in-memory, CF KV). **One key per
session holds all three tiers** (a `SessionMemory`), so `hydrate` is a single get:

```
key: memory:{session_id}   → JSON(SessionMemory { working?, observation?, reflection? })
```

- **`hydrate_async`** = **one** `kv.get("memory:{session}")` → `SessionMemory`. No fan-out, no scan.
- **`set_async`** updates one tier and re-persists. The live session caches the `SessionMemory`, so
  `set` mutates the cached copy + writes once (no RMW round-trip); a cold writer does one get first.
  Single-writer-per-session (OD-07-6) makes it race-free.
- **`clear_async`** = delete the single key.

### Optional native fjall keyspace

On native, `FjallMemoryStore` uses a dedicated fjall partition for locality. Same trait; chosen by
config (OD-07-2 — KV path is sufficient; fjall is a perf-only opt-in). Keys `{session_id}` → `SessionMemory`.

### The `MemoryCoordinator` facade (owns both stores) — NOT MemoryStore

`MemoryStore` is a **dumb fast cache** — it does **not** know about `DocumentStore`. A dedicated
**`MemoryCoordinator`** (held by `AgentSession`, F20) owns `{ MemoryStore (cache), DocumentStore (audit
log) }` and is the only thing that bridges them:

- **Dual-write (audit first, always):** when the loop (F15) produces a memory record, the coordinator
  appends it to the `DocumentStore` (audit/replay) **first**. Only on DocumentStore success does it
  `set` on the `MemoryStore` (latest pointer). **Ordering is strict:**
  - DocumentStore succeeds, MemoryStore fails → warn + retry on next access (the fallback scan handles
    this). The audit trail is intact.
  - DocumentStore fails → **do NOT** update MemoryStore. The cache becomes stale until rebuilt from the
    DocumentStore. Worst case: MemoryStore rebuilds from what was safely persisted.
  - MemoryStore is a **derived cache, never the sole record**. (Hole #8 resolved.)
- **Hydrate + fallback on resume:** `MemoryStore.hydrate()` is the fast path; if a tier is missing (e.g.
  crash before the cache write), the coordinator falls back to a `DocumentStore` scan filtered by
  `record_type` for the latest record of that tier, then **re-populates** the cache. So MemoryStore is a
  **derived cache, never the sole record**. (Resolves OD-07-3/07-4/07-7 — the drop-down to DocumentStore
  lives in the coordinator, not in MemoryStore.)

## Architecture

```mermaid
graph TD
    F15[memory generation] -->|memory SessionRecord| MC[MemoryCoordinator - held by AgentSession]
    MC -->|append audit| DS[(DocumentStore audit log)]
    MC -->|set latest| MS[(MemoryStore cache)]
    Resume[session resume] -->|MC.hydrate O(1)| MC
    MC -.fallback by record_type if miss.-> DS
    MS --> KV[KvMemoryStore over KeyValueStore]
    MS --> FJ[FjallMemoryStore native opt]
```

## HOW: Implementation Steps

1. Define `MemoryStore` + `MemoryTier` + `SessionMemory` in **`foundation_ai::agentic`** (it names
   `SessionId`/`SessionRecord`; `foundation_db` can't depend on `foundation_ai` — OD-07-4). **Stores
   `SessionRecord` directly — no `*Snapshot` structs, no `MemoryBundle`** (Item #5).
2. `KvMemoryStore<K: KeyValueStore>` impl (universal) — single key `memory:{session_id}` → `SessionMemory`.
3. `FjallMemoryStore` (native, opt-in) impl in `foundation_nativeapis`.
4. `hydrate` (one get) + `clear` (one delete).
5. Define the **`MemoryCoordinator`** facade (owns `MemoryStore` + `DocumentStore`): dual-write + the
   `record_type` fallback + re-populate; `AgentSession` (F20) holds it.
6. Tests: set/get/overwrite per tier (SessionRecord round-trips); single-get hydrate; clear; missing-tier
   → None; KV + fjall parity; `MemoryCoordinator` dual-write + fallback-to-DocumentStore (integration with
   F06, filter by `record_type`).

## Open Decisions

- **OD-07-1 — store `SessionRecord` directly: RESOLVED (user, 2026-06-15; Item #5).** No `*Snapshot`
  structs, no `MemoryBundle`, no serde-flatten. We store the **exact memory `SessionRecord`** the loop
  produced (it already serializes); `hydrate` returns `SessionMemory { working/observation/reflection:
  Option<SessionRecord> }`. The whole wire-format tradeoff disappears.
- **OD-07-2 — fjall vs KV: RESOLVED → implement BOTH** (user). `KvMemoryStore` is the universal path;
  `FjallMemoryStore` is the native perf opt-in. Test both.
- **OD-07-3 — MemoryStore does NOT wrap DocumentStore: RESOLVED (user, Item #5).** MemoryStore is a dumb
  cache. The **`MemoryCoordinator` facade owns both** stores and does the dual-write + cache→audit
  fallback; `AgentSession` (F20) holds the coordinator. The drop-down to DocumentStore lives there, not in
  MemoryStore.
- **OD-07-4 — crate placement: RESOLVED → `foundation_ai::agentic`** (names `SessionId`/`SessionRecord`;
  `foundation_db` can't depend on `foundation_ai`). Uses the `&str`-keyed `KeyValueStore` from
  `foundation_db`. `FjallMemoryStore` → `foundation_nativeapis`. `MemoryCoordinator` also lives in
  `foundation_ai::agentic`. **Key type:** the **`SessionId`** struct (wraps `foundation_compact::Id`,
  convenient methods, transparent serde — F01), not the bare id.

- **OD-07-5 — async variant: DISSOLVED (user, 2026-06-15; Item #1, discussion §A1).** There is **no
  `?Send` mirror**. There is **one `Send` async trait surface** spec-wide; on single-threaded wasm
  (`unknown-unknown`/CF/`wasip1`) a `SendWrapper`-style adapter (in `foundation_compact`/`foundation_wasm`)
  makes the `!Send` KV/Promise futures present as `Send` (sound — no real threads); native + emscripten
  require genuine `Send` and skip the adapter. So `MemoryStore`'s async surface is a normal `Send` async
  trait — no special-casing here.

- **OD-07-6 — version/CAS: RESOLVED → no CAS (last-writer-wins).** *(What "CAS" meant: compare-and-swap —
  only overwrite if a `version` matches, to catch two writers racing. Under single-agent-per-session there
  is one writer, so it's unnecessary.)* `set` unconditionally overwrites; the single-writer assumption is
  documented. (The old `version` field belonged to the deleted `*Snapshot` structs; the stored
  `SessionRecord` keeps whatever fields F01 gives it.)
- **OD-07-7 — fallback addressing: RESOLVED.** *(The issue: when the cache misses, "give me the latest
  reflection for this session" has to come from the audit log, which is append-only.)* The
  **`MemoryCoordinator`** (not MemoryStore) does it: `DocumentStore` scan filtered by **`record_type`**
  (F06 promoted column) for the latest record of that tier, then re-populate the cache.
- **OD-07-8 — single-key bundle: RESOLVED → one key `memory:{session_id}` → `SessionMemory`** so `hydrate`
  is one get; the live session caches it, so `set` mutates in memory + writes once (no RMW). If a backend
  ever needs per-tier keys, add a `BulkKeyValueStore::get_many` extension rather than N blind round-trips.
- **OD-07-9 — purpose-built API: RESOLVED.** `MemoryStore` is our own trait (`set_async` takes
  `&SessionRecord`), not bounded by `KeyValueStore`'s by-value `set`; serialization happens once at the
  store boundary.

## Target Files

- `backends/foundation_ai/src/agentic/memory_store.rs` (new) — `MemoryStore` (unified `Send`, §A1) +
  `MemoryTier` + `SessionMemory` + `KvMemoryStore<K: KeyValueStore>`
- `backends/foundation_ai/src/agentic/memory_coordinator.rs` (new) — `MemoryCoordinator` (owns
  MemoryStore + DocumentStore; dual-write + fallback); held by `AgentSession` (F20)
- `backends/foundation_nativeapis/src/.../fjall_memory_store.rs` (new, native, fjall) — optional perf backend
- coordinates with F01 (`SessionRecord` memory variants + `SessionId`), F06 (DocumentStore fallback via `record_type`), F15 (writer), F20 (`AgentSession` holds the `MemoryCoordinator`)

## Tests

```bash
cargo test -p foundation_ai -- agentic::memory_store
cargo test -p foundation_nativeapis --features vfs-fjall -- memory_store   # FjallMemoryStore
```

## Verification

```bash
cargo build  -p foundation_ai
cargo clippy -p foundation_ai -- -D warnings
cargo test   -p foundation_ai -- agentic::memory_store
cargo build  -p foundation_nativeapis --features vfs-fjall   # native fjall backend
```

## Fundamentals Documentation (zero-to-expert) — REQUIRED

Author `fundamentals/` covering: derived caches vs source-of-truth & invalidation; cache-aside &
fallback patterns; KV access patterns for latest-snapshot reads (**single-key bundle vs per-tier keys**,
why one get beats N); **designing purpose-fit traits instead of being bounded by a substrate trait**
(when to add a `BulkKeyValueStore` extension); **crate dependency direction** (why a trait can't name
types from a crate that depends on it); last-writer-wins vs CAS/versioning; the unified `Send` async
trait + single-threaded-wasm `SendWrapper` adapter (§A1).
(Task — see list.)

## Done When

- `MemoryStore` (one unified `Send` async trait — §A1) is defined **in `foundation_ai::agentic`**; stores
  the latest memory **`SessionRecord` per tier** (no `*Snapshot` structs, no `MemoryBundle`).
  `KvMemoryStore` works over any `KeyValueStore` via a single key/session (`SessionMemory`); an optional
  native `FjallMemoryStore` exists in `foundation_nativeapis`.
- `hydrate()` is **one get** on resume; keyed by the `SessionId` struct.
- **`MemoryCoordinator`** (held by `AgentSession`, F20) owns `MemoryStore` + `DocumentStore`, does
  dual-write and the `record_type` fallback + re-populate; **MemoryStore does NOT wrap DocumentStore**.
- OD-07-1..9 resolved (Item #5).
