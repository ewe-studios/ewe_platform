//! Session-level token accounting & budget (Feature 04).
//!
//! WHY: the agentic loop needs two token measures that must NOT live in the
//! memory layer: a **cumulative session budget** (halt generation when exhausted,
//! recoverable) and a **rolling "recent interaction" count** (F15's ~30k
//! observation trigger). `foundation_ai` already produces a per-call
//! [`UsageReport`] on every assistant turn; this ledger **aggregates** those — it
//! never re-counts tokens (OD-04-10).
//!
//! WHAT: [`TokenLedger`] (Arc-shared, atomic interior, all `&self` so it is
//! valtron-task friendly), [`TokenSnapshot`] (carried by
//! `SessionRecord::Summary` and surfaced to the model/UI by F18).
//!
//! HOW: counters are `AtomicU64` (available on `wasm32-unknown-unknown`). The
//! budget basis is computed `input + output + cache_read + cache_write` — NOT a
//! provider `total_tokens` field, which is inconsistent across providers
//! (Anthropic reports input+output and excludes cache; `OpenAI` includes cache in
//! its total). `record` is called **once per turn**, not per emitted message: a
//! streaming turn clones one `UsageReport` onto thinking/text/each tool-call
//! message, so per-message recording would N×-count (OD-04-6).

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

use serde::{Deserialize, Serialize};

use crate::types::{ModelParams, UsageReport};

/// `u64::MAX` is the "unlimited budget" sentinel — `0` is a VALID budget meaning
/// "halt immediately".
const UNLIMITED: u64 = u64::MAX;

/// Cost is stored as integer micro-units (`cost * 1e6`) so it can live in an
/// atomic; converted back to `f64` in [`TokenSnapshot`].
const COST_SCALE: f64 = 1_000_000.0;

/// Session-level token accounting. Fed once per turn from the model's own
/// `UsageReport`. `Arc`-shared with atomic interior so every method is `&self`.
#[derive(Debug, Clone)]
pub struct TokenLedger {
    inner: Arc<TokenLedgerInner>,
}

#[derive(Debug)]
struct TokenLedgerInner {
    // Cumulative across the whole session (budget basis).
    total_input: AtomicU64,
    total_output: AtomicU64,
    total_cache_read: AtomicU64,
    total_cache_write: AtomicU64,
    total_cost_micros: AtomicU64,
    // Rolling counter (input+output) since the last memory reset — F15's 30k trigger.
    rolling_tokens: AtomicU64,
    // Ceiling; UNLIMITED sentinel = no budget.
    budget_max_tokens: AtomicU64,
}

impl Default for TokenLedger {
    fn default() -> Self {
        Self::new()
    }
}

impl TokenLedger {
    /// A fresh, unlimited-budget ledger.
    #[must_use]
    pub fn new() -> Self {
        Self {
            inner: Arc::new(TokenLedgerInner {
                total_input: AtomicU64::new(0),
                total_output: AtomicU64::new(0),
                total_cache_read: AtomicU64::new(0),
                total_cache_write: AtomicU64::new(0),
                total_cost_micros: AtomicU64::new(0),
                rolling_tokens: AtomicU64::new(0),
                budget_max_tokens: AtomicU64::new(UNLIMITED),
            }),
        }
    }

    /// A ledger with an initial budget ceiling (`None` = unlimited).
    #[must_use]
    pub fn with_budget(max_tokens: Option<u64>) -> Self {
        let ledger = Self::new();
        ledger.set_budget(max_tokens);
        ledger
    }

    /// Fold one turn's `UsageReport` into the ledger.
    ///
    /// **Call ONCE per model turn**, not per emitted assistant message (a
    /// streaming turn shares one `UsageReport` across thinking/text/tool-call
    /// messages — per-message would N×-count). `f64` token buckets are folded as
    /// `v.max(0.0).round() as u64`; cost as micro-units.
    pub fn record(&self, usage: &UsageReport) {
        let input = fold(usage.input);
        let output = fold(usage.output);
        let cache_read = fold(usage.cache_read);
        let cache_write = fold(usage.cache_write);

        self.inner.total_input.fetch_add(input, Ordering::Relaxed);
        self.inner.total_output.fetch_add(output, Ordering::Relaxed);
        self.inner
            .total_cache_read
            .fetch_add(cache_read, Ordering::Relaxed);
        self.inner
            .total_cache_write
            .fetch_add(cache_write, Ordering::Relaxed);
        // Rolling counts input+output of recent turns (OD-04-3).
        self.inner
            .rolling_tokens
            .fetch_add(input + output, Ordering::Relaxed);

        // Cost is non-negative; round() fits in u64 for any realistic cost.
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        let cost_micros = (cost_of(usage) * COST_SCALE).round() as u64;
        self.inner
            .total_cost_micros
            .fetch_add(cost_micros, Ordering::Relaxed);
    }

    /// Cumulative session total — the budget basis
    /// (`input + output + cache_read + cache_write`).
    #[must_use]
    pub fn total(&self) -> u64 {
        self.inner.total_input.load(Ordering::Relaxed)
            + self.inner.total_output.load(Ordering::Relaxed)
            + self.inner.total_cache_read.load(Ordering::Relaxed)
            + self.inner.total_cache_write.load(Ordering::Relaxed)
    }

    /// Cumulative input tokens.
    #[must_use]
    pub fn input(&self) -> u64 {
        self.inner.total_input.load(Ordering::Relaxed)
    }

    /// Cumulative output tokens.
    #[must_use]
    pub fn output(&self) -> u64 {
        self.inner.total_output.load(Ordering::Relaxed)
    }

    /// Cumulative cost in currency units.
    #[must_use]
    pub fn cost(&self) -> f64 {
        // u64 → f64: precision loss acceptable for cost display.
        #[allow(clippy::cast_precision_loss)]
        let micros = self.inner.total_cost_micros.load(Ordering::Relaxed) as f64;
        micros / COST_SCALE
    }

    /// Rolling count (input+output) since the last memory reset. F15 reads this
    /// for the ~30k recent-interaction observation trigger.
    #[must_use]
    pub fn rolling(&self) -> u64 {
        self.inner.rolling_tokens.load(Ordering::Relaxed)
    }

    /// Reset the rolling counter (F15 calls this after an observation/reflection
    /// condenses the context).
    pub fn reset_rolling(&self) {
        self.inner.rolling_tokens.store(0, Ordering::Relaxed);
    }

    /// Set (or clear) the session budget ceiling. `None` = unlimited.
    pub fn set_budget(&self, max_tokens: Option<u64>) {
        let v = max_tokens.unwrap_or(UNLIMITED);
        self.inner.budget_max_tokens.store(v, Ordering::Relaxed);
    }

    /// The current budget ceiling (`None` = unlimited).
    #[must_use]
    pub fn budget(&self) -> Option<u64> {
        match self.inner.budget_max_tokens.load(Ordering::Relaxed) {
            UNLIMITED => None,
            v => Some(v),
        }
    }

    /// Remaining tokens before the budget is hit (`None` = unlimited; saturates
    /// at 0 when over budget).
    #[must_use]
    pub fn remaining(&self) -> Option<u64> {
        self.budget().map(|b| b.saturating_sub(self.total()))
    }

    /// Whether the session has reached/exceeded its budget. Always `false` when
    /// the budget is unlimited.
    #[must_use]
    pub fn is_exhausted(&self) -> bool {
        match self.budget() {
            None => false,
            Some(b) => self.total() >= b,
        }
    }

    /// A point-in-time snapshot for `SessionRecord::Summary` and F18 surfacing.
    #[must_use]
    pub fn snapshot(&self) -> TokenSnapshot {
        TokenSnapshot {
            total: self.total(),
            input: self.input(),
            output: self.output(),
            rolling: self.rolling(),
            budget: self.budget(),
            remaining: self.remaining(),
            cost: self.cost(),
        }
    }

    /// Clamp a model call's `max_tokens` to the remaining budget so a single
    /// request can't overshoot (OD-04-2). Defense-in-depth; the loop-level
    /// [`is_exhausted`](Self::is_exhausted) halt is the primary guard.
    #[must_use]
    pub fn effective_max_tokens(&self, params: &ModelParams) -> usize {
        match self.remaining() {
            None => params.max_tokens,
            // u64 → usize: on 32-bit targets this may truncate, but budget
            // values that large are unreachable in practice.
            #[allow(clippy::cast_possible_truncation)]
            Some(rem) => params.max_tokens.min(rem as usize),
        }
    }
}

/// A point-in-time view of the session ledger.
///
/// Carried by `SessionRecord::Summary { usage: TokenSnapshot }` (F01) and
/// surfaced to the model/UI (F18), so it derives the `SessionRecord`-compatible
/// set. `cost: f64` ⇒ `PartialEq` only (no `Eq`/`Hash`), matching `SessionRecord`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TokenSnapshot {
    /// Cumulative session total (`input + output + cache_read + cache_write`).
    pub total: u64,
    /// Cumulative input tokens.
    pub input: u64,
    /// Cumulative output tokens.
    pub output: u64,
    /// Rolling count since the last memory reset.
    pub rolling: u64,
    /// Budget ceiling (`None` = unlimited).
    pub budget: Option<u64>,
    /// Remaining tokens before the budget is hit (`None` = unlimited).
    pub remaining: Option<u64>,
    /// Cumulative cost in currency units.
    pub cost: f64,
}

/// Fold an `f64` token count into a `u64` (counts are integral; clamp negatives).
fn fold(v: f64) -> u64 {
    // Non-negative after max(0.0); truncation acceptable for token counts.
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    let result = v.max(0.0).round() as u64;
    result
}

/// Total cost (currency units) of a `UsageReport`, read from its `UsageCosting`.
fn cost_of(usage: &UsageReport) -> f64 {
    let c = &usage.cost;
    (c.input + c.output + c.cache_read + c.cache_write).max(0.0)
}

