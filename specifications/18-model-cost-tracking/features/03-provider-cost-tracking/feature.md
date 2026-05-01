---
name: "Provider Cost Tracking"
status: "completed"
priority: "high"
---

# Feature: Provider Cost Tracking

## Description

Every provider computes and returns actual cost from API response usage. Each `ModelInteraction` produces messages carrying their own `UsageReport` with calculated `UsageCosting`. The model instance accumulates cost across all interactions it has served — `Model::costing()` returns the running total for the lifetime of that model. Local models (llamacpp, candle) have $0 token pricing but still calculate and report estimated token counts.

## Design

### ModelInteraction Owns Its Costing

Each `ModelInteraction` produces `Vec<Messages>` where each message carries a `UsageReport` with calculated `UsageCosting`:

```rust
pub struct ModelInteraction {
    // ... existing fields ...
    // UsageReport.cost carries UsageCosting populated after generation.
}
```

### Model Trait Extension

The `Model` trait already has `costing(&self) -> GenerationResult<UsageReport>`. Providers should return cumulative cost from their `CostAccumulator`.

### Cumulative Accumulation on the Model

Each provider struct holds a running total:

```rust
pub struct AnthropicModel<R: DnsResolver> {
    // ... existing fields ...
    cumulative_cost: CostAccumulator,
}

impl<R> AnthropicModel<R> {
    fn costing(&self) -> GenerationResult<UsageReport> {
        Ok(UsageReport {
            input: 0.0,
            output: 0.0,
            cache_read: 0.0,
            cache_write: 0.0,
            total_tokens: 0.0,
            cost: self.cumulative_cost.result(),
        })
    }
}
```

### Per-Provider Interaction Flow

Each provider, after a successful generation:

1. Extracts token counts from the API response (already done)
2. Looks up `ModelUsageCosting` from its `ModelProviderDescriptor`
3. Calls `calculate_cost(pricing, usage, CostStatus::Actual)`
4. Sets the result on `report.cost`
5. Accumulates into `self.cumulative_cost`
6. `self.costing()` returns the running total at any time

Example for Anthropic:

```rust
// After generation completes:
let cost = calculate_cost(&self.descriptor().cost, &usage, CostStatus::Actual);
report.cost = cost.clone();
self.cumulative_cost.add(&cost);
```

### Provider Update List

All providers need these updates:

1. `anthropic_messages_provider.rs`
2. `openai_provider.rs`
3. `openai_responses_provider.rs`
4. `llamacpp.rs`
5. `candle.rs`
6. `huggingface_gguf_provider.rs`
7. `huggingface_candle_provider.rs`

For local models (llamacpp, candle, huggingface):
- Token cost per 1M tokens is `$0` → `calculate_cost()` returns `$0` USD
- Token counts are still estimated via `estimate_tokens()` for context window management
- `costing()` returns `UsageCosting` with token counts populated but all USD fields at `0.0`
- Status is `Actual` (we know the exact token estimate and the exact cost is $0)

### UsageReport Cost Population

The existing `UsageReport` creation in each provider should be updated to call `calculate_cost()` instead of zeroing:

```rust
// Before (all zeros):
cost: UsageCosting { currency: "USD", input: 0.0, output: 0.0, ..., status: CostStatus::Actual }

// After:
let cost = calculate_cost(&descriptor.cost, &usage, CostStatus::Actual);
report.cost = cost;
self.cumulative_cost.add(&cost);
```

### Message List Cost

Since each `Messages` already carries its own `UsageReport` with calculated `UsageCosting`, summing the cost of a message list is straightforward:

```rust
/// Sum the total cost of a list of messages.
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
```

## File

Each provider file in `backends/foundation_ai/src/backends/`
