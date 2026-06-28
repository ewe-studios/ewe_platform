# Fundamentals 02 — Agent types: Messages, ModelOutput, and the Stream contract

The type substrate that every provider, tool, and loop component shares.

---

## 1. Messages — the universal conversation format

Every model provider speaks a different wire format (OpenAI's message array,
Anthropic's messages+system, Llama.cpp's chat template). `Messages` is the
**normalized internal representation**:

```rust
pub enum Messages {
    User {
        id: Scru128,
        role: MessageRole::User,
        content: UserModelContent,
        signature: Option<String>,
    },
    Assistant {
        id: Scru128,
        role: MessageRole::Assistant,
        content: ModelOutput,
        stop_reason: StopReason,
        usage: UsageReport,
        signature: Option<String>,
    },
    ToolResult {
        id: Scru128,
        role: MessageRole::ToolResult,
        content: UserModelContent,
        tool_call_id: String,
        signature: Option<String>,
    },
    System { ... },
}
```

**`Scru128`** — sortable, collision-resistant unique IDs (from foundation_compact).
Every message gets one; they're the anchor for tool call matching and streaming
correlation.

## 2. ModelOutput — what the assistant produces

```rust
pub enum ModelOutput {
    Text(TextContent),           // Plain text response
    ToolCall {                   // Function/tool invocation
        id: String,              // Call ID for matching results
        name: String,            // Tool name
        arguments: Option<HashMap<String, ArgType>>,
        signature: Option<String>,
        depends_on: Vec<String>, // DAG dependencies
        execution_hint: ExecutionHint,
    },
    Refusal(RefusalContent),     // Content policy refusal
    Thinking(ThinkingContent),   // Extended reasoning (Claude)
}
```

## 3. UserModelContent — what the user/system provides

```rust
pub enum UserModelContent {
    Text(TextContent),
    Image(ImageContent),
    MultiContent(Vec<ContentBlock>),  // Mixed text + image
}
```

## 4. StopReason — why generation ended

| Variant | Meaning |
|---|---|
| `Stop` | Natural end of response |
| `ToolUse` | Model wants to call tools |
| `MaxTokens` | Hit max_tokens limit |
| `Cancel` | User/interrupt cancelled |
| `Refusal` | Content policy refusal |
| `Error(String)` | Provider error |

## 5. ArgType — tool argument values

```rust
pub enum ArgType {
    Text(String),
    Number(f64),
    Bool(bool),
    Array(Vec<ArgType>),
    Object(HashMap<String, ArgType>),
    Null,
}
```

Maps directly to JSON Schema types. Used for tool argument validation and
serialization.

## 6. Args — JSON Schema for tool arguments

```rust
pub struct Args {
    schema: serde_json::Value,  // JSON Schema object
}
```

Built via `Args::from_value(json!({...}))` or programmatically. Every tool
declares its expected arguments as a schema; the executor validates before
calling.

## 7. ExecutionHint — how to run a tool

```rust
pub enum ExecutionHint {
    Unspecified,
    Blocking,        // Must complete before continuing
    NonBlocking,     // Can run in background
    Idempotent,      // Safe to retry
    SideEffectFree,  // Read-only, no state changes
}
```

The DAG executor uses hints to parallelize independent tool calls.

## 8. ModelState — provider streaming state

```rust
pub enum ModelState {
    GeneratingTokens(Option<UsageReport>),  // Mid-generation
    Finished,                               // Generation complete
    Error(String),                          // Provider error
}
```

Emitted as `Stream::Pending(ModelState)` during generation. Consumers use it
for progress display and error detection.

## 9. VectorMatch — search result

```rust
pub struct VectorMatch {
    pub id: String,
    pub score: f32,  // Higher = more similar
}
```

Returned by vector store search. Score is metric-dependent: cosine similarity
(0..1), L2 distance (0..∞), dot product (-∞..∞).

## 10. VectorEntry — stored vector

```rust
pub struct VectorEntry {
    pub id: String,
    pub vector: Vector,
    pub metadata: VectorMetadata,  // { tags: HashMap<String, String> }
}
```

The unit of vector storage. `Vector` wraps `Vec<f32>` with dimension tracking.
`VectorMetadata` carries arbitrary key-value tags for filtering.
