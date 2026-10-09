use foundation_ai::agentic::token_ledger::*;
use foundation_ai::types::{CostStatus, ModelParams, UsageCosting, UsageReport};

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
    ledger.record(&usage(10.0, 0.0, 0.0)); // total 100 -- exactly at budget
    assert!(ledger.is_exhausted());
    assert_eq!(ledger.remaining(), Some(0));
}

#[test]
fn zero_budget_halts_immediately() {
    let ledger = TokenLedger::with_budget(Some(0));
    // 0 is a valid budget -- exhausted before any tokens are spent.
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
    // Unlimited budget -> unchanged.
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
