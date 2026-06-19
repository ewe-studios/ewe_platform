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
        self.inner.total_cost_micros.load(Ordering::Relaxed) as f64 / COST_SCALE
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
    v.max(0.0).round() as u64
}

/// Total cost (currency units) of a `UsageReport`, read from its `UsageCosting`.
fn cost_of(usage: &UsageReport) -> f64 {
    let c = &usage.cost;
    (c.input + c.output + c.cache_read + c.cache_write).max(0.0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{CostStatus, UsageCosting};

    fn usage(input: f64, output: f64, cost: f64) -> UsageReport {
        UsageReport {
            input,
            output,
            cache_read: 0.0,
            cache_write: 0.0,
            total_tokens: input + output,
            cost: UsageCosting {
                currency: "USD".into(),
                input: cost,
                output: 0.0,
                cache_read: 0.0,
                cache_write: 0.0,
                total_tokens: cost,
                status: CostStatus::Actual,
            },
        }
    }

    #[test]
    fn records_accumulate_across_turns() {
        let ledger = TokenLedger::new();
        ledger.record(&usage(100.0, 50.0, 0.01));
        ledger.record(&usage(20.0, 10.0, 0.002));
        assert_eq!(ledger.input(), 120);
        assert_eq!(ledger.output(), 60);
        assert_eq!(ledger.total(), 180);
        assert_eq!(ledger.rolling(), 180);
        assert!((ledger.cost() - 0.012).abs() < 1e-6);
    }

    #[test]
    fn rolling_resets_independently_of_total() {
        let ledger = TokenLedger::new();
        ledger.record(&usage(100.0, 50.0, 0.0));
        ledger.reset_rolling();
        assert_eq!(ledger.rolling(), 0);
        // Total is unaffected by a rolling reset.
        assert_eq!(ledger.total(), 150);
        ledger.record(&usage(10.0, 5.0, 0.0));
        assert_eq!(ledger.rolling(), 15);
        assert_eq!(ledger.total(), 165);
    }

    #[test]
    fn budget_exhaustion_boundary() {
        let ledger = TokenLedger::with_budget(Some(100));
        assert!(!ledger.is_exhausted());
        assert_eq!(ledger.remaining(), Some(100));
        ledger.record(&usage(60.0, 30.0, 0.0)); // total 90
        assert!(!ledger.is_exhausted());
        assert_eq!(ledger.remaining(), Some(10));
        ledger.record(&usage(10.0, 0.0, 0.0)); // total 100 — exactly at budget
        assert!(ledger.is_exhausted());
        assert_eq!(ledger.remaining(), Some(0));
    }

    #[test]
    fn zero_budget_halts_immediately() {
        let ledger = TokenLedger::with_budget(Some(0));
        // 0 is a valid budget — exhausted before any tokens are spent.
        assert!(ledger.is_exhausted());
        assert_eq!(ledger.remaining(), Some(0));
    }

    #[test]
    fn unlimited_budget_never_exhausts() {
        let ledger = TokenLedger::new();
        ledger.record(&usage(1_000_000.0, 1_000_000.0, 99.0));
        assert!(!ledger.is_exhausted());
        assert_eq!(ledger.remaining(), None);
        assert_eq!(ledger.budget(), None);
    }

    #[test]
    fn snapshot_reflects_state() {
        let ledger = TokenLedger::with_budget(Some(500));
        ledger.record(&usage(100.0, 50.0, 0.03));
        let snap = ledger.snapshot();
        assert_eq!(snap.total, 150);
        assert_eq!(snap.input, 100);
        assert_eq!(snap.output, 50);
        assert_eq!(snap.rolling, 150);
        assert_eq!(snap.budget, Some(500));
        assert_eq!(snap.remaining, Some(350));
        assert!((snap.cost - 0.03).abs() < 1e-6);
        // Round-trips through JSON.
        let json = serde_json::to_string(&snap).unwrap();
        let back: TokenSnapshot = serde_json::from_str(&json).unwrap();
        assert_eq!(snap, back);
    }

    #[test]
    fn effective_max_tokens_clamps_to_remaining() {
        let ledger = TokenLedger::with_budget(Some(100));
        ledger.record(&usage(60.0, 0.0, 0.0)); // remaining 40
        let params = ModelParams {
            max_tokens: 1000,
            ..Default::default()
        };
        assert_eq!(ledger.effective_max_tokens(&params), 40);
        // Unlimited budget → unchanged.
        let unlimited = TokenLedger::new();
        assert_eq!(unlimited.effective_max_tokens(&params), 1000);
    }

    #[test]
    fn concurrent_records_are_atomic() {
        use std::thread;
        let ledger = TokenLedger::new();
        let mut handles = vec![];
        for _ in 0..8 {
            let l = ledger.clone();
            handles.push(thread::spawn(move || {
                for _ in 0..1000 {
                    l.record(&usage(1.0, 1.0, 0.0));
                }
            }));
        }
        for h in handles {
            h.join().unwrap();
        }
        // 8 threads * 1000 records * (1 input + 1 output) = 16000.
        assert_eq!(ledger.total(), 16_000);
        assert_eq!(ledger.input(), 8_000);
        assert_eq!(ledger.output(), 8_000);
    }
}
