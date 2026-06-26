use foundation_ai::agentic::serialization::*;
use foundation_ai::types::{
    CostStatus, MessageRole, Messages, ModelId, ModelOutput, ModelProviders, ObservationEntry,
    ObservationKind, SessionRecord, StopReason, TextContent, UsageCosting, UsageReport,
    UserModelContent,
};
use foundation_compact::SystemTime;

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

fn assistant_record(input: f64, output: f64) -> SessionRecord {
    SessionRecord::Conversation {
        message: Messages::Assistant {
            id: foundation_compact::ids::new_scru128(),
            model: ModelId::Name("gpt".into(), None),
            timestamp: SystemTime::UNIX_EPOCH,
            usage: UsageReport {
                input,
                output,
                cache_read: 0.0,
                cache_write: 0.0,
                total_tokens: input + output,
                cost: UsageCosting::zero(CostStatus::Actual),
            },
            content: ModelOutput::Text(TextContent {
                content: "hi".into(),
                signature: None,
            }),
            stop_reason: StopReason::Stop,
            provider: ModelProviders::ANTHROPIC,
            error_detail: None,
            signature: None,
            metadata: None,
        },
    }
}

fn observation_record() -> SessionRecord {
    SessionRecord::Observation {
        id: foundation_compact::ids::new_scru128(),
        observations: vec![ObservationEntry {
            kind: ObservationKind::Assertion,
            content: "user likes rust".into(),
            timestamp: SystemTime::UNIX_EPOCH,
            source_message_id: foundation_compact::ids::new_scru128(),
            scope: None,
        }],
        token_count: 12,
        timestamp: SystemTime::UNIX_EPOCH,
    }
}

#[test]
fn json_round_trips_for_each_variant() {
    for rec in [
        user_record("hi"),
        assistant_record(10.0, 5.0),
        observation_record(),
    ] {
        let json = serde_json::to_string(&rec).unwrap();
        let back: SessionRecord = serde_json::from_str(&json).unwrap();
        assert_eq!(rec, back);
    }
}

#[test]
fn row_promotes_assistant_columns() {
    let rec = assistant_record(100.0, 40.0);
    let row = SessionRecordRow::from_record(&rec).unwrap();
    assert_eq!(row.record_type, "conversation");
    assert_eq!(row.role, "assistant");
    assert_eq!(row.model.as_deref(), Some("gpt"));
    assert_eq!(row.input_tokens, Some(100));
    assert_eq!(row.output_tokens, Some(40));
    assert!(row.created_at > 0);
    // content reconstructs the exact record.
    assert_eq!(row.into_record().unwrap(), rec);
}

#[test]
fn arrow_round_trips_with_nulls_and_all_types() {
    let records = vec![
        user_record("hello"),
        assistant_record(100.0, 40.0),
        observation_record(),
    ];
    let batch = to_record_batch(&records).unwrap();
    assert_eq!(batch.num_rows(), 3);
    let back = from_record_batch(&batch).unwrap();
    assert_eq!(back, records);
}

#[test]
fn columnar_analytics_over_rows() {
    let records = vec![
        assistant_record(100.0, 40.0),
        assistant_record(20.0, 10.0),
        observation_record(),
    ];
    let rows: Vec<SessionRecordRow> = records
        .iter()
        .map(|r| SessionRecordRow::from_record(r).unwrap())
        .collect();
    // SUM(output_tokens) = 40 + 10 (observation has null).
    assert_eq!(sum_output_tokens(&rows), 50);
    assert_eq!(sum_input_tokens(&rows), 120);
    // WHERE record_type = 'conversation' -> 2 rows.
    assert_eq!(filter_by_type(&rows, "conversation").len(), 2);
    assert_eq!(filter_by_type(&rows, "observation").len(), 1);
}

#[test]
fn schema_has_promoted_columns() {
    use foundation_arrow::ArrowSchema;
    let fields = SessionRecordRow::fields();
    let names: Vec<&str> = fields.iter().map(|f| f.name().as_str()).collect();
    for expected in [
        "id",
        "record_type",
        "role",
        "title",
        "summary",
        "model",
        "input_tokens",
        "output_tokens",
        "created_at",
        "content",
    ] {
        assert!(names.contains(&expected), "missing column {expected}");
    }
}
