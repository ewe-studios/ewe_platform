//! Cost calculation for model token usage.
//!
//! # Why this module exists
//!
//! The `foundation_ai` crate already extracts token counts from provider API
//! responses into `UsageReport`, but all `UsageCosting` fields were hard-coded
//! to `$0.0`. The pricing data already lives in `ModelUsageCosting` on each
//! `ModelProviderDescriptor` (per-million-token rates for input, output,
//! `cache_read`, `cache_write`). This module bridges the gap.
//!
//! # Design rationale
//!
//! **Single type for costs.** We extend the existing `UsageCosting` with a
//! `status` field (`Estimated`, `Actual`, `Unknown`) rather than creating a
//! separate `CostResult` type. The two types would be structurally identical
//! — the only difference is whether the numbers came from estimation or real
//! API responses. The `CostStatus` enum captures that distinction cleanly.
//!
//! **Simple formula.** Each token bucket is priced independently at a rate
//! per million tokens. The formula `(price_per_million / 1_000_000) * count`
//! is intentionally straightforward — it matches how providers publish their
//! pricing pages. No tiered pricing, no discounts, no currency conversion.
//!
//! **`f64` over `Decimal`.** Python implementations (e.g., Hermes) use
//! `Decimal` to avoid float accumulation drift. We use `f64` for individual
//! calculations because the scope is bounded to per-interaction costs (small
//! dollar amounts, limited precision needs). The `CostAccumulator` tracks
//! running totals separately from per-call reports, so accumulation error
//! stays negligible even across hundreds of calls. If this ever needs to
//! become a billing-grade system, swap to `rust_decimal` or `bigdecimal`.
//!
//! **Character-based estimation.** Not all providers have tokenizers, and
//! even when they do, we need pre-call estimates (for budget checks before
//! sending the request). The `estimate_tokens` function uses empirically
//! derived heuristics (chars-per-token ratios) that are fast and
//! tokenizer-free. The constants are documented inline with their origins.
//!
//! # How costs flow through the system
//!
//! 1. **Pre-call**: `estimate_tokens(messages)` → `UsageReport` with
//!    `status: Estimated` and `$0` cost. Used for budget checks.
//!
//! 2. **Post-call**: Provider returns `UsageReport` with real token counts.
//!    `calculate_cost(pricing, usage, Actual)` → `UsageCosting` with real
//!    USD amounts. This is attached to every `ModelInteraction` response.
//!
//! 3. **Accumulation**: `CostAccumulator` on each model instance adds up
//!    `UsageCosting` across all interactions. `Model::costing()` returns
//!    the running total for the model's lifetime.
//!
//! 4. **Message list**: `message_cost(&[Messages])` sums the cost from all
//!    assistant messages in a list, useful for retrospective cost analysis
//!    of a conversation or session.
//!
//! # What is NOT in scope
//!
//! - Live pricing API fetching (pricing is static from `ModelUsageCosting`)
//! - Account quota / usage monitoring
//! - Billing exhaustion detection
//! - Session persistence to database
//! - Service tier multipliers
//!
//! These are separate future specs.

use crate::types::{CostStatus, Messages, ModelOutput, ModelUsageCosting, UsageCosting, UsageReport};

/// Calculate cost from token usage and model pricing.
///
/// Formula: `(price_per_million / 1_000_000) * token_count`
/// for each of input, output, `cache_read`, `cache_write`.
///
/// If pricing for a used token type is zero (free model),
/// that component costs $0 but the status is still `Actual`.
#[must_use]
pub fn calculate_cost(
    pricing: &ModelUsageCosting,
    usage: &UsageReport,
    status: CostStatus,
) -> UsageCosting {
    let input = (pricing.input / 1_000_000.0) * usage.input;
    let output = (pricing.output / 1_000_000.0) * usage.output;
    let cache_read = (pricing.cache_read / 1_000_000.0) * usage.cache_read;
    let cache_write = (pricing.cache_write / 1_000_000.0) * usage.cache_write;

    UsageCosting {
        currency: String::from("USD"),
        input,
        output,
        cache_read,
        cache_write,
        total_tokens: usage.total_tokens,
        status,
    }
}

/// Accumulates costs across multiple interactions.
#[derive(Debug, Clone, Default)]
pub struct CostAccumulator {
    input: f64,
    output: f64,
    cache_read: f64,
    cache_write: f64,
    total_tokens: f64,
    call_count: u64,
}

impl CostAccumulator {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    pub fn add(&mut self, costing: &UsageCosting) {
        self.input += costing.input;
        self.output += costing.output;
        self.cache_read += costing.cache_read;
        self.cache_write += costing.cache_write;
        self.total_tokens += costing.total_tokens;
        self.call_count += 1;
    }

    #[must_use]
    pub fn result(&self) -> UsageCosting {
        UsageCosting {
            currency: String::from("USD"),
            input: self.input,
            output: self.output,
            cache_read: self.cache_read,
            cache_write: self.cache_write,
            total_tokens: self.total_tokens,
            status: if self.call_count > 0 {
                CostStatus::Actual
            } else {
                CostStatus::Unknown
            },
        }
    }

    #[must_use]
    pub fn call_count(&self) -> u64 {
        self.call_count
    }
}

/// Estimate token usage from messages without calling the model.
///
/// Uses character-based heuristics when a tokenizer is not available.
/// All constants are empirical approximations — providers with real
/// tokenizers should override.
///
/// Heuristic constants:
/// - **4.0 chars/token**: English text average (GPT-2/Observed).
///   Derived from ~300 tokens per 1200 chars of typical English prose.
/// - **1000.0 tokens/image**: Rough cost of a standard-resolution image
///   in multimodal models (e.g., GPT-4V low-res, Claude vision).
/// - **20.0 base64-chars/token**: Base64 is already an entropy-dense
///   encoding; at ~6 bits/char vs ~4 bits/char for natural English
///   text, the compression is lower, so we use a higher chars-per-token
///   divisor as a conservative over-estimate.
/// - **4.0 tokens/message overhead**: Role tokens, template formatting
///   (e.g., `<|im_start|>user<|im_end|>`).
/// - **100.0 values/token**: Embedding vectors are dense float arrays;
///   each float ≈ 4 chars, but the semantic compression is high,
///   so we use a very aggressive chars-per-token ratio.
#[must_use]
#[allow(clippy::cast_precision_loss)]
pub fn estimate_tokens(messages: &[Messages]) -> UsageReport {
    use crate::types::{UserModelContent};

    // Chars per token for natural English text.
    const CHARS_PER_TOKEN: f64 = 4.0;
    // Estimated token cost for a single image.
    const TOKENS_PER_IMAGE: f64 = 1000.0;
    // Chars per token for base64-encoded image data.
    const BASE64_CHARS_PER_TOKEN: f64 = 20.0;
    // Overhead tokens per message (role, template formatting).
    const MESSAGE_OVERHEAD: f64 = 4.0;
    // Float values per token for embedding vectors.
    const EMBED_VALUES_PER_TOKEN: f64 = 100.0;

    let mut input: f64 = 0.0;
    let mut images: f64 = 0.0;

    for msg in messages {
        input += MESSAGE_OVERHEAD;

        match msg {
            Messages::User { content, .. }
            | Messages::ToolResult { content, .. } => match content {
                UserModelContent::Text(tc) => {
                    input += tc.content.len() as f64 / CHARS_PER_TOKEN;
                }
                UserModelContent::Image(img) => {
                    images += TOKENS_PER_IMAGE;
                    input += img.b64.len() as f64 / BASE64_CHARS_PER_TOKEN;
                }
            },
            Messages::Assistant { content, .. } => match content {
                ModelOutput::Text(tc) => {
                    input += tc.content.len() as f64 / CHARS_PER_TOKEN;
                }
                ModelOutput::ThinkingContent { thinking, .. } => {
                    input += thinking.len() as f64 / CHARS_PER_TOKEN;
                }
                ModelOutput::ToolCall { arguments, .. } => {
                    if let Some(args) = arguments {
                        let json_str = serde_json::to_string(args).unwrap_or_default();
                        input += json_str.len() as f64 / CHARS_PER_TOKEN;
                    }
                }
                ModelOutput::Image(img) => {
                    images += TOKENS_PER_IMAGE;
                    input += img.b64.len() as f64 / BASE64_CHARS_PER_TOKEN;
                }
                ModelOutput::Embedding { values, .. } => {
                    input += values.len() as f64 / EMBED_VALUES_PER_TOKEN;
                }
            },
        }
    }

    UsageReport {
        input,
        output: 0.0,
        cache_read: 0.0,
        // Worst case: all input is new cache writes.
        cache_write: input,
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

/// Sum the total cost of a list of messages.
#[must_use]
pub fn message_cost(messages: &[Messages]) -> UsageCosting {
    let mut input = 0.0;
    let mut output = 0.0;
    let mut cache_read = 0.0;
    let mut cache_write = 0.0;

    for msg in messages {
        if let Messages::Assistant { usage, .. } = msg {
            input += usage.cost.input;
            output += usage.cost.output;
            cache_read += usage.cost.cache_read;
            cache_write += usage.cost.cache_write;
        }
    }

    UsageCosting {
        currency: String::from("USD"),
        input,
        output,
        cache_read,
        cache_write,
        total_tokens: 0.0,
        status: CostStatus::Actual,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_calculate_cost_basic() {
        let pricing = ModelUsageCosting {
            input: 3.0,
            output: 15.0,
            cache_read: 0.3,
            cache_write: 3.75,
        };
        let usage = UsageReport {
            input: 1000.0,
            output: 500.0,
            cache_read: 800.0,
            cache_write: 1000.0,
            total_tokens: 1500.0,
            cost: UsageCosting::zero(CostStatus::Actual),
        };

        let result = calculate_cost(&pricing, &usage, CostStatus::Actual);
        assert!((result.input - 0.003).abs() < 1e-9);
        assert!((result.output - 0.0075).abs() < 1e-9);
        assert!((result.cache_read - 0.00024).abs() < 1e-9);
        assert!((result.cache_write - 0.00375).abs() < 1e-9);
        assert_eq!(result.status, CostStatus::Actual);
    }

    #[test]
    fn test_calculate_cost_free_model() {
        let pricing = ModelUsageCosting {
            input: 0.0,
            output: 0.0,
            cache_read: 0.0,
            cache_write: 0.0,
        };
        let usage = UsageReport {
            input: 1000.0,
            output: 500.0,
            cache_read: 0.0,
            cache_write: 0.0,
            total_tokens: 1500.0,
            cost: UsageCosting::zero(CostStatus::Actual),
        };

        let result = calculate_cost(&pricing, &usage, CostStatus::Actual);
        assert_eq!(result.total(), 0.0);
        assert_eq!(result.status, CostStatus::Actual);
    }

    #[test]
    fn test_usage_costing_zero() {
        let c = UsageCosting::zero(CostStatus::Estimated);
        assert_eq!(c.total(), 0.0);
        assert_eq!(c.status, CostStatus::Estimated);
    }

    #[test]
    fn test_cost_accumulator() {
        let mut acc = CostAccumulator::new();
        assert_eq!(acc.call_count(), 0);

        let c1 = UsageCosting {
            currency: String::from("USD"),
            input: 0.003,
            output: 0.0075,
            cache_read: 0.0,
            cache_write: 0.00375,
            total_tokens: 1000.0,
            status: CostStatus::Actual,
        };
        acc.add(&c1);
        assert_eq!(acc.call_count(), 1);

        let c2 = UsageCosting {
            currency: String::from("USD"),
            input: 0.006,
            output: 0.015,
            cache_read: 0.001,
            cache_write: 0.0075,
            total_tokens: 2000.0,
            status: CostStatus::Actual,
        };
        acc.add(&c2);
        assert_eq!(acc.call_count(), 2);

        let result = acc.result();
        assert!((result.input - 0.009).abs() < 1e-9);
        assert!((result.output - 0.0225).abs() < 1e-9);
        assert!((result.cache_read - 0.001).abs() < 1e-9);
        assert!((result.cache_write - 0.01125).abs() < 1e-9);
        assert!((result.total() - 0.04375).abs() < 1e-9);
        assert_eq!(result.status, CostStatus::Actual);
    }

    #[test]
    fn test_cost_accumulator_empty() {
        let acc = CostAccumulator::new();
        let result = acc.result();
        assert_eq!(result.total(), 0.0);
        assert_eq!(result.status, CostStatus::Unknown);
    }

    #[test]
    fn test_estimate_tokens_text_only() {
        use crate::types::{ModelId, ModelProviders, StopReason, TextContent, UserModelContent};

        let messages = vec![
            Messages::User {
                role: "user".to_string(),
                content: UserModelContent::Text(TextContent {
                    content: "Hello, world!".to_string(),
                    signature: None,
                }),
                signature: None,
            },
            Messages::Assistant {
                model: ModelId::Name("test".to_string(), None),
                timestamp: std::time::SystemTime::now(),
                usage: UsageReport {
                    input: 0.0,
                    output: 0.0,
                    cache_read: 0.0,
                    cache_write: 0.0,
                    total_tokens: 0.0,
                    cost: UsageCosting::zero(CostStatus::Actual),
                },
                content: ModelOutput::Text(TextContent {
                    content: "Hi there!".to_string(),
                    signature: None,
                }),
                stop_reason: StopReason::Stop,
                provider: ModelProviders::ANTHROPIC,
                error_detail: None,
                signature: None,
                metadata: None,
            },
        ];

        let report = estimate_tokens(&messages);
        // 2 messages * 4 overhead = 8
        // "Hello, world!" = 13 chars / 4 = 3.25
        // "Hi there!" = 9 chars / 4 = 2.25
        // input = 8 + 3.25 + 2.25 = 13.5
        assert!((report.input - 13.5).abs() < 0.01);
        assert_eq!(report.output, 0.0);
    }

    #[test]
    fn test_estimate_tokens_with_image() {
        use crate::types::{ImageContent, MimeType, UserModelContent};

        let messages = vec![Messages::User {
            role: "user".to_string(),
            content: UserModelContent::Image(ImageContent {
                b64: "base64data".to_string(),
                mime_type: MimeType::ImagePng,
            }),
            signature: None,
        }];

        let report = estimate_tokens(&messages);
        // 1 message * 4 overhead = 4
        // base64data = 10 chars / 20 = 0.5
        // input = 4.5
        // images = 1000
        assert!((report.input - 4.5).abs() < 0.01);
        assert!((report.total_tokens - 1004.5).abs() < 0.01);
    }

    #[test]
    fn test_message_cost() {
        use crate::types::{ModelId, ModelProviders, StopReason, TextContent};

        let messages = vec![
            Messages::Assistant {
                model: ModelId::Name("test".to_string(), None),
                timestamp: std::time::SystemTime::now(),
                usage: UsageReport {
                    input: 100.0,
                    output: 50.0,
                    cache_read: 80.0,
                    cache_write: 100.0,
                    total_tokens: 150.0,
                    cost: UsageCosting {
                        currency: String::from("USD"),
                        input: 0.003,
                        output: 0.0075,
                        cache_read: 0.0,
                        cache_write: 0.00375,
                        total_tokens: 150.0,
                        status: CostStatus::Actual,
                    },
                },
                content: ModelOutput::Text(TextContent {
                    content: "Hi".to_string(),
                    signature: None,
                }),
                stop_reason: StopReason::Stop,
                provider: ModelProviders::ANTHROPIC,
                error_detail: None,
                signature: None,
                metadata: None,
            },
        ];

        let cost = message_cost(&messages);
        assert_eq!(cost.input, 0.003);
        assert_eq!(cost.output, 0.0075);
        assert_eq!(cost.cache_read, 0.0);
        assert_eq!(cost.cache_write, 0.00375);
        assert_eq!(cost.status, CostStatus::Actual);
    }
}
