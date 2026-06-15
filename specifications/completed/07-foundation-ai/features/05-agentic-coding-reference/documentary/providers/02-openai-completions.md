# OpenAI Chat Completions Provider -- Deep Dive

## Files Referenced

- pi-mono: `packages/ai/src/providers/openai-completions.ts`

## Tool Definition Format

```typescript
// pi-mono: convertTools() at line 708
{
    type: "function",
    function: {
        name: tool.name,
        description: tool.description,
        parameters: tool.parameters as any,
        strict: false,  // always false
    },
}
```

`strict` is always `false` because the system does not enforce strict schema compliance.

## Tool Call Output

```typescript
message.tool_calls[index].id: string       // tool call identifier
message.tool_calls[index].type: "function"
message.tool_calls[index].function.name: string
message.tool_calls[index].function.arguments: string  // JSON string, NOT parsed object
```

Arguments are returned as a JSON string, not a parsed object. This must be parsed before use.

## Streaming Tool Calls (Delta Chunks)

```typescript
// pi-mono at line 221
if (choice?.delta?.tool_calls) {
    for (const toolCall of choice.delta.tool_calls) {
        if (!currentBlock || currentBlock.type !== "toolCall" ||
            (toolCall.id && currentBlock.id !== toolCall.id)) {
            // New tool call starting
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
```

Key pattern: arguments are accumulated incrementally as JSON fragments. `parseStreamingJson` is called on each delta to get best-effort parsed object.

## Tool Result Format

```typescript
// pi-mono at line 634
const toolResultMsg = {
    role: "tool",
    content: sanitizeSurrogates(hasText ? textResult : "(see attached image)"),
    tool_call_id: toolMsg.toolCallId,
};
if (compat.requiresToolResultName && toolMsg.toolName) {
    (toolResultMsg as any).name = toolMsg.toolName;
}
```

Some providers require the `name` field on tool result messages. This is controlled by the `requiresToolResultName` compat flag.

## Stop Reason Mapping

```typescript
case "function_call":
case "tool_calls":
    return { stopReason: "toolUse" };
```

## hasToolHistory Check

```typescript
// pi-mono at line 41
function hasToolHistory(messages: Message[]): boolean {
    for (const msg of messages) {
        if (msg.role === "toolResult") return true;
        if (msg.role === "assistant") {
            if (msg.content.some((block) => block.type === "toolCall")) return true;
        }
    }
    return false;
}
```

This check exists because Anthropic (via proxy) requires the `tools` parameter to be present when messages include tool_calls or tool role messages.

## Compatibility Detection (`detectCompat()`)

The OpenAI completions provider detects non-standard providers:

| Provider | Compat Flag | Behavior |
|----------|-------------|----------|
| Cerebras | `cerebras` | No cache control, simplified response |
| xAI | `xai` | Specific header requirements |
| z.ai | `zai` | Thinking format variations, reasoning_content handling |
| OpenRouter | `openrouter` | Extra headers, specific error format |
| Qwen | `qwen` | reasoning_content field handling |

### Key Compat Flags

| Flag | Purpose |
|------|---------|
| `supportsStrictMode` | Whether `strict: true` is accepted |
| `requiresToolResultName` | Whether tool results need `name` field |
| `hasReasoningContent` | Whether reasoning is in a separate field |
| `thinkingFormat` | How thinking blocks are formatted |
| `cacheControlFormat` | Which cache control format is used |

## Message Transformation

Before sending to the API, messages go through `transformMessages()`:

```typescript
// openai-completions.ts line 503
const transformedMessages = transformMessages(context.messages, model, normalizeToolCallId);
```

For OpenAI completions, `normalizeToolCallId` is a pass-through (OpenAI accepts any ID format).

## Key Patterns for foundation_ai

1. **Arguments as JSON string** -- not parsed object, must parse before use
2. **Incremental streaming** -- arguments accumulated fragment by fragment with best-effort parse
3. **Compat flags** handle dozens of OpenAI-compatible endpoints from a single provider
4. **hasToolHistory check** determines if tools parameter must be included (needed for Anthropic via proxy)
5. **Tool result name field** required by some providers (controlled by compat flag)
6. **Pass-through ID normalization** -- OpenAI accepts any ID format
