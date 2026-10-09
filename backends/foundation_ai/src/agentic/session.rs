//! `AgentSession` — the public session API (F20).
//!
//! WHY: Callers (CLI, server, tests) should not have to wire `MessageApi`,
//! `ContextProvider`, `ToolCallManager`, `SteeringQueues`, `MemoryHierarchy`,
//! `TokenLedger`, `LoopDetector`, `CircuitBreaker`, and `ProviderRouter` by
//! hand. One builder, one handle, one lifecycle.
//!
//! WHAT: `AgentSession` — a builder taking a `ProviderRouter` (F12), optional
//! stores (typestate) and a `ToolShed` (the session's tools), preflight
//! validation (access, budget) BEFORE scheduling onto valtron, `run_turn` /
//! `run_turn_stream` / `steer` / `follow_up` / `end`, and resume by id
//! (`with_session_id` / `resume`, Decision 01 order).
//!
//! HOW: `AgentSessionBuilder` collects required + optional deps, `build()`
//! wires `SessionInner`, runs preflight, returns `AgentSession`.
//! `run_turn_stream` pushes the prompt, constructs an `AgentLoop`, calls
//! `execute()` (which cfg-selects sendables vs `non_sendables` based on the
//! `multi` feature), and returns the `DrivenStreamIterator`.

use std::sync::Arc;

use foundation_core::valtron::execute;
use foundation_db::traits::DocumentStore;
use foundation_db::{MemoryDocumentStore, MemoryStorage};
use foundation_errstacks::ErrorTrace;

use crate::agentic::access::{AllowAllAccess, SessionAccessProvider};
use crate::agentic::agent_loop::{AgentConfig, AgentLoop};
use crate::agentic::context::{ContextConfig, ContextProvider};
use crate::agentic::errors::{AgenticError, UserId};
use crate::agentic::memory::{MemoryConfig, MemoryHierarchy};
use crate::agentic::memory_coordinator::MemoryCoordinator;
use crate::agentic::memory_store::{KvMemoryStore, MemoryStore};
use crate::agentic::message_api::MessageApi;
use crate::agentic::steering::SteeringQueues;
use crate::agentic::token_ledger::TokenLedger;
use crate::agentic::tool_impl::ToolCallManager;
use crate::agentic::toolshed::{SessionParts, ToolShed};
use crate::agentic::turn::{Answer, Turn, TurnStream};
use crate::agentic::ErrorPolicy;
use crate::types::{Messages, ModelId, ProviderRouter, SessionId, SessionRecord};

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
/// HOW: Built via `AgentSession::builder(router)`. `build()` wires
/// `SessionInner`, builds the tools from the `ToolShed`, runs preflight
/// (access, budget), returns `AgentSession`. `run_turn_stream`
/// pushes the user's prompt, constructs an `AgentLoop`, and schedules it
/// via `execute()` — which cfg-selects the sendables or `non_sendables`
/// executor based on the `multi` feature.
pub struct AgentSession<D = MemoryDocumentStore, M = KvMemoryStore<MemoryStorage>> {
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
    tool_manager: ToolCallManager,
    queues: SteeringQueues,
    /// Shared with tools built from `SessionParts`.
    memory: Arc<MemoryHierarchy<M, D>>,
    message_api: MessageApi<D>,
    policy: ErrorPolicy,
    ledger: TokenLedger,
    /// Shared with tools built from `SessionParts`.
    context_provider: Arc<ContextProvider<D, M>>,
    access: Arc<dyn SessionAccessProvider>,
    user: UserId,
    config: AgentConfig,
}

// ---------------------------------------------------------------------------
// AgentSessionBuilder

/// Builder for `AgentSession`.
///
/// WHY: A session needs a `ProviderRouter` and two stores; everything else has
/// a default. The stores are part of the builder's *type*: the builder starts
/// on the in-memory stores, and a store with no `Default` (D1/R2, SQL over a
/// live connection, …) is simply passed in — `build()` has no `Default` bound.
///
/// WHAT: Started by [`AgentSession::builder`] with `MemoryDocumentStore`,
/// `KvMemoryStore<MemoryStorage>` and a fresh `SessionId`.
/// [`with_doc_store`](Self::with_doc_store) /
/// [`with_memory_store`](Self::with_memory_store) swap a store and change the
/// builder's type; the other `with_*` methods set optional overrides.
///
/// HOW: `build()` wires every component over one shared `Arc` per store and
/// runs preflight before returning a session. If preflight fails, no valtron
/// task is scheduled.
pub struct AgentSessionBuilder<D = MemoryDocumentStore, M = KvMemoryStore<MemoryStorage>> {
    session_id: SessionId,
    /// Set by [`resume`](Self::resume): `build()` fails unless the stores
    /// already hold records for `session_id`.
    require_existing: bool,
    router: ProviderRouter,
    toolshed: ToolShed,
    access: Arc<dyn SessionAccessProvider>,
    user: UserId,
    policy: Option<ErrorPolicy>,
    model: Option<ModelId>,
    fallback_models: Vec<ModelId>,
    memory_model: Option<ModelId>,
    doc_store: D,
    memory_store: M,
    system_prompt: Option<String>,
    config: AgentConfig,
    context_config: ContextConfig,
    memory_config: MemoryConfig,
    /// Optional embedding capability for real semantic recall (F16).
    embedder: Option<(Arc<dyn crate::agentic::embedding::EmbeddingProvider>, String)>,
}

impl AgentSession {
    /// Start a builder over the in-memory stores and a fresh `SessionId`.
    ///
    /// One concrete impl, so `AgentSession::builder(router)` needs no
    /// turbofish. Swap the stores with
    /// [`with_doc_store`](AgentSessionBuilder::with_doc_store) /
    /// [`with_memory_store`](AgentSessionBuilder::with_memory_store) (each
    /// changes the builder's type) and pick the session with
    /// [`with_session_id`](AgentSessionBuilder::with_session_id).
    #[must_use]
    pub fn builder(router: impl Into<ProviderRouter>) -> AgentSessionBuilder {
        AgentSessionBuilder::new(
            router.into(),
            MemoryDocumentStore::new(),
            KvMemoryStore::new(MemoryStorage::new()),
        )
    }
}

impl<D: DocumentStore + 'static, M: MemoryStore + 'static> AgentSession<D, M> {
    /// The old two-argument builder: a session id plus default-constructed
    /// stores of the turbofished types.
    #[deprecated(
        note = "use AgentSession::builder(router).with_session_id(id), plus with_doc_store / with_memory_store for other stores"
    )]
    #[must_use]
    pub fn builder_for(session_id: SessionId, router: ProviderRouter) -> AgentSessionBuilder<D, M>
    where
        D: Default,
        M: Default,
    {
        AgentSessionBuilder::new(router, D::default(), M::default()).with_session_id(session_id)
    }

    /// Start a builder over explicit stores.
    #[deprecated(
        note = "use AgentSession::builder(router).with_session_id(id).with_doc_store(doc_store).with_memory_store(memory_store)"
    )]
    #[must_use]
    pub fn builder_with_stores(
        session_id: SessionId,
        router: ProviderRouter,
        doc_store: D,
        memory_store: M,
    ) -> AgentSessionBuilder<D, M> {
        AgentSessionBuilder::new(router, doc_store, memory_store).with_session_id(session_id)
    }
}

impl<D: DocumentStore + 'static, M: MemoryStore + 'static> AgentSessionBuilder<D, M> {
    fn new(router: ProviderRouter, doc_store: D, memory_store: M) -> Self {
        Self {
            session_id: SessionId::new(),
            require_existing: false,
            router,
            toolshed: ToolShed::new(),
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

    /// Rebuild this builder over different stores, keeping every other setting.
    fn map_stores<D2, M2>(self, f: impl FnOnce(D, M) -> (D2, M2)) -> AgentSessionBuilder<D2, M2> {
        let (doc_store, memory_store) = f(self.doc_store, self.memory_store);
        AgentSessionBuilder {
            session_id: self.session_id,
            require_existing: self.require_existing,
            router: self.router,
            toolshed: self.toolshed,
            access: self.access,
            user: self.user,
            policy: self.policy,
            model: self.model,
            fallback_models: self.fallback_models,
            memory_model: self.memory_model,
            doc_store,
            memory_store,
            system_prompt: self.system_prompt,
            config: self.config,
            context_config: self.context_config,
            memory_config: self.memory_config,
            embedder: self.embedder,
        }
    }

    /// Use this session id (default: a fresh `SessionId::new()`).
    ///
    /// Create-or-continue: if the stores already hold records for `id`, the
    /// session continues from them (context assembly reads the session's
    /// messages and memory from the stores on every turn); otherwise a new
    /// session starts under `id`.
    #[must_use]
    pub fn with_session_id(mut self, id: SessionId) -> Self {
        self.session_id = id;
        self
    }

    /// Resume exactly this session: like
    /// [`with_session_id`](Self::with_session_id), but `build()` fails with
    /// [`AgenticError::SessionNotFound`] when the document store holds no
    /// records for `id` — so a typo or the wrong store can't silently start an
    /// empty session.
    #[must_use]
    pub fn resume(mut self, id: SessionId) -> Self {
        self.session_id = id;
        self.require_existing = true;
        self
    }

    /// Keep the session's message log in `store`. Changes the builder's type.
    #[must_use]
    pub fn with_doc_store<D2: DocumentStore + 'static>(
        self,
        store: D2,
    ) -> AgentSessionBuilder<D2, M> {
        self.map_stores(|_, memory_store| (store, memory_store))
    }

    /// Keep the session's memory tiers in `store`. Changes the builder's type.
    #[must_use]
    pub fn with_memory_store<M2: MemoryStore + 'static>(
        self,
        store: M2,
    ) -> AgentSessionBuilder<D, M2> {
        self.map_stores(|doc_store, _| (doc_store, store))
    }

    /// The tools the session can call — the only way to give a session tools.
    ///
    /// `build()` constructs every tool in the shed (session-dependent ones from
    /// the session's parts) and the result is the session's `ToolCallManager`.
    /// The model is offered the built-in `shed` meta-tool; the tools `shed`
    /// returns become active and are declared on later requests.
    #[must_use]
    pub fn with_toolshed(mut self, toolshed: ToolShed) -> Self {
        self.toolshed = toolshed;
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
    /// The toolshed then builds the session's `ToolCallManager` from the
    /// session's parts, so every tool is registered by construction.
    ///
    /// Preflight checks (OD-20-2): (1) access provider permits session + model,
    /// (2) budget is retrieved and applied to the ledger. If any check fails,
    /// no valtron task is scheduled.
    /// # Errors
    /// Returns [`ErrorTrace<AgenticError>`] if preflight checks fail.
    pub fn build(self) -> Result<AgentSession<D, M>, ErrorTrace<AgenticError>> {
        let session_id = self.session_id;

        let doc_store = Arc::new(self.doc_store);
        let memory_store = Arc::new(self.memory_store);

        let ledger = TokenLedger::new();
        let message_api = MessageApi::from_shared(session_id.clone(), Arc::clone(&doc_store));

        // `resume(id)`: the session must already exist in the stores.
        if self.require_existing {
            let existing = message_api.recent(1).map_err(|e| {
                ErrorTrace::new(AgenticError::MessageStore(format!(
                    "reading session {session_id} to resume it: {e}"
                )))
            })?;
            if existing.is_empty() {
                return Err(ErrorTrace::new(AgenticError::SessionNotFound(session_id)));
            }
        }

        let embedder = self.embedder;
        let context_provider = Arc::new({
            let cp = ContextProvider::new(
                session_id.clone(),
                message_api.clone(),
                Arc::clone(&memory_store),
                ledger.clone(),
                self.system_prompt,
                self.context_config,
            );
            // Wire the embedder for real semantic recall (F16) when provided.
            match &embedder {
                Some((embedder, model)) => cp.with_embedder(Arc::clone(embedder), model.clone()),
                None => cp,
            }
        });

        let queues = SteeringQueues::new();

        let coordinator = MemoryCoordinator::from_shared(memory_store, doc_store);
        let memory = Arc::new(MemoryHierarchy::new(
            session_id.clone(),
            coordinator,
            ledger.clone(),
            self.memory_config,
        ));

        // The shed's output is the session's tool manager.
        let parts = SessionParts {
            session_id: session_id.clone(),
            context: Arc::clone(&context_provider) as _,
            memory: Arc::clone(&memory) as _,
            embedder,
        };
        let tool_manager = self
            .toolshed
            .build(&parts)
            .map_err(|e| ErrorTrace::new(AgenticError::from(e)))?;

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

    /// Stream the turn as it runs — the primary API.
    ///
    /// Pushes the prompt into the follow-up queue, builds a fresh `AgentLoop`,
    /// schedules it via `execute()` (cfg-selects sendables vs `non_sendables`
    /// based on the `multi` feature), and returns a [`TurnStream`]: iterate it
    /// for the raw `Stream<SessionRecord, AgentProgress>` items, or call
    /// [`TurnStream::events`] for [`TurnEvent`](crate::agentic::TurnEvent)s.
    /// # Errors
    /// Returns [`ErrorTrace<AgenticError>`] if the loop cannot be scheduled.
    pub fn run_turn_stream(
        &self,
        prompt: impl Into<Messages>,
    ) -> Result<TurnStream<D, M>, ErrorTrace<AgenticError>> {
        self.inner.queues.push_follow_up(prompt.into());
        tracing::trace!(
            follow_up_len = self.inner.queues.follow_up.len(),
            "run_turn_stream: queued prompt for the loop"
        );

        let agent_loop = AgentLoop::new(
            self.inner.session_id.clone(),
            (*self.inner.context_provider).clone(),
            self.inner.tool_manager.clone(),
            SteeringQueues::from_shared(
                self.inner.queues.priority.clone(),
                self.inner.queues.follow_up.clone(),
                self.inner.queues.cancel_signal.clone(),
            ),
            (*self.inner.memory).clone(),
            self.inner.message_api.clone(),
            self.inner.ledger.clone(),
            self.inner.policy.clone(),
            self.inner.router.clone(),
            self.inner.config.clone(),
        )
        .with_access(Arc::clone(&self.inner.access), self.inner.user.clone());

        execute(agent_loop, None).map(TurnStream::new).map_err(|e| {
            ErrorTrace::new(AgenticError::Session(format!(
                "failed to schedule agent loop: {e}"
            )))
        })
    }

    /// Run a turn to the end and collect its records into a [`Turn`].
    ///
    /// Partial output first, then the error: a turn that a `FailedAction`
    /// ends part-way is still `Ok` — the [`Turn`] keeps the records produced
    /// before it and [`Turn::failure`] reports the error. Output the loop
    /// withdrew (`Retracted`) is dropped. `Turn` derefs to
    /// `Vec<SessionRecord>`.
    /// # Errors
    /// Returns [`ErrorTrace<AgenticError>`] only if the turn could not start.
    pub fn run_turn(&self, prompt: impl Into<Messages>) -> Result<Turn, ErrorTrace<AgenticError>> {
        Ok(Turn::collect(self.run_turn_stream(prompt)?))
    }

    /// Run a turn and return just its assistant text.
    ///
    /// A turn that fails part-way is not an `Err`: it comes back as
    /// [`Answer::Failed`] carrying the text produced before the failure.
    /// [`Answer::into_result`] turns that into an `Err` for callers that only
    /// want success.
    /// # Errors
    /// Returns [`ErrorTrace<AgenticError>`] only if the turn could not start
    /// (preflight, access, the stream failing to start).
    pub fn ask(&self, prompt: impl Into<Messages>) -> Result<Answer, ErrorTrace<AgenticError>> {
        Ok(Answer::collect(self.run_turn_stream(prompt)?))
    }

    /// Inject a high-priority steering message — interrupts current work.
    pub fn steer(&self, msg: impl Into<Messages>) {
        self.inner.queues.push_priority(msg.into());
    }

    /// Inject a follow-up message — processed after current work completes.
    pub fn follow_up(&self, msg: impl Into<Messages>) {
        self.inner.queues.push_follow_up(msg.into());
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
    /// With most store types `D::default()` / `M::default()` open fresh, empty
    /// stores, so this finds no history; and unlike the builder it can't take
    /// a system prompt, tools or an embedder.
    /// # Errors
    /// Returns [`ErrorTrace<AgenticError>`] if preflight checks fail.
    #[deprecated(
        note = "use AgentSession::builder(router).resume(id) (must exist) or .with_session_id(id) (create-or-continue), then .build()"
    )]
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
        Self::resume_from(
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
    /// Create-or-continue: the same as
    /// `builder(router).with_session_id(id).with_doc_store(..).with_memory_store(..)`.
    /// # Errors
    /// Returns [`ErrorTrace<AgenticError>`] if preflight checks fail.
    #[deprecated(
        note = "use AgentSession::builder(router).resume(id).with_doc_store(doc_store).with_memory_store(memory_store).build()"
    )]
    pub fn resume_with_stores(
        session_id: SessionId,
        router: ProviderRouter,
        config: AgentConfig,
        policy: Option<ErrorPolicy>,
        doc_store: D,
        memory_store: M,
    ) -> Result<AgentSession<D, M>, ErrorTrace<AgenticError>> {
        Self::resume_from(session_id, router, config, policy, doc_store, memory_store)
    }

    fn resume_from(
        session_id: SessionId,
        router: ProviderRouter,
        config: AgentConfig,
        policy: Option<ErrorPolicy>,
        doc_store: D,
        memory_store: M,
    ) -> Result<AgentSession<D, M>, ErrorTrace<AgenticError>> {
        let mut builder = AgentSessionBuilder::new(router, doc_store, memory_store)
            .with_session_id(session_id)
            .with_config(config);
        if let Some(policy) = policy {
            builder = builder.with_error_policy(policy);
        }
        builder.build()
    }
}
