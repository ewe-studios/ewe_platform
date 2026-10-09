---
name: "Cost Calculation Core"
status: "completed"
priority: "high"
---

# Feature: Cost Calculation Core

## Description

Core cost calculation types, the `calculate_cost()` function, and `UsageCosting` status tracking. This is the foundation that both pre-call estimation and post-call actual cost depend on.

## Implementation

### CostStatus Enum

```rust
/// Status of a cost calculation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CostStatus {
    /// Calculated from pre-call token estimation.
    Estimated,
    /// Calculated from actual API response usage.
    Actual,
    /// Pricing unavailable for used tokens.
    Unknown,
}
```

### UsageCosting Extension

The existing `UsageCosting` struct gains a `status` field and a `total()` method:

```rust
/// [`UsageCosting`] represents the overall costing in actual currency value.
#[derive(From, Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct UsageCosting {
    pub currency: String,
    pub input: f64,
    pub output: f64,
    pub cache_read: f64,
    pub cache_write: f64,
    pub total_tokens: f64,
    /// Status of this cost calculation (Estimated, Actual, Unknown).
    pub status: CostStatus,
}

impl UsageCosting {
    /// Computed total USD cost.
    #[must_use]
    pub fn total(&self) -> f64 {
        self.input + self.output + self.cache_read + self.cache_write
    }

    /// Zeroed cost with the given status.
    pub fn zero(status: CostStatus) -> Self {
        Self {
            currency: String::from("USD"),
            input: 0.0,
            output: 0.0,
            cache_read: 0.0,
            cache_write: 0.0,
            total_tokens: 0.0,
            status,
        }
    }
}
```

### Model Trait Extension

The `Model` trait gains a `descriptor()` method to access the provider descriptor (which contains `ModelUsageCosting` pricing):

```rust
pub trait Model {
    // ... existing methods ...

    /// Returns the `ModelProviderDescriptor` for this model instance,
    /// which contains pricing (`ModelUsageCosting`), context window, etc.
    /// Returns `None` for dynamically loaded or local models without descriptor metadata.
    fn descriptor(&self) -> Option<ModelProviderDescriptor>;
}
```

`ModelProviderDescriptor` uses `&'static str` for its string fields, so cloning is cheap (pointer-only).

### CostAccumulator

A running total accumulator on each model instance:

```rust
/// Accumulates costs across multiple interactions.
#[derive(Debug, Clone, Default)]
pub struct CostAccumulator {
    input: f64,
    output: f64,
    cache_read: f64,
    cache_write: f64,
    call_count: u64,
}

impl CostAccumulator {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn add(&mut self, costing: &UsageCosting) {
        self.input += costing.input;
        self.output += costing.output;
        self.cache_read += costing.cache_read;
        self.cache_write += costing.cache_write;
        self.call_count += 1;
    }

    pub fn result(&self) -> UsageCosting {
        UsageCosting {
            currency: String::from("USD"),
            input: self.input,
            output: self.output,
            cache_read: self.cache_read,
            cache_write: self.cache_write,
            total_tokens: 0.0,
            status: if self.call_count > 0 { CostStatus::Actual } else { CostStatus::Unknown },
        }
    }

    pub fn call_count(&self) -> u64 {
        self.call_count
    }
}
```

### Core Calculation Function

```rust
/// Calculate cost from token usage and model pricing.
///
/// Formula: `(price_per_million / 1_000_000) * token_count`
/// for each of input, output, cache_read, cache_write.
///
/// If pricing for a used token type is zero (free model),
/// that component costs $0 but the status is still `Actual`.
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
```

### Changes to `UsageReport`

The existing `UsageReport.cost` field (type `UsageCosting`) should be populated with calculated values instead of remaining zeroed. `UsageCosting` gains a `status` field.

### File

`backends/foundation_ai/src/costing.rs` (new module, exposed in `lib.rs`)
