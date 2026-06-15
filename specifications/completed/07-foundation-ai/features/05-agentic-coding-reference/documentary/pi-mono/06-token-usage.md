# pi-mono Token Usage and Cost Tracking

## File Locations

- Usage types: `packages/ai/src/types.ts` (lines 178-191)
- Cost calculation: `packages/ai/src/models.ts` (lines 39-46)
- Context estimation: `packages/coding-agent/src/core/compaction/compaction.ts`

## Usage Structure

```typescript
interface Usage {
  input: number;       // prompt tokens (excluding cache)
  output: number;      // completion tokens (including reasoning)
  cacheRead: number;   // cache hits from previous requests
  cacheWrite: number;  // tokens written to cache this request
  totalTokens: number; // input + output + cacheRead + cacheWrite
  cost: {
    input: number;     // $ cost for input tokens
    output: number;    // $ cost for output tokens
    cacheRead: number; // $ cost for cache reads
    cacheWrite: number;// $ cost for cache writes
    total: number;     // total $ cost
  };
}
```

## Per-Provider Token Parsing

### Anthropic Provider

Tokens are captured from two streaming events:

1. **`message_start`** event:
   - `input_tokens` -- total input tokens
   - `output_tokens` -- total output tokens
   - `cache_read_input_tokens` -- cache hit tokens
   - `cache_creation_input_tokens` -- tokens written to cache

2. **`message_delta`** event (final usage update):
   - `output_tokens` -- updated output count

Anthropic does not provide `total_tokens` -- it is computed as:
```
totalTokens = input + output + cacheRead + cacheWrite
```

### OpenAI Completions Provider

Tokens come from `chunk.usage`:
- `prompt_tokens` -- includes cache hits
- `completion_tokens` -- includes reasoning tokens
- `prompt_tokens_details.cached_tokens` -- cache hit count
- `completion_tokens_details.reasoning_tokens` -- reasoning token count

Normalization:
```
cacheRead = cached_tokens
cacheWrite = 0 (OpenAI doesn't write cache in this API)
input = prompt_tokens - cached_tokens
output = completion_tokens
reasoning = reasoning_tokens (added to output)
```

## Cost Calculation

Cost is calculated at the point of usage, using per-model pricing from the model descriptor:

```typescript
usage.cost.input = (model.cost.input / 1000000) * usage.input;
usage.cost.output = (model.cost.output / 1000000) * usage.output;
usage.cost.cacheRead = (model.cost.cacheRead / 1000000) * usage.cacheRead;
usage.cost.cacheWrite = (model.cost.cacheWrite / 1000000) * usage.cacheWrite;
usage.cost.total = sum of all above;
```

Pricing is per million tokens, stored in the model descriptor. This means cost calculation is a simple multiplication -- no API calls or lookups needed.

## Context Token Estimation

For compaction decisions, the system estimates total context tokens:

```typescript
estimateContextTokens(messages): number {
  // Find last assistant message with usage data
  const lastWithUsage = findLast(msg => msg.usage?.totalTokens);
  let total = lastWithUsage.usage.totalTokens;

  // Estimate tokens for messages after that point
  for (const msg of messagesAfter(lastWithUsage)) {
    total += estimateTokensFromChars(serializeMessage(msg));
  }

  return total;
}
```

The `estimateTokensFromChars` function uses a simple character-to-token ratio (typically ~4 chars per token for English text).

## Compaction Trigger

Compaction is triggered when:
```
estimated_tokens > contextWindow - reserveTokens - keepRecentTokens
```

Where:
- `contextWindow` -- from model descriptor
- `reserveTokens` -- buffer to avoid hitting the limit mid-request
- `keepRecentTokens` -- tokens reserved for recent conversation context

## Usage Accumulation Across Turns

The agent loop accumulates usage across turns:
- Each API call returns a `Usage` object
- Usage is emitted in the `turn_end` event
- The caller (AgentSession) accumulates total usage for the session

There is no explicit session-level counter in pi-mono -- usage is tracked per-turn and accumulated by the session manager.

## Key Patterns for foundation_ai

1. **Per-model pricing** in the descriptor enables instant cost calculation without external lookups
2. **Usage captured from streaming events** (message_start + message_delta for Anthropic, chunk.usage for OpenAI)
3. **Computed totalTokens** when provider doesn't provide it (Anthropic)
4. **Character-based estimation** for messages without usage data is pragmatic
5. **No session-level counter** -- usage is per-turn, accumulated by the session manager
6. **Cost is a derived field** on Usage, not a separate tracking system
