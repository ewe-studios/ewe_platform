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
//! `execute()` (which cfg-selects sendables vs non_sendables based on the
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
use crate::agentic::tool_impl::ToolCallManager;
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
/// via `execute()` — which cfg-selects the sendables or non_sendables
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
    access: Arc<dyn SessionAccessProvider>,
    user: UserId,
    model: Option<ModelId>,
    fallback_models: Vec<ModelId>,
    memory_model: Option<ModelId>,
    doc_store: Option<D>,
    memory_store: Option<M>,
    system_prompt: Option<String>,
    config: AgentConfig,
    context_config: ContextConfig,
    memory_config: MemoryConfig,
}

impl<D: DocumentStore + 'static, M: MemoryStore + 'static> AgentSession<D, M> {
    #[must_use]
    pub fn builder(session_id: SessionId, router: ProviderRouter) -> AgentSessionBuilder<D, M> {
        AgentSessionBuilder {
            session_id,
            router,
            toolshed: ToolShed::default(),
            access: Arc::new(AllowAllAccess),
            user: UserId("local".into()),
            model: None,
            fallback_models: Vec::new(),
            memory_model: None,
            doc_store: None,
            memory_store: None,
            system_prompt: None,
            config: AgentConfig::default(),
            context_config: ContextConfig::default(),
            memory_config: MemoryConfig::default(),
        }
    }
}

impl<D: DocumentStore + 'static, M: MemoryStore + 'static> AgentSessionBuilder<D, M> {
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
    pub fn with_doc_store(mut self, store: D) -> Self {
        self.doc_store = Some(store);
        self
    }

    #[must_use]
    pub fn with_memory_store(mut self, store: M) -> Self {
        self.memory_store = Some(store);
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
    /// Preflight checks (OD-20-2): (1) every ToolShed tool is registered with
    /// the ToolCallManager, (2) access provider permits session + model,
    /// (3) budget is retrieved and applied to the ledger. If any check fails,
    /// no valtron task is scheduled.
    pub fn build(self) -> Result<AgentSession<D, M>, ErrorTrace<AgenticError>>
    where
        D: Default,
        M: Default,
    {
        let session_id = self.session_id;

        let doc_store = self.doc_store.unwrap_or_default();
        let memory_store = self.memory_store.unwrap_or_default();
        let memory_store_arc = Arc::new(memory_store);

        let ledger = TokenLedger::new();
        let message_api = MessageApi::new(session_id.clone(), doc_store);

        let context_provider = ContextProvider::new(
            session_id.clone(),
            message_api.clone(),
            Arc::clone(&memory_store_arc),
            ledger.clone(),
            self.system_prompt,
            self.context_config,
        );

        let tool_manager = ToolCallManager::new(session_id.clone());
        let queues = SteeringQueues::new();

        let coordinator = MemoryCoordinator::new(M::default(), D::default());
        let memory = MemoryHierarchy::new(
            session_id.clone(),
            coordinator,
            ledger.clone(),
            self.memory_config,
        );

        let mut config = self.config;
        if let Some(model) = self.model {
            config.primary_model = model;
        }
        config.fallback_models = self.fallback_models;
        config.memory_model = self.memory_model;

        let inner = SessionInner {
            session_id,
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
        let shed_name = self.toolshed.shed.as_ref().map(|t| t.name.as_str());
        for tool in self.toolshed.all_tools() {
            if shed_name == Some(tool.name.as_str()) {
                continue;
            }
            if !registered.contains(&tool.name) {
                return Err(ErrorTrace::new(AgenticError::Session(format!(
                    "toolshed tool '{}' not registered with ToolCallManager",
                    tool.name
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

    /// Stream each `SessionRecord` as produced — the primary API.
    ///
    /// Pushes the prompt into the follow-up queue, builds a fresh `AgentLoop`,
    /// schedules it via `execute()` (cfg-selects sendables vs non_sendables
    /// based on the `multi` feature), and returns the driven stream iterator.
    pub fn run_turn_stream(
        &self,
        prompt: Messages,
    ) -> Result<DrivenStreamIterator<AgentLoop<D, M>>, ErrorTrace<AgenticError>> {
        self.inner.queues.push_follow_up(prompt);

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
            self.inner.router.clone(),
            self.inner.config.clone(),
        );

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

    /// Synchronous teardown (Decision 01): flush message buffer, drain queues,
    /// persist remaining messages, reset cancel signal.
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

impl<D: DocumentStore + Default + 'static, M: MemoryStore + Default + 'static> AgentSession<D, M> {
    /// Rehydrate a session by `SessionId` — deterministic Decision 01 order.
    ///
    /// Resume protocol:
    /// 1. Load WorkingMemory (F15/F07)
    /// 2. Load Observation + Reflection (F15/F07; obs if no reflection)
    /// 3. `message_api.recent(10)` (F08; Decision 01 fixes 10)
    /// 4. Semantic recall (deferred — F31/F32)
    /// 5. Context assembled by F16 (system -> working -> reflection -> recent -> recalled)
    /// 6. Queues start EMPTY (were drained+persisted on prior end)
    /// 7. ToolCallManager fresh
    pub fn resume(
        session_id: SessionId,
        router: ProviderRouter,
        config: AgentConfig,
    ) -> Result<AgentSession<D, M>, ErrorTrace<AgenticError>> {
        let toolshed = ToolShed::default();
        let doc_store = D::default();
        let memory_store = M::default();
        let memory_store_arc = Arc::new(memory_store);

        let ledger = TokenLedger::new();
        let message_api = MessageApi::new(session_id.clone(), doc_store);

        let context_provider = ContextProvider::new(
            session_id.clone(),
            message_api.clone(),
            Arc::clone(&memory_store_arc),
            ledger.clone(),
            None,
            ContextConfig::default(),
        );

        let tool_manager = ToolCallManager::new(session_id.clone());
        let queues = SteeringQueues::new();

        let coordinator = MemoryCoordinator::new(M::default(), D::default());
        let memory = MemoryHierarchy::new(
            session_id.clone(),
            coordinator,
            ledger.clone(),
            MemoryConfig::default(),
        );

        let inner = SessionInner {
            session_id,
            router,
            toolshed,
            tool_manager,
            queues,
            memory,
            message_api,
            ledger,
            context_provider,
            access: Arc::new(AllowAllAccess),
            user: UserId("local".into()),
            config,
        };

        Ok(AgentSession {
            inner: Arc::new(inner),
        })
    }
}
