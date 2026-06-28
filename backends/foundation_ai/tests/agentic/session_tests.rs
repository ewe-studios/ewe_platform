use std::sync::Arc;

use foundation_ai::agentic::{
    AgentConfig, AgentSession, KvMemoryStore, SessionAccessProvider, TokenBudget,
};
use foundation_ai::types::{
    MessageRole, Messages, ModelId, ProviderRouter, SessionId, TextContent, UserModelContent,
};
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
    let session: TestSession = AgentSession::builder(id.clone(), empty_router())
        .build()
        .expect("build should succeed");

    assert_eq!(*session.session_id(), id);
}

#[test]
fn builder_accepts_custom_session_id() {
    let custom_id = SessionId::from_name("test-session");
    let session: TestSession = AgentSession::builder(custom_id.clone(), empty_router())
        .build()
        .expect("build should succeed");

    assert_eq!(*session.session_id(), custom_id);
}

#[test]
fn builder_accepts_system_prompt() {
    let _session: TestSession = AgentSession::builder(SessionId::from_name("test"), empty_router())
        .with_system_prompt("You are a coding assistant.")
        .build()
        .expect("build should succeed with system prompt");
}

#[test]
fn builder_accepts_custom_model() {
    let _session: TestSession = AgentSession::builder(SessionId::from_name("test"), empty_router())
        .with_model(ModelId::Name("gpt-4".into(), None))
        .build()
        .expect("build should succeed with custom model");
}

#[test]
fn builder_accepts_fallback_models() {
    let _session: TestSession = AgentSession::builder(SessionId::from_name("test"), empty_router())
        .with_fallback_models(vec![
            ModelId::Name("gpt-4".into(), None),
            ModelId::Name("claude-3".into(), None),
        ])
        .build()
        .expect("build should succeed with fallbacks");
}

#[test]
fn session_is_clone() {
    let session: TestSession = AgentSession::builder(SessionId::from_name("test"), empty_router())
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
    let result: Result<TestSession, _> =
        AgentSession::builder(SessionId::from_name("test"), empty_router())
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
    let result: Result<TestSession, _> =
        AgentSession::builder(SessionId::from_name("test"), empty_router())
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
    let session: TestSession = AgentSession::builder(SessionId::from_name("test"), empty_router())
        .build()
        .expect("build should succeed");

    session.steer(user_msg("priority message"));
    session.follow_up(user_msg("follow up message"));

    // end() should drain these without error
    session.end().expect("end should succeed");
}

#[test]
fn end_is_idempotent() {
    let session: TestSession = AgentSession::builder(SessionId::from_name("test"), empty_router())
        .build()
        .expect("build should succeed");

    session.end().expect("first end should succeed");
    session.end().expect("second end should also succeed");
}

// ---------------------------------------------------------------------------
// Resume

#[test]
fn resume_creates_session_with_given_id() {
    let original_id = SessionId::from_name("resume-test");

    let resumed: TestSession = AgentSession::resume(
        original_id.clone(),
        empty_router(),
        AgentConfig::default(),
        None,
    )
    .expect("resume should succeed");

    assert_eq!(*resumed.session_id(), original_id);
}

#[test]
fn resumed_session_has_empty_queues() {
    let session: TestSession = AgentSession::resume(
        SessionId::from_name("resume-empty"),
        empty_router(),
        AgentConfig::default(),
        None,
    )
    .expect("resume should succeed");

    // end() on a fresh resumed session with empty queues should be a no-op
    session
        .end()
        .expect("end on resumed session should succeed");
}

// ---------------------------------------------------------------------------
// Config wiring

#[test]
fn builder_wires_config_overrides() {
    let mut config = AgentConfig::default();
    config.max_outer_iterations = 3;
    config.max_inner_iterations = 5;

    let _session: TestSession = AgentSession::builder(SessionId::from_name("test"), empty_router())
        .with_config(config)
        .with_user(UserId("custom-user".into()))
        .build()
        .expect("build with custom config should succeed");
}

// ---------------------------------------------------------------------------
// Extension handles (F14)

#[test]
fn extension_handles_are_accessible() {
    let session: TestSession = AgentSession::builder(SessionId::from_name("test"), empty_router())
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
    let session: TestSession =
        AgentSession::builder(SessionId::from_name("test-subscribe"), empty_router())
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
