# pi-mono Message Handling

## File Locations

- Message types: `packages/ai/src/types.ts`
- Message transformation: `packages/ai/src/providers/transform-messages.ts`
- Provider-specific message building: each `providers/*.ts` file

## Message Types

```typescript
type Message = UserMessage | AssistantMessage | ToolResultMessage;

interface UserMessage {
  role: "user";
  content: (TextContent | ImageContent)[];
}

interface AssistantMessage {
  role: "assistant";
  content: ContentBlock[];  // TextContent, ThinkingContent, ToolCall
  usage?: Usage;
  stopReason?: StopReason;
}

interface ToolResultMessage {
  role: "toolResult";
  toolCallId: string;
  toolName: string;
  content: (TextContent | ImageContent)[];
  details?: TDetails;
  isError: boolean;
  timestamp: number;
}
```

## Content Block Types

| Type | Purpose | Provider Notes |
|------|---------|----------------|
| `TextContent` | Plain text | Universal |
| `ThinkingContent` | Reasoning/thinking blocks | Provider-specific handling |
| `ToolCall` | Tool call with id, name, arguments | ID normalization per provider |
| `ImageContent` | Inline images | Some providers downgrade to text |

## The transformMessages Function

The core message transformation layer (`transform-messages.ts`) handles cross-provider normalization:

```typescript
transformMessages(messages, model, normalizeToolCallId?)
```

It performs these operations:

### 1. Tool ID Normalization

Each provider constructs its own normalizer callback:

| Provider | Normalizer |
|----------|------------|
| Anthropic | `id.replace(/[^a-zA-Z0-9_-]/g, "_").slice(0, 64)` |
| Mistral | Hash-based 9-char derivation with collision handling |
| OpenAI Responses | Split pipe-separated IDs, sanitize |
| Google | Only for Claude/GPT-oss behind Google APIs |
| OpenAI Completions | Pass-through (accepts any ID) |

The normalizer is applied in a two-pass process:
1. First pass: normalize IDs on assistant messages, build a mapping
2. Second pass: apply the same mapping to tool result messages

### 2. Thinking Block Conversion

```typescript
if (block.type === "thinking") {
  if (block.redacted) {
    return isSameModel ? block : [];  // Drop redacted thinking for cross-model
  }
  if (!block.thinking || block.thinking.trim() === "") return [];  // Skip empty
  if (isSameModel) return block;  // Keep as-is for same model
  return { type: "text", text: block.thinking };  // Convert to text for different model
}
```

Key rules:
- **Redacted thinking** (opaque encrypted content) is only valid for the same model -- dropped for cross-model replay
- **Empty thinking blocks** are always removed
- **Normal thinking** is converted to plain text when replaying to a different model

### 3. Synthetic Tool Result Insertion

For orphaned tool calls (tool call with no matching result):

```typescript
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
```

This prevents API errors from dangling tool calls without results.

### 4. Error/Abort Filtering

```typescript
if (assistantMsg.stopReason === "error" || assistantMsg.stopReason === "aborted") {
  continue;  // Skip errored/aborted assistant messages entirely
}
```

Errored or aborted assistant messages are incomplete turns that shouldn't be replayed. They may have partial content (reasoning without message, incomplete tool calls) that causes API errors on replay.

### 5. Image Downgrading

For providers that don't support images:
- Images are replaced with placeholder text: `"[Image: {description}]"`
- The original image content is lost in the transformation

## Provider-Specific Message Building

Each provider calls `transformMessages` before sending to the API, then does provider-specific post-processing:

### Anthropic
- Requires strict role alternation (user/assistant/user/assistant)
- Tool results are placed in user messages as `tool_result` content blocks
- Multiple tool results can be in a single content block
- Empty assistant content must be replaced with placeholder

### OpenAI Completions
- Supports role alternation but less strict than Anthropic
- Tool results are separate `tool` role messages with `tool_call_id`
- Tool calls are in the `tool_calls` array on assistant messages

### Google
- Function responses use `functionResponse` parts with name and response object
- Arguments are direct JSON objects, not strings
- Thought signatures can appear on any part type

### Bedrock
- Tool results are `toolResult` objects with `toolUseId` and content array
- All consecutive tool results collected into single user message

### Mistral
- Similar to OpenAI Completions
- 9-char tool call ID constraint applied during transformation

## Role Alternation Enforcement

For providers that require strict alternation (especially Anthropic), the transformation ensures:
1. No two consecutive messages have the same role
2. Tool results are wrapped in user messages
3. Empty assistant messages get placeholder content
4. Consecutive tool results are merged into a single user message

## Key Patterns for foundation_ai

1. **transformMessages with callback** is elegant -- each provider provides its own normalizer, keeping provider-specific logic isolated
2. **Two-pass ID normalization** ensures both assistant and tool result messages use consistent IDs
3. **Thinking block conversion rules** (drop redacted, convert to text for cross-model) are important for model switching
4. **Synthetic tool results** for orphaned calls prevent API errors
5. **Error/abort filtering** prevents replaying incomplete turns
6. **Role alternation enforcement** is provider-specific -- some need it strict, some don't
