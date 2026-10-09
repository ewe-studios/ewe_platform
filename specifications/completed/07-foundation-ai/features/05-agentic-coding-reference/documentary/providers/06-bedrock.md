# AWS Bedrock Provider -- Deep Dive

## Files Referenced

- pi-mono: `packages/ai/src/providers/amazon-bedrock.ts`

## Node.js Only

The Bedrock provider uses the AWS SDK, which requires Node.js. It is gated at the module level and not available in browser or Bun environments.

## Tool Definition Format

```typescript
// convertToolConfig() at line 652
{
    tools: bedrockTools.map((tool) => ({
        toolSpec: {
            name: tool.name,
            description: tool.description,
            inputSchema: { json: tool.parameters },
        },
    })),
    toolChoice: bedrockToolChoice,
}
```

Tools are wrapped in `toolSpec` objects, and `toolChoice` is a separate field.

## Tool Choice Mapping

```typescript
switch (toolChoice) {
    case "auto":  bedrockToolChoice = { auto: {} };
    case "any":   bedrockToolChoice = { any: {} };
    case tool:    bedrockToolChoice = { tool: { name: toolChoice.name } };
}
```

## Tool Call Output

```typescript
// handleContentBlockStart() at line 263
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
```

## Streaming

```typescript
// handleContentBlockDelta() at line 286
else if (delta?.toolUse && block?.type === "toolCall") {
    block.partialJson = (block.partialJson || "") + (delta.toolUse.input || "");
    block.arguments = parseStreamingJson(block.partialJson);
}
```

Same incremental JSON parsing pattern as other providers.

## Tool Result Format

```typescript
// convertMessages() at line 586
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
```

All consecutive tool results are collected into a single user message.

## Tool ID Normalization

```typescript
// normalizeToolCallId() -- sanitize to 64 chars
function normalizeToolCallId(id: string): string {
    return id.replace(/[^a-zA-Z0-9_-]/g, "_").slice(0, 64);
}
```

Same constraint as Anthropic: `[a-zA-Z0-9_-]+`, max 64 chars.

## Stop Reason Mapping

```typescript
case BedrockStopReason.END_TURN:
case BedrockStopReason.STOP_SEQUENCE:    return "stop";
case BedrockStopReason.MAX_TOKENS:
case BedrockStopReason.MODEL_CONTEXT_WINDOW_EXCEEDED: return "length";
case BedrockStopReason.TOOL_USE:         return "toolUse";
```

## Message Transformation

```typescript
// amazon-bedrock.ts line 499
const transformedMessages = transformMessages(context.messages, model, normalizeToolCallId);
```

Same `transformMessages` call as other providers, with Bedrock-specific normalizer.

## AWS Credential Checking

Bedrock checks for AWS credentials / Application Default Credentials (ADC) instead of a simple API key:
- `AWS_ACCESS_KEY_ID`, `AWS_SECRET_ACCESS_KEY`
- `AWS_PROFILE`
- ADC via `~/.aws/credentials` or IAM roles

## Key Patterns for foundation_ai

1. **toolSpec wrapper** -- tools wrapped in `{ toolSpec: { name, description, inputSchema: { json } } }`
2. **toolChoice as separate field** -- not part of the tools array
3. **toolResult with status** -- `ToolResultStatus.ERROR` or `ToolResultStatus.SUCCESS`
4. **Node.js only** -- requires AWS SDK, not available in browser/Bun
5. **AWS credentials** -- not simple API key, uses AWS credential chain
6. **Consecutive tool results merged** into single user message
7. **Same ID constraint as Anthropic** -- sanitized, max 64 chars
