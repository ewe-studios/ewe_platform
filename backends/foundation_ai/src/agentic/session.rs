//! `AgentSession` — the public session API (F20).
//!
//! WHY: Callers (CLI, server, tests) should not have to wire `MessageApi`,
//! `ContextProvider`, `ToolCallManager`, `SteeringQueues`, `MemoryHierarchy`,
//! `TokenLedger`, `LoopDetector`, `CircuitBreaker`, and `ProviderRouter` by
//! hand. One builder, one handle, one lifecycle.
//!
//! WHAT: `AgentSession` — a builder requiring a `ProviderRouter` (F12) + a
//! `ToolShed` (F10), preflight validation (tools registered, access, budget)
//! BEFORE scheduling onto valtron, `run_turn` / `run_turn_stream` / `steer` /
//! `follow_up` / `end`, and deterministic resume (Decision 01 order).
//!
//! HOW: `AgentSessionBuilder` collects required + optional deps, `build()`
//! wires `SessionInner`, runs preflight, returns `AgentSession`.
//! `run_turn_stream` pushes the prompt, constructs an `AgentLoop`, calls
//! `execute()` (which cfg-selects sendables vs `non_sendables` based on the
//! `multi` feature), and returns the `DrivenStreamIterator`.

use std::sync::Arc;

use foundation_core::valtron::{execute, DrivenStreamIterator, Stream};
use foundation_db::traits::DocumentStore;
use foundation_errstacks::ErrorTrace;

use crate::agentic::access::{AllowAllAccess, SessionAccessProvider};
use crate::agentic::agent_loop::{AgentConfig, AgentLoop};
use crate::agentic::context::{ContextConfig, ContextProvider};
use crate::agentic::errors::{AgenticError, UserId};
use crate::agentic::memory::{MemoryConfig, MemoryHierarchy};
use crate::agentic::memory_coordinator::MemoryCoordinator;
use crate::agentic::memory_store::MemoryStore;
use crate::agentic::message_api::MessageApi;
use crate::agentic::steering::SteeringQueues;
use crate::agentic::token_ledger::TokenLedger;
use crate::agentic::tool_impl::{ToolCallManager, ToolImpl};
use crate::agentic::ErrorPolicy;
use crate::types::{Messages, ModelId, ProviderRouter, SessionId, SessionRecord, ToolShed};

// ---------------------------------------------------------------------------
// AgentSession

/// The public session handle — one per conversation.
///
/// WHY: Every component in the agentic layer is a leaf; `AgentLoop` (F19) is
/// the sequencer. But callers should not have to construct the loop, the
/// context provider, the memory hierarchy, the queues, the ledger, and the
/// tool manager by hand. `AgentSession` is the single handle that wires
/// everything, validates preflight, and exposes the lifecycle API.
///
/// WHAT: Arc-wrapped `SessionInner` — cheaply clonable, shareable across
/// threads. Exposes `run_turn_stream` (primary), `run_turn` (convenience),
/// `steer` / `follow_up` (inject steering messages), and `end` (synchronous
/// teardown).
///
/// HOW: Built via `AgentSessionBuilder` (requires `ProviderRouter` +
/// `ToolShed`). `build()` wires `SessionInner`, runs preflight (tools
/// registered, access, budget), returns `AgentSession`. `run_turn_stream`
/// pushes the user's prompt, constructs an `AgentLoop`, and schedules it
/// via `execute()` — which cfg-selects the sendables or `non_sendables`
/// executor based on the `multi` feature.
pub struct AgentSession<D, M> {
    inner: Arc<SessionInner<D, M>>,
}

impl<D, M> Clone for AgentSession<D, M> {
    fn clone(&self) -> Self {
        Self {
            inner: self.inner.clone(),
        }
    }
}

struct SessionInner<D, M> {
    session_id: SessionId,
    router: ProviderRouter,
    toolshed: ToolShed,
    tool_manager: ToolCallManager,
    queues: SteeringQueues,
    memory: MemoryHierarchy<M, D>,
    message_api: MessageApi<D>,
    policy: ErrorPolicy,
    ledger: TokenLedger,
    context_provider: ContextProvider<D, M>,
    access: Arc<dyn SessionAccessProvider>,
    user: UserId,
    config: AgentConfig,
}

// ---------------------------------------------------------------------------
// AgentSessionBuilder

/// Builder for `AgentSession` — requires a `ProviderRouter` and a `ToolShed`.
///
/// WHY: The user explicitly rejected `tools(vec![...])` (Decision 18) and
/// `Arc<dyn ModelProvider>` (not object-safe). The builder requires the two
/// concrete shapes the system actually needs: a `ProviderRouter` for model
/// routing and a `ToolShed` for tool definitions.
///
/// WHAT: Collects required deps (`router`, `toolshed`) and optional overrides
/// (stores, access provider, config). `build()` wires everything and runs
/// preflight validation before returning a session.
///
/// HOW: Builder pattern — call `AgentSession::builder(router, toolshed)`,
/// chain optional `.with_*()` methods, then `.build()`. Preflight checks
/// run inside `build()` — if they fail, no valtron task is scheduled.
pub struct AgentSessionBuilder<D, M> {
    session_id: SessionId,
    router: ProviderRouter,
    toolshed: ToolShed,
    /// Tools registered on the session's `ToolCallManager` before preflight.
    tools: Vec<Arc<dyn ToolImpl>>,
    access: Arc<dyn SessionAccessProvider>,
    user: UserId,
    policy: Option<ErrorPolicy>,
    model: Option<ModelId>,
    fallback_models: Vec<ModelId>,
    memory_model: Option<ModelId>,
    doc_store: StoreSlot<D>,
    memory_store: StoreSlot<M>,
    system_prompt: Option<String>,
    config: AgentConfig,
    context_config: ContextConfig,
    memory_config: MemoryConfig,
    /// Optional embedding capability for real semantic recall (F16).
    embedder: Option<(Arc<dyn crate::agentic::embedding::EmbeddingProvider>, String)>,
}

/// A store the builder will use: one the caller supplied, or a constructor for
/// the default.
///
/// WHY: `build()` used to demand `D: Default + M: Default` just to fill in
/// stores the caller hadn't set — which shut out every persistent backend,
/// since none of them can be default-constructed. Capturing `Default::default`
/// at the entry point that has the bound (`builder()`) lets `build()` drop it.
enum StoreSlot<T> {
    Value(T),
    Default(fn() -> T),
}

impl<T> StoreSlot<T> {
    fn into_value(self) -> T {
        match self {
            StoreSlot::Value(value) => value,
            StoreSlot::Default(make) => make(),
        }
    }
}

impl<D: DocumentStore + 'static, M: MemoryStore + 'static> AgentSession<D, M> {
    /// Start a builder that falls back to `D::default()` / `M::default()` for
    /// any store not set with `with_doc_store` / `with_memory_store`.
    ///
    /// Use [`builder_with_stores`](Self::builder_with_stores) for stores that
    /// can't be default-constructed (SQL, Turso, D1/R2, …).
    #[must_use]
    pub fn builder(session_id: SessionId, router: ProviderRouter) -> AgentSessionBuilder<D, M>
    where
        D: Default,
        M: Default,
    {
        AgentSessionBuilder::new(
            session_id,
            router,
            StoreSlot::Default(D::default),
            StoreSlot::Default(M::default),
        )
    }

    /// Start a builder over explicit stores — no `Default` bound.
    ///
    /// Building with a `session_id` whose history already lives in these stores
    /// picks the conversation up where it left off: context assembly reads the
    /// session's messages and memory from them on every turn.
    #[must_use]
    pub fn builder_with_stores(
        session_id: SessionId,
        router: ProviderRouter,
        doc_store: D,
        memory_store: M,
    ) -> AgentSessionBuilder<D, M> {
        AgentSessionBuilder::new(
            session_id,
            router,
            StoreSlot::Value(doc_store),
            StoreSlot::Value(memory_store),
        )
    }
}

impl<D: DocumentStore + 'static, M: MemoryStore + 'static> AgentSessionBuilder<D, M> {
    fn new(
        session_id: SessionId,
        router: ProviderRouter,
        doc_store: StoreSlot<D>,
        memory_store: StoreSlot<M>,
    ) -> Self {
        Self {
            session_id,
            router,
            toolshed: ToolShed::default(),
            tools: Vec::new(),
            access: Arc::new(AllowAllAccess),
            user: UserId("local".into()),
            model: None,
            fallback_models: Vec::new(),
            memory_model: None,
            doc_store,
            memory_store,
            system_prompt: None,
            policy: Some(ErrorPolicy::new()),
            config: AgentConfig::default(),
            context_config: ContextConfig::default(),
            memory_config: MemoryConfig::default(),
            embedder: None,
        }
    }

    /// Declare the tools the session must expose; `build()` fails unless each
    /// one is registered (via [`with_tool`](Self::with_tool) /
    /// [`with_tools`](Self::with_tools)).
    ///
    /// You rarely need this: the model is offered whatever is registered on the
    /// session's `ToolCallManager`, so registering tools is enough.
    #[must_use]
    pub fn with_toolshed(mut self, toolshed: ToolShed) -> Self {
        self.toolshed = toolshed;
        self
    }

    /// Register a tool on the session's `ToolCallManager` at `build()`.
    #[must_use]
    pub fn with_tool(mut self, tool: Arc<dyn ToolImpl>) -> Self {
        self.tools.push(tool);
        self
    }

    /// Register several tools at `build()` (e.g. `ToolPreset::tools().to_vec()`).
    #[must_use]
    pub fn with_tools(mut self, tools: impl IntoIterator<Item = Arc<dyn ToolImpl>>) -> Self {
        self.tools.extend(tools);
        self
    }

    #[must_use]
    pub fn with_access(mut self, access: Arc<dyn SessionAccessProvider>) -> Self {
        self.access = access;
        self
    }

    #[must_use]
    pub fn with_user(mut self, user: UserId) -> Self {
        self.user = user;
        self
    }

    #[must_use]
    pub fn with_model(mut self, model: ModelId) -> Self {
        self.model = Some(model);
        self
    }

    #[must_use]
    pub fn with_fallback_models(mut self, models: Vec<ModelId>) -> Self {
        self.fallback_models = models;
        self
    }

    #[must_use]
    pub fn with_memory_model(mut self, model: ModelId) -> Self {
        self.memory_model = Some(model);
        self
    }

    #[must_use]
    pub fn with_error_policy(mut self, policy: ErrorPolicy) -> Self {
        self.policy = Some(policy);
        self
    }

    #[must_use]
    pub fn with_doc_store(mut self, store: D) -> Self {
        self.doc_store = StoreSlot::Value(store);
        self
    }

    #[must_use]
    pub fn with_memory_store(mut self, store: M) -> Self {
        self.memory_store = StoreSlot::Value(store);
        self
    }

    #[must_use]
    pub fn with_system_prompt(mut self, prompt: impl Into<String>) -> Self {
        self.system_prompt = Some(prompt.into());
        self
    }

    #[must_use]
    pub fn with_config(mut self, config: AgentConfig) -> Self {
        self.config = config;
        self
    }

    /// Enable real semantic recall (F16): `search_context` ranks prior messages
    /// by embedding cosine similarity instead of keyword matching.
    #[must_use]
    pub fn with_embedder(
        mut self,
        embedder: Arc<dyn crate::agentic::embedding::EmbeddingProvider>,
        embedding_model: impl Into<String>,
    ) -> Self {
        self.embedder = Some((embedder, embedding_model.into()));
        self
    }

    #[must_use]
    pub fn with_context_config(mut self, config: ContextConfig) -> Self {
        self.context_config = config;
        self
    }

    #[must_use]
    pub fn with_memory_config(mut self, config: MemoryConfig) -> Self {
        self.memory_config = config;
        self
    }

    /// Wire components, run preflight, return the session.
    ///
    /// Every component shares one document store and one memory store: the
    /// message log, context assembly and the memory hierarchy all see the same
    /// data. (The hierarchy used to get its own default-constructed stores, so
    /// memory it wrote never reached the assembled context.)
    ///
    /// Preflight checks (OD-20-2): (1) every `ToolShed` tool is registered with
    /// the `ToolCallManager`, (2) access provider permits session + model,
    /// (3) budget is retrieved and applied to the ledger. If any check fails,
    /// no valtron task is scheduled.
    /// # Errors
    /// Returns [`ErrorTrace<AgenticError>`] if preflight checks fail.
    pub fn build(self) -> Result<AgentSession<D, M>, ErrorTrace<AgenticError>> {
        let session_id = self.session_id;

        let doc_store = Arc::new(self.doc_store.into_value());
        let memory_store = Arc::new(self.memory_store.into_value());

        let ledger = TokenLedger::new();
        let message_api = MessageApi::from_shared(session_id.clone(), Arc::clone(&doc_store));

        let context_provider = {
            let cp = ContextProvider::new(
                session_id.clone(),
                message_api.clone(),
                Arc::clone(&memory_store),
                ledger.clone(),
                self.system_prompt,
                self.context_config,
            );
            // Wire the embedder for real semantic recall (F16) when provided.
            match self.embedder {
                Some((embedder, model)) => cp.with_embedder(embedder, model),
                None => cp,
            }
        };

        let tool_manager = ToolCallManager::new(session_id.clone());
        for tool in self.tools {
            tool_manager.register(tool);
        }
        let queues = SteeringQueues::new();

        let coordinator = MemoryCoordinator::from_shared(memory_store, doc_store);
        let memory = MemoryHierarchy::new(
            session_id.clone(),
            coordinator,
            ledger.clone(),
            self.memory_config,
        );

        // The dedicated builder methods override the matching `AgentConfig`
        // fields only when they were called — otherwise values supplied through
        // `with_config` (or `resume`) must survive.
        let mut config = self.config;
        if let Some(model) = self.model {
            config.primary_model = model;
        }
        if !self.fallback_models.is_empty() {
            config.fallback_models = self.fallback_models;
        }
        if self.memory_model.is_some() {
            config.memory_model = self.memory_model;
        }

        let inner = SessionInner {
            session_id,
            policy: self.policy.unwrap_or_default(),
            router: self.router,
            toolshed: self.toolshed,
            tool_manager,
            queues,
            memory,
            message_api,
            ledger,
            context_provider,
            access: self.access,
            user: self.user,
            config,
        };

        inner.preflight()?;

        Ok(AgentSession {
            inner: Arc::new(inner),
        })
    }
}

// ---------------------------------------------------------------------------
// Preflight

impl<D: DocumentStore, M: MemoryStore> SessionInner<D, M> {
    fn preflight(&self) -> Result<(), ErrorTrace<AgenticError>> {
        let registered = self.tool_manager.names();
        let shed_name = self.toolshed.shed.as_ref().map(|t| t.name());
        for tool in self.toolshed.all_tools() {
            if shed_name == Some(tool.name()) {
                continue;
            }
            if !registered.contains(&tool.name().to_string()) {
                return Err(ErrorTrace::new(AgenticError::Session(format!(
                    "toolshed tool '{}' not registered with ToolCallManager",
                    tool.name()
                ))));
            }
        }

        if !self
            .access
            .can_access_session(&self.user, &self.session_id)
            .map_err(|e| ErrorTrace::new(AgenticError::Auth(e)))?
        {
            return Err(ErrorTrace::new(AgenticError::Session(
                "access denied: cannot access session".into(),
            )));
        }

        let model_name = self.config.primary_model.name();
        if !self
            .access
            .can_use_model(&self.user, model_name)
            .map_err(|e| ErrorTrace::new(AgenticError::Auth(e)))?
        {
            return Err(ErrorTrace::new(AgenticError::Session(format!(
                "access denied: cannot use model '{model_name}'"
            ))));
        }

        let budget = self
            .access
            .token_budget(&self.user)
            .map_err(|e| ErrorTrace::new(AgenticError::Auth(e)))?;
        self.ledger.set_budget(budget.remaining());

        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Lifecycle

impl<D: DocumentStore + 'static, M: MemoryStore + 'static> AgentSession<D, M> {
    #[must_use]
    pub fn session_id(&self) -> &SessionId {
        &self.inner.session_id
    }

    /// Extension handle: subscribe to message events, read records (F14).
    #[must_use]
    pub fn message_api(&self) -> &MessageApi<D> {
        &self.inner.message_api
    }

    /// Extension handle: read token/budget state (F14).
    #[must_use]
    pub fn ledger(&self) -> &TokenLedger {
        &self.inner.ledger
    }

    /// Extension handle: low-level queue access for steering (F14).
    #[must_use]
    pub fn steering_queues(&self) -> &SteeringQueues {
        &self.inner.queues
    }

    /// Extension handle: the model routing table (F14).
    #[must_use]
    pub fn router(&self) -> &ProviderRouter {
        &self.inner.router
    }

    /// Extension handle: the tool registry (F14).
    #[must_use]
    pub fn tool_manager(&self) -> &ToolCallManager {
        &self.inner.tool_manager
    }

    /// Extension handle: memory hierarchy + coordinator (F14).
    #[must_use]
    pub fn memory_hierarchy(&self) -> &MemoryHierarchy<M, D> {
        &self.inner.memory
    }

    /// Extension handle: the context provider (session history + memory recall).
    ///
    /// Clone it into a `SearchContextTool` to give the agent recall that uses
    /// this session's stores and embedder.
    #[must_use]
    pub fn context_provider(&self) -> &ContextProvider<D, M> {
        &self.inner.context_provider
    }

    /// Stream each `SessionRecord` as produced — the primary API.
    ///
    /// Pushes the prompt into the follow-up queue, builds a fresh `AgentLoop`,
    /// schedules it via `execute()` (cfg-selects sendables vs `non_sendables`
    /// based on the `multi` feature), and returns the driven stream iterator.
    /// # Errors
    /// Returns [`ErrorTrace<AgenticError>`] if the loop cannot be scheduled.
    pub fn run_turn_stream(
        &self,
        prompt: Messages,
    ) -> Result<DrivenStreamIterator<AgentLoop<D, M>>, ErrorTrace<AgenticError>> {
        self.inner.queues.push_follow_up(prompt);
        tracing::trace!(
            follow_up_len = self.inner.queues.follow_up.len(),
            "run_turn_stream: queued prompt for the loop"
        );

        let agent_loop = AgentLoop::new(
            self.inner.session_id.clone(),
            self.inner.context_provider.clone(),
            self.inner.tool_manager.clone(),
            SteeringQueues::from_shared(
                self.inner.queues.priority.clone(),
                self.inner.queues.follow_up.clone(),
                self.inner.queues.cancel_signal.clone(),
            ),
            self.inner.memory.clone(),
            self.inner.message_api.clone(),
            self.inner.ledger.clone(),
            self.inner.policy.clone(),
            self.inner.router.clone(),
            self.inner.config.clone(),
        )
        .with_access(Arc::clone(&self.inner.access), self.inner.user.clone());

        execute(agent_loop, None).map_err(|e| {
            ErrorTrace::new(AgenticError::Session(format!(
                "failed to schedule agent loop: {e}"
            )))
        })
    }

    /// Convenience wrapper: drains `run_turn_stream` into a `Vec`.
    ///
    /// A terminal `FailedAction` surfaces as `Err`; otherwise returns all
    /// collected `SessionRecord`s. Prefer `run_turn_stream` for streaming.
    /// # Errors
    /// Returns [`ErrorTrace<AgenticError>`] if the loop fails.
    pub fn run_turn(
        &self,
        prompt: Messages,
    ) -> Result<Vec<SessionRecord>, ErrorTrace<AgenticError>> {
        let stream = self.run_turn_stream(prompt)?;
        let mut records = Vec::new();

        for item in stream {
            match item {
                Stream::Next(record) => {
                    if let SessionRecord::FailedAction { ref error, .. } = record {
                        return Err(ErrorTrace::new(error.clone()));
                    }
                    // The loop withdrew the turn it had already streamed. Drop
                    // the assistant messages collected so far and keep whatever
                    // the retry produces; user turns and tool results stand.
                    if let SessionRecord::Retracted { ref reason, .. } = record {
                        tracing::debug!(%reason, "run_turn: dropping a withdrawn turn");
                        records.retain(|kept| {
                            !matches!(
                                kept,
                                SessionRecord::Conversation {
                                    message: Messages::Assistant { .. }
                                }
                            )
                        });
                        continue;
                    }
                    records.push(record);
                }
                Stream::Pending(_)
                | Stream::Init
                | Stream::Ignore
                | Stream::Wait
                | Stream::Delayed(_) => {}
                Stream::Spread(items) => {
                    use foundation_core::valtron::StreamSpread;
                    for s in items {
                        if let StreamSpread::Done(rec) = s {
                            records.push(rec);
                        }
                    }
                }
            }
        }

        Ok(records)
    }

    /// Inject a high-priority steering message — interrupts current work.
    pub fn steer(&self, msg: Messages) {
        self.inner.queues.push_priority(msg);
    }

    /// Inject a follow-up message — processed after current work completes.
    pub fn follow_up(&self, msg: Messages) {
        self.inner.queues.push_follow_up(msg);
    }

    /// Request a hard abort of the running turn.
    ///
    /// The signal is shared with the `AgentLoop`, which checks it at its outer
    /// and inner boundaries and terminates the turn (emitting its `Summary`)
    /// rather than starting another generation. Safe to call from another thread
    /// while `run_turn` is in flight.
    pub fn abort(&self) {
        self.inner.queues.abort();
    }

    /// Synchronous teardown (Decision 01): flush message buffer, drain queues,
    /// persist remaining messages, reset cancel signal.
    /// # Errors
    /// Returns [`ErrorTrace<AgenticError>`] if flushing fails.
    pub fn end(&self) -> Result<(), ErrorTrace<AgenticError>> {
        let _ = self.inner.message_api.flush();

        let priority_msgs = self.inner.queues.drain_priority();
        let follow_up_msgs = self.inner.queues.drain_follow_up();

        for msg in priority_msgs.into_iter().chain(follow_up_msgs) {
            let _ = self
                .inner
                .message_api
                .append(SessionRecord::Conversation { message: msg });
        }

        let _ = self.inner.message_api.flush();
        self.inner.queues.reset_cancel();
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Resume

impl<D: DocumentStore + 'static, M: MemoryStore + 'static> AgentSession<D, M> {
    /// Rehydrate a session by `SessionId` from default-constructed stores.
    ///
    /// Equivalent to `builder(session_id, router).with_config(config)`. It only
    /// finds earlier history when `D::default()` / `M::default()` reach the
    /// same data as before; for real persistence use
    /// [`resume_with_stores`](Self::resume_with_stores).
    /// # Errors
    /// Returns [`ErrorTrace<AgenticError>`] if preflight checks fail.
    pub fn resume(
        session_id: SessionId,
        router: ProviderRouter,
        config: AgentConfig,
        policy: Option<ErrorPolicy>,
    ) -> Result<AgentSession<D, M>, ErrorTrace<AgenticError>>
    where
        D: Default,
        M: Default,
    {
        Self::resume_with_stores(
            session_id,
            router,
            config,
            policy,
            D::default(),
            M::default(),
        )
    }

    /// Rehydrate a session by `SessionId` from the stores that hold it.
    ///
    /// Resume protocol (Decision 01): nothing is replayed eagerly. Every turn's
    /// context assembly reads the session's working memory, reflection,
    /// observation and recent messages from these stores, so the first turn
    /// after resuming already sees them. Queues start empty (they were drained
    /// and persisted by the previous `end()`); the tool registry starts empty —
    /// register tools again, or use `builder_with_stores` + `with_tool`.
    /// # Errors
    /// Returns [`ErrorTrace<AgenticError>`] if preflight checks fail.
    pub fn resume_with_stores(
        session_id: SessionId,
        router: ProviderRouter,
        config: AgentConfig,
        policy: Option<ErrorPolicy>,
        doc_store: D,
        memory_store: M,
    ) -> Result<AgentSession<D, M>, ErrorTrace<AgenticError>> {
        let mut builder = Self::builder_with_stores(session_id, router, doc_store, memory_store)
            .with_config(config);
        if let Some(policy) = policy {
            builder = builder.with_error_policy(policy);
        }
        builder.build()
    }
}
