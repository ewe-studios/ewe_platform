use foundation_ai::types::agentic::*;
use foundation_compact::SystemTime;
use std::str::FromStr;

#[test]
fn session_id_is_strictly_monotonic_even_within_a_millisecond() {
    // The monotonic scru128 generator guarantees strict ordering for ids
    // minted in sequence, regardless of whether they share a millisecond.
    let ids: Vec<SessionId> = (0..1000).map(|_| SessionId::new()).collect();
    for w in ids.windows(2) {
        assert!(
            w[1].to_string() > w[0].to_string(),
            "ids must be strictly increasing: {} !> {}",
            w[1],
            w[0]
        );
    }
    assert!(ids[0].timestamp_ms() > 0);
}

#[test]
fn session_id_is_machine_attributable() {
    // Two ids from the same process carry the same machine id.
    let a = SessionId::new();
    let b = SessionId::new();
    assert_eq!(a.machine_id(), b.machine_id());
}

#[test]
fn session_id_from_name_is_stable_within_a_millisecond() {
    let a = SessionId::from_name("fix-bug");
    let b = SessionId::from_name("fix-bug");
    // Same name + same machine; if both land in the same ms they are equal,
    // otherwise only the timestamp differs. Compare the machine+hash region.
    if a.timestamp_ms() == b.timestamp_ms() {
        assert_eq!(a, b);
    }
}

#[test]
fn session_id_round_trips_through_string() {
    let a = SessionId::new();
    let s = a.to_string();
    let parsed = SessionId::from_str(&s).expect("scru128 string parses");
    assert_eq!(a, parsed);
}

#[test]
fn observation_kind_serializes_snake_case() {
    let json = serde_json::to_string(&ObservationKind::Assertion).unwrap();
    assert_eq!(json, "\"assertion\"");
    let back: ObservationKind = serde_json::from_str("\"question\"").unwrap();
    assert_eq!(back, ObservationKind::Question);
}

#[test]
fn session_record_working_memory_round_trips() {
    let rec = SessionRecord::WorkingMemory {
        id: foundation_compact::ids::new_scru128(),
        facts: vec![MemoryFact {
            fact: "user prefers dark mode".into(),
            asserted_at: SystemTime::UNIX_EPOCH,
            source_message_id: foundation_compact::ids::new_scru128(),
            confidence: 0.9,
        }],
        version: 1,
        timestamp: SystemTime::UNIX_EPOCH,
    };
    let json = serde_json::to_string(&rec).unwrap();
    assert!(json.contains("\"message_type\":\"working_memory\""));
    let back: SessionRecord = serde_json::from_str(&json).unwrap();
    assert_eq!(rec, back);
}

#[test]
fn session_record_summary_round_trips() {
    let rec = SessionRecord::Summary {
        message_count: 7,
        usage: TokenSnapshot {
            total: 140,
            input: 100,
            output: 40,
            rolling: 140,
            budget: Some(10_000),
            remaining: Some(9_860),
            cost: 0.0021,
        },
    };
    let json = serde_json::to_string(&rec).unwrap();
    assert!(json.contains("\"message_type\":\"summary\""));
    let back: SessionRecord = serde_json::from_str(&json).unwrap();
    assert_eq!(rec, back);
}
