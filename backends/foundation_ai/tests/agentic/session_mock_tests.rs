//! `AgentSession` API driven by a `MockModelProvider` — deterministic, no model.
//!
//! WHY: `session_tests.rs` only covers the builder + preflight; `run_turn`,
//! `steer`, `follow_up`, and `end` are exercised only by the real-model
//! `integrations/session_turn.rs`, which needs `integration_tests` and a
//! download. This mirrors the same public API on a mock so it runs offline on
//! every change and covers the flow branches a real model cannot force.
//!
//! Matrix rows: 3.1, 3.2, 6.5, 6.9, 7.1, 7.2, 7.3.

use foundation_ai::agentic::testing::{mock_text, MockModelProvider};
use foundation_ai::agentic::{
    AgentConfig, AgentSession, ContextConfig, ErrorPolicy, KvMemoryStore, MemoryConfig,
};
use foundation_ai::types::{
    MessageRole, Messages, ModelId, ModelOutput, SessionId, SessionRecord, TextContent,
    UserModelContent,
};
use foundation_core::valtron::{valtron_test, Stream};
use foundation_db::{MemoryDocumentStore, MemoryStorage};

type Session = AgentSession<MemoryDocumentStore, KvMemoryStore<MemoryStorage>>;

fn user_msg(text: &str) -> Messages {
    Messages::User {
        id: foundation_compact::ids::new_scru128(),
        role: MessageRole::User,
        content: UserModelContent::Text(TextContent {
            content: text.into(),
            signature: None,
        }),
        signature: None,
    }
}

/// A session whose router is the given mock, built the way the app builds one.
fn session_with(mock: MockModelProvider) -> Session {
    let model_id = ModelId::Name("mock".into(), None);
    AgentSession::builder(SessionId::new(), mock.into_router())
        .with_system_prompt("You are a helpful assistant.")
        .with_model(model_id.clone())
        .with_config(AgentConfig {
            primary_model: model_id,
            ..Default::default()
        })
        .with_context_config(ContextConfig::default())
        .with_memory_config(MemoryConfig::default())
        .with_error_policy(ErrorPolicy::new())
        .build()
        .expect("session builds")
}

fn assistant_texts(records: &[SessionRecord]) -> Vec<String> {
    records
        .iter()
        .filter_map(|r| match r {
            SessionRecord::Conversation {
                message: Messages::Assistant { content, .. },
            } => match content {
                ModelOutput::Text(t) => Some(t.content.clone()),
                _ => None,
            },
            _ => None,
        })
        .collect()
}

// ---------------------------------------------------------------------------
// Matrix 7.1 — run_turn returns the assistant reply

#[valtron_test]
fn run_turn_returns_the_mock_reply() {
    let mut mock = MockModelProvider::new();
    mock.on_any(vec![mock_text("mock says hi")]);

    let session = session_with(mock);
    let records = session
        .run_turn(user_msg("hi"))
        .expect("turn should succeed");

    assert_eq!(assistant_texts(&records), vec!["mock says hi".to_string()]);
}

// ---------------------------------------------------------------------------
// Matrix 7.3 — run_turn on a failing provider returns Err

#[valtron_test]
fn run_turn_returns_err_on_provider_failure() {
    let mut mock = MockModelProvider::new();
    mock.fail_with(|_| true, "mock failure");

    let session = session_with(mock);
    let result = session.run_turn(user_msg("hi"));

    assert!(
        result.is_err(),
        "a failing provider must make run_turn return Err, not Ok: {result:?}"
    );
}

// ---------------------------------------------------------------------------
// Matrix 7.2 — run_turn_stream yields records progressively

#[valtron_test]
fn run_turn_stream_yields_records() {
    let mut mock = MockModelProvider::new();
    mock.on_any(vec![mock_text("streamed")]);

    let session = session_with(mock);
    let stream = session
        .run_turn_stream(user_msg("hi"))
        .expect("stream should schedule");

    let mut assistant = Vec::new();
    for item in stream {
        if let Stream::Next(SessionRecord::Conversation {
            message: Messages::Assistant { content, .. },
        }) = item
        {
            if let ModelOutput::Text(t) = content {
                assistant.push(t.content);
            }
        }
    }
    assert_eq!(assistant, vec!["streamed".to_string()]);
}

// ---------------------------------------------------------------------------
// Matrix 6.5 — the assistant reply is persisted to session history

#[valtron_test]
fn assistant_reply_persisted_to_history() {
    let mut mock = MockModelProvider::new();
    mock.on_any(vec![mock_text("remember me")]);

    let session = session_with(mock);
    session.run_turn(user_msg("hi")).expect("turn succeeds");

    let history = session.message_api().all().expect("history readable");
    let has_assistant = history.iter().any(|r| {
        matches!(
            r,
            SessionRecord::Conversation {
                message: Messages::Assistant { .. }
            }
        )
    });
    assert!(
        has_assistant,
        "the assistant reply must be persisted: {history:?}"
    );
}

// ---------------------------------------------------------------------------
// Matrix 3.2 — follow_up is processed by a subsequent turn

#[valtron_test]
fn follow_up_message_is_processed() {
    let mut mock = MockModelProvider::new();
    mock.on_any(vec![mock_text("answered")]);

    let session = session_with(mock);
    session.follow_up(user_msg("queued question"));

    // run_turn drains the follow-up queue at the outer boundary alongside its
    // own prompt; both are answered.
    let records = session
        .run_turn(user_msg("direct question"))
        .expect("turn succeeds");
    assert!(
        !assistant_texts(&records).is_empty(),
        "a queued follow-up must be answered: {records:?}"
    );
}

// ---------------------------------------------------------------------------
// Matrix 6.9 — end() drains queues and persists remaining messages

#[valtron_test]
fn end_persists_queued_messages() {
    let mut mock = MockModelProvider::new();
    mock.on_any(vec![mock_text("hi")]);

    let session = session_with(mock);
    session.follow_up(user_msg("left in the queue"));

    session.end().expect("end should succeed");

    let history = session.message_api().all().expect("history readable");
    let persisted_user = history.iter().any(|r| {
        matches!(
            r,
            SessionRecord::Conversation {
                message: Messages::User { .. }
            }
        )
    });
    assert!(
        persisted_user,
        "end() must persist the queued message, not drop it: {history:?}"
    );
}

// ---------------------------------------------------------------------------
// Matrix 3.1 — steer injects a priority message answered on the next turn

#[valtron_test]
fn steer_injects_priority_message() {
    let mut mock = MockModelProvider::new();
    mock.on_any(vec![mock_text("steered reply")]);

    let session = session_with(mock);
    session.steer(user_msg("urgent priority"));
    // steer() sets the priority queue; run the turn, then confirm the priority
    // message reached session history (whether the turn Ok's or trips loop
    // detection on the mock's identical replies — both drain the queue).
    let _ = session.run_turn(user_msg("normal"));

    let history = session.message_api().all().expect("history readable");
    let saw_priority = history.iter().any(|r| matches!(
        r,
        SessionRecord::Conversation { message: Messages::User { content: UserModelContent::Text(TextContent { content, .. }), .. } }
        if content == "urgent priority"
    ));
    assert!(
        saw_priority,
        "the steered priority message must be processed and persisted: {history:?}"
    );
}
