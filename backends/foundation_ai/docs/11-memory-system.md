# How-To: Working with the Memory System

The memory tiers, how they get filled, how they reach the model, and what is
not wired up yet. Sources: `src/agentic/memory.rs`, `memory_store.rs`,
`memory_coordinator.rs`, `tools/memory.rs`.

---

## 1. The three tiers

| Tier | Record | Holds | Written by |
|---|---|---|---|
| **Working** | `SessionRecord::WorkingMemory { facts, version, .. }` | Curated facts (`MemoryFact`) | The `memory` tool, or `update_working_memory` |
| **Observation** | `SessionRecord::Observation { observations, token_count, .. }` | Time-scoped `ObservationEntry`s | `persist_observation` (yours to call) |
| **Reflection** | `SessionRecord::Reflection { reflections, .. }` | Condensed `ReflectionEntry`s | `persist_reflection` (yours to call) |

Each tier keeps only its **latest** record; writing replaces the previous one.

## 2. What is automatic today

| Step | Status |
|---|---|
| Rolling token counter (`TokenLedger::rolling`) | ✅ updated after every generation |
| Trigger check after each inner round (`check_triggers`) | ✅ |
| Signal when a threshold is crossed | ✅ `AgentProgress::ProcessingMemory { kind }` |
| Calling a memory model to write the observation / reflection | ❌ not implemented |
| Working memory curated by the agent | ✅ via the `memory` tool |
| Tiers included in the assembled context | ✅ — but see §6 |

So with only defaults, the observation and reflection tiers stay empty. If
you want them, watch for `ProcessingMemory`, generate the entries yourself
(e.g. with a cheap model through `agent.router()`), and persist them:

```rust
let hierarchy = agent.memory_hierarchy();
if hierarchy.begin_generation() {                 // prevents re-triggering meanwhile
    let entries: Vec<ObservationEntry> = summarise_with_my_model(&recent)?;
    futures_lite::future::block_on(hierarchy.persist_observation(entries, token_count))?;
    hierarchy.end_generation();
}
```

`persist_observation` resets the rolling counter and adds `token_count` to the
observation counter; `persist_reflection` resets the observation counter.

## 3. Triggers

```rust
let agent = builder
    .with_memory_config(MemoryConfig {
        observation_trigger_tokens: 30_000,   // default
        reflection_trigger_tokens: 40_000,    // default
        ..MemoryConfig::default()
    })
    .build()?;
```

`check_triggers()` returns:

- `None` while a generation is marked in progress (`begin_generation`)
- `GenerateObservation` when rolling tokens ≥ `observation_trigger_tokens`
- `GenerateReflection` when observation tokens ≥ `reflection_trigger_tokens`
- otherwise `None`

`MemoryConfig::memory_model` and `parse_strategy` are stored for the future
memory generator; nothing reads them yet.

## 4. The `memory` tool

One multi-command tool, `memory`, with `add`, `remove` and `replace` commands
over working-memory facts. Each command loads the latest working memory,
edits the fact list and writes it back with a version bump.

```rust
use foundation_ai::harness::ToolPreset;

ToolPreset::memory(Arc::new(agent.memory_hierarchy().clone()))
    .register_all(agent.tool_manager());
```

## 5. Storage

`MemoryStore` is a small async trait — get / set / hydrate / clear the latest
record per tier, plus a synchronous `hydrate_sync` the loop uses.
`KvMemoryStore<K: KeyValueStore>` implements it over any `foundation_db`
key-value store, keeping the whole bundle under one key:

```
memory:{session_id} → SessionMemory { working, observation, reflection }
```

`MemoryCoordinator<M, D>` dual-writes: it appends each memory record to an
audit `DocumentStore` and overwrites the latest copy in the `MemoryStore`.
`MemoryHierarchy` writes through the coordinator.

## 6. Known problem: two different stores

`AgentSession::build()` gives the hierarchy's coordinator a fresh
`M::default()` and `D::default()`, while the `ContextProvider` reads the store
you passed with `with_memory_store` (or its own default). With the in-memory
stores those are separate maps, so:

- facts added with the `memory` tool are stored, but never appear in the
  assembled context;
- `persist_observation` / `persist_reflection` through
  `agent.memory_hierarchy()` have the same problem.

A store whose `Default` reaches shared backing storage would not hit this, but
no store in `foundation_db` does. The fix is tracked separately; until then,
treat working memory as not reaching the model.

## 7. How memory reaches the model

At every `InnerAssemble`, the loop hydrates `SessionMemory` from the context
provider's store and prepends, as system-role messages:

1. working memory (`[working_memory]` + one line per fact)
2. reflection (`[reflection]`)
3. observation (`[observation]`) — only if newer than the reflection

then the recent conversation (Doc 01 §4).

## 8. Resume

`AgentSession::resume` starts from default-constructed stores, so memory
survives only when those defaults reach the same data. See Doc 08 §5.
