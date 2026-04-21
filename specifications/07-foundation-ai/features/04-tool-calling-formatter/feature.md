---
workspace_name: "ewe_platform"
spec_directory: "specifications/07-foundation-ai"
feature_directory: "specifications/07-foundation-ai/features/04-tool-calling-formatter"
this_file: "specifications/07-foundation-ai/features/04-tool-calling-formatter/feature.md"

feature: "tool-calling-formatter"
description: "Plugin-based ToolFormatter system supporting OpenAI, Anthropic, llama.cpp, and open-source model tool calling formats with bidirectional input/output formatting"
status: unapproved
priority: high
depends_on:
  - "00c-openai-provider"
estimated_effort: "large"
created: 2026-04-21
last_updated: 2026-04-21
author: "Main Agent"

tasks:
  completed: 0
  uncompleted: 18
  total: 18
  completion_percentage: 0%
---

# Tool Calling Formatter Plugin System

## Overview

Build a plugin-based ToolFormatter abstraction in foundation_ai that knows how to format tool inputs and responses for different LLM tool calling formats. The system handles the fragmentation across providers -- OpenAI, Anthropic, llama.cpp, and various open-source models -- each with their own tool calling conventions, special tokens, and message structures.

The architecture provides:
1. Tool schema formatting -- Convert our internal tool definitions into provider-specific formats
2. Tool call extraction -- Parse raw model output to extract structured tool calls (text-based formats)
3. Tool response formatting -- Format tool results back into provider-expected message structures
4. Plugin registry -- Register new formatters without modifying core code
5. Type coercion -- Coerce LLM-returned string arguments to JSON Schema types

## Local File Paths

- Spec: specifications/07-foundation-ai/features/04-tool-calling-formatter/feature.md
- Types: backends/foundation_ai/src/types/mod.rs
- Errors: backends/foundation_ai/src/errors/mod.rs
- Lib: backends/foundation_ai/src/lib.rs
- Reference (pi-mono): /home/darkvoid/Boxxed/@formulas/src.rust/src.llamacpp/src.AgenticLibraries/src.Pi/pi-mono/
- Reference (hermes-agent): /home/darkvoid/Boxxed/@formulas/src.rust/src.llamacpp/src.All/hermes-agent/

## Iron Laws (inherited from spec-wide requirements.md)

1. No tokio, No async-trait -- All async uses Valtron
2. Valtron-Only Async -- No tokio, no async-trait, no .await
3. Zero Warnings, Zero Suppression -- Fix all clippy/doc warnings, NEVER suppress
4. Error Convention -- derive_more::From + manual Display, no thiserror

## Why This Feature Is Needed

Different model families output tool calls in completely incompatible formats:

- OpenAI: Structured tool_calls field in the API response (JSON objects)
- Anthropic: XML-style blocks in text content (tool_use with input_json_delta streaming)
- Hermes/Qwen/Longcat: backtick-JSON-backtick tags
- Llama 3/4: Raw JSON objects with name + args keys
- Mistral: sentinel token followed by tool names and JSON
- DeepSeek V3/V3.1: Special unicode tokens with markdown-wrapped JSON
- Kimi K2: Section begin/end tokens wrapping individual tool call blocks
- GLM 4.5/4.7: backtick-arg_key/arg_value pairs instead of JSON
- Qwen3-Coder: XML-style nested function/parameter tags

Our system needs a single internal representation of tools and tool calls, with formatters that translate to/from each provider format. This is what the hermes-agent project already solves in Python -- we need the equivalent in Rust for foundation_ai.

## Reference Source Code Analysis

### pi-mono (TypeScript) - Provider Tool Handling

pi-mono implements a multi-provider AI abstraction where each provider (OpenAI, Anthropic, Google, Mistral, Bedrock) has its own tool calling adapter.

#### Core Types
File: packages/ai/src/types.ts (pi-mono)

Tool call representation:
- ToolCall: { id: string, type: string, function: { name: string, arguments: string } }
- Tool: { name: string, description: string, parameters: JSONSchema }
- ToolResultMessage: { role: "tool", tool_call_id: string, content: string }
- AssistantMessage: { role: "assistant", content: string | null, tool_calls?: ToolCall[] }

Provider enums: KnownApi, KnownProvider cover OpenAI, Anthropic, Google, Mistral, Bedrock, and others.

Stream event types for tool calls: toolcall_start, toolcall_delta, toolcall_end -- these represent the incremental streaming lifecycle of a tool call being emitted by the model.

#### OpenAI Completions Provider
File: packages/ai/src/providers/openai-completions.ts (pi-mono)

Tool formatting:
- convertTools() creates OpenAI format: { type: "function", function: { name, description, parameters }, strict: false }
- strict is always false because the system does not enforce strict schema compliance
- tool_calls appear in delta chunks with incremental JSON parsing
- hasToolHistory() checks if messages contain tool calls/results to determine if tool mode is active

Tool call ID handling for Responses API:
- Composite ID format: call_id|item_id (pipe-separated)
- IDs are split and processed to extract the actual call_id for dispatch

Streaming tool calls:
- Delta chunks contain tool_calls[index].function.name and tool_calls[index].function.arguments
- Arguments are accumulated incrementally as JSON fragments
- Final tool call is assembled when all chunks arrive

#### OpenAI Responses API
File: packages/ai/src/providers/openai-responses-shared.ts (pi-mono)

The Responses API uses a different format from Chat Completions:
- function_call items instead of tool_calls array
- function_call_output items for results
- Composite ID: call_id|id for tool calls
- convertResponsesTools() creates { type: "function", name, description, parameters, strict }
- processResponsesStream() handles streaming events: response.output_item.added, response.function_call_arguments.delta

Key difference from Completions API:
- Responses API returns function_call as a top-level item, not nested in tool_calls
- Tool results are function_call_output items with separate IDs
- Requires convertResponsesMessages() to normalize back to the internal representation

#### Anthropic Provider
File: packages/ai/src/providers/anthropic.ts (pi-mono)

Tool formatting:
- convertTools() creates Anthropic format: { name, description, input_schema: JSONSchema }
- Claude Code tool name canonicalization: claudeCodeTools array with toClaudeCodeName()/fromClaudeCodeName()
- Tool names are mapped between internal names and Claude Code names (OAuth/MCP mode)

Streaming:
- content_block_start with type: "tool_use" signals a tool call beginning
- input_json_delta provides incremental JSON fragments
- content_block_stop signals completion
- Tool call IDs are normalized: normalizeToolCallId() replaces invalid chars, truncates to 64 chars

Tool ID constraint: Anthropic requires tool IDs to match [a-zA-Z0-9_-]+ and be max 64 characters. Invalid characters are replaced, and long IDs are truncated.

#### Google Provider
File: packages/ai/src/providers/google.ts (pi-mono)

Tool formatting:
- streamGoogle handles functionCall parts with direct object args (NOT JSON string)
- Tool call counter for generating unique IDs (Google does not return tool call IDs)
- Thought signature handling for Gemini reasoning models

Google-shared:
File: packages/ai/src/providers/google-shared.ts (pi-mono)

- convertTools() creates { functionDeclarations: [{ name, description, parametersJsonSchema }] }
- requiresToolCallId() returns true for Claude/GPT-oss models behind Google APIs
- mapToolChoice() maps to FunctionCallingConfigMode enum

Key difference: Google returns parsed JSON objects as arguments, not JSON strings. This requires serialization back to string for our internal representation.

#### Mistral Provider
File: packages/ai/src/providers/mistral.ts (pi-mono)

- 9-char tool call ID requirement with hash-based derivation
- createMistralToolCallIdNormalizer() with collision handling
- Tool call IDs longer than 9 chars are hashed and truncated
- Thinking blocks as { type: "thinking", thinking: [{ type: "text", text }] }

#### AWS Bedrock Provider
File: packages/ai/src/providers/amazon-bedrock.ts (pi-mono)

- convertToolConfig() creates { tools: [{ toolSpec: { name, description, inputSchema: { json } } }], toolChoice }
- normalizeToolCallId() sanitizes to 64 chars
- Collects consecutive tool results into single user message
- Uses toolUse with toolUseId for tool calls, toolResult for results

#### Cross-Provider Message Transformation
File: packages/ai/src/providers/transform-messages.ts (pi-mono)

Core message transformation layer:
- Normalizes tool call IDs between providers (different length/format constraints)
- Handles thinking block conversion (drop redacted for cross-model, convert to text)
- Inserts synthetic tool results for orphaned tool calls (tool call with no result)
- Filters out errored/aborted assistant messages
- Ensures proper role alternation (user/assistant/tool pattern)

### hermes-agent (Python) - Adapter Pattern

hermes-agent uses an adapter pattern where OpenAI-format tools/messages are converted to/from Anthropic format.

#### Anthropic Adapter
File: agent/anthropic_adapter.py (hermes-agent)

convert_tools_to_anthropic():
- Converts OpenAI tool definitions { type: "function", function: { name, description, parameters } } to Anthropic format { name, description, input_schema }
- Sanitizes tool IDs to match [a-zA-Z0-9_-]+ pattern
- OAuth mode: prefixes tool names with mcp_ for Claude Code compatibility

convert_messages_to_anthropic():
- Handles tool_use/tool_result blocks
- Merges consecutive tool results into a single content block
- Enforces role alternation (Anthropic requires strict user/assistant alternation)
- Converts OpenAI tool_results into Anthropic tool_result content blocks

normalize_anthropic_response():
- Extracts tool_use blocks from Anthropic response
- Converts to OpenAI-style tool_calls for internal consumption
- Handles stop reason mapping

#### Tool Argument Coercion
File: model_tools.py (hermes-agent)

coerce_tool_args():
- Coerces string arguments to JSON Schema types (integer, number, boolean)
- LLMs often return numbers and booleans as strings -- this function converts them to proper types
- _coerce_number(): parses "42" -> 42, "3.14" -> 3.14
- _coerce_boolean(): parses "true"/"True"/"yes" -> true, "false"/"False"/"no" -> false
- Uses JSON Schema type information from tool definitions to determine target types

Tool registry:
- registry.get_definitions() returns provider-agnostic tool definitions
- registry.dispatch() routes tool call to handler
- Async bridging with persistent event loops

#### Prompt Builder
File: agent/prompt_builder.py (hermes-agent)

- System prompt assembly with skills index, context files, tool-use enforcement guidance
- Model-specific operational directives for Google, OpenAI models
- Appends tool-use instructions to system prompt based on model family

## Provider Tool Calling Format Reference

### 1. OpenAI Chat Completions API

Tool Definition Format:
- { type: "function", function: { name: string, description: string, parameters: JSONSchema } }
- strict: false (not enforced)

Tool Call Output (API response):
- message.tool_calls[index].id: string (tool call identifier)
- message.tool_calls[index].type: "function"
- message.tool_calls[index].function.name: string
- message.tool_calls[index].function.arguments: string (JSON string, NOT parsed object)

Streaming (delta chunks):
- delta.tool_calls[index].index: number
- delta.tool_calls[index].id: string (first chunk only)
- delta.tool_calls[index].function.name: string (first chunk only)
- delta.tool_calls[index].function.arguments: string (incremental JSON fragment)

Tool Result Format:
- { role: "tool", tool_call_id: string, content: string }

Stop Reason: "tool_calls" when model wants to use tools

### 2. OpenAI Responses API

Tool Definition Format:
- { type: "function", name: string, description: string, parameters: JSONSchema, strict: boolean }

Tool Call Output:
- function_call items with id, name, arguments
- Composite ID: call_id|item_id format
- arguments is a JSON string

Streaming Events:
- response.output_item.added: signals new tool call starting
- response.function_call_arguments.delta: incremental arguments
- response.function_call_arguments.done: arguments complete

Tool Result Format:
- function_call_output items with call_id and output string

### 3. Anthropic Messages API

Tool Definition Format:
- { name: string, description: string, input_schema: JSONSchema }
- No type wrapper like OpenAI -- tools are a flat array

Tool Call Output (content blocks):
- content blocks with type: "tool_use"
- id: string (must match [a-zA-Z0-9_-]+, max 64 chars)
- name: string
- input: object (parsed JSON object, NOT string)

Streaming:
- content_block_start: { type: "tool_use", id, name }
- input_json_delta: { partial_json: string } (incremental fragments)
- content_block_stop: signals completion

Tool Result Format:
- { role: "user", content: [{ type: "tool_result", tool_use_id: string, content: string }] }

Stop Reason: "tool_use" when model wants to use tools

Important constraints:
- Strict role alternation: user, assistant, user, assistant...
- Tool results must be in a user message
- Multiple tool results can be in a single content block
- Tool names in OAuth/MCP mode require mcp_ prefix for Claude Code

### 4. Google Gemini API

Tool Definition Format:
- { functionDeclarations: [{ name, description, parametersJsonSchema }] }
- Wrapped in functionDeclarations array

Tool Call Output:
- parts containing functionCall objects
- functionCall.name: string
- functionCall.args: object (parsed JSON, NOT string)
- No tool call ID returned by API -- must generate locally

Tool Result Format:
- parts containing functionResponse objects
- functionResponse.name: string
- functionResponse.response: object
- functionResponse.id: string (only required for Claude/GPT-oss behind Google APIs)

### 5. AWS Bedrock Converse API

Tool Definition Format:
- { toolSpec: { name, description, inputSchema: { json: JSONSchema } } }
- toolChoice: { auto, any, tool: { name } }

Tool Call Output:
- content blocks with toolUse: { toolUseId, name, input }
- input is a parsed object

Tool Result Format:
- { toolResult: { toolUseId, content: [{ text: string }], status: "success"|"error" } }

### 6. Mistral API

- OpenAI-compatible format but with 9-char tool call ID constraint
- Tool IDs longer than 9 chars are hashed (SHA256) and truncated
- Collision handling required when hash collisions occur

### 7. Open-Source Model Text-Based Formats (llama.cpp / local models)

These models do NOT have structured tool calling APIs. Tool calls are embedded in text output and must be parsed.

#### Hermes / Qwen / Longcat Format:
- Wrapped in backtick-JSON-backtick blocks
- Pattern: name followed by JSON object in backticks
- Parse: extract JSON between backticks, parse, validate against schema

#### Llama 3/4 Format:
- Raw JSON objects with name and args keys
- No wrapping markers -- must detect JSON objects in text
- Pattern: { "name": "...", "args": { ... } }

#### Mistral Open-Source Format:
- Sentinel token (special token ID)
- Followed by tool name and JSON object
- Pattern: [TOOL] tool_name { json }

#### DeepSeek V3/V3.1 Format:
- Special unicode tokens:  and
- Markdown-wrapped JSON
- Pattern: tool_name followed by markdown JSON block

#### Kimi K2 Format:
- Section begin/end tokens
- Pattern: section_begin tool_call ... section_end
- Individual tool call blocks within sections

#### GLM 4.5/4.7 Format:
- backtick-arg_key/arg_value pairs instead of JSON
- Pattern: arg_key followed by arg_value in backticks
- Requires key-value pair assembly into JSON

#### Qwen3-Coder Format:
- XML-style nested function/parameter tags
- Pattern: function name ... parameter name ... /parameter ... /function

## Unified Internal Representation

Our system uses a single internal ToolCall representation that all formatters translate to/from:

Based on existing types in backends/foundation_ai/src/types/mod.rs:

Tool struct (line 727):
- id: String
- name: String
- description: String
- arguments: Option<HashMap<String, ArgType>>

ModelOutput::ToolCall variant (line 629):
- id: String
- name: String
- arguments: Option<HashMap<String, ArgType>>
- signature: Option<String>

Messages::ToolResult variant (line 667):
- id: String
- name: String
- timestamp: SystemTime
- details: Option<String>
- content: UserModelContent
- error_detail: Option<String>
- signature: Option<String>

ArgType enum (line 557):
- Text(String), Float32, Float64, Usize, U8-U128, Isize, I8-I128, Duration
- JSON(String) for custom types

StopReason enum (line 520):
- Stop, Length, ToolUse, Error, Aborted

This provides a consistent internal representation. ToolFormatters translate between provider-specific formats and this unified representation.

## Architecture: Plugin-Based ToolFormatter System

### Core Trait

pub trait ToolFormatter: Send + Sync {
/// Returns the provider/API this formatter handles.
fn provider(&self) -> ModelAPI;

/// Convert internal Tool definitions into provider-specific tool schema.
fn format_tools(&self, tools: &[Tool]) -> ToolFormatResult;

/// Extract structured tool calls from raw model output text.
/// For API providers (OpenAI, Anthropic), this parses API response structures.
/// For text-based models (local llama.cpp), this parses text patterns.
fn extract_tool_calls(&self, text: &str) -> ExtractResult;

/// Format tool results back into provider-expected message structures.
fn format_tool_results(&self, results: &[Messages]) -> FormatResult;

/// Parse streaming delta/incremental output into tool call events.
fn parse_stream_chunk(&self, chunk: &[u8]) -> StreamParseResult;

/// Apply type coercion to tool arguments based on JSON Schema.
fn coerce_arguments(&self, name: &str, args: &HashMap<String, String>) -> HashMap<String, ArgType>;

/// Normalize tool call IDs to comply with provider constraints.
fn normalize_tool_call_id(&self, id: &str) -> String;

/// Map provider-specific stop reasons to unified StopReason.
fn map_stop_reason(&self, reason: &str) -> StopReason;
}

### Format Result Types

pub struct ToolFormatResult {
/// Provider-specific formatted tools (as JSON Value for flexibility).
pub formatted: serde_json::Value,
/// Any system prompt additions required for tool mode.
pub system_prompt_additions: Option<String>,
}

pub struct ExtractResult {
/// Extracted tool calls.
pub calls: Vec<Tool>,
/// Remaining text (non-tool-call content).
pub remaining_text: Option<String>,
/// Whether the model intends to use tools (vs pure text response).
pub has_tool_calls: bool,
}

pub struct FormatResult {
/// Provider-specific formatted messages.
pub formatted: serde_json::Value,
}

pub enum StreamEvent {
ToolCallStart { index: usize, id: String, name: String },
ToolCallDelta { index: usize, arguments: String },
ToolCallEnd { index: usize },
TextDelta { text: String },
Stop { reason: StopReason },
}

### Plugin Registry

pub struct ToolFormatterRegistry {
formatters: HashMap<ModelAPI, Box<dyn ToolFormatter>>,
}

impl ToolFormatterRegistry {
pub fn new() -> Self;
pub fn register(&mut self, formatter: Box<dyn ToolFormatter>);
pub fn get(&self, api: ModelAPI) -> Option<&dyn ToolFormatter>;
pub fn get_or_default(&self, api: ModelAPI) -> &dyn ToolFormatter;
}

The registry allows runtime registration of formatters. When a model provider is selected, the system looks up the appropriate ToolFormatter for that ModelAPI and uses it for all tool-related formatting operations.

### Implementation Modules

1. formatters/openai_formatter.rs -- OpenAI Chat Completions + Responses API
2. formatters/anthropic_formatter.rs -- Anthropic Messages API
3. formatters/google_formatter.rs -- Google Gemini API
4. formatters/bedrock_formatter.rs -- AWS Bedrock Converse API
5. formatters/mistral_formatter.rs -- Mistral API (9-char ID normalization)
6. formatters/text_based_formatter.rs -- Open-source models (Hermes, Qwen, Llama, DeepSeek, Kimi, GLM, etc.)
7. formatters/registry.rs -- Plugin registry

### Type Coercion Module

coerce.rs -- Type coercion from LLM-returned strings to JSON Schema types:

The coerce_tool_args function takes raw string arguments from the LLM and the JSON Schema definition of the tool, then:
1. Iterates over each property in the schema
2. Checks the declared type (integer, number, boolean, string)
3. Coerces the string value to the appropriate ArgType:
   - "integer" -> parse as integer, store as ArgType::I64 or similar
   - "number" -> parse as float, store as ArgType::Float64
   - "boolean" -> parse "true"/"false"/"yes"/"no" -> ArgType::JSON(true/false)
   - "string" -> keep as ArgType::Text
4. Handles nested objects and arrays recursively
5. Returns the coerced HashMap<String, ArgType>

### Message Transformation Layer

transform.rs -- Cross-provider message normalization:

1. Tool ID normalization:
   - Anthropic: [a-zA-Z0-9_-]+, max 64 chars
   - Mistral: exactly 9 chars (hash-based)
   - OpenAI: any string
   - Google: locally generated IDs
2. Thinking block handling:
   - Convert provider-specific thinking blocks to unified format
   - Drop redacted thinking content for cross-model compatibility
3. Role alternation enforcement:
   - Anthropic requires strict user/assistant alternation
   - Merge consecutive tool results into single content block
   - Insert synthetic tool results for orphaned tool calls
4. Error filtering:
   - Remove errored/aborted assistant messages from history

## Implementation Tasks

### Task 1: Define Core ToolFormatter Trait and Types
- Create src/tool_calling/mod.rs with module structure
- Define ToolFormatter trait with all required methods
- Define ToolFormatResult, ExtractResult, FormatResult, StreamEvent types
- Define StreamParseResult with appropriate variants
- Add error types to src/errors/mod.rs (ToolCallingError)
- Ensure all types follow existing patterns (derive_more::From, manual Display)

### Task 2: Implement Type Coercion Module
- Create src/tool_calling/coerce.rs
- Implement coerce_arguments() that takes raw string args and JSON Schema
- Implement _coerce_number() helper (parse "42" -> i64, "3.14" -> f64)
- Implement _coerce_boolean() helper (parse "true"/"false"/"yes"/"no")
- Handle nested objects and arrays recursively
- Write unit tests for all coercion cases

### Task 3: Implement ToolFormatterRegistry
- Create src/tool_calling/registry.rs
- Implement ToolFormatterRegistry with HashMap<ModelAPI, Box<dyn ToolFormatter>>
- Implement register(), get(), get_or_default() methods
- Support runtime registration of new formatters
- Write unit tests for registry operations

### Task 4: Implement OpenAI ToolFormatter
- Create src/tool_calling/formatters/openai_formatter.rs
- Implement format_tools(): Tool -> OpenAI { type: "function", function: { ... } }
- Implement extract_tool_calls(): parse tool_calls from API response JSON
- Implement format_tool_results(): ToolResult -> { role: "tool", ... }
- Implement parse_stream_chunk(): handle delta.tool_calls incremental parsing
- Implement normalize_tool_call_id(): pass-through (OpenAI accepts any ID)
- Implement map_stop_reason(): "tool_calls" -> StopReason::ToolUse
- Handle both Chat Completions and Responses API formats
- Handle composite IDs (call_id|item_id) for Responses API

### Task 5: Implement Anthropic ToolFormatter
- Create src/tool_calling/formatters/anthropic_formatter.rs
- Implement format_tools(): Tool -> { name, description, input_schema }
- Implement extract_tool_calls(): parse tool_use content blocks from JSON
- Implement format_tool_results(): ToolResult -> { type: "tool_result", ... }
- Implement parse_stream_chunk(): handle content_block_start, input_json_delta, content_block_stop
- Implement normalize_tool_call_id(): sanitize to [a-zA-Z0-9_-]+, truncate to 64 chars
- Implement map_stop_reason(): "tool_use" -> StopReason::ToolUse
- Implement OAuth/MCP mode: prefix tool names with mcp_ when enabled
- Enforce role alternation in message formatting

### Task 6: Implement Google Gemini ToolFormatter
- Create src/tool_calling/formatters/google_formatter.rs
- Implement format_tools(): Tool -> { functionDeclarations: [...] }
- Implement extract_tool_calls(): parse functionCall parts
- Implement format_tool_results(): ToolResult -> functionResponse
- Handle direct object args (not JSON string) -- serialize/deserialize as needed
- Generate local tool call IDs (Google does not return them)
- Handle thought signatures for reasoning models
- Implement requires_tool_call_id() logic for Claude/GPT-oss behind Google APIs

### Task 7: Implement AWS Bedrock ToolFormatter
- Create src/tool_calling/formatters/bedrock_formatter.rs
- Implement format_tools(): Tool -> { toolSpec: { name, description, inputSchema } }
- Implement extract_tool_calls(): parse toolUse content blocks
- Implement format_tool_results(): ToolResult -> { toolResult: { ... } }
- Implement normalize_tool_call_id(): sanitize to 64 chars
- Handle toolChoice mapping (auto, any, tool)
- Collect consecutive tool results into single user message

### Task 8: Implement Mistral ToolFormatter
- Create src/tool_calling/formatters/mistral_formatter.rs
- OpenAI-compatible format with 9-char tool call ID constraint
- Implement hash-based ID derivation for IDs longer than 9 chars
- Handle collision detection and resolution
- Implement thinking block format: { type: "thinking", thinking: [...] }

### Task 9: Implement Text-Based ToolFormatter (Open-Source Models)
- Create src/tool_calling/formatters/text_based_formatter.rs
- This is the most complex formatter -- handles all text-based model outputs
- Implement regex-based parsers for each model family:
  - Hermes/Qwen/Longcat: backtick-JSON-backtick pattern
  - Llama 3/4: raw JSON object detection in text
  - DeepSeek V3/V3.1: unicode token + markdown JSON
  - Kimi K2: section begin/end tokens
  - GLM 4.5/4.7: arg_key/arg_value pairs
  - Qwen3-Coder: XML-style function/parameter tags
- Each parser extracts tool name and arguments from text
- Remaining text (non-tool-call content) is preserved
- Use lazy_regex for pattern matching

### Task 10: Implement Message Transformation Layer
- Create src/tool_calling/transform.rs
- Implement tool ID normalization across providers
- Implement thinking block conversion (drop redacted, convert to text)
- Implement synthetic tool result insertion for orphaned tool calls
- Implement role alternation enforcement
- Implement error/abort filtering for assistant messages
- Write integration tests for cross-provider transformation

### Task 11: Integrate with ModelInteraction
- Modify ModelInteraction processing to use ToolFormatter
- When tools are present in ModelInteraction, select appropriate ToolFormatter based on ModelAPI
- Apply format_tools() before sending to provider
- Apply extract_tool_calls() on response to get unified Tool structures
- Apply format_tool_results() when sending tool results back

### Task 12: Integrate with Streaming
- Modify stream processing to use ToolFormatter.parse_stream_chunk()
- Emit StreamEvent values (ToolCallStart, ToolCallDelta, ToolCallEnd, TextDelta, Stop)
- Handle incremental tool call assembly for streaming providers
- Handle text-based model streaming with tool call detection

### Task 13: Update Error Types
- Add ToolCallingError to src/errors/mod.rs
- Variants:
  - ParseError(String) -- failed to parse tool call from text
  - CoercionError(String) -- failed to coerce argument type
  - FormatError(String) -- failed to format tool definition
  - ValidationError(String) -- tool call does not match schema
  - RegistryError(String) -- formatter not found in registry
- Use derive_more::From + manual Display pattern

### Task 14: Write Unit Tests
- Test each ToolFormatter independently with sample inputs/outputs
- Test type coercion with various string inputs
- Test message transformation with cross-provider scenarios
- Test tool ID normalization edge cases
- Test registry registration and lookup

### Task 15: Write Integration Tests
- Test end-to-end tool calling flow: define tools -> format -> extract -> coerce -> dispatch
- Test with real provider response samples (saved as test fixtures)
- Test streaming tool call extraction
- Test error handling paths

### Task 16: Documentation
- Add module-level documentation to src/tool_calling/mod.rs
- Document each formatter with provider-specific notes
- Document the unified internal representation
- Add examples of tool calling flow

### Task 17: Cargo Feature Flags
- No new feature flags needed -- all formatters are always available
- Text-based parsers use lazy_regex (already a dependency)

### Task 18: Verification
- cargo check --package foundation_ai -- clean compilation
- cargo clippy --package foundation_ai -- -D warnings -- zero warnings
- cargo test --package foundation_ai -- all tests pass
- cargo fmt --package foundation_ai -- --check -- formatting passes

## Dependencies

- serde_json for JSON parsing/serialization
- regex/lazy_regex for text-based tool call parsing
- Existing types from types/mod.rs (Tool, Messages, ArgType, StopReason, ModelAPI)
- Existing errors from errors/mod.rs

## Success Criteria

Functionality:
- All 6 ToolFormatter implementations complete (OpenAI, Anthropic, Google, Bedrock, Mistral, Text-Based)
- ToolFormatterRegistry supports runtime registration and lookup
- Type coercion correctly converts string arguments to JSON Schema types
- Message transformation handles cross-provider normalization
- Streaming tool call extraction works for all API providers
- Text-based parsers correctly extract tool calls from all supported model families

Code Quality:
- Zero warnings from cargo clippy -- -D warnings
- cargo fmt -- --check passes
- All unit and integration tests pass

Documentation:
- Module documentation updated
- Each formatter documented with provider-specific format reference
- LEARNINGS.md updated with design decisions

## Implementation Guidelines

1. Start with the core trait and types (Task 1-3) -- this defines the contract
2. Implement the simplest formatter first (OpenAI, Task 4) -- well-structured JSON API
3. Implement Anthropic (Task 5) -- introduces streaming complexity with input_json_delta
4. Implement text-based formatter (Task 9) -- most complex, handles many model families
5. Remaining formatters (Google, Bedrock, Mistral) follow similar patterns
6. Message transformation (Task 10) and integration (Tasks 11-12) tie everything together
7. Tests and verification last (Tasks 14-18)

Follow the existing patterns from foundation_ai:
- Error types in errors/mod.rs with derive_more::From + manual Display
- Types use serde Serialize/Deserialize
- Valtron-only async (no tokio, no async-trait)
- Zero warnings policy -- fix all clippy warnings immediately
