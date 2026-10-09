use std::sync::Arc;

use foundation_ai::agentic::{
    AgentConfig, AgentSession, AgenticError, KvMemoryStore, SessionAccessProvider, TokenBudget,
};
use foundation_ai::types::{
    MessageRole, Messages, ModelId, ProviderRouter, SessionId, SessionRecord, TextContent,
    UserModelContent,
};
use foundation_db::traits::DocumentStore;
use foundation_db::{MemoryDocumentStore, MemoryStorage};

use foundation_ai::agentic::errors::{AuthError, UserId};

// ---------------------------------------------------------------------------
// Type aliases

type TestMemStore = KvMemoryStore<MemoryStorage>;
type TestDocStore = MemoryDocumentStore;
type TestSession = AgentSession<TestDocStore, TestMemStore>;

// ---------------------------------------------------------------------------
// Helpers

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

fn empty_router() -> ProviderRouter {
    ProviderRouter::builder().build()
}

// ---------------------------------------------------------------------------
// Builder + preflight tests

#[test]
fn builder_creates_session_with_named_id() {
    let id = SessionId::from_name("test-defaults");
    let session: TestSession = AgentSession::builder(empty_router())
        .with_session_id(id.clone())
        .build()
        .expect("build should succeed");

    assert_eq!(*session.session_id(), id);
}

#[test]
fn builder_accepts_custom_session_id() {
    let custom_id = SessionId::from_name("test-session");
    let session: TestSession = AgentSession::builder(empty_router())
        .with_session_id(custom_id.clone())
        .build()
        .expect("build should succeed");

    assert_eq!(*session.session_id(), custom_id);
}

#[test]
fn builder_accepts_system_prompt() {
    let _session: TestSession = AgentSession::builder(empty_router())
        .with_session_id(SessionId::from_name("test"))
        .with_system_prompt("You are a coding assistant.")
        .build()
        .expect("build should succeed with system prompt");
}

#[test]
fn builder_accepts_custom_model() {
    let _session: TestSession = AgentSession::builder(empty_router())
        .with_session_id(SessionId::from_name("test"))
        .with_model(ModelId::Name("gpt-4".into(), None))
        .build()
        .expect("build should succeed with custom model");
}

#[test]
fn builder_accepts_fallback_models() {
    let _session: TestSession = AgentSession::builder(empty_router())
        .with_session_id(SessionId::from_name("test"))
        .with_fallback_models(vec![
            ModelId::Name("gpt-4".into(), None),
            ModelId::Name("claude-3".into(), None),
        ])
        .build()
        .expect("build should succeed with fallbacks");
}

#[test]
fn session_is_clone() {
    let session: TestSession = AgentSession::builder(empty_router())
        .with_session_id(SessionId::from_name("test"))
        .build()
        .expect("build should succeed");

    let cloned = session.clone();
    assert_eq!(session.session_id(), cloned.session_id());
}

// ---------------------------------------------------------------------------
// Access control preflight

struct DenySessionAccess;
impl SessionAccessProvider for DenySessionAccess {
    fn can_access_session(&self, _: &UserId, _: &SessionId) -> Result<bool, AuthError> {
        Ok(false)
    }
    fn can_use_model(&self, _: &UserId, _: &str) -> Result<bool, AuthError> {
        Ok(true)
    }
    fn token_budget(&self, _: &UserId) -> Result<TokenBudget, AuthError> {
        Ok(TokenBudget::unlimited())
    }
}

struct DenyModelAccess;
impl SessionAccessProvider for DenyModelAccess {
    fn can_access_session(&self, _: &UserId, _: &SessionId) -> Result<bool, AuthError> {
        Ok(true)
    }
    fn can_use_model(&self, _: &UserId, _: &str) -> Result<bool, AuthError> {
        Ok(false)
    }
    fn token_budget(&self, _: &UserId) -> Result<TokenBudget, AuthError> {
        Ok(TokenBudget::unlimited())
    }
}

#[test]
fn preflight_denies_session_access() {
    let result: Result<TestSession, _> = AgentSession::builder(empty_router())
        .with_session_id(SessionId::from_name("test"))
        .with_access(Arc::new(DenySessionAccess))
        .build();

    match result {
        Ok(_) => panic!("expected preflight to deny session access"),
        Err(err) => assert!(
            err.to_string().contains("access denied"),
            "expected access denied, got: {err}"
        ),
    }
}

#[test]
fn preflight_denies_model_access() {
    let result: Result<TestSession, _> = AgentSession::builder(empty_router())
        .with_session_id(SessionId::from_name("test"))
        .with_access(Arc::new(DenyModelAccess))
        .build();

    match result {
        Ok(_) => panic!("expected preflight to deny model access"),
        Err(err) => assert!(
            err.to_string().contains("access denied"),
            "expected access denied, got: {err}"
        ),
    }
}

// ---------------------------------------------------------------------------
// Steering queue tests

#[test]
fn steer_and_follow_up_inject_messages() {
    let session: TestSession = AgentSession::builder(empty_router())
        .with_session_id(SessionId::from_name("test"))
        .build()
        .expect("build should succeed");

    session.steer(user_msg("priority message"));
    session.follow_up(user_msg("follow up message"));

    // end() should drain these without error
    session.end().expect("end should succeed");
}

#[test]
fn end_is_idempotent() {
    let session: TestSession = AgentSession::builder(empty_router())
        .with_session_id(SessionId::from_name("test"))
        .build()
        .expect("build should succeed");

    session.end().expect("first end should succeed");
    session.end().expect("second end should also succeed");
}

// ---------------------------------------------------------------------------
// Resume

#[test]
fn with_session_id_starts_a_new_session_when_none_exists() {
    let id = SessionId::from_name("create-or-continue");

    let session: TestSession = AgentSession::builder(empty_router())
        .with_session_id(id.clone())
        .build()
        .expect("with_session_id builds even when the stores hold nothing for the id");

    assert_eq!(*session.session_id(), id);
    // end() on a fresh session with empty queues is a no-op.
    session.end().expect("end should succeed");
}

#[test]
fn resume_fails_with_session_not_found_for_an_unknown_id() {
    let id = SessionId::from_name("resume-missing");

    let err = AgentSession::builder(empty_router())
        .resume(id.clone())
        .build()
        .err()
        .expect("resume(id) over empty stores must fail");

    assert_eq!(*err.current_context(), AgenticError::SessionNotFound(id));
}

#[test]
fn resume_continues_a_session_the_stores_hold() {
    let id = SessionId::from_name("resume-existing");

    // Earlier history for `id`, already in the document store.
    let doc = MemoryDocumentStore::new();
    let earlier = user_msg("from an earlier run");
    doc.append_with_id(
        &id.to_string(),
        &earlier.id().to_string(),
        SessionRecord::Conversation { message: earlier },
    )
    .expect("seed the store");

    let session = AgentSession::builder(empty_router())
        .resume(id.clone())
        .with_doc_store(doc)
        .build()
        .expect("resume(id) builds when the stores hold the session");

    assert_eq!(*session.session_id(), id);
    let history = session.message_api().all().expect("history readable");
    assert_eq!(
        history.len(),
        1,
        "the earlier record is visible: {history:?}"
    );
}

#[test]
fn resume_looks_only_at_the_named_session() {
    // Records under a different id don't make an unknown id resumable.
    let doc = MemoryDocumentStore::new();
    let other = SessionId::from_name("someone-else");
    let msg = user_msg("not yours");
    doc.append_with_id(
        &other.to_string(),
        &msg.id().to_string(),
        SessionRecord::Conversation { message: msg },
    )
    .expect("seed the store");

    let missing = SessionId::from_name("resume-other-missing");
    let err = AgentSession::builder(empty_router())
        .resume(missing.clone())
        .with_doc_store(doc)
        .build()
        .err()
        .expect("resume(id) must not pick up another session's records");
    assert_eq!(
        *err.current_context(),
        AgenticError::SessionNotFound(missing)
    );
}

// ---------------------------------------------------------------------------
// Config wiring

#[test]
fn builder_wires_config_overrides() {
    let mut config = AgentConfig::default();
    config.max_outer_iterations = 3;
    config.max_inner_iterations = 5;

    let _session: TestSession = AgentSession::builder(empty_router())
        .with_session_id(SessionId::from_name("test"))
        .with_config(config)
        .with_user(UserId("custom-user".into()))
        .build()
        .expect("build with custom config should succeed");
}

// ---------------------------------------------------------------------------
// Extension handles (F14)

#[test]
fn extension_handles_are_accessible() {
    let session: TestSession = AgentSession::builder(empty_router())
        .with_session_id(SessionId::from_name("test"))
        .build()
        .expect("build should succeed");

    let _api = session.message_api();
    let _ledger = session.ledger();
    let _queues = session.steering_queues();
    let _router = session.router();
    let _tools = session.tool_manager();
}

#[test]
fn message_api_subscribe_receives_events() {
    let session: TestSession = AgentSession::builder(empty_router())
        .with_session_id(SessionId::from_name("test-subscribe"))
        .build()
        .expect("build should succeed");

    let rx = session.message_api().subscribe();

    session.steer(user_msg("event test"));
    session.end().expect("end should succeed");

    let events: Vec<_> = rx.try_iter().collect();
    assert!(
        !events.is_empty(),
        "subscriber should have received at least one event"
    );
}

// ---------------------------------------------------------------------------
// Proposal 15, item 9: model ids from strings

/// Records the model name preflight asks about.
struct RecordingModelAccess {
    asked: std::sync::Mutex<Vec<String>>,
}

impl SessionAccessProvider for RecordingModelAccess {
    fn can_access_session(&self, _: &UserId, _: &SessionId) -> Result<bool, AuthError> {
        Ok(true)
    }
    fn can_use_model(&self, _: &UserId, model: &str) -> Result<bool, AuthError> {
        self.asked.lock().unwrap().push(model.to_string());
        Ok(true)
    }
    fn token_budget(&self, _: &UserId) -> Result<TokenBudget, AuthError> {
        Ok(TokenBudget::unlimited())
    }
}

#[test]
fn builder_takes_model_names_as_strings() {
    let access = Arc::new(RecordingModelAccess {
        asked: std::sync::Mutex::new(Vec::new()),
    });
    let _session: TestSession = AgentSession::builder(empty_router())
        .with_model("claude-sonnet-4-6")
        .with_fallback_models(["gpt-4o", "gpt-4o-mini"])
        .with_memory_model(String::from("claude-haiku"))
        .with_access(Arc::clone(&access) as Arc<dyn SessionAccessProvider>)
        .build()
        .expect("build with string model ids succeeds");
    assert_eq!(
        *access.asked.lock().unwrap(),
        vec!["claude-sonnet-4-6".to_string()],
        "the string became the primary model preflight checks"
    );

    // A Vec<ModelId> is still accepted.
    let _session: TestSession = AgentSession::builder(empty_router())
        .with_model(ModelId::Name("m".into(), None))
        .with_fallback_models(vec![ModelId::Name("f".into(), None)])
        .build()
        .expect("ModelId arguments still work");
}
