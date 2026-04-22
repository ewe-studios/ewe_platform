# hermes-agent Message Handling

## File Locations

- Main loop message building: `run_agent.py` (lines ~6882+)
- Anthropic adapter: `agent/anthropic_adapter.py`
- Prompt builder: `agent/prompt_builder.py`

## Message Format

Messages follow OpenAI-style format:

```python
{
    "role": "system" | "user" | "assistant" | "tool",
    "content": str | list,
    # Optional fields:
    "tool_calls": [...],       # For assistant messages with tool calls
    "tool_call_id": "...",     # For tool result messages
    "name": "...",             # Optional, for some providers
}
```

## Reasoning Content Extraction

Reasoning/thinking content is extracted from multiple provider formats and stored in `assistant_msg["reasoning"]`:

| Format | Source |
|--------|--------|
| `.reasoning` | Direct field (some providers) |
| `.reasoning_content` | OpenAI-compatible responses |
| `.reasoning_details` | Array of reasoning blocks |
| Inline XML tags | `<think>`, `<thinking>`, `<reasoning>`, `<REASONING_SCRATCHPAD>` |

The extraction normalizes all these into a single internal representation.

## System Prompt Assembly

The system prompt is built once and cached. It includes:
1. **Identity** -- Agent identity and role
2. **Platform hints** -- Platform-specific operational guidance
3. **Skills index** -- Available skills and their descriptions
4. **Context files** -- Loaded project files referenced in the prompt
5. **Memory** -- Memory provider results
6. **Ephemeral prompts** -- Per-turn injected prompts

### Model-Specific Directives

The prompt builder adds model-specific operational directives:
- Google models: specific tool-use guidance
- OpenAI models: different formatting requirements
- Anthropic models: strict role alternation reminders

## Message Sanitization

Before each API call, messages are sanitized:

### Strip Orphaned Tool Results

Tool result messages with no matching tool call in the conversation history are removed. This prevents sending dangling results that the API won't recognize.

### Strip Orphaned Tool Use Blocks

Assistant messages with tool_use blocks that have no corresponding tool result are cleaned up. This handles interrupted conversations where the tool was called but the result was never received.

### Role Alternation (Anthropic)

For Anthropic API calls, strict `user/assistant/user/assistant` alternation is enforced:
- Consecutive messages of the same role are merged
- Empty assistant messages get placeholder content
- Multiple tool results are merged into a single user message

### Surrogate Character Sanitization

Characters that would cause JSON serialization crashes (e.g., from clipboard-pasted rich text) are sanitized:
```python
def sanitize_surrogates(text: str) -> str:
    # Strip or replace Unicode surrogate characters
    # Prevents json.dumps() crashes
```

## Budget Warnings

Budget warnings are injected into tool-result JSON rather than as separate messages:

```python
tool_result["metadata"] = {
    "budget_warning": f"{remaining} iterations remaining"
}
```

This design choice avoids breaking the cache prefix pattern -- adding a separate system message would invalidate Anthropic's cached prefixes.

## Oversized Tool Results

Tool results exceeding 100K characters are handled specially:
1. The full result is saved to a temporary file
2. An inline preview (first ~1K chars) is included in the tool result message
3. A reference to the temp file path is provided for full content access

This prevents a single verbose tool output from consuming most of the context window.

## Prompt Caching (Anthropic)

When using the Anthropic API:
- System prompt blocks get `cache_control: { type: "ephemeral" }` markers
- Long context files get cache markers
- The cache prefix pattern is maintained across turns
- Budget warnings in tool results (not system messages) preserve the cache prefix

## Key Patterns for foundation_ai

1. **OpenAI-format internal representation** -- all tools, messages, responses use OpenAI format; providers convert at the boundary
2. **Multi-format reasoning extraction** -- handles `.reasoning`, `.reasoning_content`, `.reasoning_details`, and inline XML tags
3. **System prompt caching** -- built once, reused across turns with ephemeral injections
4. **Budget warnings in tool results** avoids cache invalidation for Anthropic
5. **Oversized result truncation** with temp file saves prevents context explosion
6. **Surrogate character sanitization** prevents JSON crashes from rich text
7. **Role alternation enforcement** for Anthropic during message sanitization
