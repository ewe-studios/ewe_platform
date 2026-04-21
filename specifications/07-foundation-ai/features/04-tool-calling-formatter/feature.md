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

Reference: packages/ai/src/providers/openai-completions.ts (pi-mono)

Tool Definition Format:
    { type: "function", function: { name: string, description: string, parameters: JSONSchema } }
    strict: false (not enforced)

From pi-mono convertTools() at line 708:
    function convertTools(tools: Tool[], compat): ChatCompletionTool[] {
        return tools.map((tool) => ({
            type: "function",
            function: {
                name: tool.name,
                description: tool.description,
                parameters: tool.parameters as any,
                ...(compat.supportsStrictMode !== false && { strict: false }),
            },
        }));
    }

Tool Call Output (API response):
    message.tool_calls[index].id: string (tool call identifier)
    message.tool_calls[index].type: "function"
    message.tool_calls[index].function.name: string
    message.tool_calls[index].function.arguments: string (JSON string, NOT parsed object)

Streaming (delta chunks) -- from pi-mono at line 221:
    if (choice?.delta?.tool_calls) {
        for (const toolCall of choice.delta.tool_calls) {
            if (!currentBlock || currentBlock.type !== "toolCall" ||
                (toolCall.id && currentBlock.id !== toolCall.id)) {
                finishCurrentBlock(currentBlock);
                currentBlock = {
                    type: "toolCall",
                    id: toolCall.id || "",
                    name: toolCall.function?.name || "",
                    arguments: {},
                    partialArgs: "",
                };
                output.content.push(currentBlock);
                stream.push({ type: "toolcall_start", ... });
            }
            if (currentBlock.type === "toolCall") {
                if (toolCall.function?.arguments) {
                    currentBlock.partialArgs += toolCall.function.arguments;
                    currentBlock.arguments = parseStreamingJson(currentBlock.partialArgs);
                }
                stream.push({ type: "toolcall_delta", delta: ..., ... });
            }
        }
    }

Key pattern: arguments are accumulated incrementally as JSON fragments. parseStreamingJson is called on each delta to get best-effort parsed object.

Tool Result Format -- from pi-mono at line 634:
    const toolResultMsg: ChatCompletionToolMessageParam = {
        role: "tool",
        content: sanitizeSurrogates(hasText ? textResult : "(see attached image)"),
        tool_call_id: toolMsg.toolCallId,
    };
    if (compat.requiresToolResultName && toolMsg.toolName) {
        (toolResultMsg as any).name = toolMsg.toolName;
    }

Stop Reason mapping -- from pi-mono mapStopReason() at line 752:
    case "function_call":
    case "tool_calls":
        return { stopReason: "toolUse" };

Tool ID normalization for Responses API -- from pi-mono at line 488:
    const normalizeToolCallId = (id: string): string => {
        // Handle pipe-separated IDs from OpenAI Responses API
        // Format: {call_id}|{id} where {id} can be 400+ chars with special chars
        if (id.includes("|")) {
            const [callId] = id.split("|");
            return callId.replace(/[^a-zA-Z0-9_-]/g, "_").slice(0, 40);
        }
        if (model.provider === "openai") return id.length > 40 ? id.slice(0, 40) : id;
        return id;
    };

hasToolHistory() check -- from pi-mono at line 41:
    function hasToolHistory(messages: Message[]): boolean {
        for (const msg of messages) {
            if (msg.role === "toolResult") return true;
            if (msg.role === "assistant") {
                if (msg.content.some((block) => block.type === "toolCall")) return true;
            }
        }
        return false;
    }
    // Used because Anthropic (via proxy) requires the tools param to be present
    // when messages include tool_calls or tool role messages

### 2. OpenAI Responses API

Reference: packages/ai/src/providers/openai-responses-shared.ts (pi-mono)

The Responses API uses a fundamentally different format from Chat Completions.

Tool Definition Format -- from convertResponsesTools() at line 261:
    export function convertResponsesTools(tools: Tool[], options?): OpenAITool[] {
        const strict = options?.strict === undefined ? false : options.strict;
        return tools.map((tool) => ({
            type: "function",
            name: tool.name,
            description: tool.description,
            parameters: tool.parameters as any,
            strict,
        }));
    }

Tool Call Output -- composite ID format from line 187:
    // Tool calls become function_call items with composite IDs
    output.push({
        type: "function_call",
        id: itemId,         // the "fc_xxx" item ID
        call_id: callId,    // the actual tool call identifier
        name: toolCall.name,
        arguments: JSON.stringify(toolCall.arguments),
    });

Composite ID handling -- from convertResponsesMessages() at line 100:
    const normalizeToolCallId = (id: string): string => {
        if (!id.includes("|")) return normalizeIdPart(id);
        const [callId, itemId] = id.split("|");
        const normalizedCallId = normalizeIdPart(callId);
        let normalizedItemId = normalizeIdPart(itemId);
        // OpenAI Responses API requires item id to start with "fc"
        if (!normalizedItemId.startsWith("fc")) {
            normalizedItemId = normalizeIdPart(`fc_${normalizedItemId}`);
        }
        return `${normalizedCallId}|${normalizedItemId}`;
    };

Tool Result Format -- from line 210:
    messages.push({
        type: "function_call_output",
        call_id: callId,  // just the call_id part, NOT the composite
        output: sanitizeSurrogates(hasText ? textResult : "(see attached image)"),
    });

Streaming Events -- from processResponsesStream() at line 276:
    else if (event.type === "response.output_item.added") {
        if (item.type === "function_call") {
            currentBlock = {
                type: "toolCall",
                id: `${item.call_id}|${item.id}`,  // composite ID
                name: item.name,
                arguments: {},
                partialJson: item.arguments || "",
            };
        }
    }
    else if (event.type === "response.function_call_arguments.delta") {
        currentBlock.partialJson += event.delta;
        currentBlock.arguments = parseStreamingJson(currentBlock.partialJson);
    }
    else if (event.type === "response.function_call_arguments.done") {
        currentBlock.partialJson = event.arguments;
        currentBlock.arguments = parseStreamingJson(currentBlock.partialJson);
    }

Stop Reason mapping -- from mapStopReason() at line 488:
    case "completed": return "stop";
    case "incomplete": return "length";
    case "failed":
    case "cancelled": return "error";
    // If content has tool calls but stop is "stop", override to "toolUse":
    if (output.content.some((b) => b.type === "toolCall") && output.stopReason === "stop") {
        output.stopReason = "toolUse";
    }

### 3. Anthropic Messages API

References:
- packages/ai/src/providers/anthropic.ts (pi-mono)
- agent/anthropic_adapter.py (hermes-agent)

Tool Definition Format -- from pi-mono at line 862:
    function convertTools(tools: Tool[], isOAuthToken: boolean): Anthropic.Messages.Tool[] {
        return tools.map((tool) => ({
            name: isOAuthToken ? toClaudeCodeName(tool.name) : tool.name,
            description: tool.description,
            input_schema: {
                type: "object",
                properties: jsonSchema.properties || {},
                required: jsonSchema.required || [],
            },
        }));
    }

No type wrapper like OpenAI -- tools are a flat array with { name, description, input_schema }.

OAuth/Claude Code tool name prefixing -- from hermes-agent at line 1269:
    # 3. Prefix tool names with mcp_ (Claude Code convention)
    if anthropic_tools:
        for tool in anthropic_tools:
            if "name" in tool:
                tool["name"] = "mcp_" + tool["name"]

    # 4. Prefix tool names in message history (tool_use and tool_result blocks)
    for msg in anthropic_messages:
        for block in content:
            if block.get("type") == "tool_use" and "name" in block:
                if not block["name"].startswith("mcp_"):
                    block["name"] = "mcp_" + block["name"]

pi-mono canonicalizes tool names using Claude Code's known tool list at line 70:
    const claudeCodeTools = [
        "Read", "Write", "Edit", "Bash", "Grep", "Glob",
        "AskUserQuestion", "EnterPlanMode", "ExitPlanMode", ...
    ];
    const ccToolLookup = new Map(claudeCodeTools.map((t) => [t.toLowerCase(), t]));
    const toClaudeCodeName = (name: string) => ccToolLookup.get(name.toLowerCase()) ?? name;

Tool Call Output (content blocks) -- from pi-mono streaming at line 306:
    else if (event.content_block.type === "tool_use") {
        const block = {
            type: "toolCall",
            id: event.content_block.id,
            name: isOAuth ? fromClaudeCodeName(event.content_block.name, context.tools)
                          : event.content_block.name,
            arguments: event.content_block.input ?? {},
            partialJson: "",
            index: event.index,
        };
    }

Streaming -- from pi-mono at line 345:
    else if (event.delta.type === "input_json_delta") {
        const index = blocks.findIndex((b) => b.index === event.index);
        const block = blocks[index];
        if (block && block.type === "toolCall") {
            block.partialJson += event.delta.partial_json;
            block.arguments = parseStreamingJson(block.partialJson);
            stream.push({ type: "toolcall_delta", delta: event.delta.partial_json, ... });
        }
    }

Tool ID constraint -- normalizeToolCallId() at pi-mono line 693:
    function normalizeToolCallId(id: string): string {
        return id.replace(/[^a-zA-Z0-9_-]/g, "_").slice(0, 64);
    }

Tool Result Format -- from pi-mono at line 799:
    toolResults.push({
        type: "tool_result",
        tool_use_id: msg.toolCallId,
        content: convertContentBlocks(msg.content),
        is_error: msg.isError,
    });
    // Consecutive tool results are collected into a single user message (line 801-831)

Stop Reason mapping -- from pi-mono mapStopReason() at line 880:
    case "end_turn": return "stop";
    case "max_tokens": return "length";
    case "tool_use": return "toolUse";
    case "refusal": return "error";
    case "pause_turn": return "stop";  // Stop is good enough -> resubmit
    case "sensitive": return "error";  // Content flagged by safety filters

hermes-agent message conversion -- from convert_messages_to_anthropic() at line 1029:
    # Key patterns from hermes-agent:
    # 1. Extract thinking blocks from reasoning_details
    # 2. Convert tool_calls to tool_use blocks:
    blocks.append({
        "type": "tool_use",
        "id": _sanitize_tool_id(tc.get("id", "")),
        "name": fn.get("name", ""),
        "input": parsed_args,  # JSON string -> parsed object
    })
    # 3. Tool results become tool_result in user messages:
    tool_result = {
        "type": "tool_result",
        "tool_use_id": _sanitize_tool_id(m.get("tool_call_id", "")),
        "content": result_content,
    }
    # 4. Merge consecutive tool results into one user message (line 1105)
    # 5. Enforce strict role alternation (line 1171-1207)
    # 6. Strip orphaned tool_use blocks (line 1134-1149)
    # 7. Strip orphaned tool_result blocks (line 1151-1169)

Important constraints:
- Strict role alternation: user, assistant, user, assistant...
- Tool results must be in a user message
- Multiple tool results can be in a single content block
- Tool names in OAuth/MCP mode require mcp_ prefix for Claude Code
- Empty assistant content must be replaced with placeholder

### 4. Google Gemini API

References:
- packages/ai/src/providers/google.ts (pi-mono)
- packages/ai/src/providers/google-shared.ts (pi-mono)

Tool Definition Format -- from convertTools() at google-shared.ts line 250:
    export function convertTools(tools: Tool[], useParameters = false) {
        return [{
            functionDeclarations: tools.map((tool) => ({
                name: tool.name,
                description: tool.description,
                ...(useParameters
                    ? { parameters: tool.parameters }
                    : { parametersJsonSchema: tool.parameters }),
            })),
        }];
    }

Tool call counter for ID generation -- from google.ts at line 46:
    let toolCallCounter = 0;
    // Google does not return tool call IDs -- must generate locally

Tool Call Output -- from google.ts at line 156:
    if (part.functionCall) {
        // Generate unique ID if not provided or if it's a duplicate
        const providedId = part.functionCall.id;
        const needsNewId = !providedId || output.content.some(
            (b) => b.type === "toolCall" && b.id === providedId
        );
        const toolCallId = needsNewId
            ? `${part.functionCall.name}_${Date.now()}_${++toolCallCounter}`
            : providedId;

        const toolCall = {
            type: "toolCall",
            id: toolCallId,
            name: part.functionCall.name || "",
            arguments: part.functionCall.args ?? {},  // DIRECT OBJECT, not JSON string
            ...(part.thoughtSignature && { thoughtSignature: part.thoughtSignature }),
        };
        // Emit toolcall_start, toolcall_delta, toolcall_end immediately (not streamed)
        stream.push({ type: "toolcall_start", ... });
        stream.push({ type: "toolcall_delta", delta: JSON.stringify(toolCall.arguments), ... });
        stream.push({ type: "toolcall_end", toolCall, ... });
    }

Key difference: Google returns parsed JSON objects as arguments, not JSON strings. This requires serialization back to string for our internal representation if needed.

Tool Result Format -- from google-shared.ts at line 208:
    const functionResponsePart = {
        functionResponse: {
            name: msg.toolName,
            response: msg.isError ? { error: responseValue } : { output: responseValue },
            ...(hasImages && modelSupportsMultimodalFunctionResponse && { parts: imageParts }),
            ...(includeId ? { id: msg.toolCallId } : {}),
        },
    };

requiresToolCallId() -- from google-shared.ts at line 69:
    export function requiresToolCallId(modelId: string): boolean {
        return modelId.startsWith("claude-") || modelId.startsWith("gpt-oss-");
    }

Tool choice mapping -- from google-shared.ts at line 269:
    export function mapToolChoice(choice: string): FunctionCallingConfigMode {
        case "auto": return FunctionCallingConfigMode.AUTO;
        case "none": return FunctionCallingConfigMode.NONE;
        case "any":  return FunctionCallingConfigMode.ANY;
    }

Thought signature handling -- from google-shared.ts at line 27:
    export function isThinkingPart(part): boolean {
        return part.thought === true;  // definitive marker, not thoughtSignature
    }
    // thoughtSignature can appear on ANY part type (text, functionCall, etc.)
    // It does NOT indicate the part itself is thinking content

### 5. AWS Bedrock Converse API

Reference: packages/ai/src/providers/amazon-bedrock.ts (pi-mono)

Tool Definition Format -- from convertToolConfig() at line 652:
    function convertToolConfig(tools: Tool[], toolChoice): ToolConfiguration | undefined {
        const bedrockTools = tools.map((tool) => ({
            toolSpec: {
                name: tool.name,
                description: tool.description,
                inputSchema: { json: tool.parameters },
            },
        }));
        // toolChoice mapping
        switch (toolChoice) {
            case "auto":  bedrockToolChoice = { auto: {} };
            case "any":   bedrockToolChoice = { any: {} };
            case tool:    bedrockToolChoice = { tool: { name: toolChoice.name } };
        }
        return { tools: bedrockTools, toolChoice: bedrockToolChoice };
    }

Tool Call Output -- from handleContentBlockStart() at line 263:
    if (start?.toolUse) {
        const block = {
            type: "toolCall",
            id: start.toolUse.toolUseId || "",
            name: start.toolUse.name || "",
            arguments: {},
            partialJson: "",
            index,
        };
    }

Streaming -- from handleContentBlockDelta() at line 286:
    else if (delta?.toolUse && block?.type === "toolCall") {
        block.partialJson = (block.partialJson || "") + (delta.toolUse.input || "");
        block.arguments = parseStreamingJson(block.partialJson);
    }

Tool Result Format -- from convertMessages() at line 586:
    toolResults.push({
        toolResult: {
            toolUseId: m.toolCallId,
            content: m.content.map((c) =>
                c.type === "image"
                    ? { image: createImageBlock(c.mimeType, c.data) }
                    : { text: sanitizeSurrogates(c.text) }
            ),
            status: m.isError ? ToolResultStatus.ERROR : ToolResultStatus.SUCCESS,
        },
    });
    // All consecutive tool results collected into single user message

Stop Reason mapping -- from mapStopReason() at line 683:
    case BedrockStopReason.END_TURN:
    case BedrockStopReason.STOP_SEQUENCE:    return "stop";
    case BedrockStopReason.MAX_TOKENS:
    case BedrockStopReason.MODEL_CONTEXT_WINDOW_EXCEEDED: return "length";
    case BedrockStopReason.TOOL_USE:         return "toolUse";

### 6. Mistral API

Reference: packages/ai/src/providers/mistral.ts (pi-mono)

OpenAI-compatible format but with 9-char tool call ID constraint.

Tool call ID normalizer with collision handling -- at line 144:
    const MISTRAL_TOOL_CALL_ID_LENGTH = 9;

    function createMistralToolCallIdNormalizer(): (id: string) => string {
        const idMap = new Map<string, string>();
        const reverseMap = new Map<string, string>();
        return (id: string): string => {
            const existing = idMap.get(id);
            if (existing) return existing;
            let attempt = 0;
            while (true) {
                const candidate = deriveMistralToolCallId(id, attempt);
                const owner = reverseMap.get(candidate);
                if (!owner || owner === id) {
                    idMap.set(id, candidate);
                    reverseMap.set(candidate, id);
                    return candidate;
                }
                attempt++;
            }
        };
    }

    function deriveMistralToolCallId(id: string, attempt: number): string {
        const normalized = id.replace(/[^a-zA-Z0-9]/g, "");
        if (attempt === 0 && normalized.length === MISTRAL_TOOL_CALL_ID_LENGTH) return normalized;
        const seed = attempt === 0 ? normalized : `${normalized}:${attempt}`;
        return shortHash(seed).replace(/[^a-zA-Z0-9]/g, "").slice(0, MISTRAL_TOOL_CALL_ID_LENGTH);
    }

Key insight: If the ID is already exactly 9 alphanumeric chars, it passes through unchanged. Otherwise it's hashed. Collision detection uses a reverse map -- if a hash collides with an existing ID, the attempt counter increments to produce a different hash.

Tool Definition Format -- from toFunctionTools() at line 437:
    function toFunctionTools(tools: Tool[]): Array<FunctionTool & { type: "function" }> {
        return tools.map((tool) => ({
            type: "function",
            function: {
                name: tool.name,
                description: tool.description,
                parameters: tool.parameters as unknown as Record<string, unknown>,
                strict: false,
            },
        }));
    }

Thinking blocks format -- from toChatMessages() at line 486:
    contentParts.push({
        type: "thinking",
        thinking: [{ type: "text", text: sanitizeSurrogates(block.thinking) }],
    });

Streaming tool calls -- from consumeChatStream() at line 372:
    const toolCalls = delta.toolCalls || [];
    for (const toolCall of toolCalls) {
        const callId = toolCall.id && toolCall.id !== "null"
            ? toolCall.id
            : deriveMistralToolCallId(`toolcall:${toolCall.index ?? 0}`, 0);
        // ... accumulate partialArgs, parseStreamingJson, emit deltas
    }

Stop Reason mapping -- from mapChatStopReason() at line 570:
    case "stop":              return "stop";
    case "length":
    case "model_length":      return "length";
    case "tool_calls":        return "toolUse";
    case "error":             return "error";

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

Our system uses a single internal ToolCall representation that all formatters translate to/from. This is the same pattern both reference projects use internally before routing to providers.

### pi-mono internal representation
File: packages/ai/src/types.ts (pi-mono)

    export interface ToolCall {
        type: "toolCall";
        id: string;
        name: string;
        arguments: Record<string, any>;
        thoughtSignature?: string; // Google-specific: opaque signature for reusing thought context
    }

    export interface ToolResultMessage<TDetails = any> {
        role: "toolResult";
        toolCallId: string;
        toolName: string;
        content: (TextContent | ImageContent)[];
        details?: TDetails;
        isError: boolean;
        timestamp: number;
    }

    export type StopReason = "stop" | "length" | "toolUse" | "error" | "aborted";

    // Event protocol for streaming tool calls
    export type AssistantMessageEvent =
        | { type: "toolcall_start"; contentIndex: number; partial: AssistantMessage }
        | { type: "toolcall_delta"; contentIndex: number; delta: string; partial: AssistantMessage }
        | { type: "toolcall_end"; contentIndex: number; toolCall: ToolCall; partial: AssistantMessage }
        | { type: "done"; reason: Extract<StopReason, "stop" | "length" | "toolUse">; message: AssistantMessage }
        | { type: "error"; reason: Extract<StopReason, "aborted" | "error">; error: AssistantMessage };

### hermes-agent internal representation
File: agent/anthropic_adapter.py, normalize_anthropic_response() (hermes-agent)

    # Anthropic response normalized to OpenAI-style tool_calls
    tool_calls.append(
        SimpleNamespace(
            id=block.id,
            type="function",
            function=SimpleNamespace(
                name=name,
                arguments=json.dumps(block.input),  // input is object, serialized to string
            ),
        )
    )

    # Stop reason mapping
    stop_reason_map = {
        "end_turn": "stop",
        "tool_use": "tool_calls",
        "max_tokens": "length",
        "stop_sequence": "stop",
    }

### foundation_ai existing types (target)

Based on existing types in backends/foundation_ai/src/types/mod.rs:

Tool struct (line 727):
    id: String
    name: String
    description: String
    arguments: Option<HashMap<String, ArgType>>

ModelOutput::ToolCall variant (line 629):
    id: String
    name: String
    arguments: Option<HashMap<String, ArgType>>
    signature: Option<String>

Messages::ToolResult variant (line 667):
    id: String
    name: String
    timestamp: SystemTime
    details: Option<String>
    content: UserModelContent
    error_detail: Option<String>
    signature: Option<String>

StopReason enum (line 520):
    Stop, Length, ToolUse, Error, Aborted

### Key observation from both projects

pi-mono stores arguments as Record<string, any> (untyped JSON objects). hermes-agent normalizes to json.dumps(block.input) (JSON string). foundation_ai uses HashMap<String, ArgType> which is more structured -- we already have typed variants (Text, Float32, I64, etc.). The ToolFormatter must handle the bridge: API providers return either parsed objects (Anthropic input, Google args) or JSON strings (OpenAI function.arguments), and we must coerce them to HashMap<String, ArgType>.

## Architecture: Plugin-Based ToolFormatter System

### Design Decision: Trait vs Callback Pattern

Both reference projects use different approaches:
- pi-mono: Direct provider functions (streamAnthropic, streamOpenAICompletions, etc.) with inline tool conversion
- hermes-agent: Adapter pattern (anthropic_adapter.py) with convert_tools, convert_messages, normalize_response

Our approach combines both: a ToolFormatter trait for compile-time dispatch (matching foundation_ai's ModelProvider pattern) with the adapter concepts from hermes-agent. Each formatter handles:
1. format_tools() -- our Tool[] -> provider schema (like pi-mono's convertTools)
2. format_messages() -- our Messages[] -> provider messages (like hermes-agent's convert_messages_to_anthropic)
3. extract_tool_calls() -- provider response -> unified ToolCall (like hermes-agent's normalize_anthropic_response)
4. parse_stream_chunk() -- incremental delta parsing (like pi-mono's streaming event handlers)
5. coerce_arguments() -- string args -> typed values (like hermes-agent's coerce_tool_args)

### Core Trait

pub trait ToolFormatter: Send + Sync {
    /// Returns the provider/API this formatter handles.
    fn provider(&self) -> ModelAPI;

    /// Convert internal Tool definitions into provider-specific tool schema.
    /// Maps to: pi-mono convertTools(), hermes-agent convert_tools_to_anthropic()
    fn format_tools(&self, tools: &[Tool], options: ToolFormatOptions) -> ToolFormatResult;

    /// Convert our Messages into provider-specific message format.
    /// Maps to: hermes-agent convert_messages_to_anthropic(), pi-mono convertMessages()
    fn format_messages(&self, messages: &[Messages], options: MessageFormatOptions) -> MessageFormatResult;

    /// Extract structured tool calls from raw provider response.
    /// Maps to: hermes-agent normalize_anthropic_response(), pi-mono streaming block assembly
    fn extract_tool_calls(&self, response: &ToolResponse) -> ExtractResult;

    /// Parse streaming delta/incremental output into tool call events.
    /// Maps to: pi-mono content_block_delta handling, delta.tool_calls handling
    fn parse_stream_chunk(&self, chunk: &[u8]) -> StreamParseResult;

    /// Apply type coercion to tool arguments based on JSON Schema.
    /// Maps to: hermes-agent coerce_tool_args()
    fn coerce_arguments(&self, tool_name: &str, args: HashMap<String, String>) -> HashMap<String, ArgType>;

    /// Normalize tool call IDs to comply with provider constraints.
    /// Maps to: pi-mono normalizeToolCallId(), _sanitize_tool_id() in hermes-agent
    fn normalize_tool_call_id(&self, id: &str) -> String;

    /// Map provider-specific stop reasons to unified StopReason.
    /// Maps to: pi-mono mapStopReason() in each provider
    fn map_stop_reason(&self, reason: &str) -> StopReason;
}

### Format Result Types

These types reflect the patterns observed across both projects:

ToolFormatResult -- what both projects produce when formatting tool definitions:
    pub struct ToolFormatResult {
        /// Provider-specific formatted tools (serde_json::Value for flexibility).
        /// e.g., OpenAI: [{ type: "function", function: { ... } }]
        ///       Anthropic: [{ name, description, input_schema: { ... } }]
        ///       Google: [{ functionDeclarations: [{ name, description, parametersJsonSchema }] }]
        ///       Bedrock: { tools: [{ toolSpec: { ... } }], toolChoice: { ... } }
        pub formatted: serde_json::Value,
        /// Any system prompt additions required for tool mode.
        /// e.g., OAuth Anthropic needs "You are Claude Code..." prefix
        pub system_prompt_additions: Option<String>,
    }

    pub struct ToolFormatOptions {
        /// Whether OAuth/MCP mode is active (affects tool naming with mcp_ prefix).
        pub is_oauth: bool,
        /// Provider-specific tool choice (auto, any, none, specific tool).
        pub tool_choice: Option<ToolChoice>,
    }

ExtractResult -- what both projects produce after parsing provider responses:
    pub struct ExtractResult {
        /// Extracted tool calls in unified Tool format.
        pub calls: Vec<Tool>,
        /// Remaining text content (non-tool-call text).
        /// For API providers this is the assistant's text content.
        /// For text-based models this is the non-tool-call portions of output.
        pub remaining_text: Option<String>,
        /// Whether the model intends to use tools (vs pure text response).
        pub has_tool_calls: bool,
    }

    pub struct ToolResponse {
        /// Raw provider response (JSON for APIs, text for local models).
        pub raw: String,
        /// Optional stop reason from provider.
        pub stop_reason: Option<String>,
    }

MessageFormatResult -- what hermes-agent's convert_messages_to_anthropic returns:
    pub struct MessageFormatResult {
        /// Provider-specific formatted messages.
        pub formatted: serde_json::Value,
        /// System prompt extracted from messages (for providers that separate it).
        pub system_prompt: Option<String>,
    }

StreamParseResult -- what pi-mono's streaming handlers produce per chunk:
    pub enum StreamEvent {
        ToolCallStart { index: usize, id: String, name: String },
        ToolCallDelta { index: usize, arguments: String },
        ToolCallEnd { index: usize },
        TextDelta { text: String },
        Stop { reason: StopReason },
    }

    pub enum StreamParseResult {
        Event(StreamEvent),
        Pending,    // Chunk had no actionable tool call data
        Error(String),
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

coerce.rs -- Type coercion from LLM-returned strings to JSON Schema types.

Reference: hermes-agent model_tools.py, coerce_tool_args() at line 372:

    def coerce_tool_args(tool_name: str, args: Dict[str, Any]) -> Dict[str, Any]:
        """Coerce tool call arguments to match their JSON Schema types.

        LLMs frequently return numbers as strings ("42" instead of 42)
        and booleans as strings ("true" instead of true).
        """
        schema = registry.get_schema(tool_name)
        properties = (schema.get("parameters") or {}).get("properties")
        for key, value in args.items():
            if not isinstance(value, str):
                continue
            prop_schema = properties.get(key)
            expected = prop_schema.get("type")  # "integer", "number", "boolean"
            coerced = _coerce_value(value, expected)
            if coerced is not value:
                args[key] = coerced
        return args

    def _coerce_value(value: str, expected_type):
        if isinstance(expected_type, list):
            # Union type — try each in order
            for t in expected_type:
                result = _coerce_value(value, t)
                if result is not value:
                    return result
            return value
        if expected_type in ("integer", "number"):
            return _coerce_number(value, integer_only=(expected_type == "integer"))
        if expected_type == "boolean":
            return _coerce_boolean(value)
        return value

    def _coerce_number(value: str, integer_only: bool = False):
        try:
            f = float(value)
        except (ValueError, OverflowError):
            return value
        if f == int(f):
            return int(f)
        if integer_only:
            return value  # Schema wants int but value has decimals
        return f

    def _coerce_boolean(value: str):
        low = value.strip().lower()
        if low == "true": return True
        if low == "false": return False
        return value

The coerce_arguments function in Rust must:
1. Iterate over each property in the JSON Schema
2. Check the declared type (integer, number, boolean, string)
3. Coerce the string value to the appropriate ArgType:
   - "integer" -> parse as integer, store as ArgType::I64
   - "number" -> parse as float, store as ArgType::Float64
   - "boolean" -> parse "true"/"false" -> ArgType::JSON(true/false)
   - "string" -> keep as ArgType::Text
4. Handle union types (e.g., ["integer", "string"]) by trying each in order
5. Return the coerced HashMap<String, ArgType>
6. Preserve original values when coercion fails

### Message Transformation Layer

transform.rs -- Cross-provider message normalization.

Reference: packages/ai/src/providers/transform-messages.ts (pi-mono)

1. Tool ID normalization:
   The transformMessages function accepts a normalizeToolCallId callback:

    export function transformMessages<TApi extends Api>(
        messages: Message[],
        model: Model<TApi>,
        normalizeToolCallId?: (id: string, model: Model<TApi>, source: AssistantMessage) => string,
    ): Message[] {
        const toolCallIdMap = new Map<string, string>();
        // First pass: normalize IDs on assistant messages
        // Second pass: insert synthetic tool results for orphaned tool calls
    }

   Each provider constructs its own normalizer:
   - Anthropic: id.replace(/[^a-zA-Z0-9_-]/g, "_").slice(0, 64)
   - Mistral: hash-based 9-char derivation with collision handling
   - Google: normalize only for Claude/GPT-oss behind Google APIs
   - OpenAI Responses: split pipe-separated IDs, sanitize

2. Thinking block handling -- from transform-messages.ts at line 40:

    if (block.type === "thinking") {
        // Redacted thinking is opaque encrypted content, only valid for same model
        if (block.redacted) {
            return isSameModel ? block : [];  // drop for cross-model
        }
        // Skip empty thinking blocks, convert others to plain text
        if (!block.thinking || block.thinking.trim() === "") return [];
        if (isSameModel) return block;
        return { type: "text", text: block.thinking };  // convert to text
    }

3. Synthetic tool result insertion -- from transform-messages.ts at line 98:

    // Second pass: insert synthetic empty tool results for orphaned tool calls
    for (const tc of pendingToolCalls) {
        if (!existingToolResultIds.has(tc.id)) {
            result.push({
                role: "toolResult",
                toolCallId: tc.id,
                toolName: tc.name,
                content: [{ type: "text", text: "No result provided" }],
                isError: true,
                timestamp: Date.now(),
            });
        }
    }

4. Error/abort filtering -- from transform-messages.ts at line 126:

    const assistantMsg = msg as AssistantMessage;
    if (assistantMsg.stopReason === "error" || assistantMsg.stopReason === "aborted") {
        continue;  // Skip errored/aborted assistant messages entirely
    }
    // These are incomplete turns that shouldn't be replayed:
    // - May have partial content (reasoning without message, incomplete tool calls)
    // - Replaying them can cause API errors
    // - The model should retry from the last valid state

5. Cross-provider transformation flow:

    // Each provider calls transformMessages before sending to API
    const transformedMessages = transformMessages(messages, model, normalizeToolCallId);

   The normalizer callback is provider-specific:
   - openai-completions.ts line 503: transformMessages(context.messages, model, normalizeToolCallId)
   - anthropic.ts line 706: transformMessages(messages, model, normalizeToolCallId)
   - google-shared.ts line 97: transformMessages(context.messages, model, normalizeToolCallId)
   - amazon-bedrock.ts line 499: transformMessages(context.messages, model, normalizeToolCallId)
   - mistral.ts line 70: transformMessages(context.messages, model, normalizeMistralToolCallId)

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
