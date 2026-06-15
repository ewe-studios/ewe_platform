# hermes-agent Error Recovery

## File Location

`run_agent.py` -- within `run_conversation()` and API call methods

## Multi-Level Error Recovery

hermes-agent implements a hierarchy of error recovery strategies, tried in order:

### Level 1: Retry with Backoff

- Up to 3 retries per API call
- Exponential backoff between retries
- Applies to transient errors: timeouts, 5xx server errors, network issues

### Level 2: Fallback Provider

When retries are exhausted, the fallback chain is tried:
1. Primary provider (e.g., OpenRouter)
2. First fallback (e.g., Anthropic direct)
3. Second fallback (e.g., OpenAI direct)
4. Further fallbacks as configured

The fallback chain supports both:
- **Legacy format**: single dict with one fallback
- **New format**: list of fallback providers

### Level 3: Credential Rotation

When a specific credential is rate-limited or errored:
1. Current credential enters cooldown (1 hour for 429, 24 hours for others)
2. Next credential selected based on pool strategy (fill_first, round_robin, random, least_used)
3. Request retried with new credential
4. If all credentials in cooldown, proceed to fallback provider

### Level 4: Context Compression

When context overflow is detected (400/413 errors or token budget exceeded):
1. Trigger context compression
2. Middle conversation turns summarized via auxiliary model
3. Head (first 3 messages) and tail (last ~20 messages) protected
4. Retry the API call with compressed context

### Level 5: Context Length Probing

If compression isn't enough or the exact limit is unknown:
1. Binary search for the maximum token count
2. Reduce the context size incrementally
3. Retry with reduced context
4. If even minimal context exceeds the limit, abort

### Level 6: Abort

Unrecoverable errors:
- Client configuration errors (missing API key, invalid model)
- All fallbacks exhausted
- All credentials in cooldown
- Context too small to compress further

## Error Type Handling

| Error | Code | Handling |
|-------|------|----------|
| Rate limit | 429 | Backoff + credential rotation + fallback |
| Context overflow | 400/413 | Context compression + length probing |
| Bad request | 400 | Abort (usually configuration error) |
| Unauthorized | 401 | Credential rotation (next key) |
| Server error | 5xx | Retry with backoff |
| Timeout | - | Retry with backoff |
| Client error | - | Abort |

## Surrogate Character Sanitization

Before any API call, messages are sanitized:
- Unicode surrogate characters (from clipboard-pasted rich text) are stripped
- This prevents `json.dumps()` crashes that would otherwise abort the entire conversation

## Stream Health Monitoring

When using streaming:
- 90-second stale-stream detection
- If no event arrives for 90 seconds, the stream is considered failed
- Fallback to non-streaming or retry is triggered

## Key Patterns for foundation_ai

1. **Six-level error recovery hierarchy** provides multiple chances to recover before aborting
2. **Exponential backoff** for transient errors (up to 3 retries)
3. **Credential rotation** with cooldown periods (1h for 429, 24h for others)
4. **Context compression** as error recovery, not just proactive management
5. **Context length probing** via binary search when exact limit is unknown
6. **Surrogate sanitization** prevents JSON crashes from rich text
7. **Stream health monitoring** with 90s stale detection
8. **Abort on configuration errors** -- don't retry on permanent failures
