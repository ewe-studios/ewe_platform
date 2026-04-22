# Google Gemini Provider -- Deep Dive

## Files Referenced

- pi-mono: `packages/ai/src/providers/google.ts`
- pi-mono: `packages/ai/src/providers/google-shared.ts`

## Three Separate Implementations

pi-mono has three Google provider implementations:
1. **google-generative-ai** -- Google Generative AI API (`@google/generative-ai` SDK)
2. **google-gemini-cli** -- Google Gemini CLI API
3. **google-vertex** -- Google Vertex AI

All three share common code in `google-shared.ts`.

## Tool Definition Format

```typescript
// google-shared.ts: convertTools() at line 250
[{
    functionDeclarations: tools.map((tool) => ({
        name: tool.name,
        description: tool.description,
        ...(useParameters
            ? { parameters: tool.parameters }
            : { parametersJsonSchema: tool.parameters }),
    })),
}]
```

Key differences:
- Tools wrapped in `functionDeclarations` array
- Two schema formats: `parameters` (object) or `parametersJsonSchema` (JSON schema string)
- Single-element array containing the function declarations object

## Tool Call Output -- Direct Objects (Not JSON Strings)

```typescript
// google.ts at line 156
if (part.functionCall) {
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
```

Key difference: Google returns parsed JSON objects as arguments, not JSON strings. This requires serialization back to string for the internal representation.

## Tool Call Counter for ID Generation

Google does not return tool call IDs. The provider generates local IDs:

```typescript
let toolCallCounter = 0;
const toolCallId = `${part.functionCall.name}_${Date.now()}_${++toolCallCounter}`;
```

The ID format is `{name}_{timestamp}_{counter}` to ensure uniqueness.

## Thought Signature Handling

```typescript
// google-shared.ts at line 27
export function isThinkingPart(part): boolean {
    return part.thought === true;  // definitive marker, not thoughtSignature
}
// thoughtSignature can appear on ANY part type (text, functionCall, etc.)
// It does NOT indicate the part itself is thinking content
```

The `thoughtSignature` is an opaque signature that can be reused to continue the same "thought context" across turns. It can appear on any part type (text, functionCall, etc.), not just thinking parts.

## requiresToolCallId()

```typescript
// google-shared.ts at line 69
export function requiresToolCallId(modelId: string): boolean {
    return modelId.startsWith("claude-") || modelId.startsWith("gpt-oss-");
}
```

When Claude or GPT-oss models are accessed via Google's API, they require tool call IDs. Native Gemini models do not.

## Tool Choice Mapping

```typescript
// google-shared.ts at line 269
case "auto":  return FunctionCallingConfigMode.AUTO;
case "none":  return FunctionCallingConfigMode.NONE;
case "any":   return FunctionCallingConfigMode.ANY;
```

## Tool Result Format

```typescript
// google-shared.ts at line 208
const functionResponsePart = {
    functionResponse: {
        name: msg.toolName,
        response: msg.isError ? { error: responseValue } : { output: responseValue },
        ...(hasImages && modelSupportsMultimodalFunctionResponse && { parts: imageParts }),
        ...(includeId ? { id: msg.toolCallId } : {}),
    },
};
```

Error results use `{ error: responseValue }` format, success uses `{ output: responseValue }`.

## Streaming Behavior

Google does not stream tool calls incrementally. When a tool call is detected:
1. `toolcall_start` emitted
2. `toolcall_delta` emitted with the full arguments (JSON stringified)
3. `toolcall_end` emitted immediately

This is different from Anthropic/OpenAI which stream tool calls incrementally.

## Key Patterns for foundation_ai

1. **Direct object args** -- Google returns parsed JSON objects, not strings; must serialize for internal representation
2. **Local ID generation** -- Google doesn't return tool call IDs; must generate locally
3. **Thought signature** -- opaque string for reusing thought context across turns
4. **Three separate implementations** sharing common code in google-shared.ts
5. **Non-streaming tool calls** -- tool calls emitted as complete blocks, not incrementally
6. **requiresToolCallId()** -- Claude/GPT-oss behind Google APIs need IDs, native Gemini doesn't
7. **Error vs success response format** -- `{ error: ... }` vs `{ output: ... }`
