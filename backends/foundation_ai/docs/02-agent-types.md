# Fundamentals 02 — Agent types: Messages, ModelOutput, SessionRecord, and the Stream contract

The types every provider, tool and loop component shares. All of them live in
`foundation_ai::types` (re-exported from `types/base_types.rs` and
`types/agentic.rs`) unless noted.

---

## 1. `Messages` — the normalized conversation format

Each backend has its own wire format; `Messages` is the internal one. There are
three variants. System prompts are **not** a variant — they ride on
`ModelInteraction::system_prompt`, and loop-injected instructions are
`Messages::User` with `role: MessageRole::System`.

```rust
pub enum Messages {
    User {
        id: Id,                        // scru128, time-ordered
        role: MessageRole,             // User | Agent | System | Tool | Custom(String)
        content: UserModelContent,
        signature: Option<String>,
    },
    Assistant {
        id: Id,
        model: ModelId,
        timestamp: SystemTime,
        usage: UsageReport,
        content: ModelOutput,          // one output per message
        stop_reason: StopReason,
        provider: ModelProviders,
        error_detail: Option<String>,
        signature: Option<String>,
        metadata: Option<Vec<GenerationMetadata>>,  // logprobs, fingerprint, timing
    },
    ToolResult {
        id: Id,
        tool_call_id: String,          // matches ModelOutput::ToolCall::id
        name: String,
        timestamp: SystemTime,
        details: Option<String>,
        content: UserModelContent,
        error_detail: Option<String>,
        signature: Option<String>,
    },
}
```

- `id` is a `foundation_compact::ids::Id` (scru128). Make one with
  `foundation_compact::ids::new_scru128()`; legacy JSON without an `id`
  deserializes with a fresh one. `Messages::id()` reads it from any variant.
- `MessageRole` tells apart human input (`User`), peer agents (`Agent`),
  instructions and loop redirects (`System`) and tool context (`Tool`). It
  serializes to the lowercase strings providers expect.
- A streaming backend yields **one `Messages::Assistant` per chunk**, each
  carrying a single `ModelOutput`. What a text chunk holds depends on the
  backend: the HTTP backends (OpenAI, Responses, Anthropic) send the **whole
  text so far**, so the last chunk is the answer; llama.cpp and Candle send
  **only the new piece**, so the answer is the concatenation. The agent loop
  handles both and persists one merged message per turn.
- `Messages::is_context_overflow(context_window)` recognises overflow errors
  from ~15 providers' error strings, plus "silent" overflow where usage exceeds
  the window.

Building a user message by hand:

```rust
let prompt = Messages::User {
    id: new_scru128(),
    role: MessageRole::User,
    content: UserModelContent::Text(TextContent { content: "Hi".into(), signature: None }),
    signature: None,
};
```

(`agentic::testing::mock_user("Hi")` does this in tests.)

## 2. `ModelOutput` — what the assistant produces

```rust
pub enum ModelOutput {
    Text(TextContent),
    Image(ImageContent),
    ThinkingContent { thinking: String, signature: Option<String> },
    ToolCall {
        id: String,                                   // correlation id
        name: String,
        arguments: Option<HashMap<String, ArgType>>,
        signature: Option<String>,
        depends_on: Vec<String>,                      // ids of calls this waits on
        execution_hint: ExecutionHint,                // Unspecified | Parallel | Sequential
    },
    Embedding { dimensions: usize, values: Vec<f32> },
}
```

There is no refusal variant; refusals surface through `StopReason` /
`GenerationMetadata::RefusalReason`.

## 3. `UserModelContent`

```rust
pub enum UserModelContent {
    Text(TextContent),     // { content: String, signature: Option<String> }
    Image(ImageContent),   // { b64: String, mime_type: MimeType }
}
```

One content item per message — there is no multi-part variant.

## 4. `StopReason`

| Variant | Meaning |
|---|---|
| `Stop` | Natural end |
| `Length` | Hit `max_tokens` |
| `ToolUse` | The model wants tools (`"tool_calls"` maps here too) |
| `Error` | Provider error — see `error_detail` |
| `Aborted` | Cancelled |
| `Message(String)` | Any other provider value, preserved |

## 5. `ArgType` — tool argument values

```rust
pub enum ArgType {
    Text(String),
    Float32(f32), Float64(f64),
    Usize(usize), U8(u8), U16(u16), U32(u32), U64(u64), U128(u128),
    Isize(isize), I8(i8), I16(i16), I32(i32), I64(i64), I128(i128),
    Duration(std::time::Duration),
    JSON(String),                          // raw JSON (arrays, booleans, null, …)
    JSONMap(HashMap<String, ArgType>),     // nested object
}
```

Every backend maps model-supplied JSON the same way
(`types::json_value_to_arg_type`): strings → `Text`, integers → `I64`, other
numbers → `Float64`, and booleans, `null`, arrays and objects → `JSON(text)`.
`ArgType::to_json_value()` goes back. The helpers
`agentic::tool_impl::{arg_bool, arg_usize}` read the common cases.

## 6. `Args` — a JSON Schema plus its validator

```rust
pub struct Args {
    pub schema: serde_json::Value,
    pub validator: foundation_jsonschema::ValidationOptions,
}
```

Build it with:

- `Args::new(scheme::object().required("q", scheme::string()).build())` — the
  `foundation_jsonschema::scheme` builder (preferred)
- `Args::from_value(json!({ "type": "object", ... }))` — a raw schema
- `Args::empty()` — a tool with no arguments

## 7. Tools as the model sees them

```rust
pub struct ToolDefinition {
    pub name: String,
    pub category: String,
    pub description: String,
    pub arguments: Args,
    pub returns: Option<Args>,
}

pub enum Tool {
    SingleCommand(ToolDefinition),
    MultiCommands(String, Vec<ToolDefinition>),  // e.g. "memory" → add/remove/replace
}

pub struct ToolShed {
    pub shed: Option<Tool>,   // the discovery meta-tool
    pub tools: Vec<Tool>,
}
```

Doc 04 covers how these are built and rendered per provider.

## 8. `ModelInteraction` and `ModelParams` — a request

```rust
pub struct ModelInteraction {
    pub system_prompt: Option<String>,
    pub soul: Option<String>,
    pub tools_shed: ToolShed,
    pub messages: Vec<Messages>,
    pub chat_template: Option<String>,   // local backends: override the model's template
    pub tool_choice: Option<ToolChoice>, // Auto | None | Required | Function(..)
}
```

`ModelParams` (default in parentheses): `max_tokens` (2048), `temperature`
(0.7), `top_p` (0.9), `top_k` (40.0), `repeat_penalty` (1.1), `seed`,
`stop_tokens`, `thinking_level`, `thinking_budget`, `cache_retention`,
`output_format` (`Text` / `JsonObject` / `JsonSchema`), `frequency_penalty`,
`presence_penalty`, `logit_bias`. Backends ignore what they don't support.

## 9. `UsageReport`

```rust
pub struct UsageReport {
    pub input: f64,
    pub output: f64,
    pub cache_read: f64,
    pub cache_write: f64,
    pub total_tokens: f64,
    pub cost: UsageCosting,
}
```

Streaming backends report **cumulative** usage on each chunk.

## 10. The `Stream` contract

`Stream<D, P>` comes from `foundation_core::valtron`:

| Item | Meaning |
|---|---|
| `Stream::Init` | Started |
| `Stream::Pending(P)` | Progress, no data |
| `Stream::Next(D)` | A data item |
| `Stream::Spread(items)` | Several items at once |
| `Stream::Wait`, `Stream::Delayed(_)`, `Stream::Ignore` | Scheduling signals — skip them |

At the **model** level (`Model::stream`): `D = Messages`, `P = ModelState`
(`GeneratingTokens(Option<UsageReport>)`, `GeneratingEmbeddings`, `Finished`,
`Error(String)`).

At the **agent** level (`AgentSession::run_turn_stream`):
`D = SessionRecord`, `P = AgentProgress` — the alias is
`agentic::AgentStream`.

## 11. `SessionRecord` — what a turn produces

```rust
pub enum SessionRecord {
    Conversation { message: Messages },
    Retracted { id, reason: String, timestamp },   // drop the assistant output streamed so far
    WorkingMemory { id, facts: Vec<MemoryFact>, version: u64, timestamp },
    Observation  { id, observations: Vec<ObservationEntry>, token_count: u64, timestamp },
    Reflection   { id, reflections: Vec<ReflectionEntry>, generated_at,
                   observation_token_count_before: u64, reflection_token_count_after: u64 },
    FailedAction { error: AgenticError, trace: StructuredErrorTrace },  // never persisted
    Summary      { message_count: u64, usage: TokenSnapshot },          // end of turn
}
```

## 12. `AgentProgress` — progress hints

`Initializing { step }`, `Generating { model, tokens_so_far }`,
`ToolCallRequested { name }`, `ExecutingTools { total, completed }`,
`ToolCallCancelled { id }`, `ProcessingMemory { kind }`,
`FlushingRecords { count }`, `Steering { source }`,
`TurnComplete { usage }`, `SessionEnding`.

`AgentProgress` is `#[non_exhaustive]`, so a `match` needs a `_` arm. Each
variant is an advisory hint about upcoming records, not a framing guarantee.

## 13. `ModelId`

```rust
pub enum ModelId {
    Name(String, Option<Quantization>),
    Alias(String, Option<Quantization>),
    Group(String, Option<Quantization>),
    Architecture(String, Option<Quantization>),
}
```

The second field is a GGUF quantization (`Q4_K_M`, `Q8_0`, …) for local
models; cloud models use `None`. Routing matches on `ModelId::name()` only
(Doc 03).

## Vector types

`VectorEntry`, `VectorMatch` and the `VectorStore` trait belong to
`foundation_vectors`, not this crate. Doc 07 shows how they are used here.
