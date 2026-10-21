# Fundamentals 06 — Serialization: JSON and Arrow

How session data is stored and exported. Source: `src/agentic/serialization.rs`,
`src/agentic/message_api.rs`.

---

## 1. JSON — the storage format

Every core type (`Messages`, `ModelOutput`, `SessionRecord`, `ModelId`, …)
derives serde `Serialize` / `Deserialize`. `SessionRecord` is
internally tagged on `message_type` (snake_case); the other enums use serde's
default externally-tagged form:

```rust
let json = serde_json::to_string(&record)?;
// {"message_type":"conversation","message":{"User":{"id":"…","role":"user",…}}}
let back: SessionRecord = serde_json::from_str(&json)?;
```

A few fields have custom handling:

- `MessageRole` serializes to `"user"` / `"agent"` / `"system"` / `"tool"`;
  `Custom(s)` serializes as the bare string.
- `ExecutionHint` serializes lowercase (`"parallel"`, …).
- `Args` serializes as its JSON Schema only; the validator is rebuilt on load.

`MessageApi` stores each record as the JSON `content` of a document in an
`AsyncDocumentStore`, keyed by session id.

## 2. How records are persisted

`MessageApi::append` only buffers a record in memory; writing is async.
The buffer is written by `flush().await`, which every read (`recent()`,
`all()`, `scan_from()`, `clear()`) runs first. Inside a turn the agent loop
awaits a flush as soon as the buffer reaches `MessageApiConfig::flush_threshold`
(default 50; set it with `AgentSessionBuilder::with_message_config` or
`MessageApi::with_config`), at every context assembly and when the turn ends;
`AgentSession::end()` flushes too. Code that appends outside a turn bounds the
buffer itself: await `flush()` once `needs_flush()` is true.

| Record | Written as |
|---|---|
| `Conversation { message }` | `append_with_id_async`, using the message's scru128 id as the document id |
| `WorkingMemory`, `Observation`, `Reflection` | `append_promotable_async` (promoted columns filled) — when written through `MessageApi` |
| `Retracted`, `Summary`, `FailedAction` | `append_async` |

The agent loop appends user messages, accepted assistant messages and tool
results. It does not append `FailedAction` or `Summary`; those only reach the
caller through the stream.

Memory tiers have their own path: `MemoryCoordinator` writes the record to an
audit document store and overwrites the latest copy in the `MemoryStore`
(`memory:{session_id}` → `SessionMemory { working, observation, reflection }`).
See Doc 11.

Subscribers can watch the log: `message_api.subscribe()` returns a receiver of
`MessageEvent`s (appended, flushed, errors).

## 3. Arrow — for analytics and bulk export

`SessionRecordRow` is a flat row with promoted columns plus the full JSON:

| Column | Content |
|---|---|
| `id` | Record id |
| `record_type` | `conversation`, `retracted`, `working_memory`, `observation`, `reflection`, `failed_action`, `summary` |
| `role` | Message role for conversation records |
| `title`, `summary` | Promoted text |
| `model` | Model name for assistant messages |
| `input_tokens`, `output_tokens` | From the message's `UsageReport` |
| `created_at` | Timestamp |
| `content` | The full record as JSON (lossless) |

```rust
use foundation_ai::agentic::internals::{to_record_batch, from_record_batch};

let records = agent.message_api().all().await?;
let batch = to_record_batch(&records)?;          // arrow RecordBatch
let restored = from_record_batch(&batch)?;       // Vec<SessionRecord>, round-trips via `content`
```

`serialization` also has small helpers over rows: `sum_input_tokens`,
`sum_output_tokens`, `filter_by_type`. Errors are `SerError::{Json, Arrow}`.

## 4. Schema evolution

Add fields compatibly with `#[serde(default)]` (or `Option`). The crate does
this already, for example:

```rust
Messages::User {
    #[serde(default = "fresh_message_id")]   // pre-id fixtures get a fresh scru128
    id: Id,
    ..
}

ModelOutput::ToolCall {
    #[serde(default)] depends_on: Vec<String>,
    #[serde(default)] execution_hint: ExecutionHint,
    ..
}
```

Renaming or removing a field, or changing an enum's shape, breaks stored
sessions and needs a migration.
