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

use crate::types::base_types::{
    CostStatus, Messages, ModelOutput, ModelUsageCosting, UsageCosting, UsageReport,
    UserModelContent,
};

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
            Messages::User { content, .. } | Messages::ToolResult { content, .. } => {
                match content {
                    UserModelContent::Text(tc) => {
                        input += tc.content.len() as f64 / CHARS_PER_TOKEN;
                    }
                    UserModelContent::Image(img) => {
                        images += TOKENS_PER_IMAGE;
                        input += img.b64.len() as f64 / BASE64_CHARS_PER_TOKEN;
                    }
                }
            }
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

