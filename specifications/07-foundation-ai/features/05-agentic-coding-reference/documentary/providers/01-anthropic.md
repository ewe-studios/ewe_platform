# Anthropic Provider -- Deep Dive

## Files Referenced

- pi-mono: `packages/ai/src/providers/anthropic.ts`
- hermes-agent: `agent/anthropic_adapter.py`

## Tool Definition Format

```typescript
// pi-mono: convertTools() at line 862
{
  name: tool.name,
  description: tool.description,
  input_schema: {
    type: "object",
    properties: jsonSchema.properties || {},
    required: jsonSchema.required || [],
  },
}
```

Key difference from OpenAI: No `type: "function"` wrapper. Tools are a flat array with `{ name, description, input_schema }`.

## OAuth / Claude Code Mode

When using OAuth tokens (e.g., Claude Code integration), tool names are canonicalized:

### hermes-agent approach
```python
# Prefix tool names with mcp_ (Claude Code convention)
for tool in anthropic_tools:
    if "name" in tool:
        tool["name"] = "mcp_" + tool["name"]
# Also prefix in message history (tool_use and tool_result blocks)
```

### pi-mono approach
```typescript
const claudeCodeTools = [
    "Read", "Write", "Edit", "Bash", "Grep", "Glob",
    "AskUserQuestion", "EnterPlanMode", "ExitPlanMode", ...
];
const ccToolLookup = new Map(claudeCodeTools.map((t) => [t.toLowerCase(), t]));
const toClaudeCodeName = (name: string) => ccToolLookup.get(name.toLowerCase()) ?? name;
```

pi-mono uses a known tool list to map to Claude Code's canonical names (e.g., `read_file` -> `Read`). hermes-agent uses a simple `mcp_` prefix.

## Streaming Tool Calls

Anthropic streams tool calls as content blocks:

```
1. content_block_start with type: "tool_use" -- signals a tool call beginning
   { id: "toolu_xxx", name: "bash", input: {} }
2. input_json_delta provides incremental JSON fragments
   { type: "input_json_delta", partial_json: '{"command": "ls ' }
3. content_block_stop signals completion
```

The streaming handler in pi-mono:
```typescript
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
else if (event.delta.type === "input_json_delta") {
    block.partialJson += event.delta.partial_json;
    block.arguments = parseStreamingJson(block.partialJson);
}
```

## Tool ID Constraint

Anthropic requires tool IDs to match `[a-zA-Z0-9_-]+` and be max 64 characters:

```typescript
// pi-mono: normalizeToolCallId() at line 693
function normalizeToolCallId(id: string): string {
    return id.replace(/[^a-zA-Z0-9_-]/g, "_").slice(0, 64);
}
```

Invalid characters are replaced with `_`, and long IDs are truncated to 64 chars.

## Tool Result Format

```typescript
// pi-mono at line 799
{
    type: "tool_result",
    tool_use_id: msg.toolCallId,
    content: convertContentBlocks(msg.content),
    is_error: msg.isError,
}
```

Multiple consecutive tool results are collected into a single user message (Anthropic requires this).

## Stop Reason Mapping

```typescript
case "end_turn":     return "stop";
case "max_tokens":   return "length";
case "tool_use":     return "toolUse";
case "refusal":      return "error";
case "pause_turn":   return "stop";  // Resubmit
case "sensitive":    return "error"; // Content flagged
```

## Prompt Caching

Anthropic supports prompt caching with `cache_control` markers:

```typescript
{
    type: "text",
    text: "...",
    cache_control: { type: "ephemeral" },
}
```

Applied to:
- System prompt blocks
- Long context files
- Conversation history (for repeated turns)

Budget warnings are injected into tool results (not system messages) to avoid breaking the cache prefix.

## Message Conversion (hermes-agent)

hermes-agent's `convert_messages_to_anthropic()`:

1. Extract thinking blocks from `reasoning_details`
2. Convert `tool_calls` to `tool_use` blocks:
   ```python
   blocks.append({
       "type": "tool_use",
       "id": _sanitize_tool_id(tc.get("id", "")),
       "name": fn.get("name", ""),
       "input": parsed_args,  # JSON string -> parsed object
   })
   ```
3. Tool results become `tool_result` in user messages
4. Merge consecutive tool results into one user message
5. Enforce strict role alternation (user/assistant/user/assistant)
6. Strip orphaned `tool_use` blocks (no matching result)
7. Strip orphaned `tool_result` blocks (no matching tool call)
8. Replace empty assistant content with placeholder

## Stealth Mode (pi-mono)

Anthropic provider has "stealth mode" that mimics Claude Code tool naming when using OAuth tokens. This maps internal tool names to Claude Code's canonical names for compatibility.

## Key Patterns for foundation_ai

1. **No type wrapper** -- tools are flat array `{ name, description, input_schema }` vs OpenAI's `{ type: "function", function: {...} }`
2. **OAuth tool name canonicalization** -- either prefix with `mcp_` or map to known Claude Code names
3. **Content block streaming** -- tool calls stream as `content_block_start` -> `input_json_delta` -> `content_block_stop`
4. **Tool ID constraint** -- `[a-zA-Z0-9_-]+`, max 64 chars
5. **Consecutive tool results merged** into single user message
6. **Prompt caching** via `cache_control` markers on content blocks
7. **Role alternation enforcement** is strict for Anthropic
8. **pause_turn stop reason** means "resubmit" -- not an error
