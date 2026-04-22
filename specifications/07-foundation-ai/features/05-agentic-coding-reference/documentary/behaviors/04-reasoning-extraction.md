# Reasoning Extraction Behavior

## Multi-Format Reasoning Extraction

Both projects handle multiple formats for reasoning/thinking content from different providers.

### Source Formats

| Format | Provider | Field |
|--------|----------|-------|
| `.reasoning` | Some providers | Direct field on message |
| `.reasoning_content` | OpenAI-compatible | Separate field in response |
| `.reasoning_details` | Array format | Array of reasoning blocks |
| `<think>` tags | Text-based models | Inline XML |
| `<thinking>` tags | Text-based models | Inline XML |
| `<reasoning>` tags | Text-based models | Inline XML |
| `<REASONING_SCRATCHPAD>` | Text-based models | Inline XML |

### pi-mono Approach

Thinking content is represented as `ThinkingContent` blocks within `AssistantMessage.content`:

```typescript
interface ThinkingContent {
  type: "thinking";
  thinking: string;
  redacted?: boolean;  // Opaque encrypted content
}
```

Cross-model replay rules:
- **Redacted thinking**: Only valid for same model, dropped for cross-model
- **Empty thinking**: Always removed
- **Normal thinking**: Converted to text for different model replay

### hermes-agent Approach

Reasoning content is stored in `assistant_msg["reasoning"]` as a string:

```python
# Extracted from multiple possible locations
reasoning = extract_reasoning(response)
assistant_msg["reasoning"] = reasoning
```

The extraction function checks all possible formats in order.

## Key Patterns for foundation_ai

1. **Multiple source formats** must be normalized to a single internal representation
2. **Redacted thinking** is opaque encrypted content -- only valid for same model
3. **Empty thinking blocks** should be removed
4. **Cross-model conversion** -- thinking blocks become text when replaying to different model
5. **Inline XML tags** in text-based models must be parsed out of text content
