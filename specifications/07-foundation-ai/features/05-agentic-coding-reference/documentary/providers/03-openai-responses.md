# OpenAI Responses API -- Deep Dive

## Files Referenced

- pi-mono: `packages/ai/src/providers/openai-responses-shared.ts`
- pi-mono: `packages/ai/src/providers/openai-responses.ts`

## Fundamental Difference from Chat Completions

The Responses API uses a completely different format:
- `function_call` items instead of `tool_calls` array
- `function_call_output` items for results
- Responses are structured as `response.output` arrays of items

## Tool Definition Format

```typescript
// convertResponsesTools() at line 261
{
    type: "function",
    name: tool.name,
    description: tool.description,
    parameters: tool.parameters as any,
    strict: false,  // or configurable
}
```

Note: No `function` wrapper -- `name`, `description`, `parameters` are top-level fields.

## Tool Call Output -- Composite IDs

```typescript
// line 187
output.push({
    type: "function_call",
    id: itemId,         // the "fc_xxx" item ID
    call_id: callId,    // the actual tool call identifier
    name: toolCall.name,
    arguments: JSON.stringify(toolCall.arguments),
});
```

Tool calls get composite IDs: `call_id|item_id` (pipe-separated). The `item_id` must start with "fc" for the Responses API.

## Composite ID Handling

```typescript
// convertResponsesMessages() at line 100
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
```

The composite ID must be preserved and re-normalized when sending messages back to the Responses API.

## Tool Result Format

```typescript
// line 210
messages.push({
    type: "function_call_output",
    call_id: callId,  // just the call_id part, NOT the composite
    output: sanitizeSurrogates(hasText ? textResult : "(see attached image)"),
});
```

Tool results use only the `call_id` part, not the composite `call_id|item_id`.

## Streaming Events

```typescript
// processResponsesStream() at line 276
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
```

Event types differ from Chat Completions:
- `response.output_item.added` -- new function_call item
- `response.function_call_arguments.delta` -- incremental arguments
- `response.function_call_arguments.done` -- arguments complete

## Stop Reason Mapping

```typescript
case "completed":    return "stop";
case "incomplete":   return "length";
case "failed":
case "cancelled":    return "error";
```

Override behavior: if content has tool calls but stop is "stop", override to "toolUse":
```typescript
if (output.content.some((b) => b.type === "toolCall") && output.stopReason === "stop") {
    output.stopReason = "toolUse";
}
```

## Codex Adapter (hermes-agent)

hermes-agent implements `_CodexCompletionsAdapter` that translates `chat.completions.create()` calls to the Responses API format internally. This allows the rest of the code to use the Chat Completions interface while the adapter handles the Responses API details.

## Key Patterns for foundation_ai

1. **Different format entirely** -- `function_call` items, not `tool_calls` array
2. **Composite IDs** -- `call_id|item_id` format, item must start with "fc"
3. **Tool results use only call_id** -- not the composite
4. **Different streaming events** -- `response.output_item.added`, `response.function_call_arguments.delta`
5. **Stop reason override** -- tool calls in content with stop="stop" becomes toolUse
6. **Codex adapter pattern** -- translate Chat Completions calls to Responses API internally
