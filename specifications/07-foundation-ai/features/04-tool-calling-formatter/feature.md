---
workspace_name: "ewe_platform"
spec_directory: "specifications/07-foundation-ai"
feature_directory: "specifications/07-foundation-ai/features/04-tool-calling-formatter"
this_file: "specifications/07-foundation-ai/features/04-tool-calling-formatter/feature.md"

feature: "tool-calling-formatter"
description: "Stateless ToolFormatter trait as Model associated type for bidirectional tool definition/call/response formatting across providers"
status: unapproved
priority: high
depends_on:
  - "00c-openai-provider"
  - "07-anthropic-provider"
  - "01-llamacpp-integration"
estimated_effort: "medium"
created: 2026-04-21
last_updated: 2026-04-28
author: "Main Agent"

tasks:
  completed: 0
  uncompleted: 8
  total: 8
  completion_percentage: 0%
---

## Learnings from pi-mono

pi-mono is a TypeScript multi-provider AI abstraction layer that implements tool calling adapters for OpenAI (both Chat Completions and Responses APIs), Anthropic Messages, Google Gemini, AWS Bedrock Converse, and Mistral. Each provider has its own set of inline formatting functions embedded directly in the provider implementation file.

### Tool Definition Formatting — `convertTools()`

Every provider in pi-mono implements a `convertTools(tools: Tool[])` function that translates from an internal tool representation (name, description, parameters as JSON Schema) into the provider's native tool schema. The implementations are structurally similar — iterate over tools, map fields — but the output shapes differ significantly:

**OpenAI Chat Completions** wraps each tool in `{ type: "function", function: { name, description, parameters }, strict: false }`. The `strict: false` is always hardcoded because pi-mono determined that strict schema compliance causes more problems than it solves — models often produce slightly non-conforming output and strict mode would reject it. This is a learned constraint: strict mode is theoretically good but practically fragile.

**OpenAI Responses API** uses a flatter format: `{ type: "function", name, description, parameters, strict }` — no nested `function` wrapper. This means the same internal tool definition produces two different output shapes depending on which OpenAI API is in use, and pi-mono handles this with two separate conversion functions (`convertTools` vs `convertResponsesTools`).

**Anthropic** strips the wrapper entirely: `{ name, description, input_schema: { type: "object", properties, required } }`. Additionally, when operating in OAuth/MCP mode for Claude Code compatibility, tool names get prefixed with `mcp_` — and this prefixing must be applied consistently to both tool definitions AND all historical tool_use/tool_result blocks in the message history. The tool name transformation is bidirectional: `toClaudeCodeName()` maps internal names to Claude Code names using a known tool lookup table, and `fromClaudeCodeName()` reverses the mapping on incoming tool calls.

**Google Gemini** wraps all tools inside a `functionDeclarations` array: `[{ functionDeclarations: [{ name, description, parametersJsonSchema }] }]`. The `parametersJsonSchema` field name (vs `parameters`) is a Google-specific quirk. Google also doesn't return tool call IDs in responses, so pi-mono generates them locally using a counter pattern: `${tool_name}_${timestamp}_${++counter}`.

**AWS Bedrock** wraps each tool in `{ toolSpec: { name, description, inputSchema: { json: parameters } } }` and bundles tool choice into the same structure. The `toolChoice` enum maps differently: `auto` → `{ auto: {} }`, `any` → `{ any: {} }`, specific tool → `{ tool: { name } }`.

**Key insight**: Every provider's `convertTools` is pure, stateless, and deterministic. There's no mutable state, no dependency on prior calls, no configuration beyond the tool definitions themselves. The function is a simple mapping from one schema to another.

### Tool Call Extraction — Structured API Providers

For API providers, tool calls arrive in structured response fields. pi-mono handles this through streaming event processors that accumulate incremental data into complete tool calls.

**OpenAI Chat Completions** delivers tool calls in `delta.tool_calls[]` within SSE chunks. Each chunk carries an `index` field, and the first chunk contains the `id` and `function.name` while subsequent chunks only contain incremental `function.arguments` fragments. pi-mono maintains a `currentBlock` state object that accumulates `partialArgs` by string concatenation, calling `parseStreamingJson(partialArgs)` on each delta to get best-effort parsed objects. The critical detail: arguments arrive as JSON **strings**, not parsed objects, and the incremental fragments are not valid JSON until the final chunk. The `parseStreamingJson` function handles this by attempting to parse on each accumulation, returning the last successfully parsed object when the current fragment is incomplete.

**OpenAI Responses API** is structurally different — tool calls arrive as top-level `function_call` items in the response output, not nested in a `tool_calls` array. The streaming uses different event types: `response.output_item.added` signals a new tool call starting, `response.function_call_arguments.delta` provides incremental fragments, and `response.function_call_arguments.done` signals completion with the final complete arguments. The composite ID format `call_id|item_id` requires splitting to extract the actual tool call identifier for dispatch.

**Anthropic** delivers tool calls as `content_block` entries with `type: "tool_use"`. The streaming lifecycle is `content_block_start` (provides `id`, `name`), `content_block_delta` with `input_json_delta` (provides `partial_json` fragments), and `content_block_stop` (signals completion). Arguments arrive as incremental JSON fragments, same pattern as OpenAI. Tool IDs must match `[a-zA-Z0-9_-]+` and be max 64 characters — pi-mono sanitizes with `id.replace(/[^a-zA-Z0-9_-]/g, "_").slice(0, 64)`.

**Google Gemini** is the odd one out — it returns tool calls as `functionCall` parts with **parsed JSON objects** as arguments, not JSON strings. This means no incremental accumulation is needed; the arguments are already structured. However, Google doesn't return tool call IDs, so they must be generated locally.

**AWS Bedrock** follows the same pattern as Anthropic — `content_block_start` with `toolUse`, incremental `toolUse.input` fragments, same JSON accumulation pattern.

### Tool Call Extraction — Text-Based Models

pi-mono does not implement text-based tool calling because it only targets API providers. This is a gap our system must fill — local models (llama.cpp, Candle) don't have structured tool calling APIs. Tool calls are embedded in the model's text output and must be parsed using pattern matching. The reference spec documents the formats for Hermes/Qwen/Longcat (backtick-JSON-backtick), Llama 3/4 (raw JSON objects), DeepSeek V3/V3.1 (unicode tokens with markdown JSON), Kimi K2 (section begin/end tokens), GLM 4.5/4.7 (backtick arg_key/arg_value pairs), and Qwen3-Coder (XML-style function/parameter tags).

The fundamental difference: for API providers, extraction is deserialization (parsing known JSON structures). For text-based models, extraction is parsing (discovering and extracting tool calls from free-form text). The return shape must accommodate this — text-based extraction needs to return both the extracted tool calls AND the remaining text content (non-tool-call portions of the output).

### Tool Response Formatting — `format_tool_results()`

When tool execution completes, the results must be formatted back into the provider's expected message structure for the next turn. This is the most complex cross-cutting concern because it involves message history transformation, not just single-field formatting.

**OpenAI** formats tool results as `{ role: "tool", tool_call_id, content }`. The `name` field is conditionally added when `compat.requiresToolResultName` is true (certain models behind the OpenAI API require it).

**Anthropic** formats tool results as `{ type: "tool_result", tool_use_id, content, is_error }`. Critically, Anthropic requires strict role alternation (user/assistant/user/assistant), so consecutive tool results must be merged into a single user message. If a tool_use block has no corresponding tool_result (orphaned), it must be stripped from history. pi-mono's `transformMessages()` layer handles this through a two-pass algorithm: first pass collects all tool call IDs from assistant messages, second pass verifies each has a corresponding tool_result and inserts synthetic empty results for orphans.

**Google** formats tool results as `{ functionResponse: { name, response: { output: value } } }` with optional `parts` for multimodal responses. The response value structure differs — success wraps in `{ output: value }`, error wraps in `{ error: value }`.

**AWS Bedrock** formats tool results as `{ toolResult: { toolUseId, content: [{ text: value }], status: "success"|"error" } }`. All consecutive tool results are collected into a single user message, same as Anthropic.

### Message Transformation Layer — `transformMessages()`

pi-mono implements a centralized message transformation layer (`packages/ai/src/providers/transform-messages.ts`) that normalizes message history across providers. This is separate from tool formatting but intimately related — it handles:

1. **Tool ID normalization**: Each provider has different tool ID constraints. Anthropic requires `[a-zA-Z0-9_-]+` max 64 chars, Mistral requires exactly 9 chars (hash-based), OpenAI Responses API uses pipe-separated composite IDs. The transformation accepts a `normalizeToolCallId` callback specific to each provider.

2. **Thinking block conversion**: Redacted thinking blocks (opaque encrypted content) are only valid for the same model and must be dropped for cross-model turns. Regular thinking blocks are converted to plain text for providers that don't support native thinking.

3. **Synthetic tool result insertion**: When an assistant message contains tool_use blocks without corresponding tool_results, the transformation inserts synthetic empty results to prevent API errors.

4. **Error/abort filtering**: Assistant messages with `stopReason: "error"` or `"aborted"` are skipped entirely because they represent incomplete turns that, if replayed, cause API errors.

### Streaming Architecture

pi-mono uses an event protocol for streaming tool calls: `toolcall_start` (new tool call begins with id, name), `toolcall_delta` (incremental argument fragment), `toolcall_end` (complete tool call with assembled arguments). These events are pushed to a stream as SSE chunks arrive, allowing consumers to react to tool call lifecycle events incrementally. The stream accumulates a `partial` message object that represents the current state of the assistant's output.

## Learnings from hermes-agent

hermes-agent is a Python agent framework that uses an adapter pattern to convert between OpenAI-format tools/messages and Anthropic format. Unlike pi-mono which has inline formatting per provider, hermes-agent centralizes the conversion logic in dedicated adapter modules.

### The Adapter Pattern — `anthropic_adapter.py`

hermes-agent implements three core functions for the Anthropic adapter:

**`convert_tools_to_anthropic()`** converts OpenAI-format tool definitions (`{ type: "function", function: { name, description, parameters } }`) to Anthropic format (`{ name, description, input_schema }`). Tool name canonicalization happens here — in OAuth mode, tool names get prefixed with `mcp_` for Claude Code compatibility. This is the same pattern pi-mono uses, confirming it's a real-world requirement, not a pi-mono quirk.

**`convert_messages_to_anthropic()`** is the heavyweight function. It processes an entire message history through multiple transformation stages:
1. Extract thinking blocks from `reasoning_details` metadata
2. Convert tool_calls to tool_use blocks: `{ type: "tool_use", id: sanitized_id, name, input: parsed_args }` — critically, the input is converted from JSON **string** to parsed **object**, which is what Anthropic expects
3. Merge consecutive tool_results into a single user message (Anthropic requires strict role alternation)
4. Enforce role alternation by inserting empty user/assistant messages when needed
5. Strip orphaned tool_use blocks (tool calls with no corresponding result)
6. Strip orphaned tool_result blocks (results with no corresponding tool call)
7. Replace empty assistant content with placeholder text

The merge-consecutive-tool-results logic is particularly important: Anthropic's API rejects requests where two user messages appear consecutively, which happens naturally when multiple tool results need to be sent back. hermes-agent scans the message sequence, identifies runs of consecutive tool_result messages, and merges them into a single user message with multiple content blocks.

**`normalize_anthropic_response()`** extracts tool_use blocks from Anthropic's response content and converts them to OpenAI-style tool_calls for internal consumption. This is the reverse direction — it takes `{ type: "tool_use", id, name, input: { ... } }` and produces `{ id, type: "function", function: { name, arguments: json.dumps(input) } }`. The key transformation: Anthropic returns arguments as a parsed JSON **object**, but the internal representation expects a JSON **string** (matching OpenAI's format). So `json.dumps(block.input)` serializes the object back to a string.

### Tool Argument Coercion — `model_tools.py`

hermes-agent implements `coerce_tool_args(tool_name, args)` which addresses a pervasive problem: LLMs frequently return numbers and booleans as strings when tool calling. The function takes raw string arguments and the tool's JSON Schema definition, then:

1. Iterates over each property in the schema's `properties` object
2. Checks the declared type (`"integer"`, `"number"`, `"boolean"`, `"string"`)
3. Coerces the string value to the appropriate type:
   - `"integer"` → `int(value)` with overflow handling
   - `"number"` → `float(value)` with decimal preservation
   - `"boolean"` → parses `"true"`, `"True"`, `"yes"`, `"1"` → `True`; `"false"`, `"False"`, `"no"`, `"0"` → `False`
4. Handles union types (e.g., `["integer", "string"]`) by trying each type in order
5. Preserves original values when coercion fails (never throws — degrade gracefully)

The `_coerce_number()` helper is defensive: it tries `float()` first, then checks if the result is a whole number (`f == int(f)`). If the schema requires an integer and the value has decimals, it returns the original string rather than truncating. The `_coerce_boolean()` helper is generous — it accepts multiple truthy/falsey string representations because different models express booleans differently.

### The Prompt Builder — `prompt_builder.py`

hermes-agent assembles system prompts by combining skills index, context files, and tool-use enforcement guidance. For models that don't natively support tool calling (open-source models), it appends explicit tool-use instructions to the system prompt describing the expected format. This is critical for text-based models — without prompt-level guidance, they won't produce parseable tool calls.

## Cross-Project Insights: Why This Matters

### The Three-Facet Problem

Both projects, despite their different architectures (pi-mono's inline functions vs hermes-agent's adapter modules), solve the same three-facet problem:

**Facet 1 — Outgoing: Tool Definition Formatting.** Convert internal tool definitions (name, description, parameters as JSON Schema) into the provider's native tool schema. This is always a pure, stateless, deterministic mapping. The complexity is in knowing each provider's schema quirks — field names, wrapper structures, required vs optional fields.

**Facet 2 — Incoming: Tool Call Extraction.** Parse the provider's response to extract structured tool calls. For API providers, this is deserialization of known JSON structures with incremental accumulation for streaming. For text-based models, this is parsing — discovering tool calls embedded in free-form text using pattern matching, while preserving the non-tool-call text content. This is where the real complexity lives: streaming JSON fragment accumulation, ID normalization, type coercion from string representations to typed values.

**Facet 3 — Round-Trip: Tool Response Formatting.** Convert tool execution results back into the provider's expected message structure for multi-turn tool calling conversations. This is not just formatting a single value — it's transforming message history. Anthropic requires strict role alternation and merges consecutive tool results into a single user message. OpenAI uses simple tool-role messages. Text-based models may need the result appended as continuation text to the model's output.

### What's Universal vs Provider-Specific

**Universal patterns** (both projects agree):
- Tool definitions always map: name → name, description → description, parameters → schema object
- Tool calls always decompose into: id, name, arguments (as either JSON string or parsed object)
- Tool results always reference: the tool call id, the tool name, the result content
- Stop reasons always map to: normal completion, length exceeded, tool use requested, error

**Provider-specific quirks** (what makes a unified trait necessary):
- Tool ID constraints: Anthropic (alphanumeric + `-` + `_`, max 64), Mistral (exactly 9 chars, hash-based), OpenAI (any string), Google (no IDs returned, generate locally)
- Argument representation: OpenAI returns JSON **string**, Anthropic returns parsed **object**, Google returns parsed **object**
- Role alternation: Anthropic enforces strict user/assistant alternation, OpenAI does not
- Tool result placement: Anthropic merges into single user message, OpenAI uses separate tool-role messages
- Streaming event names: Anthropic uses `content_block_delta` with `input_json_delta`, OpenAI uses `delta.tool_calls` with incremental `function.arguments`

### What We Should NOT Build

Both projects implement things we should not replicate:

**Plugin registry** — pi-mono doesn't have one (functions are inline per provider), hermes-agent doesn't have one (adapters are module imports). A runtime registry for formatters is overengineering when the formatter is a compile-time property of the model type.

**Message transformation layer** — pi-mono's `transformMessages()` is a heavyweight centralized layer that handles thinking blocks, role alternation, synthetic tool results, and error filtering. This is message history management, not tool formatting. Our providers already handle this in their `build_*_request` functions (e.g., `build_anthropic_request` converts Messages to provider-native messages). The formatter should not duplicate this.

**Type coercion as a trait method** — hermes-agent's `coerce_tool_args()` operates on string-to-string coercion guided by JSON Schema. Our `extract_tool_call` receives already-parsed JSON and converts to `HashMap<String, ArgType>` — the coercion is inherent to the extraction process, not a separate concern.

**Tool call ID normalization as a trait method** — this is an internal implementation detail. The formatter should produce valid IDs for its provider, and the provider should sanitize incoming IDs as needed. External callers should not be calling normalization functions.

### What We Should Build

A minimal stateless trait with exactly three methods, each addressing one facet of the tool calling lifecycle:

1. **`format_tools()`** — outgoing: our `Tool[]` → provider schema (`serde_json::Value`)
2. **`extract_tool_calls()`** — incoming: provider response → `Vec<ToolCall>` + remaining text
3. **`format_tool_response()`** — round-trip: our `Messages::ToolResult` → provider message structure

The trait is stateless (no mutable fields, no configuration, no dependency on provider instances). Each method is a pure function from input to output. Zero-sized types via `Default` so any code can get an instance without plumbing generics.

# Tool Calling Formatter

## Overview

A stateless `ToolFormatter` trait attached to `Model` as an associated type. Each provider implements the three facets of tool calling: format tool definitions for outgoing requests, extract tool calls from incoming responses, and format tool results for multi-turn round-trips.

Design is derived from observing both pi-mono and hermes-agent: both projects solve the same three-facet problem (outgoing definitions, incoming calls, round-trip responses) but with different architectures. We distill this into a minimal three-method trait — no registry, no plugin system, no message transformation layer, no standalone coercion function.

## Trait

```rust
pub trait ToolFormatter: Default + Send + Sync {
    /// Convert internal `Tool[]` definitions → provider-specific tool schema.
    ///
    /// For API providers: produces the JSON structure expected in the `tools`
    /// field of the request (e.g., OpenAI `{type: "function", ...}`,
    /// Anthropic `{name, description, input_schema}`).
    ///
    /// For text-based models (llama.cpp, Candle): produces a JSON array of
    /// available tools for the calling side to inspect.
    fn format_tools(&self, tools: &[Tool])
        -> Result<ToolFormatResult, ErrorTrace<ToolCallingError>>;

    /// System prompt instructions for tool calling format.
    ///
    /// Returns `Some(instructions)` when the model needs guidance on how to
    /// format tool calls (text-based models, no tool-aware template).
    /// Returns `None` for API providers (Anthropic, OpenAI) since their
    /// native tool format handles it.
    ///
    /// The provider prepends this to the system prompt when present.
    fn tool_calling_instructions(&self) -> Option<String>;

    /// Extract tool calls from provider response text.
    ///
    /// For API providers: parses structured tool call fields from the response
    /// JSON string (e.g., OpenAI `message.tool_calls[]`, Anthropic content
    /// blocks with `type: "tool_use"`).
    ///
    /// For text-based models: parses tool calls embedded in text output using
    /// pattern matching (XML tags, raw JSON objects, etc.).
    /// The `remaining_text` field preserves non-tool-call portions of the
    /// output (assistant's prose, explanations, etc.).
    ///
    /// **Accumulation is the provider's responsibility.** The formatter is
    /// stateless — it receives complete text and parses it. For API providers
    /// this means the provider has assembled the full response body; for
    /// text-based models with streaming it means the provider has accumulated
    /// all tokens until generation completed. The formatter does not see
    /// partial data or handle streaming.
    fn extract_tool_calls(&self, response: &str)
        -> Result<ExtractResult, ErrorTrace<ToolCallingError>>;

    /// Format a tool execution result into provider message structure.
    ///
    /// This is used when sending tool results back to the model for multi-turn
    /// tool calling. Each provider expects tool results in a different format:
    ///
    /// - OpenAI: `{ role: "tool", tool_call_id, content }`
    /// - Anthropic: `{ type: "tool_result", tool_use_id, content }` (merged
    ///   with other tool results into a single user message)
    /// - Text-based: may be appended as continuation text to the model output
    ///
    /// Takes `Messages::ToolResult` directly — no need to re-invent the
    /// parameter list. Returns the provider-specific message structure as
    /// `serde_json::Value`. The caller is responsible for inserting this
    /// into the correct position in the message history (the provider's
    /// `build_*_request` function handles that).
    fn format_tool_response(
        &self,
        result: &Messages,
    ) -> Result<serde_json::Value, ErrorTrace<ToolCallingError>>;
}
```

## Result Types

```rust
/// Output of `format_tools()` — provider tool schema plus any system prompt additions.
pub struct ToolFormatResult {
    /// Provider-specific tool definitions as JSON.
    /// For API providers: the array/object to include in the `tools` field.
    /// For text-based models: may be `Null` (no structured tools) with
    /// `system_prompt_additions` containing the tool calling format instructions.
    pub schema: serde_json::Value,
    /// Additional system prompt text required for tool mode.
    /// e.g., Anthropic OAuth mode needs tool-use guidance,
    /// text-based models need format instructions.
    pub system_prompt_additions: Option<String>,
}

/// Output of `extract_tool_calls()` — extracted tool calls plus remaining text.
pub struct ExtractResult {
    /// Extracted tool calls in unified `ModelOutput::ToolCall` format.
    pub calls: Vec<ModelOutput>,
    /// Non-tool-call portions of the response.
    /// For API providers: assistant's text content alongside tool calls.
    /// For text-based models: prose/explanations surrounding tool call blocks.
    pub remaining_text: Option<String>,
    /// Whether the model intends to use tools (vs pure text response).
    /// Derived from presence of parseable tool calls.
    pub has_tool_calls: bool,
}
```

## Model Integration

```rust
pub trait Model {
    type Formatter: ToolFormatter;
    // ...
}
```

Models declare their formatter via an associated type. Any code that needs to format tools uses `<M::Formatter as ToolFormatter>::default()` — no generics needed everywhere, since the trait is stateless (zero-sized).

## Provider Strategy

| Provider | Formatter | Notes |
|---|---|---|
| `AnthropicModel` | `AnthropicFormatter` | Hardcoded — Anthropic only works with its own format |
| `OpenAIModel<F>` | `OpenAIFormatter` (default) | Generic `F: ToolFormatter` — caller picks the formatter |
| `LlamaCppModel` | `TextBasedFormatter` | Hardcoded — local inference, text-based parsing |
| `CandleModel` | `TextBasedFormatter` | Hardcoded — local inference, text-based parsing |

OpenAI is the only provider that needs a generic because it can act as a proxy to many backends (llama.cpp server, vLLM, Ollama, OpenRouter). When used natively with OpenAI it defaults to `OpenAIFormatter`; when used as a proxy to Anthropic-compatible endpoints the caller supplies `AnthropicFormatter`.

## Format Reference

Each provider's native tool format:

**Anthropic**
```json
// Tool definition (input)
{"name": "get_weather", "description": "...", "input_schema": {"type": "object", "properties": {"location": {"type": "string"}}}}

// Tool call (output content block)
{"type": "tool_use", "id": "tool_abc", "name": "get_weather", "input": {"location": "Paris"}}

// Tool result (round-trip, in user message content array)
{"type": "tool_result", "tool_use_id": "tool_abc", "content": [{"type": "text", "text": "22°C and sunny"}]}
```

**OpenAI Chat Completions**
```json
// Tool definition (input)
{"type": "function", "function": {"name": "get_weather", "description": "...", "parameters": {"type": "object", "properties": {"location": {"type": "string"}}}}}

// Tool call (output, in message.tool_calls[])
{"id": "call_abc", "type": "function", "function": {"name": "get_weather", "arguments": "{\"location\":\"Paris\"}"}}

// Tool result (round-trip, as separate message)
{"role": "tool", "tool_call_id": "call_abc", "content": "22°C and sunny"}
```

**llama.cpp / local models**
- No structured `tools` API — tool calling is mediated by Jinja chat templates
- If template supports tools: provider passes tools as template parameter
- If template doesn't support tools: formatter returns system prompt instructions
- Tool calls embedded in text output, extracted via regex pattern matching
- Primary format: XML-wrapped JSON `<ToolCall>{"name":"...","arguments":{...}}</ToolCall>`
- Disambiguation: validate tool names against known tools to avoid false positives
- Streaming: provider accumulates tokens, formatter parses on generation complete
- Tool results formatted as `(role, content)` tuples for chat template rendering

## Error Types

Uses `foundation_errstacks` for context-aware error handling. The project convention is to use enums as the default error type — each variant carries the fields relevant to that failure mode.

```rust
use derive_more::{Display, Error, From};
use foundation_errstacks::{ErrorTrace, PlainResultExt};

#[derive(Debug, Display, Error, From)]
pub enum ToolCallingError {
    #[display("failed to format tool '{tool_name}': {reason}")]
    Format { tool_name: String, reason: String },
    #[display("failed to extract tool calls: {reason}")]
    Extract { reason: String },
    #[display("failed to format result for '{tool_name}': {reason}")]
    Response { tool_name: String, reason: String },
}
```

All trait methods return `ErrorTrace<ToolCallingError>`. Callers attach context using `.attach()` from `PlainResultExt`:

```rust
// Provider attaching model and tool context
formatter.format_tools(&tools)
    .map_err(|trace| trace.attach("model=llamacpp"))
    .map_err(|trace| trace.attach("tool_count=3"))

// Extractor attaching raw response snippet for debugging
formatter.extract_tool_calls(&response_text)
    .map_err(|trace| trace.attach(format!("response_preview={}", &response_text[..100])))
```

This aligns with the project's convention of enum-first error types with `derive_more::From` + manual `Display` (no `thiserror`), while allowing the full error trace to propagate through provider boundaries.

## Formatter Implementation Details

### AnthropicFormatter

**`format_tools()`**: Converts `Tool[]` to Anthropic's `{name, description, input_schema: {type: "object", properties: {...}}}` format. Iterates over each tool's `arguments`, matches `Args::Named(key, value)` to extract parameter names and infer JSON Schema types from `ArgType` variants (`Float32`/`Float64` → `"number"`, integer types → `"integer"`, everything else → `"string"`). Returns `Ok(ToolFormatResult { schema: Value::Array, system_prompt_additions: None })`. On failure, returns `Err(ErrorTrace<ToolCallingError::Format>)` with the failing tool name and reason.

**`extract_tool_calls()`**: Parses the `MessagesResponse` JSON and iterates over `content` blocks. For each `ToolUse` block, extracts `id`, `name`, and `input` (already a parsed JSON object — this is the Anthropic difference vs OpenAI which returns JSON strings). Converts the object to `HashMap<String, ArgType>` using the same `json_value_to_arg_type` conversion. Text and thinking blocks become `remaining_text`. Sets `has_tool_calls = true` if any `ToolUse` blocks were found.

**Streaming**: The provider's stream iterator accumulates tool call fragments across `content_block_start`, `content_block_delta`, and `content_block_stop` events. When the stream completes (`message_stop`), the provider assembles the complete response JSON and calls `formatter.extract_tool_calls()` on it. The formatter does not handle incremental parsing.

**`format_tool_response()`**: Takes `Messages::ToolResult` and produces `{"type": "tool_result", "tool_use_id": id, "content": [{"type": "text", "text": result}], "is_error": bool}`. The caller must merge this into the user message's content array alongside any other tool results — Anthropic requires all tool results in a single user message for role alternation. This merge is the provider's responsibility in `build_anthropic_request`, not the formatter's.

### OpenAIFormatter

**`format_tools()`**: Converts `Tool[]` to OpenAI's `[{type: "function", function: {name, description, parameters: {...}}}]` format. Same type inference from `ArgType` as Anthropic. Returns `Ok(ToolFormatResult { schema: Value::Array, system_prompt_additions: None })`. On failure, returns `Err(ErrorTrace<ToolCallingError::Format>)`.

**`extract_tool_calls()`**: Parses `ChatCompletionResponse` JSON, extracts `choices[0].message.tool_calls[]`. Each tool call has `id`, `function.name`, and `function.arguments` (a JSON **string**, not parsed object — this is the OpenAI difference vs Anthropic). Deserializes the arguments string to `HashMap<String, ArgType>`. If no tool calls present, `remaining_text` gets the message's `content` field. Sets `has_tool_calls` based on presence of `tool_calls` in the response.

**Streaming**: The provider accumulates tool call deltas across SSE chunks. The incremental `function.arguments` fragments are concatenated and parsed on stream completion. The provider assembles the final complete response and calls `formatter.extract_tool_calls()` — the formatter does not handle incremental parsing.

**`format_tool_response()`**: Takes `Messages::ToolResult` and produces `{"role": "tool", "tool_call_id": id, "content": result}`. Simple — OpenAI doesn't require merging; each tool result is a separate message.

### TextBasedFormatter — Text-Based Model Tool Calling

A single shared formatter for models that do NOT have structured tool calling APIs. Used by both llama.cpp (via `llama.cpp server`'s `/v1/messages` endpoint or direct inference) and Candle (local GGUF/safetensors inference). Other providers can wrap or customize it if they need slightly different formats.

#### How llama.cpp Server Handles Tool Calling (Observed Behavior)

The llama.cpp server's OpenAI-compatible `/v1/messages` endpoint implements tool calling through the model's Jinja chat template. When a request includes a `tools` array, the server:

1. Converts tool definitions to a format the chat template expects (the template itself defines how tools are represented — some templates use special tokens, others use XML-like tags)
2. Passes tools as a `"tools"` parameter to the Jinja template alongside messages
3. The template renders tool instructions into the prompt
4. The model generates text, and if it decides to use a tool, outputs text matching the template's tool call format
5. The server's response parser looks for tool call markers in the generated text and converts them to OpenAI-compatible `tool_calls[]` in the response JSON

**Critical observation**: The tool calling format is determined by the **chat template**, not by llama.cpp itself. A Llama 3.1 model's template uses `<|python_tag|>` prefixes and JSON. A Qwen model's template may use different markers. A model without tool-aware template support won't produce parseable tool calls at all.

#### Our llama.cpp Integration (Current State)

Our `llamacpp.rs` uses `LlamaChatMessage` (simple `role + content` struct from `infrastructure/llama-cpp/src/model.rs:78-94`) and `LlamaChatTemplate` (wrapper around Jinja template string). The current `apply_chat_template` function converts `Messages` to `(role, content)` tuples:

- `Messages::User` → `("user", text)`
- `Messages::Assistant { ToolCall }` → `("assistant", "[Tool call: name(args_str)]")` — this is a lossy text representation
- `Messages::ToolResult` → `("tool", text)` — just the result content

There is **no structured tool calling** — everything becomes text strings that the chat template processes. The current approach works for models whose templates tolerate arbitrary text content, but it doesn't leverage templates that have native tool calling support.

#### Candle's Current State

Candle has **no tool-related code at all**. It's the most primitive — just raw token generation with no chat template support, no role-based messages, no tool calling. Any tool calling support for Candle must be built from scratch.

#### Architecture Decision: Chat Template vs System Prompt

Two approaches exist for instructing text-based models on tool calling format:

**Approach A — Chat Template (preferred when available)**: If the model's GGUF file includes a Jinja chat template with tool calling support, we pass tools as a template parameter and let the template render the correct format. The template knows the model's native tool calling convention (e.g., Llama 3.1's `<|python_tag|>{"name":"...","arguments":{...}}`). This is what llama.cpp server does.

**Approach B — System Prompt Injection (fallback)**: If the model has no tool-aware template (or no template at all, as with Candle), we inject tool calling format instructions into the system prompt. The formatter produces a text block describing the expected format, and the provider prepends it to the system prompt before template application.

**Our strategy**: `TextBasedFormatter` carries a JSON Schema that defines the expected tool call structure. A `tool_calling_instructions() -> Option<String>` method on the formatter generates human-readable instructions from that schema. The provider calls this when no chat template supports tools and prepends the result to the system prompt:

```rust
// On the ToolFormatter trait
fn tool_calling_instructions(&self) -> Option<String>;
```

- When `Some(instructions)` — provider prepends to system prompt for system prompt injection
- When `None` — provider uses the chat template route or has no tool support

This means the JSON Schema is the single source of truth — it drives both `format_tools()` (the available tools list) and `extract_tool_calls()` (the regex validator), while `tool_calling_instructions()` derives the human-readable guidance the model needs to produce valid output.

#### Custom Wrappers — Model-Specific Formats

`TextBasedFormatter` defaults to `<ToolCall>...</ToolCall>` XML-wrapped JSON. If a model was pre-trained on a different convention (e.g., Qwen3-Coder's `<function>`/`<parameter>` tags), wrap it with a custom schema:

```rust
// Default: XML-wrapped JSON
TextBasedFormatter::default()

// Wrapped with custom tag format for a specific model
TextBasedFormatter::with_schema(FunctionTagSchema)
```

The wrapper supplies a different tag format and JSON Schema; parsing, instruction generation, and validation all derive from it automatically. The user is responsible for ensuring the format matches what their model's prompt training expects.

#### `format_tools()` — Available Tools List

`TextBasedFormatter` takes the `Tool[]` definitions and produces a JSON representation of the available tools:

```json
[
  {"name": "get_weather", "description": "Get weather for a location.", "parameters": {"location": "string"}},
  {"name": "search", "description": "Search the web.", "parameters": {"query": "string"}}
]
```

The `schema` field in `ToolFormatResult` holds this JSON. `system_prompt_additions` is `None` — the instruction block comes from `tool_calling_instructions()` instead.

Returns `Ok(ToolFormatResult)` on success. Errors are unlikely for text-based format, but if a tool has invalid metadata, returns `Err(ErrorTrace<ToolCallingError::Format>)` with the offending tool name.

#### `tool_calling_instructions()` — System Prompt Format Instructions

Generates the human-readable instruction block from the formatter's JSON Schema:

```
You have access to the following tools:
- get_weather: Get weather for a location. Parameters: location (string)
- search: Search the web. Parameters: query (string)

To use a tool, wrap your call in <ToolCall> tags with valid JSON inside:
<ToolCall>{"name":"tool_name","arguments":{"param":"value"}}</ToolCall>

Only output tool calls when necessary. Do not invent tool calls.
```

Returns `Some(instructions)` when the formatter has instructions to inject. Returns `None` for API-based formatters (Anthropic, OpenAI) since they don't need system prompt injection — their native tool format handles it.

The provider calls this when Approach B applies (no tool-aware template) and prepends the result to the system prompt. This way the JSON Schema is the single source of truth — `format_tools()` lists the available tools, `tool_calling_instructions()` derives the guidance the model needs, and `extract_tool_calls()` validates against the schema.

#### `extract_tool_calls()` — Text Parsing Strategy

The formatter parses tool calls from the model's complete text output. The provider accumulates tokens until generation completes, then passes the full text to the formatter.

Returns `Ok(ExtractResult)` on success (even if zero tool calls found — that's a valid result, not an error). Returns `Err(ErrorTrace<ToolCallingError::Extract>)` only when the response text is malformed in a way that prevents parsing (e.g., unclosed `<ToolCall>` tag with tool-like content that suggests a truncated tool call).

**XML-wrapped JSON (only pattern):**
```
<ToolCall>{"name": "get_weather", "arguments": {"location": "Paris"}}</ToolCall>
```
Regex: matches `<ToolCall>` opening tag, then a JSON object with required `name` field, then `</ToolCall>` closing tag. This is the only accepted format — no fallback to raw JSON or backticks. XML tags provide unambiguous boundaries that don't collide with natural prose.

**Disambiguation strategy**: After regex extraction, the parser validates each match by checking if the extracted tool `name` matches one of the known tools provided in the original request. If the name doesn't match any known tool, the match is treated as prose (not a tool call) and preserved in `remaining_text`. This prevents the parser from misidentifying JSON in the model's explanations as tool calls.

**Interleaved text handling**: When multiple tool calls are found with text between them, the parser splits the input at each tool call boundary. Text before the first tool call, between consecutive tool calls, and after the last tool call are concatenated into `remaining_text`. The `calls` vector contains all extracted tool calls in order of appearance. This preserves the model's full output: the caller can reconstruct the original text if needed, and gets structured tool calls for dispatch.

Example input:
```
I'll check the weather in all three cities.
<ToolCall>{"name": "get_weather", "arguments": {"location": "Paris"}}</ToolCall>
<ToolCall>{"name": "get_weather", "arguments": {"location": "London"}}</ToolCall>
Let me also check Tokyo.
<ToolCall>{"name": "get_weather", "arguments": {"location": "Tokyo"}}</ToolCall>
```

Output:
- `calls`: 3 `ModelOutput::ToolCall` entries (Paris, London, Tokyo)
- `remaining_text`: "I'll check the weather in all three cities.\nLet me also check Tokyo.\n"
- `has_tool_calls`: true

**Streaming integration**: During streaming, the provider emits text tokens as they arrive. When generation completes (EOS token or stop sequence), the provider:
1. Passes the accumulated complete text to `formatter.extract_tool_calls()`
2. If tool calls are found: yields them as additional `Stream::Next` messages after the final text token
3. If no tool calls found: the stream completes with the text message alone

This mirrors how our Anthropic and OpenAI streaming already work — accumulate during stream, parse on completion, emit tool calls after text.

**Candle-specific note**: Candle doesn't use chat templates. Its output is raw token sequences. The `extract_tool_calls` parser works the same way (regex on complete text), but the provider has no template-mediated formatting to fall back on — the system prompt instructions are the only mechanism for guiding tool calling format.

#### `format_tool_response()` — Tool Result as Message

For text-based models, a tool result must be formatted as text the model can understand as a tool response. Takes `Messages::ToolResult` and produces:

```json
{"role": "tool", "name": "get_weather", "content": "22°C and sunny in Paris"}
```

Returns `Ok(Value)` on success. Errors are rare (tool result formatting is straightforward), but if the tool name is empty or the result is malformed, returns `Err(ErrorTrace<ToolCallingError::Response>)` with the tool name and reason.

The provider converts this to a `LlamaChatMessage { role: "tool", content: "[get_weather] 22°C and sunny in Paris" }` and appends it to the conversation. The role `"tool"` signals to the chat template (if present) that this is a tool result. For models without template support, the provider prepends the tool name in brackets as a text convention: `[tool_name] result_text`.

**Multi-turn flow for text-based models**:
1. User message + available tools → provider injects tool instructions into system prompt
2. Model generates text (possibly with tool calls) → provider accumulates tokens
3. Generation completes → provider calls `formatter.extract_tool_calls()` on complete text
4. Tool calls found → provider yields text message, then tool call messages to the stream
5. Caller executes tools → caller sends tool results back via `Messages::ToolResult`
6. Provider converts tool results to messages using `formatter.format_tool_response(&Messages::ToolResult { ... })`
7. Provider re-invokes `generate()` or `stream()` with extended conversation
8. Model generates continuation (final answer or more tool calls)

## Task List

1. **[types]** Define `ToolFormatter` trait + `ToolFormatResult` + `ExtractResult` + `ToolCallingError` enum in `types/mod.rs` using `derive_more` + `foundation_errstacks`
2. **[anthropic]** Implement `AnthropicFormatter` with all three methods + attach to `AnthropicModel`
3. **[openai]** Implement `OpenAIFormatter` with all three methods + `OpenAIModel<F>` with default
4. **[llamacpp]** Attach `TextBasedFormatter` to `LlamaCppModel`
5. **[candle]** Attach `TextBasedFormatter` to `CandleModel`
6. **[text-parsers]** Implement `TextBasedFormatter` regex-based tool call parsers (shared between llama.cpp and Candle)
7. **[integration]** Wire formatters into provider `generate()` and `stream()` methods — replace inline tool formatting with formatter calls
8. **[tests]** Unit tests for all formatters with sample provider responses + streaming accumulation

## Affected files

- `backends/foundation_ai/src/types/mod.rs` — trait, result types, `ToolCallingError` enum
- `backends/foundation_ai/src/backends/anthropic_messages_provider.rs` — `AnthropicFormatter`
- `backends/foundation_ai/src/backends/openai_provider.rs` — `OpenAIFormatter`, `OpenAIModel<F>`
- `backends/foundation_ai/src/backends/llamacpp.rs` — attach `TextBasedFormatter` to `LlamaCppModel`
- `backends/foundation_ai/src/backends/candle.rs` — attach `TextBasedFormatter` to `CandleModel`
- `backends/foundation_ai/tests/tool_calling_formatter.rs` — unit tests

## Design Philosophy

We learn from pi-mono and hermes-agent; we don't replicate them. The learnings above document what we observed, what patterns are worth keeping, and how we build a cleaner native design for our architecture. The trait is minimal because the problem is three facets, not ten methods. The registry is absent because the formatter is a compile-time property of the model type. The message transformation layer is unnecessary because our providers already handle it in their request builders.
