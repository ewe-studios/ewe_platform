# Mistral Provider -- Deep Dive

## Files Referenced

- pi-mono: `packages/ai/src/providers/mistral.ts`

## 9-Char Tool Call ID Constraint

Mistral requires tool call IDs to be exactly 9 alphanumeric characters.

## Hash-Based ID Derivation

```typescript
// mistral.ts at line 144
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
```

### Collision Handling

The normalizer maintains two maps:
- `idMap`: original ID -> derived 9-char ID
- `reverseMap`: derived 9-char ID -> original ID (for collision detection)

If a hash collision occurs (two different original IDs hash to the same 9-char ID), the attempt counter increments to produce a different hash.

### Fast Path

If the original ID is already exactly 9 alphanumeric characters, it passes through unchanged (attempt 0).

## Tool Definition Format

```typescript
// toFunctionTools() at line 437
{
    type: "function",
    function: {
        name: tool.name,
        description: tool.description,
        parameters: tool.parameters as unknown as Record<string, unknown>,
        strict: false,
    },
}
```

OpenAI-compatible format.

## Thinking Block Format

```typescript
// toChatMessages() at line 486
{
    type: "thinking",
    thinking: [{ type: "text", text: sanitizeSurrogates(block.thinking) }],
}
```

Thinking blocks have a nested structure: `thinking` array of text parts.

## Streaming Tool Calls

```typescript
// consumeChatStream() at line 372
const toolCalls = delta.toolCalls || [];
for (const toolCall of toolCalls) {
    const callId = toolCall.id && toolCall.id !== "null"
        ? toolCall.id
        : deriveMistralToolCallId(`toolcall:${toolCall.index ?? 0}`, 0);
    // ... accumulate partialArgs, parseStreamingJson, emit deltas
}
```

If the tool call ID is missing or the string "null", a local ID is derived from the tool call index.

## Stop Reason Mapping

```typescript
case "stop":              return "stop";
case "length":
case "model_length":      return "length";
case "tool_calls":        return "toolUse";
case "error":             return "error";
```

## Key Patterns for foundation_ai

1. **Exactly 9-char IDs** -- strict constraint requiring hash-based derivation
2. **Collision detection via reverse map** -- incrementing attempt counter on collision
3. **Fast path** -- IDs already 9 alphanumeric chars pass through unchanged
4. **OpenAI-compatible tool format** -- same as Chat Completions with `type: "function"` wrapper
5. **Nested thinking blocks** -- `thinking: [{ type: "text", text: "..." }]`
6. **Fallback ID generation** -- derive from tool call index if ID is missing
7. **Two length stop reasons** -- "length" and "model_length" both map to length
