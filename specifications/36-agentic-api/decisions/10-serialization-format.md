# Decision 10: Serialization Format — JSON and Arrow

**Status:** Proposed  
**Date:** 2026-06-11  
**Context:** Specification 36 — Agentic API for foundation_ai

## Problem

All AI interactions must be serializable for storage, transport, and replay. The format must support:
- Human-readable debugging
- Efficient batch processing
- WASM-compatible encoding
- Transport across process boundaries (UI, monitoring, external services)
- Columnar analytics (token usage, latency metrics)

## Decision

All AI interactions are **always serializable to both JSON and Arrow formats**. JSON is the primary format for storage and debugging; Arrow is the primary format for batch processing and transport.

### JSON Format (Primary for Storage)

Every message, tool call, memory snapshot, and event is a JSON-serializable record:

```json
{
  "session_id": "scru128-...",
  "id": "scru128-...",
  "message_type": "assistant",
  "role": "assistant",
  "content": {
    "type": "text",
    "text": "I'll fix the bug in the valtron executor..."
  },
  "metadata": {
    "model": "claude-sonnet-4-6",
    "token_count": 245,
    "created_at": 1718100000000,
    "finish_reason": "stop"
  }
}
```

**Why JSON for storage:**
- Human-readable — can inspect with `jq`, `cat`, text editors
- Append-friendly — NDJSON (one JSON object per line) is ideal for append-only logs
- WASM-compatible — `serde_json` works in WASM environments
- Tooling — every language has JSON parsers

### Arrow Format (Primary for Batch Processing)

For batch operations, analytics, and efficient transport, messages are encoded as Arrow records:

```rust
// Arrow schema for messages
pub fn message_schema() -> Schema {
    Schema::new(vec![
        Field::new("session_id", DataType::Utf8, false),
        Field::new("id", DataType::Utf8, false),
        Field::new("message_type", DataType::Utf8, false),
        Field::new("role", DataType::Utf8, false),
        Field::new("content", DataType::Utf8, false),  // JSON-encoded content
        Field::new("metadata", DataType::Utf8, false),  // JSON-encoded metadata
        Field::new("created_at", DataType::UInt64, false),
        Field::new("token_count", DataType::UInt32, true),
    ])
}
```

**Why Arrow for batch processing:**
- Columnar format — efficient for analytics (sum token counts, filter by message type)
- Zero-copy deserialization — faster than JSON for large batches
- Interoperable — Arrow libraries exist for Rust, Python, JavaScript, Go
- Transport-efficient — binary format, smaller than JSON

### Serialization Trait

All serializable types implement a common trait:

```rust
pub trait Serializable: serde::Serialize + serde::de::DeserializeOwned {
    /// Serialize to JSON string
    fn to_json(&self) -> Result<String> {
        serde_json::to_string(self).map_err(|e| SerializationError::Json(e.to_string()))
    }
    
    /// Deserialize from JSON string
    fn from_json(json: &str) -> Result<Self> {
        serde_json::from_str(json).map_err(|e| SerializationError::Json(e.to_string()))
    }
    
    /// Serialize to Arrow record batch
    fn to_arrow(&self) -> Result<RecordBatch>;
    
    /// Deserialize from Arrow record batch
    fn from_arrow(batch: &RecordBatch) -> Result<Self>;
}
```

### Content Type Handling

LLM message content can be complex (text, tool calls, thinking blocks, images). The content type is preserved in both formats:

```json
{
  "content": {
    "type": "multi_part",
    "parts": [
      { "type": "text", "text": "Let me call the tool..." },
      { "type": "tool_call", "name": "read_file", "arguments": {"path": "/foo.rs"} },
      { "type": "thinking", "text": "I need to understand the file structure first..." }
    ]
  }
}
```

### FlatBuffers Fallback

**TODO**: Arrow works in wasm, and it uses flatbuffers underneath so the flatbuffer option is not even necessary

If Arrow proves problematic in certain environments (e.g., WASM), **FlatBuffers** is the fallback format:

| Format | Pros | Cons | Status |
|--------|------|------|--------|
| **JSON** | Human-readable, universal | Verbose, slow to parse | Primary |
| **Arrow** | Columnar, fast, interoperable | Heavier dependency tree | Primary for batch |
| **FlatBuffers** | Zero-copy, WASM-friendly, compact | Schema definition required, codegen | Conditional fallback |

**Decision:** JSON + Arrow are the primary formats. FlatBuffers is implemented conditionally — if Arrow's dependency tree causes issues in WASM environments, the FlatBuffers path is activated via feature flag. The feature flag and conditional compilation strategy is designed upfront so either path works at compile time.

### Transport Layer

All inter-component communication uses the same serialization format:

```
Agent Loop → UI (via HTTP/WebSocket)
├── JSON for real-time streaming (text/event-stream)
└── Arrow for batch download (full session export)

Agent Loop → External Service (via HTTP)
├── JSON for single requests
└── Arrow for batch requests (multiple messages)

Agent Loop → Monitoring (via telemetry)
└── Arrow for efficient metric collection (token counts, latency)
```

## Rationale

**Why both JSON and Arrow?**  
- JSON for human readability and debugging — developers can `cat` a session file and understand it
- Arrow for performance and analytics — batch processing and columnar queries are significantly faster
- Having both formats means no trade-off — use the right format for the job

**Why NDJSON for storage?**  
- Append-only friendly — just add a line
- Each line is independently parseable — can read partial files
- Compatible with ripgrep/fff for fast text search over session content
- Simple — no file header, no complex format

**Why JSON-encoded content within Arrow?**  

**TODO**: But then we loose all the benefits of arrow for a well structured message that can be easily searched,i get somethings should be json encoded and stuffed into a `content` column, infact we can still stuff the data in the `content` column, but we should extract the sensible fields, scru128, title, summary, whatever makes sense that can be placed into a column on its own for easy search, change, reference.

- Content is heterogeneous (text, tool calls, thinking, images) — encoding as JSON within Arrow preserves the structure
- Arrow schema has a `content` column (UTF8) that holds JSON-encoded content
- This avoids complex nested Arrow types while preserving full content fidelity

## Alternatives Considered

### JSON only
- **Pros:** Simplest, human-readable
- **Cons:** Slow for batch processing, verbose for transport
- **Insufficient because:** Analytics and batch operations need columnar format

### Arrow only
- **Pros:** Fast, efficient
- **Cons:** Not human-readable, requires Arrow library to inspect
- **Insufficient because:** Developers need to be able to `cat` and `jq` session files

### Protocol Buffers
- **Pros:** Efficient, versioned schemas
- **Cons:** Requires code generation, not human-readable, less common in Rust ecosystem
- **Rejected because:** Arrow provides columnar analytics that Protobuf does not

### MessagePack
- **Pros:** Binary, compact, fast
- **Cons:** Not human-readable, not columnar
- **Rejected because:** Arrow's columnar format is better for analytics
