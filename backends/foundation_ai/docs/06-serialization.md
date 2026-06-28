# Fundamentals 06 — Serialization: JSON and Arrow columns

How agent data is persisted and queried across JSON and columnar formats.

---

## 1. JSON serialization (the primary format)

All core types implement `Serialize + Deserialize`:

```rust
// Message round-trip
let json = serde_json::to_string(&message)?;
let restored: Messages = serde_json::from_str(&json)?;
```

### Custom serialization
- **`ModelId`** — serializes as the model name string, deserializes by
  checking known providers
- **`ArgType`** — serializes as native JSON (Text → string, Number → number, etc.)
- **`ModelOutput`** — tagged union: `{"type": "text", "content": "..."}` or
  `{"type": "tool_call", "name": "...", "arguments": {...}}`

## 2. Arrow serialization (for analytics)

Promoted fields (`title`, `summary`, `record_type`) can be exported as
Arrow columns for efficient querying:

```rust
// DocumentStore → Arrow batch
let batch = store.export_as_arrow(key, time_range)?;
// batch: RecordBatch with columns: doc_id, title, summary, record_type, created_at
```

This is used for:
- Analytics dashboards (token usage over time)
- Batch reprocessing (regenerate embeddings)
- Export to external systems

## 3. State persistence

Agent session state is persisted via `MessageApi` (F08):

```
SessionRecord::Conversation { message }  → append to message log
SessionRecord::WorkingMemory { facts }   → overwrite working memory
SessionRecord::Observation { observations } → append to observation log
SessionRecord::Reflection { reflections }   → overwrite reflection
SessionRecord::FailedAction { error }    → append to error log
```

Each record type has its own persistence strategy (append vs overwrite) and
storage location (message log vs KV store).

## 4. Schema evolution

The serialization format is versioned. New fields are added with `#[serde(default)]`
to maintain backward compatibility:

```rust
#[derive(Serialize, Deserialize)]
pub struct Messages {
    // ... existing fields ...
    #[serde(default)]
    pub signature: Option<String>,  // New field, old data deserializes fine
}
```

Breaking changes (renaming, removing) require a migration step.
