---
feature: "MemoryStore — fast latest-memory retrieval per session"
description: "A MemoryStore trait (over KeyValueStore + optional fjall) that stores the newest Working/Observation/Reflection snapshot per SessionId for O(1) hydration on resume — distinct from the append-only DocumentStore audit log"
status: "pending"
priority: "high"
depends_on: ["01-message-model", "06-documentstore-trait-sql-memory"]
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

# Feature 07: MemoryStore

> **Standing directive (user, 2026-06-15):** don't let an existing API bound the design — understand the
> limitation, redesign, and **create new purpose-fit traits where the existing ones are too bounded**.
> This applies to **every** feature/decision in this spec; each is updated toward the desired end goal,
> with genuine holes surfaced to the user rather than papered over. F07's application of this is resolved
> below.

> **RESOLVED (user, 2026-06-15) — don't let `KeyValueStore` bound the design; build the trait we need.**
> `MemoryStore` is a **purpose-built trait** for the latest-snapshot-per-session use case, NOT a thin
> reskin constrained by `KeyValueStore`'s shape. Where the existing KV trait is too bounded we either
> design around it or add a new, specific trait — that's fine. Concretely, addressing the limitations the
> review found:
> - **No bulk get (hydrate = 3 sequential gets):** `hydrate()` is a **first-class single-shot op** in our
>   trait, and the `KeyValueStore` impl stores all three tiers under **one key** `memory:{session_id}` as
>   a single `MemoryBundle` JSON → **hydrate is ONE get** (the hot resume path wins). `set_*` becomes a
>   read-modify-write of the bundle; that's acceptable because writes happen at memory triggers (rare),
>   reads happen on every resume (hot). If per-tier keys are ever needed for a backend, add a small
>   `BulkKeyValueStore { get_many(&[key]) }` extension rather than living with N round-trips. (OD-07-8.)
> - **`set` clones by value:** our `set_*` takes `&Snapshot`; the bundle path serializes once. We don't
>   inherit the by-value clone at the MemoryStore API. (OD-07-9.)
> - **Async surface:** one unified **`Send`** async trait (Item #1, §A1) — **no `?Send` mirror**;
>   single-threaded wasm uses the `SendWrapper`-style adapter. See OD-07-5.
>
> So `MemoryStore` is **our** trait; `KvMemoryStore`/`FjallMemoryStore` are impls that adapt the storage
> substrate to it — the substrate does not dictate the API.

> **Review status (2026-06-14) — crate placement reversed (was fatal).** The typed `MemoryStore`
> **cannot** live in `foundation_db` (OD-07-4 was wrong): it names `SessionId` + the snapshot types,
> which live in `foundation_ai`, and `foundation_db` cannot depend on `foundation_ai` (the arrow
> points the other way). **Fix:** the typed `MemoryStore`/`AsyncMemoryStore` + `MemoryBundle` live in
> **`foundation_ai::agentic`** (which already depends on `foundation_db` for `KeyValueStore`); the
> `KvMemoryStore` serializes `SessionId`/snapshots over the `&str`-keyed, JSON-valued
> `KeyValueStore`. The optional `FjallMemoryStore` lives in **`foundation_nativeapis`** (where `fjall`
> actually is — not `foundation_db`). Also: `KeyValueStore::set` takes `value` **by value** (clone),
> there is **no bulk get** (`hydrate` = 3 sequential gets), the async surface is a single unified `Send`
> trait (Item #1/§A1 — `?Send` mirror dropped), and the F01 snapshot factoring is messier than "share the
> struct" (shapes differ). See revised OD-07-1/4/5 + OD-07-6/7.

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
(SQLite, Turso/D1, in-memory, CF KV). **One key per session holding the whole bundle**, so the hot
resume path is a single get (OD-07-8):

```
key: memory:{session_id}   → JSON(MemoryBundle { working?, observation?, reflection? })
```

- **`hydrate`** = **one** `kv.get("memory:{session}")` → deserialize `MemoryBundle`. No 3-get fan-out,
  no `list_keys` scan.
- **`set_*`** updates one tier of the bundle and re-persists it. Because the live session already holds
  the hydrated `MemoryBundle` in memory, `set_*` mutates that in-memory copy and writes the whole bundle
  once — **no read-modify-write round-trip** on the hot path; a cold writer (no cached bundle) does one
  get first. Single-writer-per-session (OD-07-6) makes this race-free.
- **`get_*`** reads the cached/persisted bundle and returns the one tier.
- **`clear`** = delete the single key.

This is the universal path. (Per-tier keys remain a fallback for any backend that needs them, via a
`BulkKeyValueStore::get_many` extension — OD-07-8 — never N blind round-trips.)

### Optional native fjall keyspace

On native, a `FjallMemoryStore` uses a dedicated fjall partition for locality and fast point-reads,
mirroring the F22 index philosophy. fjall reads are cheap and local, so it may key **per session**
(`{session_id}` → bundle, single read, consistent with KV) or **per tier** (`{session_id}:{tier}`,
avoids any RMW) — either satisfies `hydrate`. Same trait; chosen by config. (OD-07-2 — is the KV path
sufficient, making fjall a perf-only option? Rec: yes, fjall is opt-in.)

### Relationship to DocumentStore (source of truth)

Writing memory is **dual**: the loop (F15) appends the snapshot to `DocumentStore` (audit, replay)
**and** updates `MemoryStore` (latest pointer). On resume, `MemoryStore.hydrate()` is the fast path;
if a snapshot is missing (e.g. crash before MemoryStore write), fall back to a `DocumentStore`
`scan` for the latest record of that tier. So MemoryStore is a **derived cache**, never the sole
record. OD-07-3.

## Architecture

```mermaid
graph TD
    F15[memory generation] -->|append snapshot| DS[(DocumentStore audit log)]
    F15 -->|overwrite latest| MS[(MemoryStore)]
    Resume[session resume] -->|hydrate O(1)| MS
    Resume -.fallback if missing.-> DS
    MS --> KV[KvMemoryStore over KeyValueStore]
    MS --> FJ[FjallMemoryStore native opt]
```

## HOW: Implementation Steps

1. Factor F01's memory payloads into shared `*Snapshot` structs so the record and the `MemoryStore`
   snapshot can't diverge — via the sharing strategy chosen in **OD-07-1** (flatten vs nest vs duplicate;
   needs the user's wire-format call).
2. Define `MemoryStore`/`AsyncMemoryStore` + `MemoryBundle` in **`foundation_ai::agentic`** (it names
   `SessionId`/snapshots; `foundation_db` can't depend on `foundation_ai` — OD-07-4).
3. `KvMemoryStore<K: KeyValueStore>` impl (universal) — single-key bundle (OD-07-8).
4. `FjallMemoryStore` (native, opt-in) impl in `foundation_nativeapis`.
5. `hydrate` (one get) + `clear` (one delete); document the DocumentStore fallback contract.
6. Tests: set/get/overwrite per tier; single-get hydrate bundle; clear; missing-tier → None; KV + fjall
   parity; fallback-to-DocumentStore (integration with F06, filter by `record_type`).

## Open Decisions

- **OD-07-1 — shared snapshot structs (NEEDS USER CALL — wire-format tradeoff):** the record and the
  snapshot must share the payload structs (`WorkingMemorySnapshot`/`ObservationSnapshot`/
  `ReflectionSnapshot`) to avoid divergence, but F01's `SessionRecord` variants carry **extra** record-level
  fields (`timestamp`, the reflection before/after counts) the bare snapshot doesn't. Three ways to share,
  each with a cost:
  - **(a) `#[serde(flatten)] snapshot: WorkingMemorySnapshot` + outer fields** — keeps Decision 03's
    **flat** JSON, shares the struct. **Risk:** `SessionRecord` is an internally-tagged enum
    (`tag = "message_type"`); serde `flatten` inside internally-tagged variants has known edge cases —
    must be tested.
  - **(b) nest `snapshot: { … }`** — clean Rust, but **changes the wire shape** (Decision 03 fixtures
    become nested). A deliberate break to document.
  - **(c) duplicate the structs** — no wire change, but reintroduces the divergence we're trying to kill.
  **Rec: (a) flatten** if the internally-tagged-enum test passes, else (b) with a documented wire change.
  This is the one open hole in F07 — flagged for the user.

      Unless the SessionRecord is hard to  serialize, why do we even need any of this, should not what we store just be the SessionRecord? Explain to me better and lets talk about it.
  
- **OD-07-2 — fjall vs KV:** is `KvMemoryStore` sufficient, fjall a perf-only opt-in? Rec: yes.
        Implement both, then we can use whichever we want. This is the time to get it all right and done.

- **OD-07-3 — derived-cache semantics:** MemoryStore is a cache over DocumentStore; resume falls back
  to a DocumentStore scan if a tier snapshot is absent. Confirm.
        Yes, exactly, i am even wondering why MemoryStore needs to wrap a DocumentStore, the whole point of it is just a Memorystore probably for testing or caching, but whatever owns those two should own the drop down to DocumentStore and not MemoryStore itself.

- **OD-07-4 — crate placement:** **Resolved → typed `MemoryStore` in `foundation_ai::agentic`**
  (it names `SessionId`/snapshots; `foundation_db` can't depend on `foundation_ai`). It uses the
  `&str`-keyed `KeyValueStore` from `foundation_db`. `FjallMemoryStore` → `foundation_nativeapis`
  (fjall's home).
      Ya, i can see your wrapping DocumentStore cuasing issues here. Why not just split them and let something own both and use them properly then they each can stay where they are and use waht works, more so why MemoryStore use SessionId - which are just scru128 ids?

- **OD-07-5 — async variant: DISSOLVED (user, 2026-06-15; Item #1, discussion §A1).** There is **no
  `?Send` mirror**. There is **one `Send` async trait surface** spec-wide; on single-threaded wasm
  (`unknown-unknown`/CF/`wasip1`) a `SendWrapper`-style adapter (in `foundation_compact`/`foundation_wasm`)
  makes the `!Send` KV/Promise futures present as `Send` (sound — no real threads); native + emscripten
  require genuine `Send` and skip the adapter. So `MemoryStore`'s async surface is a normal `Send` async
  trait — no special-casing here.

- **OD-07-6 — version/CAS:** snapshots carry `version: u64` but `set_*` is last-writer-wins. Define
  concurrent-writer behavior: unconditional overwrite (rec, single-agent-per-session) vs compare-
  version CAS. Rec: unconditional now; note the single-writer assumption.
      Why and for what ? Explain to me clearly the issue

- **OD-07-7 — fallback addressing:** "latest of tier T" via `DocumentStore` requires either separate
  collections per tier or scan-and-filter by `record_type` (F06 promoted column!). Rec: filter by
  `record_type` via F06's `scan_documents`; re-populate `MemoryStore` on a fallback hit.
        Explain to me again and be detailed so i  understand the issue

- **OD-07-8 — single-key bundle vs per-tier keys:** **Resolved (user, 2026-06-15)** → store the whole
  `MemoryBundle` under **one key** `memory:{session_id}` so `hydrate` is one get (hot resume path). The
  live session caches the bundle, so `set_*` mutates in memory + writes once (no RMW round-trip). If a
  backend ever needs per-tier keys, add a `BulkKeyValueStore::get_many(&[key])` extension trait rather
  than N blind round-trips — don't let the bounded `KeyValueStore` shape force a slow hydrate.

        Explain to me again and be detailed so i  understand the issue

- **OD-07-9 — API doesn't inherit KV's by-value `set`:** **Resolved (user, 2026-06-15)** → `MemoryStore`
  is our purpose-built trait: `set_*` takes `&Snapshot`, serialization happens once at the bundle
  boundary; we do not propagate `KeyValueStore::set`'s by-value clone into the MemoryStore API.

        Explain to me again and be detailed so i  understand the issue


## Target Files

- `backends/foundation_ai/src/agentic/memory_store.rs` (new) — typed `MemoryStore` +
  `AsyncMemoryStore` (unified `Send`, §A1) + `MemoryBundle` + `KvMemoryStore<K: KeyValueStore>`
- `backends/foundation_nativeapis/src/.../fjall_memory_store.rs` (new, native, fjall) — optional perf backend
- coordinates with F01 (snapshot structs — factoring), F06 (DocumentStore fallback via `record_type`), F15 (writer)

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

- `MemoryStore` (+ `AsyncMemoryStore`, one unified `Send` trait — §A1) is defined **in `foundation_ai::agentic`** (not
  `foundation_db`); `KvMemoryStore` works over any `KeyValueStore` via a single-key bundle; an optional
  native `FjallMemoryStore` exists in `foundation_nativeapis`.
- `hydrate()` is **one get** on resume; missing tiers fall back to a DocumentStore `record_type` scan and
  re-populate the cache.
- Snapshot structs are shared with F01's records via the OD-07-1 factoring (no divergence).
- `MemoryStore` is a purpose-built trait, not bounded by `KeyValueStore` (single-key hydrate, `&`-taking
  `set_*`).
- OD-07-2..9 resolved; **OD-07-1 flagged for the user** (snapshot-sharing wire-format call).
