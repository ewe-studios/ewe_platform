# hermes-agent Main Loop

## File Location

`run_agent.py` -- `AIAgent.run_conversation()` at line ~6882

## The Core Loop

```python
while api_call_count < self.max_iterations and self.iteration_budget.remaining > 0:
    # 1. Build api_messages (system prompt + conversation history + ephemeral injections)
    # 2. Apply prompt caching (Anthropic), sanitize messages
    # 3. Make API call (streaming preferred, with retry logic)
    # 4. Parse response - check finish_reason, validate structure
    # 5. If tool_calls: execute them, append results, continue
    # 6. If no tool_calls: final response, break
```

## IterationBudget

Thread-safe counter controlling the maximum number of LLM turns:
- **Default**: 90 iterations
- **Subagents**: Get their own budget (default 50)
- **Refunded turns**: `execute_code` turns don't consume budget (they're "refunded")
- **Remaining check**: `self.iteration_budget.remaining > 0` is checked each loop iteration

The budget is separate from `max_iterations` -- both conditions must be true to continue.

## Retry Logic

Up to 3 retries per API call with exponential backoff. Handles:

| Error | Handling |
|-------|----------|
| Rate limit (429) | Backoff + fallback provider + credential rotation |
| Context overflow | Triggers context compression + context length probing |
| Payload too large (413) | Triggers context compression |
| Client errors | Abort (unrecoverable) |
| Timeout | Retry with backoff |

## Fallback Chain

Ordered list of backup providers tried when the primary is exhausted:

1. Primary provider (e.g., OpenRouter)
2. First fallback (e.g., Anthropic direct)
3. Second fallback (e.g., OpenAI direct)
4. Further fallbacks as configured

Supports both legacy single-dict fallback format and new list format.

## Context Compression Trigger

When approaching the model's context limit:
- **Default threshold**: 50% of context window
- **Action**: Middle conversation turns are summarized using an auxiliary model
- **Head protection**: First 3 messages are never compressed
- **Tail protection**: Last ~20 messages (or token budget) are never compressed

## Streaming vs Non-Streaming

Streaming is preferred even without display consumers:
- Used for health checking (90s stale-stream detection)
- Falls back to non-streaming for mock clients in tests
- Health monitoring: if no streaming event arrives for 90 seconds, the stream is considered stale

## Message Building Per Turn

Each turn, `api_messages` is built from:
1. System prompt (cached once, includes identity, platform hints, skills index, context files, memory, ephemeral prompts)
2. Conversation history (all previous user/assistant/tool messages)
3. Ephemeral injections (budget warnings, memory nudges, etc.)

### Anthropic Cache Control

When using Anthropic, prompt caching markers are applied:
- `cache_control: { type: "ephemeral" }` on specific content blocks
- System prompt blocks get cache markers
- Long context files get cache markers
- This enables Anthropic's prompt caching feature for faster/cheaper repeated calls

### Message Sanitization

Before each API call:
- Strip orphaned tool results (tool result with no matching tool call)
- Strip orphaned tool_use blocks (tool call with no result)
- Ensure role alternation for Anthropic
- Sanitize surrogate characters (prevents JSON serialization crashes from clipboard-pasted rich text)

## Budget Warning Injection

Budget warnings are injected into tool-result JSON, not as separate messages:
```json
{
  "result": "...",
  "metadata": {
    "budget_warning": "10 iterations remaining"
  }
}
```

This avoids breaking the cache prefix pattern (adding a separate system message would invalidate cached prefixes).

## Oversized Tool Results

Tool results exceeding 100K characters:
1. Saved to a temporary file
2. Inline preview included in the tool result message
3. Reference to the temp file for full content

This prevents context explosion from very verbose tool outputs.

## Post-Turn Processing

After each turn (whether tool calls or final response):
1. Background memory/skill review spawned in thread
2. Session persisted to JSON + SQLite
3. Context compression check for next turn

## Error Recovery Flow

```
API call fails
  ├── Retry (up to 3 times with exponential backoff)
  ├── Fallback provider (try next in chain)
  ├── Credential rotation (next key in pool)
  ├── Context compression (if context overflow)
  ├── Context length probing (binary search for max tokens)
  └── Abort (unrecoverable error)
```

## Key Patterns for foundation_ai

1. **Dual budget** (max_iterations + iteration_budget) provides two levers for controlling turn count
2. **Refunded turns** for specific tool types (execute_code) don't consume budget
3. **Budget warnings in tool results** avoids cache invalidation
4. **Streaming for health checking** even when no display is needed
5. **Oversized result truncation** with temp file saves prevents context explosion
6. **Background memory review** after each turn is a post-processing hook
7. **Surrogate character sanitization** prevents JSON crashes from rich text
