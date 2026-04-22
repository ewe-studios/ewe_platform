# Context Compression Behavior

## pi-mono Approach

### Trigger Condition

```
estimated_tokens > contextWindow - reserveTokens - keepRecentTokens
```

### Token Estimation

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

Uses the last known `usage.totalTokens` as an anchor, then character-based estimation (~4 chars per token) for subsequent messages.

### Compaction Process

1. Identify messages to compact (middle of conversation)
2. Call `completeSimple()` -- an LLM summarization call
3. Generate a summary of the compacted messages
4. Store file operations (read/modified) in compaction entries
5. Replace compacted messages with the summary

### Branch Summarization

When switching git branches:
- Generates a summary of the previous branch's conversation
- Stores it as a compaction entry
- Enables context continuity across branch switches

## hermes-agent Approach

### Trigger Condition

Default threshold: 50% of context window. When the conversation approaches this limit, compression is triggered.

### Protection Zones

| Zone | Size | Behavior |
|------|------|----------|
| Head | First 3 messages | Never compressed |
| Tail | Last ~20 messages or token budget | Never compressed |
| Middle | Everything between | Summarizable |

### Compression Process

1. Middle conversation turns identified for compression
2. Auxiliary model called to generate summary
3. Summary replaces the compressed turns
4. Head and tail remain intact

### Auxiliary Model Resolution

Side tasks (including compression) use a separate resolution chain:
1. OpenRouter
2. Nous Portal
3. Custom endpoint
4. Codex OAuth
5. Native Anthropic
6. Direct API-key providers

This ensures the best available model is used for compression, even if the primary model is different.

### Context Overflow Recovery

When context overflow occurs during an API call:
1. Trigger compression as error recovery
2. Context length probing via binary search
3. Retry with compressed context

## Key Differences

| Aspect | pi-mono | hermes-agent |
|--------|---------|-------------|
| Trigger | contextWindow - reserve - keepRecent | 50% threshold |
| Token estimation | Last usage + char-based estimation | Token budget tracking |
| Summarization | `completeSimple()` LLM call | Auxiliary model |
| Branch support | Git branch summarization | None |
| Recovery | Proactive (before limit) | Reactive (on overflow) |

## Key Patterns for foundation_ai

1. **Proactive vs reactive** -- pi-mono compresses before hitting the limit; hermes-agent compresses on overflow
2. **Token estimation** using last known usage + character-based estimation is pragmatic
3. **Protection zones** (head/tail) preserve critical context
4. **Auxiliary model** for compression may differ from primary model
5. **Branch summarization** is a unique pi-mono feature for git workflows
6. **Context length probing** via binary search for exact limits
