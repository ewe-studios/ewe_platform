//! Model-call and memory timestamps read the injected `Clock` (spec 70 feature 133).
//!
//! WHY: the agentic memory stores and the model providers stamped their
//! records with `SystemTime::now()`, so a deterministic simulation test could
//! not control the recorded time.
//!
//! WHAT: `MemoryHierarchy::persist_observation` writes a `SessionRecord` whose
//! `timestamp` comes from the clock the hierarchy was built with. A test
//! advances a `SimulatedClock` and reads the stored record back.
//!
//! HOW: the memory path uses only in-memory stores, so the test needs no
//! network. `SimulatedClock::advance_wall` moves the injected wall clock by
//! hand. Before this feature the record used `SystemTime::now()`, so the
//! stamped value did not match the simulated time and did not follow the
//! advance.

use std::sync::Arc;
use std::time::Duration;

use foundation_ai::agentic::memory::{MemoryConfig, MemoryHierarchy};
use foundation_ai::agentic::memory_coordinator::MemoryCoordinator;
use foundation_ai::agentic::memory_store::KvMemoryStore;
use foundation_ai::agentic::token_ledger::TokenLedger;
use foundation_ai::types::{ObservationEntry, ObservationKind, SessionId, SessionRecord};
use foundation_compact::SystemTime;
use foundation_core::valtron::SimulatedClock;
use foundation_db::{MemoryDocumentStore, MemoryStorage};
use futures_lite::future::block_on;

fn observation(content: &str) -> ObservationEntry {
    ObservationEntry {
        kind: ObservationKind::Assertion,
        content: content.to_string(),
        timestamp: SystemTime::UNIX_EPOCH,
        source_message_id: foundation_compact::ids::new_scru128(),
        scope: None,
    }
}

fn stored_observation_timestamp(
    hierarchy: &MemoryHierarchy<KvMemoryStore<MemoryStorage>, MemoryDocumentStore>,
    session_id: &SessionId,
) -> SystemTime {
    let memory =
        block_on(hierarchy.coordinator().hydrate(session_id)).expect("hydrate must succeed");
    match memory.observation {
        Some(SessionRecord::Observation { timestamp, .. }) => timestamp,
        other => panic!("expected a stored observation, got {other:?}"),
    }
}

#[test]
fn observation_timestamp_follows_the_injected_clock() {
    let clock = Arc::new(SimulatedClock::new(Duration::ZERO));
    let session_id = SessionId::new();
    let coordinator = MemoryCoordinator::new(
        KvMemoryStore::new(MemoryStorage::new()),
        MemoryDocumentStore::new(),
    );
    let hierarchy = MemoryHierarchy::new(
        session_id.clone(),
        coordinator,
        TokenLedger::new(),
        MemoryConfig::default(),
        clock.clone(),
    );

    block_on(hierarchy.persist_observation(vec![observation("first")], 10))
        .expect("persist observation");

    assert_eq!(
        stored_observation_timestamp(&hierarchy, &session_id),
        SystemTime::UNIX_EPOCH,
        "the first record stamps the simulated clock's wall origin"
    );

    clock.advance_wall(Duration::from_secs(3_600));
    block_on(hierarchy.persist_observation(vec![observation("second")], 10))
        .expect("persist observation");

    assert_eq!(
        stored_observation_timestamp(&hierarchy, &session_id),
        SystemTime::UNIX_EPOCH + Duration::from_secs(3_600),
        "the record timestamp follows advance_wall on the injected clock"
    );
}
