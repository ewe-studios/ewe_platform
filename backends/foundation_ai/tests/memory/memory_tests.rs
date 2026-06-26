use foundation_ai::agentic::memory::*;
use foundation_ai::agentic::memory_coordinator::MemoryCoordinator;
use foundation_ai::agentic::memory_store::KvMemoryStore;
use foundation_ai::agentic::token_ledger::TokenLedger;
use foundation_ai::types::{
    MemoryFact, ObservationEntry, ObservationKind, ReflectionEntry, SessionId, UsageReport,
};
use foundation_compact::SystemTime;
use foundation_db::{MemoryDocumentStore, MemoryStorage};

fn setup() -> (
    MemoryHierarchy<KvMemoryStore<MemoryStorage>, MemoryDocumentStore>,
    TokenLedger,
) {
    let kv_store = KvMemoryStore::new(MemoryStorage::new());
    let doc_store = MemoryDocumentStore::new();
    let session_id = SessionId::new();
    let coordinator = MemoryCoordinator::new(kv_store, doc_store);
    let ledger = TokenLedger::new();

    let hierarchy = MemoryHierarchy::new(
        session_id,
        coordinator,
        ledger.clone(),
        MemoryConfig::default(),
    );
    (hierarchy, ledger)
}

fn usage(input: f64, output: f64) -> UsageReport {
    UsageReport {
        input,
        output,
        cache_read: 0.0,
        cache_write: 0.0,
        total_tokens: input + output,
        cost: foundation_ai::types::UsageCosting {
            currency: String::new(),
            input: 0.0,
            output: 0.0,
            cache_read: 0.0,
            cache_write: 0.0,
            total_tokens: 0.0,
            status: foundation_ai::types::CostStatus::Estimated,
        },
    }
}

#[test]
fn no_trigger_when_below_thresholds() {
    let (h, _ledger) = setup();
    assert_eq!(h.check_triggers(), MemoryAction::None);
}

#[test]
fn observation_trigger_at_30k() {
    let (h, ledger) = setup();
    ledger.record(&usage(20_000.0, 10_000.0));
    assert!(ledger.rolling() >= 30_000);
    assert_eq!(h.check_triggers(), MemoryAction::GenerateObservation);
}

#[test]
fn reflection_trigger_at_40k_observation_tokens() {
    let (h, _ledger) = setup();
    h.set_observation_memory_tokens(40_000);
    assert_eq!(h.check_triggers(), MemoryAction::GenerateReflection);
}

#[test]
fn observation_trigger_takes_priority_over_reflection() {
    let (h, ledger) = setup();
    ledger.record(&usage(20_000.0, 10_000.0));
    h.set_observation_memory_tokens(40_000);
    assert_eq!(h.check_triggers(), MemoryAction::GenerateObservation);
}

#[test]
fn no_trigger_while_generating() {
    let (h, ledger) = setup();
    ledger.record(&usage(20_000.0, 10_000.0));
    assert!(h.begin_generation());
    assert_eq!(h.check_triggers(), MemoryAction::None);
    h.end_generation();
    assert_eq!(h.check_triggers(), MemoryAction::GenerateObservation);
}

#[test]
fn persist_observation_resets_rolling() {
    use futures_lite::future::block_on;
    let (h, ledger) = setup();
    ledger.record(&usage(20_000.0, 10_000.0));
    assert!(ledger.rolling() >= 30_000);

    block_on(async {
        let obs = vec![ObservationEntry {
            kind: ObservationKind::Assertion,
            content: "user likes rust".into(),
            timestamp: SystemTime::UNIX_EPOCH,
            source_message_id: foundation_compact::ids::new_scru128(),
            scope: None,
        }];
        h.persist_observation(obs, 500).await.unwrap();
    });

    assert_eq!(ledger.rolling(), 0);
    assert_eq!(h.observation_memory_tokens(), 500);
}

#[test]
fn persist_reflection_resets_observation_tokens() {
    use futures_lite::future::block_on;
    let (h, _ledger) = setup();
    h.set_observation_memory_tokens(45_000);

    block_on(async {
        let refls = vec![ReflectionEntry {
            summary: "condensed summary".into(),
            time_range: None,
            observation_refs: vec![],
            importance: 0.8,
        }];
        h.persist_reflection(refls, 45_000).await.unwrap();
    });

    assert_eq!(h.observation_memory_tokens(), 0);
}

#[test]
fn update_working_memory_versions() {
    use futures_lite::future::block_on;
    let (h, _ledger) = setup();

    block_on(async {
        let facts = vec![MemoryFact {
            fact: "user is a Rust developer".into(),
            asserted_at: SystemTime::now(),
            source_message_id: foundation_compact::ids::new_scru128(),
            confidence: 0.95,
        }];
        h.update_working_memory(facts, 0).await.unwrap();
    });
}

#[test]
fn begin_generation_is_exclusive() {
    let (h, _ledger) = setup();
    assert!(h.begin_generation());
    assert!(!h.begin_generation());
    assert!(h.is_generating());
    h.end_generation();
    assert!(!h.is_generating());
    assert!(h.begin_generation());
}

#[test]
fn clone_shares_state() {
    let (h, ledger) = setup();
    let h2 = h.clone();
    ledger.record(&usage(20_000.0, 10_000.0));
    assert_eq!(h.check_triggers(), h2.check_triggers());
}
