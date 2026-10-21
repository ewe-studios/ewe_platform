//! Regression tests for the session wiring fixes: shared stores, builder-owned
//! tools, context message order, chunk coalescing, text-protocol tool calls,
//! argument validation, and access-provider enforcement.
//!
//! Each test drives the public `AgentSession` API over a `MockModelProvider`.

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use foundation_ai::agentic::internals::{CacheStats, EmbeddingError, EmbeddingVector};
use foundation_ai::agentic::testing::{
    mock_text, mock_text_usage, mock_tool_call, MockModelProvider,
};
use foundation_ai::agentic::{
    AgentSession, AuthError, EmbeddingProvider, KvMemoryStore, MemoryStore, SessionAccessProvider,
    TokenBudget, ToolCallResult, ToolError, ToolImpl, ToolShed, UserId,
};
use foundation_ai::harness::ToolPreset;
use foundation_ai::types::{
    Args, MessageRole, Messages, ModelId, ModelInteraction, ModelOutput, ProviderRouter, SessionId,
    SessionRecord, TextContent, Tool, ToolArguments, ToolDefinition, UsageCosting, UsageReport,
    UserModelContent,
};
use foundation_core::valtron::valtron_test;
use foundation_db::traits::AsyncDocumentStore;
use foundation_db::{MemoryDocumentStore, MemoryStorage};
use foundation_jsonschema::scheme;

type Doc = MemoryDocumentStore;
type Mem = KvMemoryStore<MemoryStorage>;
type Session = AgentSession<Doc, Mem>;
type SeenArgs = Arc<Mutex<Vec<ToolArguments>>>;

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

async fn session_with(mock: MockModelProvider) -> Session {
    session_with_tools(mock, ToolShed::new()).await
}

async fn session_with_tools(mock: MockModelProvider, tools: ToolShed) -> Session {
    AgentSession::builder(mock.into_router())
        .with_model(mock_model())
        .with_toolshed(tools)
        .build()
        .await
        .expect("session builds")
}

/// Mark tools active, as if `shed` had returned them — for tests about what a
/// tool does once the model may call it.
fn activate(session: &Session, names: &[&str]) {
    let names: Vec<String> = names.iter().map(|n| (*n).to_string()).collect();
    session.tool_manager().activate(&names);
}

/// The tool names declared on each model request, in call order.
type Offered = Arc<Mutex<Vec<Vec<String>>>>;

/// Record what every request declares. Registered first, it never matches, so
/// the scripts added after it still answer.
fn record_offered(mock: &mut MockModelProvider) -> Offered {
    let offered: Offered = Arc::new(Mutex::new(Vec::new()));
    let sink = Arc::clone(&offered);
    mock.on(
        move |mi: &ModelInteraction| {
            let names = mi
                .tools_shed
                .all_tools()
                .iter()
                .map(|t| t.name().to_string())
                .collect();
            sink.lock().unwrap().push(names);
            false
        },
        vec![],
    );
    offered
}

fn shed_call(description: &str) -> Messages {
    mock_tool_call(
        "shed",
        ToolArguments::from_iter([(
            "description".to_string(),
            serde_json::Value::String(description.into()),
        )]),
    )
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

    async fn execute(&self, arguments: ToolArguments) -> Result<ToolCallResult, ToolError> {
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

async fn stored_assistant_texts(session: &Session) -> Vec<String> {
    session
        .message_api()
        .all()
        .await
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

/// Compiles only if `with_doc_store` / `with_memory_store` + `build` need no
/// `Default` bound — the requirement that used to shut out every persistent
/// store.
async fn build_over_any_stores<D, M>(router: ProviderRouter, doc: D, mem: M) -> AgentSession<D, M>
where
    D: AsyncDocumentStore + 'static,
    M: MemoryStore + 'static,
{
    AgentSession::builder(router)
        .with_doc_store(doc)
        .with_memory_store(mem)
        .with_model(mock_model())
        .build()
        .await
        .expect("session builds over explicit stores")
}

#[valtron_test]
async fn store_setters_need_no_default_bound() {
    let mut mock = MockModelProvider::new();
    mock.on_any(vec![mock_text("hello")]);
    let session = build_over_any_stores(
        mock.into_router(),
        MemoryDocumentStore::new(),
        KvMemoryStore::new(MemoryStorage::new()),
    )
    .await;
    let records = session.run_turn(user_msg("hi")).await.expect("turn succeeds");
    assert!(records.iter().any(|r| matches!(
        r,
        SessionRecord::Conversation {
            message: Messages::Assistant { .. }
        }
    )));
}

#[valtron_test]
async fn memory_tool_writes_reach_the_assembled_context() {
    let mut mock = MockModelProvider::new();
    mock.on_nth_call(
        0,
        vec![mock_tool_call(
            "memory",
            ToolArguments::from_iter([
                (
                    "command".to_string(),
                    serde_json::Value::String("add".into()),
                ),
                (
                    "fact".to_string(),
                    serde_json::Value::String("the user prefers tea".into()),
                ),
            ]),
        )],
    );
    mock.on_any(vec![mock_text("noted")]);

    let session =
        session_with_tools(mock, ToolShed::new().tools(ToolPreset::session_memory())).await;
    activate(&session, &["memory"]);

    let records = session
        .run_turn(user_msg("remember I like tea")).await
        .expect("turn succeeds");
    assert_eq!(
        tool_results(&records),
        vec![("memory".to_string(), None)],
        "the memory tool must run without error"
    );

    // What the next turn's context will contain: the fact, read back through
    // the context provider's own store.
    let assembled = session
        .context_provider()
        .assemble()
        .await
        .expect("context assembles");
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
async fn resume_sees_earlier_history() {
    let doc = MemoryDocumentStore::new();
    let mem = KvMemoryStore::new(MemoryStorage::new());
    let id = SessionId::new();

    let mut mock = MockModelProvider::new();
    mock.on_any(vec![mock_text("first answer")]);
    let first = AgentSession::builder(mock.into_router())
        .with_session_id(id.clone())
        .with_doc_store(doc)
        .with_memory_store(mem)
        .with_model(mock_model())
        .build()
        .await
        .expect("first session builds");
    first
        .run_turn(user_msg("first question")).await
        .expect("turn succeeds");
    first.end().await.expect("end succeeds");

    // MemoryDocumentStore isn't Clone, so hand the resumed session the same
    // underlying records by re-reading them into a fresh store.
    let carried = MemoryDocumentStore::new();
    for record in first.message_api().all().await.expect("history readable") {
        let id_str = match &record {
            SessionRecord::Conversation { message } => message.id().to_string(),
            _ => foundation_compact::ids::new_scru128_string(),
        };
        carried
            .append_with_id_async(&id.to_string(), &id_str, record)
            .await
            .expect("copy record");
    }

    let mut mock = MockModelProvider::new();
    mock.on_any(vec![mock_text("second answer")]);
    let resumed = AgentSession::builder(mock.into_router())
        .resume(id)
        .with_doc_store(carried)
        .with_memory_store(KvMemoryStore::new(MemoryStorage::new()))
        .with_model(mock_model())
        .build()
        .await
        .expect("resume succeeds");

    let context = resumed
        .context_provider()
        .assemble()
        .await
        .expect("context assembles");
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
async fn recent_messages_reach_the_model_oldest_first() {
    let mut mock = MockModelProvider::new();
    mock.on_any(vec![mock_text("ok")]);
    let session = session_with(mock).await;

    session.run_turn(user_msg("first")).await.expect("turn 1");
    session.run_turn(user_msg("second")).await.expect("turn 2");

    let context = session
        .context_provider()
        .assemble()
        .await
        .expect("context assembles");
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
async fn streamed_deltas_are_persisted_as_one_message() {
    let mut mock = MockModelProvider::new();
    mock.on_any(vec![
        mock_text("Hel"),
        mock_text("lo, "),
        mock_text("world"),
    ]);
    let session = session_with(mock).await;

    session.run_turn(user_msg("hi")).await.expect("turn succeeds");
    assert_eq!(
        stored_assistant_texts(&session).await,
        vec!["Hello, world".to_string()]
    );
}

#[valtron_test]
async fn streamed_snapshots_are_persisted_as_one_message() {
    let mut mock = MockModelProvider::new();
    mock.on_any(vec![
        mock_text("Hel"),
        mock_text("Hello"),
        mock_text("Hello, world"),
    ]);
    let session = session_with(mock).await;

    session.run_turn(user_msg("hi")).await.expect("turn succeeds");
    assert_eq!(
        stored_assistant_texts(&session).await,
        vec!["Hello, world".to_string()]
    );
}

// ---------------------------------------------------------------------------
// Tools

#[valtron_test]
async fn toolshed_tools_are_registered_at_build() {
    let mut mock = MockModelProvider::new();
    mock.on_any(vec![mock_text("ok")]);
    let (tool, _) = RecordingTool::new();

    let session = session_with_tools(mock, ToolShed::new().tool(tool)).await;
    assert!(session
        .tool_manager()
        .names()
        .contains(&"greet".to_string()));
}

#[valtron_test]
async fn duplicate_tool_names_fail_the_build() {
    let mut mock = MockModelProvider::new();
    mock.on_any(vec![mock_text("ok")]);
    let (first, _) = RecordingTool::new();
    let (second, _) = RecordingTool::new();

    let err = AgentSession::builder(mock.into_router())
        .with_model(mock_model())
        .with_toolshed(ToolShed::new().tool(first).tool(second))
        .build()
        .await
        .err()
        .expect("two tools named 'greet' must fail the build");
    assert!(
        err.to_string()
            .contains("'greet' was added to the toolshed twice"),
        "{err}"
    );
}

#[valtron_test]
async fn the_model_is_offered_only_shed_until_shed_returns_a_tool() {
    let mut mock = MockModelProvider::new();
    let offered = record_offered(&mut mock);
    mock.on_nth_call(0, vec![shed_call("greet someone")]);
    mock.on_nth_call(
        1,
        vec![mock_tool_call(
            "greet",
            ToolArguments::from_iter([(
                "name".to_string(),
                serde_json::Value::String("Ada".into()),
            )]),
        )],
    );
    mock.on_any(vec![mock_text("done")]);

    let (tool, seen) = RecordingTool::new();
    let session = session_with_tools(mock, ToolShed::new().tool(tool)).await;
    let records = session
        .run_turn(user_msg("greet Ada")).await
        .expect("turn succeeds");

    let offered = offered.lock().unwrap().clone();
    assert_eq!(
        offered[0],
        vec!["shed".to_string()],
        "the first request declares only shed"
    );
    assert_eq!(
        offered[1],
        vec!["shed".to_string(), "greet".to_string()],
        "a tool shed returned is declared on the next request"
    );
    assert_eq!(
        tool_results(&records),
        vec![("shed".to_string(), None), ("greet".to_string(), None)]
    );
    assert_eq!(seen.lock().unwrap().len(), 1, "the activated tool ran");
    assert!(session.tool_manager().is_active("greet"));
}

#[valtron_test]
async fn calling_a_tool_before_shed_returns_it_is_a_tool_error() {
    let mut mock = MockModelProvider::new();
    mock.on_nth_call(
        0,
        vec![mock_tool_call(
            "greet",
            ToolArguments::from_iter([(
                "name".to_string(),
                serde_json::Value::String("Ada".into()),
            )]),
        )],
    );
    mock.on_any(vec![mock_text("ok")]);

    let (tool, seen) = RecordingTool::new();
    let session = session_with_tools(mock, ToolShed::new().tool(tool)).await;
    let records = session
        .run_turn(user_msg("greet Ada")).await
        .expect("turn succeeds");

    let results = tool_results(&records);
    assert_eq!(results.len(), 1);
    let detail = results[0].1.as_deref().unwrap_or_default();
    assert!(
        detail.contains("not active") && detail.contains("`shed`"),
        "the model is told to discover the tool first: {detail}"
    );
    assert!(
        seen.lock().unwrap().is_empty(),
        "an inactive tool must not run"
    );
}

#[valtron_test]
async fn a_tool_less_session_declares_no_tools() {
    let mut mock = MockModelProvider::new();
    let offered = record_offered(&mut mock);
    mock.on_any(vec![mock_text("hi")]);

    let session = session_with(mock).await;
    session.run_turn(user_msg("hi")).await.expect("turn succeeds");
    assert_eq!(offered.lock().unwrap()[0], Vec::<String>::new());
}

/// Maps any text mentioning greeting to one direction and everything else to
/// another, so embedding search finds `greet` for a query with no word in
/// common with its name or description.
struct GreetingEmbedder;

impl EmbeddingProvider for GreetingEmbedder {
    fn embed(&self, text: &str, _model_id: &str) -> Result<EmbeddingVector, EmbeddingError> {
        let lower = text.to_lowercase();
        let data = if lower.contains("greet") || lower.contains("salute") {
            vec![1.0, 0.0]
        } else {
            vec![0.0, 1.0]
        };
        Ok(EmbeddingVector {
            data,
            dimensions: 2,
            model_id: "greeting".into(),
        })
    }
    fn embed_batch(
        &self,
        texts: &[String],
        model_id: &str,
    ) -> Result<Vec<EmbeddingVector>, EmbeddingError> {
        texts.iter().map(|t| self.embed(t, model_id)).collect()
    }
    fn register_model(&self, _model_id: &str, _dimensions: u16) {}
    fn cache_stats(&self) -> CacheStats {
        CacheStats::default()
    }
    fn clear_cache(&self) {}
}

#[valtron_test]
async fn shed_searches_by_embedding_when_the_session_has_an_embedder() {
    let mut mock = MockModelProvider::new();
    mock.on_any(vec![mock_text("ok")]);
    let (tool, _) = RecordingTool::new();

    let session = AgentSession::builder(mock.into_router())
        .with_model(mock_model())
        .with_embedder(Arc::new(GreetingEmbedder), "greeting")
        .with_toolshed(ToolShed::new().tool(tool))
        .build()
        .await
        .expect("session builds");

    // No word of "salute somebody" appears in greet's name or description.
    let hits = session
        .tool_manager()
        .search_tools("salute somebody", 5)
        .expect("search runs");
    assert_eq!(
        hits.iter().map(|h| h.name.as_str()).collect::<Vec<_>>(),
        vec!["greet"]
    );
}

#[valtron_test]
async fn shed_matches_names_and_descriptions_without_an_embedder() {
    let mut mock = MockModelProvider::new();
    mock.on_any(vec![mock_text("ok")]);
    let (tool, _) = RecordingTool::new();
    let session = session_with_tools(mock, ToolShed::new().tool(tool)).await;

    let mgr = session.tool_manager();
    let by_description = mgr.search_tools("someone by name", 5).expect("search runs");
    assert_eq!(by_description.len(), 1);
    assert_eq!(by_description[0].name, "greet");
    assert!(
        by_description[0].schema.is_some(),
        "hits carry the argument schema"
    );
    assert!(mgr
        .search_tools("salute somebody", 5)
        .expect("search runs")
        .is_empty());
}

#[valtron_test]
async fn session_dependent_tools_are_built_from_the_session() {
    let mut mock = MockModelProvider::new();
    mock.on_any(vec![mock_text("ok")]);
    let seen_id: Arc<Mutex<Option<SessionId>>> = Arc::new(Mutex::new(None));
    let sink = Arc::clone(&seen_id);

    let session = session_with_tools(
        mock,
        ToolShed::new()
            .tools(ToolPreset::search_context())
            .tool(foundation_ai::agentic::tool_fn("greet", move |parts| {
                *sink.lock().unwrap() = Some(parts.session_id.clone());
                let (tool, _) = RecordingTool::new();
                Arc::new(tool) as Arc<dyn ToolImpl>
            })),
    )
    .await;

    let mut names = session.tool_manager().names();
    names.sort();
    assert_eq!(
        names,
        vec!["greet".to_string(), "search_context".to_string()]
    );
    assert_eq!(
        seen_id.lock().unwrap().as_ref(),
        Some(session.session_id()),
        "the constructor sees the session being built"
    );
}

#[valtron_test]
async fn text_protocol_tool_calls_run_the_tool() {
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

    let (tool, seen) = RecordingTool::new();
    let session = session_with_tools(mock, ToolShed::new().tool(tool)).await;
    activate(&session, &["greet"]);

    let records = session
        .run_turn(user_msg("greet Ada")).await
        .expect("turn succeeds");

    assert_eq!(tool_results(&records), vec![("greet".to_string(), None)]);
    {
        let calls = seen.lock().unwrap();
        assert_eq!(calls.len(), 1, "the tool runs exactly once");
        assert_eq!(
            calls[0].get("name"),
            Some(&serde_json::Value::String("Ada".into())),
            "the tool receives the call's arguments, not the envelope"
        );
    }
    assert!(stored_assistant_texts(&session).await.contains(&"Let me greet them.".to_string()));
}

#[valtron_test]
async fn arguments_violating_the_schema_are_rejected_before_execution() {
    let mut mock = MockModelProvider::new();
    mock.on_nth_call(0, vec![mock_tool_call("greet", ToolArguments::new())]);
    mock.on_any(vec![mock_text("sorry")]);

    let (tool, seen) = RecordingTool::new();
    let session = session_with_tools(mock, ToolShed::new().tool(tool)).await;
    activate(&session, &["greet"]);

    let records = session.run_turn(user_msg("greet")).await.expect("turn succeeds");
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

async fn session_with_access(mock: MockModelProvider, access: Arc<ScriptedAccess>) -> Session {
    let (tool, _) = RecordingTool::new();
    AgentSession::builder(mock.into_router())
        .with_model(mock_model())
        .with_toolshed(ToolShed::new().tool(tool))
        .with_access(access)
        .build()
        .await
        .expect("session builds")
}

#[valtron_test]
async fn denied_tools_do_not_run() {
    let mut mock = MockModelProvider::new();
    mock.on_nth_call(
        0,
        vec![mock_tool_call(
            "greet",
            ToolArguments::from_iter([(
                "name".to_string(),
                serde_json::Value::String("Ada".into()),
            )]),
        )],
    );
    mock.on_any(vec![mock_text("ok")]);
    let access = Arc::new(ScriptedAccess {
        allow_tools: false,
        allow_spend: true,
        recorded: AtomicU64::new(0),
    });
    let session = session_with_access(mock, access).await;
    // Re-register a recording greet so the test can see whether it ran.
    let (tool, seen) = RecordingTool::new();
    session.tool_manager().register(Arc::new(tool));
    activate(&session, &["greet"]);

    let records = session
        .run_turn(user_msg("greet Ada")).await
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
async fn refused_spend_ends_the_turn() {
    let mut mock = MockModelProvider::new();
    mock.on_any(vec![mock_text("never sent")]);
    let access = Arc::new(ScriptedAccess {
        allow_tools: true,
        allow_spend: false,
        recorded: AtomicU64::new(0),
    });
    let session = session_with_access(mock, access).await;

    let turn = session.run_turn(user_msg("hi")).await.expect("the turn started");
    let err = turn.failure().expect("spend refused");
    assert!(err.to_string().contains("budget exceeded"), "{err}");
    assert!(turn.text().is_empty(), "nothing was generated");
}

#[valtron_test]
async fn usage_is_reported_to_the_access_provider() {
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
    let session = session_with_access(mock, Arc::clone(&access)).await;

    session.run_turn(user_msg("hi")).await.expect("turn succeeds");
    assert_eq!(access.recorded.load(Ordering::Relaxed), 12);
}
