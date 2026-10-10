use std::sync::Arc;

use foundation_ai::agentic::{
    AgentConfig, AgentSession, AgentSessionBuilder, AgenticError, KvMemoryStore, ModelSelection,
    SessionAccessProvider, TokenBudget,
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

/// A builder over the in-memory stores with a primary model set — `build()`
/// refuses a session without one.
fn builder() -> AgentSessionBuilder {
    AgentSession::builder(empty_router()).with_model("test-model")
}

// ---------------------------------------------------------------------------
// Builder + preflight tests

#[test]
fn builder_creates_session_with_named_id() {
    let id = SessionId::from_name("test-defaults");
    let session: TestSession = builder()
        .with_session_id(id.clone())
        .build()
        .expect("build should succeed");

    assert_eq!(*session.session_id(), id);
}

#[test]
fn builder_accepts_custom_session_id() {
    let custom_id = SessionId::from_name("test-session");
    let session: TestSession = builder()
        .with_session_id(custom_id.clone())
        .build()
        .expect("build should succeed");

    assert_eq!(*session.session_id(), custom_id);
}

#[test]
fn builder_accepts_system_prompt() {
    let _session: TestSession = builder()
        .with_session_id(SessionId::from_name("test"))
        .with_system_prompt("You are a coding assistant.")
        .build()
        .expect("build should succeed with system prompt");
}

#[test]
fn builder_accepts_custom_model() {
    let _session: TestSession = builder()
        .with_session_id(SessionId::from_name("test"))
        .with_model(ModelId::Name("gpt-4".into(), None))
        .build()
        .expect("build should succeed with custom model");
}

#[test]
fn builder_accepts_fallback_models() {
    let _session: TestSession = builder()
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
    let session: TestSession = builder()
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
    let result: Result<TestSession, _> = builder()
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
    let result: Result<TestSession, _> = builder()
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
    let session: TestSession = builder()
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
    let session: TestSession = builder()
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

    let session: TestSession = builder()
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

    let err = builder()
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

    let session = builder()
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
    let err = builder()
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

    let _session: TestSession = builder()
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
    let session: TestSession = builder()
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
    let session: TestSession = builder()
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
    let _session: TestSession = builder()
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
    let _session: TestSession = builder()
        .with_model(ModelId::Name("m".into(), None))
        .with_fallback_models(vec![ModelId::Name("f".into(), None)])
        .build()
        .expect("ModelId arguments still work");
}

// ---------------------------------------------------------------------------
// Model selection (item 14)

#[test]
fn build_fails_when_no_model_was_set() {
    let err = AgentSession::builder(empty_router())
        .build()
        .err()
        .expect("a session with no model must not build");
    assert_eq!(
        *err.current_context(),
        AgenticError::Session("no model set".into())
    );
}

#[test]
fn config_without_models_does_not_satisfy_build() {
    // AgentConfig no longer carries a model, so passing one cannot stand in
    // for `with_model`.
    let err = AgentSession::builder(empty_router())
        .with_config(AgentConfig::default())
        .build()
        .err()
        .expect("with_config alone sets no model");
    assert_eq!(
        *err.current_context(),
        AgenticError::Session("no model set".into())
    );
}

#[test]
fn builder_fills_the_model_selection() {
    let session: TestSession = AgentSession::builder(empty_router())
        .with_model("primary")
        .with_fallback_models(["first-fallback", "second-fallback"])
        .with_memory_model("memory")
        .build()
        .expect("build succeeds");

    assert_eq!(
        *session.models(),
        ModelSelection::new("primary")
            .with_fallbacks(["first-fallback", "second-fallback"])
            .with_memory("memory")
    );
}

#[test]
fn memory_model_reaches_the_memory_hierarchy() {
    let session: TestSession = builder()
        .with_memory_model("memory")
        .build()
        .expect("build succeeds");
    assert_eq!(
        session.memory_hierarchy().memory_model(),
        Some(&ModelId::from("memory"))
    );

    let session: TestSession = builder().build().expect("build succeeds");
    assert_eq!(session.models().memory, None);
    assert_eq!(session.memory_hierarchy().memory_model(), None);
}

// ---------------------------------------------------------------------------
// Public surface (doc 15 item 18)

#[test]
fn session_handles_are_named_through_internals() {
    use foundation_ai::agentic::internals::{
        AgentLoop, ContextProvider, MemoryHierarchy, MessageApi, SteeringQueues, TokenLedger,
        ToolCallManager,
    };

    let session: TestSession = builder().build().expect("build succeeds");
    // The extension handles return loop internals; each is reachable by name
    // under `agentic::internals`.
    let _: &MessageApi<TestDocStore> = session.message_api();
    let _: &TokenLedger = session.ledger();
    let _: &SteeringQueues = session.steering_queues();
    let _: &ToolCallManager = session.tool_manager();
    let _: &MemoryHierarchy<TestMemStore, TestDocStore> = session.memory_hierarchy();
    let _: &ContextProvider<TestDocStore, TestMemStore> = session.context_provider();
    assert!(std::any::type_name::<AgentLoop<TestDocStore, TestMemStore>>().contains("AgentLoop"));
}

#[test]
fn an_application_needs_only_the_agentic_root() {
    use foundation_ai::agentic::{
        AgentSession, Answer, ModelSelection, ToolPreset, ToolShed, TurnEvent, TurnSummary,
    };

    let session: TestSession = AgentSession::builder(empty_router())
        .with_model("test-model")
        .with_toolshed(ToolShed::new().tools(ToolPreset::shell()))
        .build()
        .expect("build succeeds");
    assert_eq!(session.models(), &ModelSelection::new("test-model"));
    // Types a caller matches on are nameable from the root.
    let _ = std::any::type_name::<(Answer, TurnEvent, TurnSummary)>();
}

// ---------------------------------------------------------------------------
// end() reports store failures

mod failing_store {
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::Arc;

    use foundation_db::traits::{Document, DocumentStore, PromotableDocument};
    use foundation_db::{MemoryDocumentStore, StorageError, StorageItemStream, StorageResult};
    use serde::de::DeserializeOwned;
    use serde::Serialize;

    /// A `MemoryDocumentStore` whose writes fail while `failing` is set.
    #[derive(Clone)]
    pub struct FailingDocStore {
        inner: Arc<MemoryDocumentStore>,
        pub failing: Arc<AtomicBool>,
    }

    impl FailingDocStore {
        pub fn new() -> Self {
            Self {
                inner: Arc::new(MemoryDocumentStore::new()),
                failing: Arc::new(AtomicBool::new(false)),
            }
        }

        fn check(&self) -> StorageResult<()> {
            if self.failing.load(Ordering::SeqCst) {
                Err(StorageError::Backend("disk full".into()))
            } else {
                Ok(())
            }
        }
    }

    impl DocumentStore for FailingDocStore {
        fn append<V: Serialize + Send + 'static>(
            &self,
            key: &str,
            content: V,
        ) -> StorageResult<Document> {
            self.check()?;
            self.inner.append(key, content)
        }
        fn append_with_id<V: Serialize + Send + 'static>(
            &self,
            key: &str,
            doc_id: &str,
            content: V,
        ) -> StorageResult<Document> {
            self.check()?;
            self.inner.append_with_id(key, doc_id, content)
        }
        fn scan<V: DeserializeOwned + Send + 'static>(
            &self,
            key: &str,
            limit: usize,
        ) -> StorageResult<StorageItemStream<'_, V>> {
            self.inner.scan(key, limit)
        }
        fn scan_all<V: DeserializeOwned + Send + 'static>(
            &self,
            key: &str,
        ) -> StorageResult<StorageItemStream<'_, V>> {
            self.inner.scan_all(key)
        }
        fn scan_from<V: DeserializeOwned + Send + 'static>(
            &self,
            key: &str,
            from_id: &str,
            limit: usize,
        ) -> StorageResult<StorageItemStream<'_, V>> {
            self.inner.scan_from(key, from_id, limit)
        }
        fn append_promotable<V: Serialize + PromotableDocument + Send + 'static>(
            &self,
            key: &str,
            content: V,
        ) -> StorageResult<Document> {
            self.check()?;
            self.inner.append_promotable(key, content)
        }
        fn append_promotable_with_id<V: Serialize + PromotableDocument + Send + 'static>(
            &self,
            key: &str,
            doc_id: &str,
            content: V,
        ) -> StorageResult<Document> {
            self.check()?;
            self.inner.append_promotable_with_id(key, doc_id, content)
        }
        fn scan_documents(&self, key: &str, limit: usize) -> StorageResult<Vec<Document>> {
            self.inner.scan_documents(key, limit)
        }
        fn scan_documents_from(
            &self,
            key: &str,
            from_id: &str,
            limit: usize,
        ) -> StorageResult<Vec<Document>> {
            self.inner.scan_documents_from(key, from_id, limit)
        }
        fn delete(&self, key: &str, doc_id: &str) -> StorageResult<()> {
            self.inner.delete(key, doc_id)
        }
        fn delete_all(&self, key: &str) -> StorageResult<u64> {
            self.inner.delete_all(key)
        }
        fn count(&self, key: &str) -> StorageResult<u64> {
            self.inner.count(key)
        }
    }
}

use failing_store::FailingDocStore;

fn session_over(store: &FailingDocStore) -> AgentSession<FailingDocStore, TestMemStore> {
    builder()
        .with_session_id(SessionId::from_name("end-failure"))
        .with_doc_store(store.clone())
        .build()
        .expect("build succeeds while the store works")
}

#[test]
fn end_returns_the_store_error_when_persisting_queued_messages_fails() {
    let store = FailingDocStore::new();
    let session = session_over(&store);

    session.follow_up(user_msg("queued before end"));
    store
        .failing
        .store(true, std::sync::atomic::Ordering::SeqCst);

    let err = session
        .end()
        .expect_err("a failing append must surface from end()");
    match err.current_context() {
        AgenticError::MessageStore(msg) => {
            assert!(msg.contains("flushing the message log"), "{msg}");
            assert!(
                msg.contains("disk full"),
                "the store's cause is kept: {msg}"
            );
        }
        other => panic!("expected MessageStore, got {other:?}"),
    }
    // Teardown still ran: the queue was drained and the cancel signal reset.
    assert!(!session.steering_queues().has_follow_up());
    assert!(!session.steering_queues().is_aborted());
    // Nothing was lost: the refused record waits for the next flush.
    assert_eq!(session.message_api().unflushed(), 1);
}

#[test]
fn end_retries_a_refused_record_once_the_store_recovers() {
    let store = FailingDocStore::new();
    let session = session_over(&store);

    session.steer(user_msg("first"));
    session.follow_up(user_msg("second"));
    store
        .failing
        .store(true, std::sync::atomic::Ordering::SeqCst);
    assert!(
        session.end().is_err(),
        "the flush fails while the store does"
    );

    store
        .failing
        .store(false, std::sync::atomic::Ordering::SeqCst);
    session.end().expect("end succeeds once the store recovers");
    assert_eq!(session.message_api().unflushed(), 0);

    // Both records reached the store, in order.
    let texts: Vec<String> = session
        .message_api()
        .all()
        .expect("read back")
        .iter()
        .map(|record| match record {
            SessionRecord::Conversation {
                message:
                    Messages::User {
                        content: UserModelContent::Text(t),
                        ..
                    },
            } => t.content.clone(),
            other => panic!("expected a user message, got {other:?}"),
        })
        .collect();
    assert_eq!(texts, vec!["first".to_string(), "second".to_string()]);
}

#[test]
fn reads_report_a_failed_flush_instead_of_returning_stale_records() {
    let store = FailingDocStore::new();
    let session = session_over(&store);

    session.follow_up(user_msg("buffered"));
    // Move the queued message into the log's buffer without flushing it.
    let msg = session
        .steering_queues()
        .pop_follow_up()
        .expect("queued message");
    let _id = session
        .message_api()
        .append(SessionRecord::Conversation { message: msg });
    store
        .failing
        .store(true, std::sync::atomic::Ordering::SeqCst);

    assert!(session.message_api().recent(10).is_err());
    assert!(session.message_api().all().is_err());
    assert!(session.message_api().flush().is_err());
}
