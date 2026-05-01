---
name: "Token Estimation"
status: "completed"
priority: "high"
---

# Feature: Token Estimation

## Description

Pre-call token usage estimation from a `Vec<Messages>` without calling the model tokenizer or API. Used for budget checks, context window validation, and local models that don't report tokens.

## Inspiration

- **Hermes context engine**: `len(content) // 4` for text, ~1K tokens for images, `len(messages) * 4` overhead
- **Pi faux provider**: character-based heuristic with cache-aware prefix comparison

## Design

```rust
/// Estimate token usage from messages without calling the model.
///
/// Uses character-based heuristics:
/// - Text: ~4 chars per token (English average)
/// - Images: ~1000 tokens per image
/// - Message structure: ~4 tokens per message
///
/// Returns a `UsageReport` with estimated token counts and
/// `CostStatus::Estimated` on the cost field.
pub fn estimate_tokens(messages: &[Messages]) -> UsageReport {
    let mut input: f64 = 0.0;
    let mut images: f64 = 0.0;

    for msg in messages {
        // Message structure overhead
        input += 4.0;

        match msg {
            Messages::User { content, .. } => match content {
                UserModelContent::Text(tc) => {
                    input += tc.content.len() as f64 / 4.0;
                }
                UserModelContent::Image(img) => {
                    images += 1000.0;
                    input += img.b64.len() as f64 / 20.0;
                }
            },
            Messages::Assistant { content, .. } => {
                match content {
                    ModelOutput::Text(tc) => {
                        input += tc.content.len() as f64 / 4.0;
                    }
                    ModelOutput::ThinkingContent { thinking, .. } => {
                        input += thinking.len() as f64 / 4.0;
                    }
                    ModelOutput::ToolCall { arguments, .. } => {
                        if let Some(args) = arguments {
                            let json_str = serde_json::to_string(args).unwrap_or_default();
                            input += json_str.len() as f64 / 4.0;
                        }
                    }
                    ModelOutput::Image(img) => {
                        images += 1000.0;
                        input += img.b64.len() as f64 / 20.0;
                    }
                    _ => {}
                }
            }
            Messages::ToolResult { content, .. } => {
                match content {
                    UserModelContent::Text(tc) => {
                        input += tc.content.len() as f64 / 4.0;
                    }
                    UserModelContent::Image(img) => {
                        images += 1000.0;
                        input += img.b64.len() as f64 / 20.0;
                    }
                }
            }
        }
    }

    UsageReport {
        input,
        output: 0.0,
        cache_read: 0.0,
        cache_write: input, // worst case: all input is new cache writes
        total_tokens: input + images,
        cost: UsageCosting {
            currency: String::from("USD"),
            input: 0.0,
            output: 0.0,
            cache_read: 0.0,
            cache_write: 0.0,
            total_tokens: input + images,
            status: CostStatus::Estimated,
        },
    }
}
```

## Provider Override

Providers may override the default heuristic with real tokenizer-based estimation:

```rust
/// Estimate tokens for this interaction. Default uses character heuristics.
/// Providers with access to a tokenizer should override for accuracy.
fn estimate_tokens(&self, messages: &[Messages]) -> UsageReport {
    estimate_tokens(messages)
}
```

## File

`backends/foundation_ai/src/costing.rs` (extends the costing module)
