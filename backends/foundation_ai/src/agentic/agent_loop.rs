//! `AgentLoop` — the orchestrator state machine (F19).
//!
//! WHY: Everything else in the agentic layer is a component; nothing runs a
//! *turn*. The loop sequences: context assembly → model generate/stream →
//! tool execution → output processing → follow-up continuation, all as a
//! non-blocking valtron `TaskIterator`.
//!
//! WHAT: `AgentLoop` implements `TaskIterator` with
//! `Ready = SessionRecord`, `Pending = AgentProgress`. The inner loop handles
//! tool calls + steering; the outer loop handles follow-up messages.
//!
//! HOW: State machine transitions drive components (F12 router, F11 tools,
//! F16 context, F15 memory, F17 loop detection, F02 circuit breaker). The
//! loop orchestrates — it owns no logic beyond sequencing.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use foundation_core::valtron::{
    BoxedSendExecutionAction, CancellableFutureTask, CancelOutcome, DrivenTaskIterator,
    FuturePollState, Stream, TaskIterator, TaskStatus,
};
use foundation_db::traits::DocumentStore;

use crate::agentic::context::{AgentContext, ContextProvider};
use crate::agentic::errors::{
    AgentAction, AgenticError, CircuitBreaker, ErrorPolicy,
};
use crate::agentic::loop_detection::{Escalation, LoopDetection, LoopDetector, LoopDetectorConfig};
use crate::agentic::memory::{MemoryAction, MemoryHierarchy};
use crate::agentic::memory_store::MemoryStore;
use crate::agentic::message_api::MessageApi;
use crate::agentic::progress::{lift_model_item, AgentProgress, MemoryKind};
use crate::agentic::steering::SteeringQueues;
use crate::agentic::token_ledger::TokenLedger;
use crate::agentic::tool_impl::{ToolCallManager, ToolCallRequest, ToolCallResult, ToolError};
use crate::types::{
    MessageRole, Messages, ModelId, ModelInteraction, ModelOutput, ModelParams,
    ModelState, SessionId, SessionRecord, TextContent, UserModelContent,
};

// ---------------------------------------------------------------------------
// AgentConfig

/// Configuration for the agent loop.
#[derive(Debug, Clone)]
pub struct AgentConfig {
    pub primary_model: ModelId,
    pub fallback_models: Vec<ModelId>,
    pub memory_model: Option<ModelId>,
    pub max_inner_iterations: usize,
    pub max_outer_iterations: usize,
    pub circuit_breaker_threshold: u32,
    pub preflight_compression_threshold: f32,
    pub context_pressure_threshold: f32,
    pub model_params: ModelParams,
}

impl Default for AgentConfig {
    fn default() -> Self {
        Self {
            primary_model: ModelId::Name(String::new(), None),
            fallback_models: Vec::new(),
            memory_model: None,
            max_inner_iterations: 25,
            max_outer_iterations: 10,
            circuit_breaker_threshold: 3,
            preflight_compression_threshold: 0.85,
            context_pressure_threshold: 0.70,
            model_params: ModelParams::default(),
        }
    }
}

// ---------------------------------------------------------------------------
// TurnOutput — collected outputs from a single generate step

struct TurnOutput {
    messages: Vec<Messages>,
    tool_calls: Vec<ToolCallRequest>,
}

// ---------------------------------------------------------------------------
// AgentLoopState

/// The state machine driving the orchestrator.
pub enum AgentLoopState {
    /// Initial setup — emit Init, transition to OuterBoundary.
    Initializing,
    /// Check FollowUpQueue for continuation; if empty, go to Ending.
    OuterBoundary,
    /// Run input processors, assemble context, generate.
    InnerAssemble,
    /// Model generation in progress — pump the stream step-wise.
    InnerGenerate {
        stream: Box<dyn Iterator<Item = Stream<Messages, ModelState>>>,
        collected: Vec<Messages>,
    },
    /// Tool calls extracted from the model's output.
    InnerToolCalls {
        calls: Vec<ToolCallRequest>,
    },
    /// Tool execution in progress — drives CancellableFutureTask per call.
    InnerExecuting {
        calls: Vec<ToolCallRequest>,
        results: Vec<(ToolCallRequest, Result<ToolCallResult, ToolError>)>,
        idx: usize,
        active: Option<DrivenTaskIterator<CancellableFutureTask<core::pin::Pin<Box<dyn core::future::Future<Output = Result<ToolCallResult, ToolError>> + Send>>>>>,
        cancel_signals: Vec<Arc<AtomicBool>>,
    },
    /// Emit tool results back as records and loop back to InnerAssemble.
    InnerEmitResults {
        results: Vec<Messages>,
        idx: usize,
    },
    /// Output processing — fire memory triggers, persist.
    OutputProcessing,
    /// Emit the final Summary record and end.
    Ending,
    /// Terminal state.
    Done,
}

// ---------------------------------------------------------------------------
// AgentLoop

/// The agentic orchestrator — a valtron `TaskIterator` running the nested
/// inner (tool calls + steering) / outer (follow-up) loop.
pub struct AgentLoop<D, M> {
    session_id: SessionId,
    context_provider: ContextProvider<D, M>,
    tool_manager: ToolCallManager,
    queues: SteeringQueues,
    memory: MemoryHierarchy<M, D>,
    message_api: MessageApi<D>,
    ledger: TokenLedger,
    detector: LoopDetector,
    policy: ErrorPolicy,
    breaker: CircuitBreaker,
    router: crate::types::ProviderRouter,

    state: AgentLoopState,
    current_model: ModelId,
    config: AgentConfig,
    inner_iteration: usize,
    outer_iteration: usize,
    message_count: u64,

    /// Pending messages to prepend (from steering/follow-up).
    pending_user_messages: Vec<Messages>,
    /// Last assembled context (reused on retry).
    last_context: Option<AgentContext>,
}

impl<D: DocumentStore, M: MemoryStore> AgentLoop<D, M> {
    #[must_use]
    pub fn new(
        session_id: SessionId,
        context_provider: ContextProvider<D, M>,
        tool_manager: ToolCallManager,
        queues: SteeringQueues,
        memory: MemoryHierarchy<M, D>,
        message_api: MessageApi<D>,
        ledger: TokenLedger,
        router: crate::types::ProviderRouter,
        config: AgentConfig,
    ) -> Self {
        let breaker = CircuitBreaker::new(
            config.circuit_breaker_threshold,
            config.fallback_models.clone(),
        );
        let current_model = config.primary_model.clone();
        Self {
            session_id,
            context_provider,
            tool_manager,
            queues,
            memory,
            message_api,
            ledger,
            detector: LoopDetector::new(LoopDetectorConfig::default()),
            policy: ErrorPolicy::new(),
            breaker,
            router,
            state: AgentLoopState::Initializing,
            current_model,
            config,
            inner_iteration: 0,
            outer_iteration: 0,
            message_count: 0,
            pending_user_messages: Vec::new(),
            last_context: None,
        }
    }

    /// Push a user message to be processed in the next inner iteration.
    pub fn push_user_message(&mut self, msg: Messages) {
        self.message_api.append(SessionRecord::Conversation {
            message: msg.clone(),
        });
        self.pending_user_messages.push(msg);
    }

    /// Current state label (for diagnostics).
    #[must_use]
    pub fn state_label(&self) -> &'static str {
        match &self.state {
            AgentLoopState::Initializing => "initializing",
            AgentLoopState::OuterBoundary => "outer_boundary",
            AgentLoopState::InnerAssemble => "inner_assemble",
            AgentLoopState::InnerGenerate { .. } => "inner_generate",
            AgentLoopState::InnerToolCalls { .. } => "inner_tool_calls",
            AgentLoopState::InnerExecuting { .. } => "inner_executing",
            AgentLoopState::InnerEmitResults { .. } => "inner_emit_results",
            AgentLoopState::OutputProcessing => "output_processing",
            AgentLoopState::Ending => "ending",
            AgentLoopState::Done => "done",
        }
    }

    /// The model currently being used (may have changed via circuit breaker).
    #[must_use]
    pub fn current_model(&self) -> &ModelId {
        &self.current_model
    }

    // -----------------------------------------------------------------------
    // State transition helpers

    fn transition_outer_boundary(&mut self) -> Option<TaskStatus<SessionRecord, AgentProgress, BoxedSendExecutionAction>> {
        self.outer_iteration += 1;
        if self.outer_iteration > self.config.max_outer_iterations {
            self.state = AgentLoopState::Ending;
            return Some(TaskStatus::Pending(AgentProgress::SessionEnding));
        }

        // Drain priority queue first.
        let priority_msgs = self.queues.drain_priority();
        if !priority_msgs.is_empty() {
            self.queues.reset_cancel();
            for msg in priority_msgs {
                self.push_user_message(msg);
            }
            self.inner_iteration = 0;
            self.state = AgentLoopState::InnerAssemble;
            return Some(TaskStatus::Pending(AgentProgress::Steering {
                source: std::borrow::Cow::Borrowed("priority_queue"),
            }));
        }

        // Drain follow-up queue.
        let follow_up_msgs = self.queues.drain_follow_up();
        if !follow_up_msgs.is_empty() {
            for msg in follow_up_msgs {
                self.push_user_message(msg);
            }
            self.inner_iteration = 0;
            self.state = AgentLoopState::InnerAssemble;
            return Some(TaskStatus::Pending(AgentProgress::Steering {
                source: std::borrow::Cow::Borrowed("follow_up_queue"),
            }));
        }

        // No more work — end the session.
        self.state = AgentLoopState::Ending;
        Some(TaskStatus::Pending(AgentProgress::SessionEnding))
    }

    fn transition_inner_assemble(&mut self) -> Option<TaskStatus<SessionRecord, AgentProgress, BoxedSendExecutionAction>> {
        // Check priority queue — front-inject interruption.
        if self.queues.has_priority() {
            let msgs = self.queues.drain_priority();
            self.queues.reset_cancel();
            for msg in msgs {
                self.push_user_message(msg);
            }
            return Some(TaskStatus::Pending(AgentProgress::Steering {
                source: std::borrow::Cow::Borrowed("priority_interrupt"),
            }));
        }

        // Check budget exhaustion.
        if self.ledger.is_exhausted() {
            let snapshot = self.ledger.snapshot();
            self.state = AgentLoopState::Ending;
            return Some(TaskStatus::Ready(
                AgenticError::BudgetExhausted { snapshot }.into_failed_action(),
            ));
        }

        // Attempt to get the model from the router.
        let model = match self.router.get_model(&self.current_model) {
            Ok(m) => m,
            Err(e) => {
                let err = AgenticError::from(e);
                return Some(self.handle_error(err));
            }
        };

        // Hydrate memory synchronously and assemble context.
        let memory = self.context_provider.memory_store()
            .hydrate_sync(&self.session_id)
            .unwrap_or_default();
        let ctx = self.context_provider.assemble_from_memory(&memory);

        // Ephemeral context-pressure layer (OD-19-7).
        let system_prompt = self.apply_context_pressure(&ctx);

        // Build the model interaction.
        let toolshed = self.tool_manager.build_toolshed();
        let interaction = ModelInteraction {
            system_prompt: system_prompt.or(ctx.system_prompt),
            soul: None,
            tools_shed: toolshed,
            messages: ctx.messages,
            chat_template: None,
            tool_choice: None,
        };

        let params = self.config.model_params.clone();
        let effective_max = self.ledger.effective_max_tokens(&params);
        let params = ModelParams {
            max_tokens: effective_max,
            ..params
        };

        // Start streaming generation.
        match model.stream(interaction, Some(params)) {
            Ok(stream) => {
                self.state = AgentLoopState::InnerGenerate {
                    stream: Box::new(stream),
                    collected: Vec::new(),
                };
                Some(TaskStatus::Pending(AgentProgress::Generating {
                    model: self.current_model.clone(),
                    tokens_so_far: None,
                }))
            }
            Err(gen_err) => {
                let err = AgenticError::from_generation(&gen_err, None, 0);
                Some(self.handle_error(err))
            }
        }
    }

    fn transition_inner_generate(&mut self) -> Option<TaskStatus<SessionRecord, AgentProgress, BoxedSendExecutionAction>> {
        // We need to take ownership of the state to pump the stream.
        let (mut stream, mut collected) = match std::mem::replace(
            &mut self.state,
            AgentLoopState::Done, // temporary placeholder
        ) {
            AgentLoopState::InnerGenerate { stream, collected } => (stream, collected),
            _ => unreachable!(),
        };

        // Pump one item from the stream.
        match stream.next() {
            Some(item) => {
                // Check for mid-generation steering interruption (OD-19-3).
                if self.queues.has_priority() {
                    self.queues.reset_cancel();
                    let msgs = self.queues.drain_priority();
                    for msg in msgs {
                        self.push_user_message(msg);
                    }
                    // Discard current generation, re-assemble with new priority.
                    self.state = AgentLoopState::InnerAssemble;
                    return Some(TaskStatus::Pending(AgentProgress::Steering {
                        source: std::borrow::Cow::Borrowed("mid_gen_priority"),
                    }));
                }

                let lifted = lift_model_item(item, &self.current_model);
                match lifted {
                    Stream::Next(record) => {
                        // Collect the message for tool-call extraction + loop detection.
                        if let SessionRecord::Conversation { ref message } = record {
                            collected.push(message.clone());
                            self.message_count += 1;
                        }
                        self.state = AgentLoopState::InnerGenerate { stream, collected };
                        Some(TaskStatus::Ready(record))
                    }
                    Stream::Pending(progress) => {
                        self.state = AgentLoopState::InnerGenerate { stream, collected };
                        Some(TaskStatus::Pending(progress))
                    }
                    Stream::Init | Stream::Ignore | Stream::Wait => {
                        self.state = AgentLoopState::InnerGenerate { stream, collected };
                        Some(TaskStatus::Ignore)
                    }
                    Stream::Delayed(d) => {
                        self.state = AgentLoopState::InnerGenerate { stream, collected };
                        Some(TaskStatus::Delayed(d))
                    }
                    Stream::Spread(items) => {
                        use foundation_core::valtron::StreamSpread;
                        for s in &items {
                            if let StreamSpread::Done(SessionRecord::Conversation { ref message }) = s {
                                collected.push(message.clone());
                                self.message_count += 1;
                            }
                        }
                        self.state = AgentLoopState::InnerGenerate { stream, collected };
                        // Emit the first spread item; rest will be picked up on next poll.
                        if let Some(first) = items.into_iter().next() {
                            match first {
                                StreamSpread::Done(rec) => Some(TaskStatus::Ready(rec)),
                                StreamSpread::Pending(p) => Some(TaskStatus::Pending(p)),
                            }
                        } else {
                            Some(TaskStatus::Ignore)
                        }
                    }
                }
            }
            None => {
                // Stream finished — run loop detection, extract tool calls.
                self.breaker.on_success();
                self.on_generation_complete(collected)
            }
        }
    }

    fn on_generation_complete(
        &mut self,
        collected: Vec<Messages>,
    ) -> Option<TaskStatus<SessionRecord, AgentProgress, BoxedSendExecutionAction>> {
        // Run loop detection (F17) synchronously on the model outputs.
        for msg in &collected {
            if let Messages::Assistant { ref content, .. } = msg {
                let detection = self.detector.check(content);
                if detection != LoopDetection::NoLoop {
                    let escalation = self.detector.escalate();
                    match escalation {
                        Escalation::Redirect => {
                            // Inject a system redirect message and re-assemble.
                            let redirect = Messages::User {
                                id: foundation_compact::ids::new_scru128(),
                                role: MessageRole::System,
                                content: UserModelContent::Text(TextContent {
                                    content: "Loop detected. Please try a different approach — \
                                              avoid repeating the same actions or responses."
                                        .into(),
                                    signature: None,
                                }),
                                signature: None,
                            };
                            self.push_user_message(redirect);
                            self.state = AgentLoopState::InnerAssemble;
                            return Some(TaskStatus::Pending(AgentProgress::Steering {
                                source: std::borrow::Cow::Borrowed("loop_redirect"),
                            }));
                        }
                        Escalation::SwitchModelOrTemperature { .. } => {
                            if let Some(fallback) = self.breaker.on_failure() {
                                self.current_model = fallback;
                            }
                            let redirect = Messages::User {
                                id: foundation_compact::ids::new_scru128(),
                                role: MessageRole::System,
                                content: UserModelContent::Text(TextContent {
                                    content: "Loop detected after redirect. Switching approach."
                                        .into(),
                                    signature: None,
                                }),
                                signature: None,
                            };
                            self.push_user_message(redirect);
                            self.state = AgentLoopState::InnerAssemble;
                            return Some(TaskStatus::Pending(AgentProgress::Steering {
                                source: std::borrow::Cow::Borrowed("loop_model_switch"),
                            }));
                        }
                        Escalation::Terminate => {
                            let err = AgenticError::LoopDetected(
                                crate::agentic::errors::LoopDetection {
                                    kind: format!("{detection:?}"),
                                    occurrences: self.detector.redirect_count() as u32,
                                },
                            );
                            self.state = AgentLoopState::Ending;
                            return Some(TaskStatus::Ready(err.into_failed_action()));
                        }
                    }
                }
            }
        }

        // Extract tool calls from assistant messages.
        let tool_calls = self.extract_tool_calls(&collected);

        if tool_calls.is_empty() {
            // No tools — go to output processing.
            self.state = AgentLoopState::OutputProcessing;
            Some(TaskStatus::Ignore)
        } else {
            let total = tool_calls.len();
            self.state = AgentLoopState::InnerToolCalls { calls: tool_calls };
            Some(TaskStatus::Pending(AgentProgress::ExecutingTools {
                total,
                completed: 0,
            }))
        }
    }

    fn extract_tool_calls(&self, messages: &[Messages]) -> Vec<ToolCallRequest> {
        let mut calls = Vec::new();
        for msg in messages {
            if let Messages::Assistant { content, .. } = msg {
                if let ModelOutput::ToolCall {
                    id,
                    name,
                    arguments,
                    depends_on,
                    execution_hint,
                    ..
                } = content
                {
                    calls.push(ToolCallRequest {
                        id: id.clone(),
                        name: name.clone(),
                        arguments: arguments.clone().unwrap_or_default(),
                        depends_on: depends_on.clone(),
                        execution_hint: *execution_hint,
                    });
                }
            }
        }
        calls
    }

    fn transition_inner_tool_calls(&mut self) -> Option<TaskStatus<SessionRecord, AgentProgress, BoxedSendExecutionAction>> {
        let calls = match std::mem::replace(&mut self.state, AgentLoopState::Done) {
            AgentLoopState::InnerToolCalls { calls } => calls,
            _ => unreachable!(),
        };

        // Build the workflow (topological sort by depends_on).
        let workflow = match self.tool_manager.build_workflow(calls.clone()) {
            Ok(w) => w,
            Err(e) => {
                let err = AgenticError::ToolCall {
                    tool_name: "workflow".into(),
                    reason: e.to_string(),
                };
                self.state = AgentLoopState::OutputProcessing;
                return Some(TaskStatus::Ready(err.into_failed_action()));
            }
        };

        // Flatten the workflow stages into a flat execution order.
        let mut flat_calls = Vec::new();
        for stage in workflow.stages {
            match stage {
                crate::agentic::tool_impl::ToolCallStage::Parallel { calls: sc, .. }
                | crate::agentic::tool_impl::ToolCallStage::Sequential { calls: sc, .. } => {
                    flat_calls.extend(sc);
                }
            }
        }

        if flat_calls.is_empty() {
            self.state = AgentLoopState::OutputProcessing;
            return Some(TaskStatus::Ignore);
        }

        self.state = AgentLoopState::InnerExecuting {
            calls: flat_calls,
            results: Vec::new(),
            idx: 0,
            active: None,
            cancel_signals: Vec::new(),
        };
        Some(TaskStatus::Ignore)
    }

    fn transition_inner_executing(&mut self) -> Option<TaskStatus<SessionRecord, AgentProgress, BoxedSendExecutionAction>> {
        let (calls, mut results, idx, mut active, mut cancel_signals) =
            match std::mem::replace(&mut self.state, AgentLoopState::Done) {
                AgentLoopState::InnerExecuting {
                    calls,
                    results,
                    idx,
                    active,
                    cancel_signals,
                } => (calls, results, idx, active, cancel_signals),
                _ => unreachable!(),
            };

        // Check for steering cancel — abort all in-flight tool futures.
        if self.queues.has_priority() {
            for sig in &cancel_signals {
                sig.store(true, Ordering::Release);
            }
            let msgs = self.queues.drain_priority();
            self.queues.reset_cancel();
            for msg in msgs {
                self.push_user_message(msg);
            }
            self.state = AgentLoopState::InnerAssemble;
            return Some(TaskStatus::Pending(AgentProgress::Steering {
                source: std::borrow::Cow::Borrowed("cancel_executing"),
            }));
        }

        if idx >= calls.len() {
            // All tools executed — build result messages and emit them.
            let result_messages: Vec<Messages> = results
                .into_iter()
                .map(|(req, result)| {
                    let (content, error_detail) = match result {
                        Ok(r) => (r.content, r.error_detail),
                        Err(e) => (
                            UserModelContent::Text(TextContent {
                                content: e.to_string(),
                                signature: None,
                            }),
                            Some(e.to_string()),
                        ),
                    };
                    Messages::ToolResult {
                        id: foundation_compact::ids::new_scru128(),
                        tool_call_id: req.id,
                        name: req.name,
                        timestamp: foundation_compact::SystemTime::now(),
                        details: None,
                        content,
                        error_detail,
                        signature: None,
                    }
                })
                .collect();

            if result_messages.is_empty() {
                self.state = AgentLoopState::OutputProcessing;
                return Some(TaskStatus::Ignore);
            }

            self.state = AgentLoopState::InnerEmitResults {
                results: result_messages,
                idx: 0,
            };
            return Some(TaskStatus::Ignore);
        }

        // If no active driven iterator, start one via drive_future with CancellableFutureTask.
        if active.is_none() {
            let call = calls[idx].clone();
            let mgr = self.tool_manager.clone();
            let retry_config = mgr.retry_config(&call.name);
            let fut: core::pin::Pin<Box<dyn core::future::Future<Output = Result<ToolCallResult, ToolError>> + Send>> =
                Box::pin(async move {
                    mgr.execute_with_retry(&call, &retry_config).await
                });
            let signal = Arc::new(AtomicBool::new(false));
            cancel_signals.push(Arc::clone(&signal));
            let task = CancellableFutureTask::new(fut, signal);
            active = Some(foundation_core::valtron::drive_iterator(task));
        }

        // Poll the driven iterator.
        let total = calls.len();
        let driven = active.as_mut().expect("just created");
        match driven.next() {
            Some(TaskStatus::Ready(Ok(result))) => {
                results.push((calls[idx].clone(), result));
                self.state = AgentLoopState::InnerExecuting {
                    calls,
                    results,
                    idx: idx + 1,
                    active: None,
                    cancel_signals,
                };
                Some(TaskStatus::Pending(AgentProgress::ExecutingTools {
                    total,
                    completed: idx + 1,
                }))
            }
            Some(TaskStatus::Ready(Err(CancelOutcome::Cancelled))) => {
                let err = ToolError::Cancelled(calls[idx].name.clone());
                results.push((calls[idx].clone(), Err(err)));
                self.state = AgentLoopState::InnerExecuting {
                    calls,
                    results,
                    idx: idx + 1,
                    active: None,
                    cancel_signals,
                };
                Some(TaskStatus::Pending(AgentProgress::ExecutingTools {
                    total,
                    completed: idx + 1,
                }))
            }
            Some(TaskStatus::Pending(_)) => {
                self.state = AgentLoopState::InnerExecuting {
                    calls,
                    results,
                    idx,
                    active,
                    cancel_signals,
                };
                Some(TaskStatus::Pending(AgentProgress::ExecutingTools {
                    total,
                    completed: idx,
                }))
            }
            None => {
                // Driven iterator exhausted without Ready — treat as error.
                let err = ToolError::Execution {
                    tool: calls[idx].name.clone(),
                    reason: "future completed without result".into(),
                };
                results.push((calls[idx].clone(), Err(err)));
                self.state = AgentLoopState::InnerExecuting {
                    calls,
                    results,
                    idx: idx + 1,
                    active: None,
                    cancel_signals,
                };
                Some(TaskStatus::Ignore)
            }
            _ => {
                self.state = AgentLoopState::InnerExecuting {
                    calls,
                    results,
                    idx,
                    active,
                    cancel_signals,
                };
                Some(TaskStatus::Ignore)
            }
        }
    }

    fn transition_inner_emit_results(&mut self) -> Option<TaskStatus<SessionRecord, AgentProgress, BoxedSendExecutionAction>> {
        let (results, idx) = match std::mem::replace(&mut self.state, AgentLoopState::Done) {
            AgentLoopState::InnerEmitResults { results, idx } => (results, idx),
            _ => unreachable!(),
        };

        if idx >= results.len() {
            // All results emitted — loop back to InnerAssemble for the LLM
            // to process tool results (guarded by max_inner_iterations).
            self.inner_iteration += 1;
            if self.inner_iteration >= self.config.max_inner_iterations {
                self.state = AgentLoopState::OutputProcessing;
                return Some(TaskStatus::Pending(AgentProgress::SessionEnding));
            }
            self.state = AgentLoopState::InnerAssemble;
            return Some(TaskStatus::Ignore);
        }

        let msg = results[idx].clone();
        self.message_api.append(SessionRecord::Conversation {
            message: msg.clone(),
        });
        self.message_count += 1;

        self.state = AgentLoopState::InnerEmitResults {
            results,
            idx: idx + 1,
        };
        Some(TaskStatus::Ready(SessionRecord::Conversation {
            message: msg,
        }))
    }

    fn transition_output_processing(&mut self) -> Option<TaskStatus<SessionRecord, AgentProgress, BoxedSendExecutionAction>> {
        // Fire memory triggers (F15) — check if observation/reflection needed.
        let action = self.memory.check_triggers();
        match action {
            MemoryAction::GenerateObservation => {
                self.state = AgentLoopState::OuterBoundary;
                Some(TaskStatus::Pending(AgentProgress::ProcessingMemory {
                    kind: MemoryKind::Observation,
                }))
            }
            MemoryAction::GenerateReflection => {
                self.state = AgentLoopState::OuterBoundary;
                Some(TaskStatus::Pending(AgentProgress::ProcessingMemory {
                    kind: MemoryKind::Reflection,
                }))
            }
            MemoryAction::None => {
                self.state = AgentLoopState::OuterBoundary;
                Some(TaskStatus::Ignore)
            }
        }
    }

    fn transition_ending(&mut self) -> Option<TaskStatus<SessionRecord, AgentProgress, BoxedSendExecutionAction>> {
        let snapshot = self.ledger.snapshot();
        let summary = SessionRecord::Summary {
            message_count: self.message_count,
            usage: snapshot,
        };
        self.state = AgentLoopState::Done;
        Some(TaskStatus::Ready(summary))
    }

    // -----------------------------------------------------------------------
    // Error handling (F02)

    fn handle_error(&mut self, error: AgenticError) -> TaskStatus<SessionRecord, AgentProgress, BoxedSendExecutionAction> {
        let action = self.policy.classify(error.clone());
        match action {
            AgentAction::Continue => TaskStatus::Ignore,
            AgentAction::RetryWithReducedContext => {
                // Stay in InnerAssemble — the context provider will re-pack
                // with reduced budget on the next assemble call.
                self.state = AgentLoopState::InnerAssemble;
                TaskStatus::Pending(AgentProgress::Steering {
                    source: std::borrow::Cow::Borrowed("retry_reduced_context"),
                })
            }
            AgentAction::SwitchModel => {
                if let Some(fallback) = self.breaker.on_failure() {
                    self.current_model = fallback;
                    self.state = AgentLoopState::InnerAssemble;
                    TaskStatus::Pending(AgentProgress::Steering {
                        source: std::borrow::Cow::Borrowed("model_switch"),
                    })
                } else {
                    // All fallbacks exhausted — terminate.
                    self.state = AgentLoopState::Ending;
                    TaskStatus::Ready(error.into_failed_action())
                }
            }
            AgentAction::Terminate(err) => {
                self.state = AgentLoopState::Ending;
                TaskStatus::Ready(err.into_failed_action())
            }
        }
    }

    // -----------------------------------------------------------------------
    // Context pressure (OD-19-7)

    fn apply_context_pressure(&self, ctx: &AgentContext) -> Option<String> {
        if self.config.context_pressure_threshold <= 0.0 {
            return ctx.system_prompt.clone();
        }

        let budget = self.ledger.budget().unwrap_or(u64::MAX);
        let usage_ratio = ctx.token_estimate as f64 / budget as f64;

        if usage_ratio >= f64::from(self.config.context_pressure_threshold) {
            let pct = (usage_ratio * 100.0) as u32;
            let pressure_note = format!(
                "Context is at {pct}% capacity — prefer concise responses \
                 and avoid requesting large tool outputs."
            );
            let base = ctx.system_prompt.as_deref().unwrap_or("");
            Some(format!("{base}\n\n[{pressure_note}]"))
        } else {
            ctx.system_prompt.clone()
        }
    }
}

// ---------------------------------------------------------------------------
// TaskIterator impl

impl<D, M> TaskIterator for AgentLoop<D, M>
where
    D: DocumentStore + 'static,
    M: MemoryStore + 'static,
{
    type Ready = SessionRecord;
    type Pending = AgentProgress;
    type Spawner = BoxedSendExecutionAction;

    fn next_status(&mut self) -> Option<TaskStatus<Self::Ready, Self::Pending, Self::Spawner>> {
        match &self.state {
            AgentLoopState::Initializing => {
                self.state = AgentLoopState::OuterBoundary;
                Some(TaskStatus::Init)
            }
            AgentLoopState::OuterBoundary => self.transition_outer_boundary(),
            AgentLoopState::InnerAssemble => self.transition_inner_assemble(),
            AgentLoopState::InnerGenerate { .. } => self.transition_inner_generate(),
            AgentLoopState::InnerToolCalls { .. } => self.transition_inner_tool_calls(),
            AgentLoopState::InnerExecuting { .. } => self.transition_inner_executing(),
            AgentLoopState::InnerEmitResults { .. } => self.transition_inner_emit_results(),
            AgentLoopState::OutputProcessing => self.transition_output_processing(),
            AgentLoopState::Ending => self.transition_ending(),
            AgentLoopState::Done => None,
        }
    }
}

// ---------------------------------------------------------------------------
// Tests

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agentic::context::ContextConfig;
    use crate::agentic::memory_coordinator::MemoryCoordinator;
    use crate::agentic::memory_store::KvMemoryStore;
    use crate::types::{
        ModelId, ModelInteraction, ModelOutput, ModelParams, ModelSpec, ModelState, StopReason,
        TextContent, UsageCosting, UsageReport, CostStatus, ModelProviders,
    };
    use foundation_db::{MemoryDocumentStore, MemoryStorage};
    use std::sync::Arc;

    // -----------------------------------------------------------------------
    // Test helpers

    type TestMemStore = KvMemoryStore<MemoryStorage>;
    type TestDocStore = MemoryDocumentStore;

    fn usage_zero() -> UsageReport {
        UsageReport {
            input: 0.0,
            output: 0.0,
            cache_read: 0.0,
            cache_write: 0.0,
            total_tokens: 0.0,
            cost: UsageCosting::zero(CostStatus::Estimated),
        }
    }

    fn setup() -> AgentLoop<TestDocStore, TestMemStore> {
        let kv = KvMemoryStore::new(MemoryStorage::new());
        let doc = MemoryDocumentStore::new();
        let session_id = SessionId::new();
        let coordinator = MemoryCoordinator::new(kv.clone(), doc.clone());
        let ledger = TokenLedger::new();
        let message_api = MessageApi::new(session_id.clone(), doc.clone());
        let memory_store = Arc::new(kv.clone());
        let context_provider = ContextProvider::new(
            session_id.clone(),
            message_api.clone(),
            memory_store,
            ledger.clone(),
            Some("You are a helpful assistant.".into()),
            ContextConfig::default(),
        );
        let tool_manager = ToolCallManager::new(session_id.clone());
        let queues = SteeringQueues::new();
        let memory = MemoryHierarchy::new(
            session_id.clone(),
            coordinator,
            ledger.clone(),
            MemoryConfig::default(),
        );
        let router = crate::types::ProviderRouter::new(vec![]);
        let config = AgentConfig {
            primary_model: ModelId::Name("test-model".into(), None),
            ..Default::default()
        };

        AgentLoop::new(
            session_id,
            context_provider,
            tool_manager,
            queues,
            memory,
            message_api,
            ledger,
            router,
            config,
        )
    }

    // -----------------------------------------------------------------------
    // Tests

    #[test]
    fn initializing_emits_init_then_transitions() {
        let mut agent = setup();
        assert_eq!(agent.state_label(), "initializing");

        let status = agent.next_status();
        assert!(matches!(status, Some(TaskStatus::Init)));
        assert_eq!(agent.state_label(), "outer_boundary");
    }

    #[test]
    fn outer_boundary_with_no_messages_ends() {
        let mut agent = setup();
        // Skip init.
        agent.state = AgentLoopState::OuterBoundary;

        let status = agent.next_status();
        // Should transition toward ending (no follow-up, no priority).
        match status {
            Some(TaskStatus::Pending(AgentProgress::SessionEnding)) => {
                assert_eq!(agent.state_label(), "ending");
            }
            other => panic!("expected SessionEnding, got {other:?}"),
        }
    }

    #[test]
    fn outer_boundary_with_follow_up_transitions_to_inner() {
        let mut agent = setup();
        agent.state = AgentLoopState::OuterBoundary;

        // Push a follow-up message.
        let msg = Messages::User {
            id: foundation_compact::ids::new_scru128(),
            role: MessageRole::User,
            content: UserModelContent::Text(TextContent {
                content: "hello".into(),
                signature: None,
            }),
            signature: None,
        };
        agent.queues.push_follow_up(msg);

        let status = agent.next_status();
        match status {
            Some(TaskStatus::Pending(AgentProgress::Steering { source })) => {
                assert!(source.contains("follow_up"));
                assert_eq!(agent.state_label(), "inner_assemble");
            }
            other => panic!("expected Steering follow_up, got {other:?}"),
        }
    }

    #[test]
    fn priority_queue_interrupts_outer_boundary() {
        let mut agent = setup();
        agent.state = AgentLoopState::OuterBoundary;

        let msg = Messages::User {
            id: foundation_compact::ids::new_scru128(),
            role: MessageRole::System,
            content: UserModelContent::Text(TextContent {
                content: "urgent redirect".into(),
                signature: None,
            }),
            signature: None,
        };
        agent.queues.push_priority(msg);

        let status = agent.next_status();
        match status {
            Some(TaskStatus::Pending(AgentProgress::Steering { source })) => {
                assert!(source.contains("priority"));
                assert_eq!(agent.state_label(), "inner_assemble");
            }
            other => panic!("expected Steering priority, got {other:?}"),
        }
    }

    #[test]
    fn ending_emits_summary() {
        let mut agent = setup();
        agent.state = AgentLoopState::Ending;
        agent.message_count = 42;

        let status = agent.next_status();
        match status {
            Some(TaskStatus::Ready(SessionRecord::Summary {
                message_count,
                usage,
            })) => {
                assert_eq!(message_count, 42);
                assert_eq!(usage.total, 0);
            }
            other => panic!("expected Summary, got {other:?}"),
        }
        assert_eq!(agent.state_label(), "done");
    }

    #[test]
    fn done_returns_none() {
        let mut agent = setup();
        agent.state = AgentLoopState::Done;
        assert!(agent.next_status().is_none());
    }

    #[test]
    fn budget_exhausted_terminates() {
        let mut agent = setup();
        agent.state = AgentLoopState::InnerAssemble;
        // Set a budget and exhaust it.
        agent.ledger.set_budget(Some(100));
        let usage = UsageReport {
            input: 100.0,
            output: 50.0,
            cache_read: 0.0,
            cache_write: 0.0,
            total_tokens: 150.0,
            cost: UsageCosting::zero(CostStatus::Estimated),
        };
        agent.ledger.record(&usage);

        let status = agent.next_status();
        match status {
            Some(TaskStatus::Ready(SessionRecord::FailedAction { error, .. })) => {
                assert!(matches!(error, AgenticError::BudgetExhausted { .. }));
            }
            other => panic!("expected BudgetExhausted, got {other:?}"),
        }
    }

    #[test]
    fn error_policy_classify_integration() {
        let policy = ErrorPolicy::new();

        // Context overflow → retry with reduced.
        assert_eq!(
            policy.classify(AgenticError::Generation(GenerationFailure {
                kind: GenKind::ContextOverflow,
                message: String::new(),
            })),
            AgentAction::RetryWithReducedContext
        );

        // Rate limit → switch model.
        assert_eq!(
            policy.classify(AgenticError::Generation(GenerationFailure {
                kind: GenKind::RateLimit,
                message: String::new(),
            })),
            AgentAction::SwitchModel
        );
    }

    #[test]
    fn context_pressure_injects_note() {
        let mut agent = setup();
        agent.config.context_pressure_threshold = 0.5;
        agent.ledger.set_budget(Some(1000));

        let ctx = AgentContext {
            system_prompt: Some("Base prompt".into()),
            messages: vec![],
            token_estimate: 800, // 80% > 50% threshold
        };

        let result = agent.apply_context_pressure(&ctx);
        assert!(result.is_some());
        let prompt = result.unwrap();
        assert!(prompt.contains("capacity"));
        assert!(prompt.contains("Base prompt"));
    }

    #[test]
    fn context_pressure_disabled_when_zero() {
        let mut agent = setup();
        agent.config.context_pressure_threshold = 0.0;

        let ctx = AgentContext {
            system_prompt: Some("Base prompt".into()),
            messages: vec![],
            token_estimate: 999,
        };

        let result = agent.apply_context_pressure(&ctx);
        assert_eq!(result, Some("Base prompt".into()));
    }

    #[test]
    fn max_outer_iterations_guard() {
        let mut agent = setup();
        agent.config.max_outer_iterations = 1;
        agent.outer_iteration = 1; // Already at max.
        agent.state = AgentLoopState::OuterBoundary;

        let status = agent.next_status();
        match status {
            Some(TaskStatus::Pending(AgentProgress::SessionEnding)) => {
                assert_eq!(agent.state_label(), "ending");
            }
            other => panic!("expected SessionEnding, got {other:?}"),
        }
    }

    #[test]
    fn handle_error_terminate() {
        let mut agent = setup();
        agent.state = AgentLoopState::InnerAssemble;

        let err = AgenticError::Budget { limit: 100 };
        let status = agent.handle_error(err);
        match status {
            TaskStatus::Ready(SessionRecord::FailedAction { error, .. }) => {
                assert!(matches!(error, AgenticError::Budget { limit: 100 }));
            }
            other => panic!("expected FailedAction, got {other:?}"),
        }
    }

    #[test]
    fn handle_error_switch_model() {
        let mut agent = setup();
        agent.config.fallback_models = vec![ModelId::Name("fallback-1".into(), None)];
        agent.breaker = CircuitBreaker::new(1, agent.config.fallback_models.clone());

        let err = AgenticError::Generation(GenerationFailure {
            kind: GenKind::RateLimit,
            message: "rate limited".into(),
        });
        let status = agent.handle_error(err);
        match status {
            TaskStatus::Pending(AgentProgress::Steering { source }) => {
                assert!(source.contains("model_switch"));
                assert_eq!(
                    agent.current_model,
                    ModelId::Name("fallback-1".into(), None)
                );
            }
            other => panic!("expected model switch, got {other:?}"),
        }
    }

    #[test]
    fn extract_tool_calls_from_messages() {
        let agent = setup();
        let msgs = vec![
            Messages::Assistant {
                id: foundation_compact::ids::new_scru128(),
                model: ModelId::Name("m".into(), None),
                timestamp: foundation_compact::SystemTime::now(),
                usage: usage_zero(),
                content: ModelOutput::ToolCall {
                    id: "tc-1".into(),
                    name: "read_file".into(),
                    arguments: Some(HashMap::from([(
                        "path".into(),
                        crate::types::ArgType::Text("/foo".into()),
                    )])),
                    signature: None,
                    depends_on: vec![],
                    execution_hint: crate::types::ExecutionHint::default(),
                },
                stop_reason: StopReason::ToolUse,
                provider: ModelProviders::default(),
                error_detail: None,
                signature: None,
                metadata: None,
            },
            Messages::Assistant {
                id: foundation_compact::ids::new_scru128(),
                model: ModelId::Name("m".into(), None),
                timestamp: foundation_compact::SystemTime::now(),
                usage: usage_zero(),
                content: ModelOutput::Text(TextContent {
                    content: "some text".into(),
                    signature: None,
                }),
                stop_reason: StopReason::EndTurn,
                provider: ModelProviders::default(),
                error_detail: None,
                signature: None,
                metadata: None,
            },
        ];

        let calls = agent.extract_tool_calls(&msgs);
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].name, "read_file");
        assert_eq!(calls[0].id, "tc-1");
    }

    #[test]
    fn full_lifecycle_init_to_ending() {
        let mut agent = setup();

        // Init.
        let s1 = agent.next_status();
        assert!(matches!(s1, Some(TaskStatus::Init)));

        // OuterBoundary — no messages, goes to ending.
        let s2 = agent.next_status();
        assert!(matches!(
            s2,
            Some(TaskStatus::Pending(AgentProgress::SessionEnding))
        ));

        // Ending — emits Summary.
        let s3 = agent.next_status();
        assert!(matches!(
            s3,
            Some(TaskStatus::Ready(SessionRecord::Summary { .. }))
        ));

        // Done.
        let s4 = agent.next_status();
        assert!(s4.is_none());
    }
}
