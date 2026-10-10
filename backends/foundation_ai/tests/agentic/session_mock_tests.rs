//! `AgentSession` API driven by a `MockModelProvider` — deterministic, no model.
//!
//! WHY: `session_tests.rs` only covers the builder + preflight; `run_turn`,
//! `steer`, `follow_up`, and `end` are exercised only by the real-model
//! `integrations/session_turn.rs`, which needs `integration_tests` and a
//! download. This mirrors the same public API on a mock so it runs offline on
//! every change and covers the flow branches a real model cannot force.
//!
//! Matrix rows: 3.1, 3.2, 6.5, 6.9, 7.1, 7.2, 7.3.

use foundation_ai::agentic::testing::{
    last_user_contains, mock_text, mock_tool_call, MockModelProvider,
};
use foundation_ai::agentic::{
    AgentSession, Answer, ContextConfig, ErrorPolicy, KvMemoryStore, MemoryConfig, TurnEvent,
    TurnOutcome,
};
use foundation_ai::types::{
    MessageRole, Messages, ModelOutput, SessionRecord, TextContent, ToolArguments, UserModelContent,
};
use foundation_core::valtron::{valtron_test, Stream};
use foundation_db::{MemoryDocumentStore, MemoryStorage, SqlDocumentStore, TursoStorage};

type Session = AgentSession<MemoryDocumentStore, KvMemoryStore<MemoryStorage>>;

/// The same session over SQL-backed stores (Turso / `SQLite`).
type SqlSession = AgentSession<SqlDocumentStore<TursoStorage>, KvMemoryStore<TursoStorage>>;

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
    AgentSession::builder(mock.into_router())
        .with_system_prompt("You are a helpful assistant.")
        .with_model("mock")
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
// Matrix 7.3 — run_turn on a failing provider reports the failure

#[valtron_test]
fn run_turn_reports_a_provider_failure() {
    let mut mock = MockModelProvider::new();
    mock.fail_with(|_| true, "mock failure");

    let session = session_with(mock);
    let turn = session.run_turn(user_msg("hi")).expect("the turn started");

    assert!(
        turn.failure().is_some(),
        "a failing provider must end the turn with a failure: {turn:?}"
    );
    assert!(turn.text().is_empty(), "nothing was produced before it");
    assert!(
        turn.into_result().is_err(),
        "into_result turns the failure into Err"
    );
}

// ---------------------------------------------------------------------------
// Proposal 15, item 7/8 — partial output first, then the error

/// A model that says something and looks for a tool on its first call, then
/// fails on the next generation (the one that sees the tool result).
fn fails_after_partial_output() -> MockModelProvider {
    let mut mock = MockModelProvider::new();
    mock.fail_with(
        |mi| {
            mi.messages
                .iter()
                .any(|m| matches!(m, Messages::ToolResult { .. }))
        },
        "mock failure after the tool round",
    );
    mock.on_nth_call(
        0,
        vec![
            mock_text("Working on it. "),
            mock_tool_call(
                "shed",
                ToolArguments::from_iter([(
                    "description".to_string(),
                    serde_json::Value::String("anything".into()),
                )]),
            ),
        ],
    );
    mock
}

#[valtron_test]
fn run_turn_keeps_the_partial_output_before_a_failure() {
    let session = session_with(fails_after_partial_output());
    let turn = session
        .run_turn("do the thing")
        .expect("a mid-turn failure is not an Err");

    assert_eq!(turn.text(), "Working on it. ");
    assert_eq!(turn.tool_calls().count(), 1);
    assert_eq!(turn.tool_results().count(), 1);
    let failure = turn.failure().expect("the failure is reported");
    assert!(
        failure
            .to_string()
            .contains("mock failure after the tool round"),
        "{failure}"
    );
    assert!(matches!(turn.outcome(), TurnOutcome::Failed { .. }));
    // Deref to Vec<SessionRecord>: old call sites keep working.
    assert!(!turn.is_empty());
    assert!(!turn
        .iter()
        .any(|r| matches!(r, SessionRecord::FailedAction { .. })));
}

#[valtron_test]
fn ask_returns_the_partial_text_and_the_error() {
    let session = session_with(fails_after_partial_output());
    let answer = session.ask("do the thing").expect("the turn started");

    match &answer {
        Answer::Failed {
            partial_text,
            error,
            ..
        } => {
            assert_eq!(partial_text, "Working on it. ");
            assert!(error.to_string().contains("mock failure"), "{error}");
        }
        other @ Answer::Complete(_) => panic!("expected Answer::Failed, got {other:?}"),
    }
    assert_eq!(answer.text(), "Working on it. ");
    assert!(!answer.is_complete());
    assert!(answer.into_result().is_err());
}

#[valtron_test]
fn events_yield_the_partial_output_then_a_terminal_failure() {
    let session = session_with(fails_after_partial_output());
    let events: Vec<TurnEvent> = session
        .run_turn_stream("do the thing")
        .expect("the turn started")
        .events()
        .filter(|e| !matches!(e, TurnEvent::Progress(_)))
        .collect();

    let text = events
        .iter()
        .position(|e| matches!(e, TurnEvent::Text(t) if t == "Working on it. "))
        .expect("the text arrives");
    let call = events
        .iter()
        .position(|e| matches!(e, TurnEvent::ToolCall { name, .. } if name == "shed"))
        .expect("the tool call arrives");
    let result = events
        .iter()
        .position(|e| matches!(e, TurnEvent::ToolResult { name, .. } if name == "shed"))
        .expect("the tool result arrives");
    assert!(text < call && call < result, "{events:?}");
    assert!(
        matches!(events.last(), Some(TurnEvent::Failed(_))),
        "Failed is the last event: {events:?}"
    );
    assert!(
        !events.iter().any(|e| matches!(e, TurnEvent::Done(_))),
        "no Done follows a failure: {events:?}"
    );
}

#[valtron_test]
fn ask_and_events_on_a_completed_turn() {
    let mut mock = MockModelProvider::new();
    mock.on_any(vec![mock_text("Hel"), mock_text("lo")]);
    let session = session_with(mock);

    let answer = session.ask("hi").expect("turn runs");
    assert_eq!(answer, Answer::Complete("Hello".into()));

    let events: Vec<TurnEvent> = session
        .run_turn_stream("hi again")
        .expect("turn runs")
        .events()
        .collect();
    let text: String = events
        .iter()
        .filter_map(|e| match e {
            TurnEvent::Text(t) => Some(t.as_str()),
            _ => None,
        })
        .collect();
    assert_eq!(text, "Hello");
    match events.last() {
        Some(TurnEvent::Done(summary)) => assert!(summary.message_count > 0),
        other => panic!("expected a terminal Done, got {other:?}"),
    }

    let turn = session.run_turn("and again").expect("turn runs");
    assert_eq!(turn.text(), "Hello");
    assert!(turn.failure().is_none());
    assert!(turn.usage().is_some(), "the summary's usage is exposed");
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

// ---------------------------------------------------------------------------
// Persistent stores satisfy build()'s `Default` bound

#[valtron_test]
fn build_and_run_a_turn_on_sql_backed_stores() {
    let mut mock = MockModelProvider::new();
    mock.on_any(vec![mock_text("hello from sqlite")]);

    let session: SqlSession = AgentSession::builder(mock.into_router())
        .with_doc_store(SqlDocumentStore::<TursoStorage>::default())
        .with_memory_store(KvMemoryStore::<TursoStorage>::default())
        .with_model("mock")
        .build()
        .expect("a session over default SQL stores builds");

    let records = session
        .run_turn(user_msg("hi"))
        .expect("turn should succeed");
    assert_eq!(
        assistant_texts(&records),
        vec!["hello from sqlite".to_string()]
    );
}

// ---------------------------------------------------------------------------
// Proposal 15, item 6: turns and steering take anything Into<Messages>

#[valtron_test]
fn run_turn_accepts_a_plain_string() {
    let mut mock = MockModelProvider::new();
    mock.on(
        last_user_contains("plain prompt"),
        vec![mock_text("got it")],
    );
    mock.on_any(vec![mock_text("wrong prompt")]);
    let session = session_with(mock);

    let records = session
        .run_turn("plain prompt")
        .expect("turn should succeed");
    assert_eq!(assistant_texts(&records), vec!["got it".to_string()]);

    // An owned String works the same way.
    let records = session
        .run_turn(String::from("plain prompt again"))
        .expect("turn should succeed");
    assert_eq!(assistant_texts(&records), vec!["got it".to_string()]);
}

#[valtron_test]
fn follow_up_accepts_a_plain_string() {
    let mut mock = MockModelProvider::new();
    mock.on_any(vec![mock_text("ok")]);
    let session = session_with(mock);

    session.follow_up("queued as a user message");
    let queued = session.steering_queues().drain_follow_up();
    assert_eq!(queued.len(), 1);
    assert!(matches!(
        &queued[0],
        Messages::User { role: MessageRole::User, content: UserModelContent::Text(t), .. }
            if t.content == "queued as a user message"
    ));
}

// ---------------------------------------------------------------------------
// TurnStream keeps the raw valtron stream reachable (review on #65)

mod raw_stream_access {
    use super::*;
    use foundation_ai::agentic::{AgentProgress, Turn, TurnStream};
    use foundation_core::valtron::{DrivenStreamIterator, StreamIterator, StreamIteratorExt};

    type Raw = DrivenStreamIterator<
        foundation_ai::agentic::internals::AgentLoop<
            MemoryDocumentStore,
            KvMemoryStore<MemoryStorage>,
        >,
    >;

    /// Compile-time: a `TurnStream` is a valtron `StreamIterator` over
    /// `SessionRecord` / `AgentProgress`.
    fn assert_stream_iterator<S>(_: &S)
    where
        S: StreamIterator<D = SessionRecord, P = AgentProgress>,
    {
    }

    fn texts<I>(items: I) -> Vec<String>
    where
        I: Iterator<Item = Stream<String, AgentProgress>>,
    {
        items
            .filter_map(|item| match item {
                Stream::Next(text) if !text.is_empty() => Some(text),
                _ => None,
            })
            .collect::<Vec<_>>()
    }

    fn streamed(
        reply: &str,
    ) -> (
        Session,
        TurnStream<MemoryDocumentStore, KvMemoryStore<MemoryStorage>>,
    ) {
        let mut mock = MockModelProvider::new();
        mock.on_any(vec![mock_text(reply)]);
        let session = session_with(mock);
        let stream = session
            .run_turn_stream("hi")
            .expect("stream should schedule");
        (session, stream)
    }

    #[valtron_test]
    fn stream_iterator_ext_combinators_apply_to_a_turn_stream() {
        let (_session, stream) = streamed("mapped");
        assert_stream_iterator(&stream);

        // `map_done` straight on the TurnStream.
        let mapped = stream.map_done(|record| Turn::text_of(&record));
        assert_eq!(texts(mapped), vec!["mapped".to_string()]);
    }

    #[valtron_test]
    fn into_stream_iter_and_filter_done_on_a_turn_stream() {
        let (_session, stream) = streamed("filtered");

        let only_conversation = stream
            .into_stream_iter()
            .filter_done(|record| matches!(record, SessionRecord::Conversation { .. }))
            .map_done(|record| Turn::text_of(&record));
        assert_eq!(texts(only_conversation), vec!["filtered".to_string()]);
    }

    #[valtron_test]
    fn inner_mut_drives_the_raw_iterator_and_the_turn_stream_stays_usable() {
        let (_session, mut stream) = streamed("raw");

        // Drive the raw valtron iterator through the borrow…
        let mut assistant = Vec::new();
        while let Some(item) = stream.inner_mut().next() {
            if let Stream::Next(record) = item {
                let text = Turn::text_of(&record);
                if !text.is_empty() {
                    assistant.push(text);
                }
            }
        }
        assert_eq!(assistant, vec!["raw".to_string()]);
        // …and the TurnStream is still there, reporting the same state.
        assert!(stream.inner().is_closed());
        assert!(stream.is_closed());
        assert!(stream.next().is_none());
    }

    #[valtron_test]
    fn into_inner_and_from_hand_over_the_raw_iterator() {
        let (_session, stream) = streamed("handed over");
        let raw: Raw = stream.into_inner();
        let mapped = raw.map_done(|record| Turn::text_of(&record));
        assert_eq!(texts(mapped), vec!["handed over".to_string()]);

        let (_session, stream) = streamed("converted");
        let raw = Raw::from(stream);
        let mapped = raw.map_done(|record| Turn::text_of(&record));
        assert_eq!(texts(mapped), vec!["converted".to_string()]);
    }
}
