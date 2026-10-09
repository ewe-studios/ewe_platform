//! Public-API coverage for `agentic::serialization` — the SessionRecord <-> row
//! <-> Arrow batch path plus the columnar analytics helpers. All pure and
//! deterministic; serialization.rs was 70% region coverage.

use foundation_ai::agentic::serialization::{
    filter_by_type, from_record_batch, sum_input_tokens, sum_output_tokens, to_record_batch,
    SessionRecordRow,
};
use foundation_ai::types::{
    MessageRole, Messages, ModelId, ModelOutput, SessionRecord, TextContent, TokenSnapshot,
    UsageReport, UserModelContent,
};

fn user_record(text: &str) -> SessionRecord {
    SessionRecord::Conversation {
        message: Messages::User {
            id: foundation_compact::ids::new_scru128(),
            role: MessageRole::User,
            content: UserModelContent::Text(TextContent {
                content: text.into(),
                signature: None,
            }),
            signature: None,
        },
    }
}

fn assistant_record(text: &str, input: f64, output: f64) -> SessionRecord {
    let usage = UsageReport {
        input,
        output,
        ..zero_usage()
    };
    SessionRecord::Conversation {
        message: Messages::Assistant {
            id: foundation_compact::ids::new_scru128(),
            model: ModelId::Name("m".into(), None),
            timestamp: foundation_compact::SystemTime::UNIX_EPOCH,
            usage,
            content: ModelOutput::Text(TextContent {
                content: text.into(),
                signature: None,
            }),
            stop_reason: foundation_ai::types::StopReason::Stop,
            provider: foundation_ai::types::ModelProviders::Custom("m".into()),
            error_detail: None,
            signature: None,
            metadata: None,
        },
    }
}

fn summary_record() -> SessionRecord {
    SessionRecord::Summary {
        message_count: 3,
        usage: TokenSnapshot {
            total: 0,
            input: 0,
            output: 0,
            rolling: 0,
            budget: None,
            remaining: None,
            cost: 0.0,
        },
    }
}

fn zero_usage() -> UsageReport {
    UsageReport {
        input: 0.0,
        output: 0.0,
        cache_read: 0.0,
        cache_write: 0.0,
        total_tokens: 0.0,
        cost: foundation_ai::types::UsageCosting {
            currency: "USD".into(),
            input: 0.0,
            output: 0.0,
            cache_read: 0.0,
            cache_write: 0.0,
            total_tokens: 0.0,
            status: foundation_ai::types::CostStatus::Actual,
        },
    }
}

// ---------------------------------------------------------------------------
// Row projection + round-trip

#[test]
fn row_round_trips_a_user_conversation() {
    let rec = user_record("hello");
    let row = SessionRecordRow::from_record(&rec).expect("project to row");
    assert_eq!(row.record_type, "conversation");
    let back = row.into_record().expect("reconstruct");
    assert_eq!(back, rec, "row must reconstruct the exact record");
}

#[test]
fn row_promotes_assistant_token_columns() {
    let rec = assistant_record("hi", 12.0, 5.0);
    let row = SessionRecordRow::from_record(&rec).expect("project");
    assert_eq!(row.input_tokens, Some(12), "input tokens promoted to a column");
    assert_eq!(row.output_tokens, Some(5), "output tokens promoted");
    assert!(row.model.is_some(), "assistant model promoted");
}

#[test]
fn row_round_trips_a_summary() {
    let rec = summary_record();
    let row = SessionRecordRow::from_record(&rec).expect("project");
    assert_eq!(row.record_type, "summary");
    assert_eq!(row.into_record().expect("reconstruct"), rec);
}

// ---------------------------------------------------------------------------
// Arrow batch round-trip — matrix support for serialization fidelity

#[test]
fn record_batch_round_trips_mixed_records() {
    let records = vec![
        user_record("q"),
        assistant_record("a", 8.0, 3.0),
        summary_record(),
    ];
    let batch = to_record_batch(&records).expect("to batch");
    assert_eq!(batch.num_rows(), 3, "one Arrow row per record");

    let back = from_record_batch(&batch).expect("from batch");
    assert_eq!(back, records, "the Arrow batch must round-trip every record");
}

#[test]
fn empty_records_produce_an_empty_batch() {
    let batch = to_record_batch(&[]).expect("empty batch");
    assert_eq!(batch.num_rows(), 0);
    assert_eq!(from_record_batch(&batch).expect("from empty"), Vec::new());
}

// ---------------------------------------------------------------------------
// Columnar analytics helpers

#[test]
fn token_sums_skip_non_assistant_rows() {
    let rows: Vec<SessionRecordRow> = [
        user_record("q"),
        assistant_record("a1", 10.0, 4.0),
        assistant_record("a2", 7.0, 6.0),
        summary_record(),
    ]
    .iter()
    .map(|r| SessionRecordRow::from_record(r).unwrap())
    .collect();

    assert_eq!(sum_input_tokens(&rows), 17, "10 + 7, nulls skipped");
    assert_eq!(sum_output_tokens(&rows), 10, "4 + 6, nulls skipped");
}

#[test]
fn filter_by_type_selects_matching_rows() {
    let rows: Vec<SessionRecordRow> = [
        user_record("q"),
        assistant_record("a", 1.0, 1.0),
        summary_record(),
    ]
    .iter()
    .map(|r| SessionRecordRow::from_record(r).unwrap())
    .collect();

    assert_eq!(filter_by_type(&rows, "conversation").len(), 2);
    assert_eq!(filter_by_type(&rows, "summary").len(), 1);
    assert_eq!(filter_by_type(&rows, "observation").len(), 0);
}
