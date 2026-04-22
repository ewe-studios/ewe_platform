# Streaming Behavior

## pi-mono Streaming

### Preference

Streaming is always preferred when available. The `stream()` function is the primary interface, with `streamSimple()` providing a simplified wrapper.

### Event Protocol

```typescript
type AssistantMessageEvent =
  | { type: "toolcall_start"; contentIndex: number; partial: AssistantMessage }
  | { type: "toolcall_delta"; contentIndex: number; delta: string; partial: AssistantMessage }
  | { type: "toolcall_end"; contentIndex: number; toolCall: ToolCall; partial: AssistantMessage }
  | { type: "text_delta"; delta: string; partial: AssistantMessage }
  | { type: "done"; reason: StopReason; message: AssistantMessage }
  | { type: "error"; reason: StopReason; error: AssistantMessage };
```

### Per-Provider Stream Parsing

| Provider | Stream Format | Parser |
|----------|--------------|--------|
| Anthropic | SSE with `RawMessageStreamEvent` | Custom SSE decoder |
| OpenAI Completions | `ChatCompletionChunk` async iterator | OpenAI SDK |
| OpenAI Responses | Server-sent events with typed events | Custom parser |
| Google | SDK async iterator | Google SDK |
| Bedrock | Converse Stream events | AWS SDK |
| Mistral | SSE with delta chunks | Custom SSE decoder |

### Tool Call Streaming

All providers follow the same pattern:
1. Receive delta chunk with partial tool call data
2. Accumulate partial JSON fragment into a buffer
3. Call `parseStreamingJson(buffer)` for best-effort parse
4. Emit `toolcall_start` / `toolcall_delta` / `toolcall_end` events
5. Final arguments assembled when stream completes

## hermes-agent Streaming

### Preference

Streaming is preferred even without display consumers:
- Used for health checking (90s stale-stream detection)
- Falls back to non-streaming for mock clients in tests

### Health Monitoring

- 90-second stale-stream detection
- If no streaming event arrives for 90 seconds, the stream is considered failed
- Fallback to non-streaming or retry is triggered

### Non-Streaming Fallback

When streaming fails or is unavailable:
- Falls back to non-streaming API call
- Same response parsing and processing logic
- No real-time updates, but functionality preserved

## Key Patterns for foundation_ai

1. **Streaming always preferred** for real-time updates and health checking
2. **Unified event protocol** enables multiple UI backends
3. **Per-provider stream parsers** handle different SSE/chunk formats
4. **90s stale detection** for health monitoring
5. **Non-streaming fallback** when streaming fails
6. **Incremental JSON parsing** for tool call streaming with `parseStreamingJson()`
