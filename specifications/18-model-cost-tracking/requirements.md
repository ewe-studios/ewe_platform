---
description: "Implement runtime token usage estimation and actual cost tracking in foundation_ai using ModelUsageCosting from model descriptors, with pre-call estimation and post-call actual cost reporting"
status: "pending"
priority: "high"
created: 2026-05-01
author: "Main Agent"
metadata:
  version: "1.0"
  last_updated: 2026-05-01
  estimated_effort: "medium"
  tags:
    - cost-tracking
    - token-usage
    - model-descriptors
    - pricing
  skills:
    - specifications-management
    - rust-patterns
  tools:
    - Rust
    - cargo
has_features: true
has_fundamentals: false
builds_on:
  - "07-foundation-ai"
related_specs: []
features:
  completed: 3
  uncompleted: 1
  in_progress: 0
  total: 4
  completion_percentage: 75%
---

# Model Cost Tracking

## Overview

foundation_ai currently extracts token counts from API responses into `UsageReport` but hardcodes all `UsageCosting` fields to `0.0`. The pricing data already exists in `ModelUsageCosting` on each `ModelProviderDescriptor` in `backends/foundation_ai/src/models/providers/` (per-million-token rates for input, output, cache_read, cache_write). This spec adds:

1. **Pre-call token estimation** — estimate token usage from a `Vec<Message>` without calling the model, using runtime heuristics.
2. **Post-call cost calculation** — compute actual USD cost from usage tokens × model pricing. Each `ModelInteraction` carries its own `UsageCosting`.
3. **Cumulative model costing** — each model instance accumulates cost across all interactions it has served. `Model::costing()` returns the running total for the lifetime of that model.
4. **Message list cost** — a method that takes `&[Message]` and calculates total cost by summing `UsageCosting` from all matching interactions.

Inspired by Pi's `calculateCost()` (simple formula with pricing on `Model<T>`) and Hermes's multi-layer pipeline (normalization → billing route → pricing entry → session accumulation).

## Goals

- Add `estimate_cost(&self, messages: &[Message]) -> CostResult` for pre-call estimation
- Each `ModelInteraction` carries its own `UsageCosting` populated after generation
- Add `costing(&self) -> CostResult` on the model trait — cumulative across ALL interactions for the model's lifetime
- Add `message_cost(&self, messages: &[Message]) -> CostResult` — sum costs from a message list
- Fix the existing gap: populate `UsageCosting` on `UsageReport` with calculated values
- Use `ModelUsageCosting` from `ModelProviderDescriptor` for pricing (per 1M tokens)
- Apply the formula: `(price_per_million / 1_000_000) * token_count` for each bucket
- Local models (llamacpp, candle) calculate estimated token counts but $0 cost
- `CostAccumulator` running total on each model provider struct

## Non-Goals

- Live pricing API fetching (pricing comes from static `ModelUsageCosting`)
- Account quota/usage monitoring (separate future spec)
- Billing exhaustion detection (separate future spec)
- Session persistence to database (separate future spec)
- Service tier multipliers (separate future spec)

## Design Decisions

### 1. Cost formula

Matches Pi's approach exactly, as it is simple and correct:

```
cost.input  = (model.cost.input  / 1_000_000) * usage.input
cost.output = (model.cost.output / 1_000_000) * usage.output
cost.cache_read  = (model.cost.cache_read  / 1_000_000) * usage.cache_read
cost.cache_write = (model.cost.cache_write / 1_000_000) * usage.cache_write
cost.total = cost.input + cost.output + cost.cache_read + cost.cache_write
```

### 2. Decimal arithmetic

Python Hermes uses `Decimal` to avoid float accumulation. Rust equivalent: use `f64` for individual calculations but accumulate via a dedicated `UsageAccumulator` that tracks running totals separately from per-call reports. No external Decimal crate needed for this scope — the spec is bounded to per-interaction accumulation, not long-running session totals.

### 3. Pre-call estimation

For providers that don't report tokens (local models), or for pre-flight budget checks. Strategy inspired by Hermes's context engine:

- Text content: `len(content) / 4` characters per token heuristic
- Image content: ~1000 tokens per image
- Message structure overhead: ~4 tokens per message
- This is fast and tokenizer-free; providers may override with real tokenizers later

### 4. Pricing resolution

Single source: `ModelUsageCosting` from `ModelProviderDescriptor`. No billing route resolution needed — the provider already knows which model it's serving and has the descriptor with pricing. If pricing is all zeros (free/local model), cost is `0.0`.

### 5. Cost status

`UsageCosting` gains a `status` field:
- `Estimated` — from pre-call estimation
- `Actual` — from post-call API response usage
- `Unknown` — pricing data unavailable for used token type

## Implementation Location

- `backends/foundation_ai/src/types/mod.rs` — new types and trait methods
- `backends/foundation_ai/src/costing.rs` — new module for cost calculation
- `backends/foundation_ai/src/backends/*.rs` — per-provider updates

## Tasks

### Feature 01: Cost Calculation Core

- [ ] Create `backends/foundation_ai/src/costing.rs` with `CostStatus`, `CostResult`, and `calculate_cost()` function
- [ ] Add `total` and `status` fields to `UsageCosting` in `types/mod.rs`
- [ ] Export costing module from `lib.rs`
- [ ] Write unit tests for `calculate_cost()` with known token/pricing inputs

### Feature 02: Token Estimation

- [ ] Add `estimate_tokens(messages: &[Message]) -> UsageReport` to costing module
- [ ] Add `estimate_cost(&self, interaction: &ModelInteraction)` to `ModelProvider` trait
- [ ] Implement default trait method in trait definition
- [ ] Write unit tests for token estimation with text-only and multipart messages

### Feature 03: Provider Cost Tracking

- [ ] Add `get_cost(&self) -> CostResult` to `ModelProvider` trait
- [ ] Update `anthropic_messages_provider.rs` — populate cost on UsageReport, implement trait methods
- [ ] Update `openai_provider.rs` — populate cost on UsageReport, implement trait methods
- [ ] Update `openai_responses_provider.rs` — populate cost on UsageReport, implement trait methods
- [ ] Update `llamacpp.rs` — implement estimation-based cost (zero USD)
- [ ] Update `candle.rs` — implement estimation-based cost (zero USD)
- [ ] Update `huggingface_gguf_provider.rs` — implement estimation-based cost (zero USD)
- [ ] Update `huggingface_candle_provider.rs` — implement estimation-based cost (zero USD)
- [ ] Write integration tests verifying cost flows through at least one provider

### Feature 04: Provider Prompt Assembly

- [ ] Update `anthropic_messages_provider.rs` — system prompt + soul as combined system, tool_shed tools as tools array, messages mapped
- [ ] Update `openai_provider.rs` — system prompt message, soul as second system message, tool_shed tools as function definitions, messages mapped
- [ ] Update `openai_responses_provider.rs` — system prompt + soul as combined instructions, tool_shed tools, messages mapped
- [ ] Update `llamacpp.rs` — system prompt + soul as system message in chat template, tool_shed instructions injected, messages through chat template
- [ ] Update `candle.rs` — system prompt, soul, tool_shed instructions as `System:` prefixes, messages as `User:/Assistant:` lines
- [ ] Remove all references to `interaction.tools` (replaced by `tools_shed` flattening)
- [ ] Verify compilation across all providers after changes
