//! Regression tests for the session wiring fixes: shared stores, builder-owned
//! tools, context message order, chunk coalescing, text-protocol tool calls,
//! argument validation, and access-provider enforcement.
//!
//! Each test drives the public `AgentSession` API over a `MockModelProvider`.

use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use foundation_ai::agentic::testing::{
    mock_text, mock_text_usage, mock_tool_call, MockModelProvider,
};
use foundation_ai::agentic::{
    AgentConfig, AgentSession, AuthError, KvMemoryStore, MemoryStore, SessionAccessProvider,
    TokenBudget, ToolCallResult, ToolDefinition, ToolError, ToolImpl, UserId,
};
use foundation_ai::harness::ToolPreset;
use foundation_ai::types::{
    ArgType, Args, MessageRole, Messages, ModelId, ModelOutput, ProviderRouter, SessionId,
    SessionRecord, TextContent, Tool, ToolShed, UsageCosting, UsageReport, UserModelContent,
};
use foundation_core::valtron::valtron_test;
use foundation_db::traits::DocumentStore;
use foundation_db::{MemoryDocumentStore, MemoryStorage};
use foundation_jsonschema::scheme;

type Doc = MemoryDocumentStore;
type Mem = KvMemoryStore<MemoryStorage>;
type Session = AgentSession<Doc, Mem>;
type SeenArgs = Arc<Mutex<Vec<HashMap<String, ArgType>>>>;

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

fn mock_model() -> ModelId {
    ModelId::Name("mock".into(), None)
}

fn session_with(mock: MockModelProvider) -> Session {
    AgentSession::builder(SessionId::new(), mock.into_router())
        .with_model(mock_model())
        .build()
        .expect("session builds")
}

/// A tool that records the arguments it was called with.
struct RecordingTool {
    seen: SeenArgs,
}

impl RecordingTool {
    fn new() -> (Self, SeenArgs) {
        let seen = Arc::new(Mutex::new(Vec::new()));
        (
            Self {
                seen: Arc::clone(&seen),
            },
            seen,
        )
    }
}

#[async_trait]
impl ToolImpl for RecordingTool {
    fn definition(&self) -> Tool {
        Tool::SingleCommand(ToolDefinition {
            name: "greet".into(),
            category: "custom".into(),
            description: "Greet someone by name.".into(),
            arguments: Args::new(
                scheme::object()
                    .required("name", scheme::string().min_len(1))
                    .build(),
            ),
            returns: None,
        })
    }

    async fn execute(
        &self,
        arguments: HashMap<String, ArgType>,
    ) -> Result<ToolCallResult, ToolError> {
        self.seen.lock().unwrap().push(arguments);
        Ok(ToolCallResult {
            content: UserModelContent::Text(TextContent {
                content: "greeted".into(),
                signature: None,
            }),
            error_detail: None,
        })
    }
}

fn tool_results(records: &[SessionRecord]) -> Vec<(String, Option<String>)> {
    records
        .iter()
        .filter_map(|record| match record {
            SessionRecord::Conversation {
                message:
                    Messages::ToolResult {
                        name, error_detail, ..
                    },
            } => Some((name.clone(), error_detail.clone())),
            _ => None,
        })
        .collect()
}

fn stored_assistant_texts(session: &Session) -> Vec<String> {
    session
        .message_api()
        .all()
        .expect("history readable")
        .into_iter()
        .filter_map(|record| match record {
            SessionRecord::Conversation {
                message:
                    Messages::Assistant {
                        content: ModelOutput::Text(text),
                        ..
                    },
            } => Some(text.content),
            _ => None,
        })
        .collect()
}

// ---------------------------------------------------------------------------
// Stores

/// Compiles only if `builder_with_stores` + `build` need no `Default` bound —
/// the requirement that used to shut out every persistent store.
fn build_over_any_stores<D, M>(router: ProviderRouter, doc: D, mem: M) -> AgentSession<D, M>
where
    D: DocumentStore + 'static,
    M: MemoryStore + 'static,
{
    AgentSession::builder_with_stores(SessionId::new(), router, doc, mem)
        .with_model(mock_model())
        .build()
        .expect("session builds over explicit stores")
}

#[valtron_test]
fn builder_with_stores_needs_no_default_bound() {
    let mut mock = MockModelProvider::new();
    mock.on_any(vec![mock_text("hello")]);
    let session = build_over_any_stores(
        mock.into_router(),
        MemoryDocumentStore::new(),
        KvMemoryStore::new(MemoryStorage::new()),
    );
    let records = session.run_turn(user_msg("hi")).expect("turn succeeds");
    assert!(records.iter().any(|r| matches!(
        r,
        SessionRecord::Conversation {
            message: Messages::Assistant { .. }
        }
    )));
}

#[valtron_test]
fn memory_tool_writes_reach_the_assembled_context() {
    let mut mock = MockModelProvider::new();
    mock.on_nth_call(
        0,
        vec![mock_tool_call(
            "memory",
            HashMap::from([
                ("command".to_string(), ArgType::Text("add".into())),
                (
                    "fact".to_string(),
                    ArgType::Text("the user prefers tea".into()),
                ),
            ]),
        )],
    );
    mock.on_any(vec![mock_text("noted")]);

    let session = session_with(mock);
    ToolPreset::memory(Arc::new(session.memory_hierarchy().clone()))
        .register_all(session.tool_manager());

    let records = session
        .run_turn(user_msg("remember I like tea"))
        .expect("turn succeeds");
    assert_eq!(
        tool_results(&records),
        vec![("memory".to_string(), None)],
        "the memory tool must run without error"
    );

    // What the next turn's context will contain: the fact, read back through
    // the context provider's own store.
    let context = session.context_provider();
    let memory = context
        .memory_store()
        .hydrate_sync(session.session_id())
        .expect("memory readable");
    let assembled = context.assemble_from_memory(&memory);
    let has_fact = assembled.messages.iter().any(|message| {
        matches!(
            message,
            Messages::User { content: UserModelContent::Text(t), .. }
                if t.content.contains("the user prefers tea")
        )
    });
    assert!(
        has_fact,
        "working memory must be visible to context assembly"
    );
}

#[valtron_test]
fn resume_with_stores_sees_earlier_history() {
    let doc = MemoryDocumentStore::new();
    let mem = KvMemoryStore::new(MemoryStorage::new());
    let id = SessionId::new();

    let mut mock = MockModelProvider::new();
    mock.on_any(vec![mock_text("first answer")]);
    let first = AgentSession::builder_with_stores(id.clone(), mock.into_router(), doc, mem)
        .with_model(mock_model())
        .build()
        .expect("first session builds");
    first
        .run_turn(user_msg("first question"))
        .expect("turn succeeds");
    first.end().expect("end succeeds");

    // MemoryDocumentStore isn't Clone, so hand the resumed session the same
    // underlying records by re-reading them into a fresh store.
    let carried = MemoryDocumentStore::new();
    for record in first.message_api().all().expect("history readable") {
        let id_str = match &record {
            SessionRecord::Conversation { message } => message.id().to_string(),
            _ => foundation_compact::ids::new_scru128_string(),
        };
        carried
            .append_with_id(&id.to_string(), &id_str, record)
            .expect("copy record");
    }

    let mut mock = MockModelProvider::new();
    mock.on_any(vec![mock_text("second answer")]);
    let resumed = Session::resume_with_stores(
        id,
        mock.into_router(),
        AgentConfig {
            primary_model: mock_model(),
            ..AgentConfig::default()
        },
        None,
        carried,
        KvMemoryStore::new(MemoryStorage::new()),
    )
    .expect("resume succeeds");

    let memory = resumed
        .context_provider()
        .memory_store()
        .hydrate_sync(resumed.session_id())
        .expect("memory readable");
    let context = resumed.context_provider().assemble_from_memory(&memory);
    assert!(
        context.messages.iter().any(|m| matches!(
            m,
            Messages::User { content: UserModelContent::Text(t), .. } if t.content == "first question"
        )),
        "resumed context must include the earlier turn"
    );
}

// ---------------------------------------------------------------------------
// Context order and persistence

#[valtron_test]
fn recent_messages_reach_the_model_oldest_first() {
    let mut mock = MockModelProvider::new();
    mock.on_any(vec![mock_text("ok")]);
    let session = session_with(mock);

    session.run_turn(user_msg("first")).expect("turn 1");
    session.run_turn(user_msg("second")).expect("turn 2");

    let memory = session
        .context_provider()
        .memory_store()
        .hydrate_sync(session.session_id())
        .expect("memory readable");
    let context = session.context_provider().assemble_from_memory(&memory);
    let users: Vec<String> = context
        .messages
        .iter()
        .filter_map(|m| match m {
            Messages::User {
                role: MessageRole::User,
                content: UserModelContent::Text(t),
                ..
            } => Some(t.content.clone()),
            _ => None,
        })
        .collect();
    assert_eq!(users, vec!["first".to_string(), "second".to_string()]);
}

#[valtron_test]
fn streamed_deltas_are_persisted_as_one_message() {
    let mut mock = MockModelProvider::new();
    mock.on_any(vec![
        mock_text("Hel"),
        mock_text("lo, "),
        mock_text("world"),
    ]);
    let session = session_with(mock);

    session.run_turn(user_msg("hi")).expect("turn succeeds");
    assert_eq!(
        stored_assistant_texts(&session),
        vec!["Hello, world".to_string()]
    );
}

#[valtron_test]
fn streamed_snapshots_are_persisted_as_one_message() {
    let mut mock = MockModelProvider::new();
    mock.on_any(vec![
        mock_text("Hel"),
        mock_text("Hello"),
        mock_text("Hello, world"),
    ]);
    let session = session_with(mock);

    session.run_turn(user_msg("hi")).expect("turn succeeds");
    assert_eq!(
        stored_assistant_texts(&session),
        vec!["Hello, world".to_string()]
    );
}

// ---------------------------------------------------------------------------
// Tools

#[valtron_test]
fn builder_registered_tools_satisfy_preflight() {
    let mut mock = MockModelProvider::new();
    mock.on_any(vec![mock_text("ok")]);
    let (tool, _) = RecordingTool::new();
    let tool: Arc<dyn ToolImpl> = Arc::new(tool);

    let session = Session::builder(SessionId::new(), mock.into_router())
        .with_model(mock_model())
        .with_toolshed(ToolShed::default().with_tool(tool.definition()))
        .with_tool(tool)
        .build()
        .expect("a declared and registered tool passes preflight");
    assert!(session
        .tool_manager()
        .names()
        .contains(&"greet".to_string()));
}

#[valtron_test]
fn text_protocol_tool_calls_run_the_tool() {
    let mut mock = MockModelProvider::new();
    // A local model's streamed tokens spelling out a text-protocol call.
    mock.on_nth_call(
        0,
        vec![
            mock_text("Let me greet them. <ToolCall>{\"name\":\"greet\","),
            mock_text("\"arguments\":{\"name\":\"Ada\"}}</ToolCall>"),
        ],
    );
    mock.on_any(vec![mock_text("Done.")]);

    let session = session_with(mock);
    let (tool, seen) = RecordingTool::new();
    session.tool_manager().register(Arc::new(tool));

    let records = session
        .run_turn(user_msg("greet Ada"))
        .expect("turn succeeds");

    assert_eq!(tool_results(&records), vec![("greet".to_string(), None)]);
    let calls = seen.lock().unwrap();
    assert_eq!(calls.len(), 1, "the tool runs exactly once");
    assert_eq!(
        calls[0].get("name"),
        Some(&ArgType::Text("Ada".into())),
        "the tool receives the call's arguments, not the envelope"
    );
    assert!(stored_assistant_texts(&session).contains(&"Let me greet them.".to_string()));
}

#[valtron_test]
fn arguments_violating_the_schema_are_rejected_before_execution() {
    let mut mock = MockModelProvider::new();
    mock.on_nth_call(0, vec![mock_tool_call("greet", HashMap::new())]);
    mock.on_any(vec![mock_text("sorry")]);

    let session = session_with(mock);
    let (tool, seen) = RecordingTool::new();
    session.tool_manager().register(Arc::new(tool));

    let records = session.run_turn(user_msg("greet")).expect("turn succeeds");
    let results = tool_results(&records);
    assert_eq!(results.len(), 1);
    assert!(
        results[0]
            .1
            .as_deref()
            .is_some_and(|e| e.contains("invalid arguments")),
        "the model sees a validation error: {results:?}"
    );
    assert!(seen.lock().unwrap().is_empty(), "the tool must not run");
}

// ---------------------------------------------------------------------------
// Access provider

struct ScriptedAccess {
    allow_tools: bool,
    allow_spend: bool,
    recorded: AtomicU64,
}

impl SessionAccessProvider for ScriptedAccess {
    fn can_access_session(&self, _: &UserId, _: &SessionId) -> Result<bool, AuthError> {
        Ok(true)
    }
    fn can_use_model(&self, _: &UserId, _: &str) -> Result<bool, AuthError> {
        Ok(true)
    }
    fn can_use_tool(&self, _: &UserId, _: &str) -> Result<bool, AuthError> {
        Ok(self.allow_tools)
    }
    fn can_spend(&self, _: &UserId, _: u64) -> Result<bool, AuthError> {
        Ok(self.allow_spend)
    }
    fn token_budget(&self, _: &UserId) -> Result<TokenBudget, AuthError> {
        Ok(TokenBudget::unlimited())
    }
    fn record_usage(&self, _: &UserId, tokens: u64) -> Result<(), AuthError> {
        self.recorded.fetch_add(tokens, Ordering::Relaxed);
        Ok(())
    }
}

fn session_with_access(mock: MockModelProvider, access: Arc<ScriptedAccess>) -> Session {
    Session::builder(SessionId::new(), mock.into_router())
        .with_model(mock_model())
        .with_access(access)
        .build()
        .expect("session builds")
}

#[valtron_test]
fn denied_tools_do_not_run() {
    let mut mock = MockModelProvider::new();
    mock.on_nth_call(
        0,
        vec![mock_tool_call(
            "greet",
            HashMap::from([("name".to_string(), ArgType::Text("Ada".into()))]),
        )],
    );
    mock.on_any(vec![mock_text("ok")]);
    let access = Arc::new(ScriptedAccess {
        allow_tools: false,
        allow_spend: true,
        recorded: AtomicU64::new(0),
    });
    let session = session_with_access(mock, access);
    let (tool, seen) = RecordingTool::new();
    session.tool_manager().register(Arc::new(tool));

    let records = session
        .run_turn(user_msg("greet Ada"))
        .expect("turn succeeds");
    let results = tool_results(&records);
    assert!(
        results[0]
            .1
            .as_deref()
            .is_some_and(|e| e.contains("not authorized")),
        "{results:?}"
    );
    assert!(
        seen.lock().unwrap().is_empty(),
        "a denied tool must not run"
    );
}

#[valtron_test]
fn refused_spend_ends_the_turn() {
    let mut mock = MockModelProvider::new();
    mock.on_any(vec![mock_text("never sent")]);
    let access = Arc::new(ScriptedAccess {
        allow_tools: true,
        allow_spend: false,
        recorded: AtomicU64::new(0),
    });
    let session = session_with_access(mock, access);

    let err = session.run_turn(user_msg("hi")).expect_err("spend refused");
    assert!(format!("{err:?}").contains("budget exceeded"), "{err:?}");
}

#[valtron_test]
fn usage_is_reported_to_the_access_provider() {
    let usage = UsageReport {
        input: 7.0,
        output: 5.0,
        cache_read: 0.0,
        cache_write: 0.0,
        total_tokens: 12.0,
        cost: UsageCosting::zero(foundation_ai::types::CostStatus::Estimated),
    };
    let mut mock = MockModelProvider::new();
    mock.on_any(vec![mock_text_usage("hi there", usage)]);
    let access = Arc::new(ScriptedAccess {
        allow_tools: true,
        allow_spend: true,
        recorded: AtomicU64::new(0),
    });
    let session = session_with_access(mock, Arc::clone(&access));

    session.run_turn(user_msg("hi")).expect("turn succeeds");
    assert_eq!(access.recorded.load(Ordering::Relaxed), 12);
}
