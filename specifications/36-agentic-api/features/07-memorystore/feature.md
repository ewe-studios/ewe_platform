---
feature: "MemoryStore — fast latest-memory retrieval per session"
description: "A MemoryStore trait (over KeyValueStore + optional fjall) that stores the newest Working/Observation/Reflection snapshot per SessionId for O(1) hydration on resume — distinct from the append-only DocumentStore audit log"
status: "pending"
priority: "high"
depends_on: ["01-message-model", "04-documentstore-trait-sql-memory"]
estimated_effort: "medium"
created: 2026-06-14
last_updated: 2026-06-14
author: "Main Agent"
tasks:
  completed: 0
  uncompleted: 9
  total: 9
  completion_percentage: 0%
---

**TODO**: Same statement dont let existing API limit you, we build it better, we understand what the limitations are, see if we need to redesign what we need and make it work. If existing traits are to bounded, lets create new specific traits for our usecases that we design for that fit our needs, that is ok.

# Feature 07: MemoryStore

> **Review status (2026-06-14) — crate placement reversed (was fatal).** The typed `MemoryStore`
> **cannot** live in `foundation_db` (OD-07-4 was wrong): it names `SessionId` + the snapshot types,
> which live in `foundation_ai`, and `foundation_db` cannot depend on `foundation_ai` (the arrow
> points the other way). **Fix:** the typed `MemoryStore`/`AsyncMemoryStore` + `MemoryBundle` live in
> **`foundation_ai::agentic`** (which already depends on `foundation_db` for `KeyValueStore`); the
> `KvMemoryStore` serializes `SessionId`/snapshots over the `&str`-keyed, JSON-valued
> `KeyValueStore`. The optional `FjallMemoryStore` lives in **`foundation_nativeapis`** (where `fjall`
> actually is — not `foundation_db`). Also: `KeyValueStore::set` takes `value` **by value** (clone),
> there is **no bulk get** (`hydrate` = 3 sequential gets), `AsyncMemoryStore` must be `(?Send)` (to
> match `AsyncKeyValueStore`), and the F01 snapshot factoring is messier than "share the struct"
> (shapes differ). See revised OD-07-1/4/5 + OD-07-6/7.

> Implements the user's MemoryStore idea: "a new MemoryStore that works on top of fjall, the usual
> `KeyValueStore` trait and its implementers, storing the latest/last Memories per `SessionId` for
> fast retrieval." Complements the append-only `DocumentStore` (the full audit log) with a
> **latest-snapshot** cache the resume path reads directly.

## WHY: Problem Statement

On resume (Decision 01), the agent must hydrate Working / Observation / Reflection memory **fast** —
it should not scan the whole message log to find the latest snapshot of each tier. The append-only
`DocumentStore` is the source of truth (every memory snapshot is also appended there, Decision 03),
but finding "the newest reflection for this session" via the log is a scan. A `MemoryStore` keeps a
**direct, overwriting pointer** to the latest snapshot of each tier per session — an O(1) get on
resume. It must work over the platform's existing `KeyValueStore` (every backend) and, on native,
optionally a dedicated fjall keyspace for locality.

## WHAT: Solution

### `MemoryStore` trait

```rust
/// Latest-snapshot store for the three memory tiers, keyed by SessionId.
/// Overwriting (not append-only): set() replaces the prior snapshot of that tier.
pub trait MemoryStore: Send + Sync {
    fn get_working(&self, session: &SessionId)    -> StorageResult<Option<WorkingMemorySnapshot>>;
    fn set_working(&self, session: &SessionId, v: &WorkingMemorySnapshot) -> StorageResult<()>;

    fn get_observation(&self, session: &SessionId) -> StorageResult<Option<ObservationSnapshot>>;
    fn set_observation(&self, session: &SessionId, v: &ObservationSnapshot) -> StorageResult<()>;

    fn get_reflection(&self, session: &SessionId)  -> StorageResult<Option<ReflectionSnapshot>>;
    fn set_reflection(&self, session: &SessionId, v: &ReflectionSnapshot) -> StorageResult<()>;

    /// Load all three at once (resume fast-path).
    fn hydrate(&self, session: &SessionId) -> StorageResult<MemoryBundle>;
    /// Clear a session's memory snapshots.
    fn clear(&self, session: &SessionId) -> StorageResult<()>;
}

pub struct MemoryBundle {
    pub working:     Option<WorkingMemorySnapshot>,
    pub observation: Option<ObservationSnapshot>,
    pub reflection:  Option<ReflectionSnapshot>,
}
```

The snapshot types reuse F01's memory entry types: `WorkingMemorySnapshot { facts: Vec<MemoryFact>,
version: u64 }`, `ObservationSnapshot { observations: Vec<ObservationEntry>, token_count: u64 }`,
`ReflectionSnapshot { reflections: Vec<ReflectionEntry>, ... }` (these are the payloads of F01's
`SessionRecord::WorkingMemory/Observation/Reflection` variants — factor the shared structs so both
the record and the snapshot use them). OD-07-1.

### Backing: `KvMemoryStore` over `KeyValueStore`

The default impl wraps any `KeyValueStore` (`storage_provider.rs:309`) — so it works on every backend
(SQLite, Turso/D1, in-memory, CF KV):

```
key: memory:{session_id}:working      → JSON(WorkingMemorySnapshot)
key: memory:{session_id}:observation  → JSON(ObservationSnapshot)
key: memory:{session_id}:reflection   → JSON(ReflectionSnapshot)
```

`set_*` = `kv.set(key, json)` (overwrite); `get_*` = `kv.get(key)`; `hydrate` = three gets (or one
`list_keys("memory:{session}:")` + bulk). This is the universal path.

### Optional native fjall keyspace

On native, a `FjallMemoryStore` uses a dedicated fjall partition keyed `{session_id}:{tier}` for
locality and fast point-reads, mirroring the F05 sidecar-index philosophy. Same trait; chosen by
config. (OD-07-2 — is the KV path sufficient, making fjall a perf-only option? Rec: yes, fjall is
opt-in.)

### Relationship to DocumentStore (source of truth)

Writing memory is **dual**: the loop (F19) appends the snapshot to `DocumentStore` (audit, replay)
**and** updates `MemoryStore` (latest pointer). On resume, `MemoryStore.hydrate()` is the fast path;
if a snapshot is missing (e.g. crash before MemoryStore write), fall back to a `DocumentStore`
`scan` for the latest record of that tier. So MemoryStore is a **derived cache**, never the sole
record. OD-07-3.

## Architecture

```mermaid
graph TD
    F19[memory generation] -->|append snapshot| DS[(DocumentStore audit log)]
    F19 -->|overwrite latest| MS[(MemoryStore)]
    Resume[session resume] -->|hydrate O(1)| MS
    Resume -.fallback if missing.-> DS
    MS --> KV[KvMemoryStore over KeyValueStore]
    MS --> FJ[FjallMemoryStore native opt]
```

## HOW: Implementation Steps

1. Factor F01's memory payloads into shared `*Snapshot` structs (used by both `SessionRecord` and
   `MemoryStore`).
2. Define `MemoryStore` trait + `MemoryBundle` in `foundation_db` (or `foundation_ai::agentic`? —
   OD-07-4: it's storage → `foundation_db`).
3. `KvMemoryStore<K: KeyValueStore>` impl (universal).
4. `FjallMemoryStore` (native, opt-in) impl.
5. `hydrate` + `clear`; document the DocumentStore fallback contract.
6. Tests: set/get/overwrite per tier; hydrate bundle; clear; missing-tier → None; KV + fjall parity;
   fallback-to-DocumentStore (integration with F04).

## Open Decisions

- **OD-07-1 — shared snapshot structs:** factor F01's memory payloads so record + snapshot share
  types (avoid divergence). Rec: yes.
- **OD-07-2 — fjall vs KV:** is `KvMemoryStore` sufficient, fjall a perf-only opt-in? Rec: yes.
- **OD-07-3 — derived-cache semantics:** MemoryStore is a cache over DocumentStore; resume falls back
  to a DocumentStore scan if a tier snapshot is absent. Confirm.
- **OD-07-4 — crate placement:** **Resolved → typed `MemoryStore` in `foundation_ai::agentic`**
  (it names `SessionId`/snapshots; `foundation_db` can't depend on `foundation_ai`). It uses the
  `&str`-keyed `KeyValueStore` from `foundation_db`. `FjallMemoryStore` → `foundation_nativeapis`
  (fjall's home).
- **OD-07-5 — async variant:** `AsyncMemoryStore` mirrors `AsyncKeyValueStore`'s **`(?Send)`**
  constraint (it cannot be `Send + Sync` like the sync trait) — needed for CF KV on wasm.
- **OD-07-6 — version/CAS:** snapshots carry `version: u64` but `set_*` is last-writer-wins. Define
  concurrent-writer behavior: unconditional overwrite (rec, single-agent-per-session) vs compare-
  version CAS. Rec: unconditional now; note the single-writer assumption.
- **OD-07-7 — fallback addressing:** "latest of tier T" via `DocumentStore` requires either separate
  collections per tier or scan-and-filter by `record_type` (F04 promoted column!). Rec: filter by
  `record_type` via F04's `scan_documents`; re-populate `MemoryStore` on a fallback hit.

## Target Files

- `backends/foundation_ai/src/agentic/memory_store.rs` (new) — typed `MemoryStore` +
  `AsyncMemoryStore` (`?Send`) + `MemoryBundle` + `KvMemoryStore<K: KeyValueStore>`
- `backends/foundation_nativeapis/src/.../fjall_memory_store.rs` (new, native, fjall) — optional perf backend
- coordinates with F01 (snapshot structs — factoring), F04 (DocumentStore fallback via `record_type`), F19 (writer)

## Tests

```bash
cargo test -p foundation_db -- memory_store
```

## Verification

```bash
cargo build -p foundation_db
cargo clippy -p foundation_db -- -D warnings
cargo test  -p foundation_db
```

## Fundamentals Documentation (zero-to-expert) — REQUIRED

Author `fundamentals/` covering: derived caches vs source-of-truth & invalidation; cache-aside &
fallback patterns; KV access patterns for latest-snapshot reads; **crate dependency direction** (why
a trait can't name types from a crate that depends on it); last-writer-wins vs CAS/versioning;
`?Send` async mirrors. (Task — see list.)

## Done When

- `MemoryStore` (+ async mirror) is defined; `KvMemoryStore` works over any `KeyValueStore`; an
  optional native `FjallMemoryStore` exists.
- `hydrate()` loads all three tiers fast on resume; missing tiers fall back to DocumentStore.
- Snapshot structs are shared with F01's records (no divergence).
- OD-07-1..5 resolved.
