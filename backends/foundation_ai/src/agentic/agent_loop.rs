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

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use foundation_core::valtron::{
    BoxedSendExecutionAction, CancelOutcome, CancellableFutureTask, DrivenTaskIterator, Stream,
    TaskIterator, TaskStatus,
};
use foundation_db::traits::DocumentStore;

use crate::agentic::context::{AgentContext, ContextProvider};
use crate::agentic::errors::{AgentAction, AgenticError, CircuitBreaker, ErrorPolicy};
use crate::agentic::loop_detection::{
    is_vacuous_answer, Escalation, LoopDetection, LoopDetector, LoopDetectorConfig,
};
use crate::agentic::memory::{MemoryAction, MemoryHierarchy};
use crate::agentic::memory_store::MemoryStore;
use crate::agentic::message_api::MessageApi;
use crate::agentic::progress::{lift_model_item, AgentProgress, MemoryKind};
use crate::agentic::steering::SteeringQueues;
use crate::agentic::token_ledger::TokenLedger;
use crate::agentic::tool_impl::{ToolCallManager, ToolCallRequest, ToolCallResult, ToolError};
use crate::types::{
    MessageRole, Messages, ModelId, ModelInteraction, ModelOutput, ModelParams, ModelState,
    SessionId, SessionRecord, TextContent, UserModelContent,
};

// ---------------------------------------------------------------------------
// AgentConfig

/// Tunable knobs for `AgentLoop` behaviour.
///
/// WHY: Hard-coding iteration caps, budget thresholds, and model fallback
/// lists inside the loop would force recompilation on every tuning change.
/// Externalising them lets callers (CLI, server, tests) vary policy without
/// touching orchestration logic.
///
/// WHAT: Flat bag of limits and thresholds — primary/fallback models,
/// inner/outer iteration caps, circuit-breaker threshold, context-pressure
/// ratio, and base `ModelParams`. All fields have sensible defaults via
/// `Default`.
///
/// HOW: Passed to `AgentLoop::new`; the loop reads fields at each state
/// transition. `fallback_models` feeds the `CircuitBreaker`; thresholds
/// gate the context-pressure and preflight-compression layers.
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
// Type aliases for complex types

/// Type alias for the driven tool-execution future used in `InnerExecuting`.
type ToolDrivenIterator = DrivenTaskIterator<
    CancellableFutureTask<
        core::pin::Pin<
            Box<dyn core::future::Future<Output = Result<ToolCallResult, ToolError>> + Send>,
        >,
    >,
>;

// ---------------------------------------------------------------------------
// AgentLoopState

/// The state machine driving the orchestrator.
///
/// WHY: The agent loop is a non-blocking `TaskIterator` — it cannot park on
/// async I/O. A flat enum encodes every suspension point so `next_status`
/// can resume exactly where it left off without a call stack.
///
/// WHAT: Ten states forming two nested loops. The *outer* loop
/// (`OuterBoundary`) drains follow-up messages; the *inner* loop
/// (`InnerAssemble → InnerGenerate → InnerToolCalls → InnerExecuting →
/// InnerEmitResults`) runs one model turn + tool execution cycle.
/// `OutputProcessing` fires memory triggers; `Ending` emits a summary;
/// `Done` is terminal.
///
/// HOW: Each variant carries the state needed to resume — stream handles,
/// partial results, indices. `next_status` pattern-matches the current
/// variant, does one quantum of work, and replaces `self` with the next
/// variant.
pub enum AgentLoopState {
    /// Initial setup — emit Init, transition to `OuterBoundary`.
    Initializing,
    /// Check `FollowUpQueue` for continuation; if empty, go to Ending.
    OuterBoundary,
    /// Run input processors, assemble context, generate.
    InnerAssemble,
    /// Model generation in progress — pump the stream step-wise.
    InnerGenerate {
        stream: Box<dyn Iterator<Item = Stream<Messages, ModelState>> + Send>,
        collected: Vec<Messages>,
    },
    /// Tool calls extracted from the model's output.
    InnerToolCalls { calls: Vec<ToolCallRequest> },
    /// Tool execution in progress — drives `CancellableFutureTask` per call.
    InnerExecuting {
        calls: Vec<ToolCallRequest>,
        results: Vec<(ToolCallRequest, Result<ToolCallResult, ToolError>)>,
        idx: usize,
        active: Option<ToolDrivenIterator>,
        cancel_signals: Vec<Arc<AtomicBool>>,
    },
    /// Emit tool results back as records and loop back to `InnerAssemble`.
    InnerEmitResults { results: Vec<Messages>, idx: usize },
    /// Output processing — fire memory triggers, persist.
    OutputProcessing,
    /// Emit the final Summary record and end.
    Ending,
    /// Terminal state.
    Done,
}

impl AgentLoopState {
    /// The variant's name, for diagnostics.
    ///
    /// WHY: the state carries live stream handles, so it cannot derive `Debug`.
    /// Each `transition_*` method takes the state by value and requires one
    /// specific variant; when that expectation is violated the panic needs to
    /// say which state was actually found, or the report is just "entered
    /// unreachable code" and the caller has to reconstruct the sequence by hand.
    pub(crate) const fn variant_name(&self) -> &'static str {
        match self {
            Self::Initializing => "Initializing",
            Self::OuterBoundary => "OuterBoundary",
            Self::InnerAssemble => "InnerAssemble",
            Self::InnerGenerate { .. } => "InnerGenerate",
            Self::InnerToolCalls { .. } => "InnerToolCalls",
            Self::InnerExecuting { .. } => "InnerExecuting",
            Self::InnerEmitResults { .. } => "InnerEmitResults",
            Self::OutputProcessing => "OutputProcessing",
            Self::Ending => "Ending",
            Self::Done => "Done",
        }
    }
}

// ---------------------------------------------------------------------------
// AgentLoop

/// The agentic orchestrator — a valtron `TaskIterator` state machine.
///
/// WHY: Every other agentic component is a leaf: context assembly, memory
/// generation, tool execution, steering. Nothing sequences them into a
/// *turn*. `AgentLoop` is the single piece that drives the full cycle
/// (context → generate → tools → emit → follow-up) as a non-blocking
/// iterator the valtron executor can schedule alongside other work.
///
/// WHAT: Implements `TaskIterator<Ready = SessionRecord, Pending =
/// AgentProgress, Spawner = BoxedSendExecutionAction>`. The outer loop
/// drains `SteeringQueues` for follow-up messages; the inner loop runs
/// model generation, tool execution, and output processing until the model
/// stops requesting tool calls or a guard (budget, iteration cap, loop
/// detection) fires.
///
/// HOW: Owns the component instances (`ContextProvider`, `ToolCallManager`,
/// `MemoryHierarchy`, `SteeringQueues`, `TokenLedger`, `ErrorPolicy`,
/// `CircuitBreaker`, `LoopDetector`, `ProviderRouter`) and an
/// `AgentLoopState` enum. Each `next_status` call pattern-matches the
/// current state, delegates to the appropriate component, and transitions
/// to the next state. Tool futures are wrapped in `CancellableFutureTask`
/// and driven via `drive_iterator`.
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
    /// Text of the user message this turn is answering.
    ///
    /// Kept so a bare-number reply can be judged against what was asked — `0`
    /// answering "how many" is correct, `0` answering "hello" is not.
    last_user_prompt: String,
    message_count: u64,
}

/// The text of a whole turn, as the caller will eventually read it.
///
/// Streaming models emit one `Messages::Assistant` per token, so the turn's
/// answer only exists once the fragments are put back together. Thinking
/// content is excluded: it is the model reasoning to itself, not its answer,
/// and a turn whose only visible output is punctuation is vacuous however much
/// it thought first.
fn assembled_answer(collected: &[Messages]) -> String {
    let mut answer = String::new();
    for message in collected {
        if let Messages::Assistant {
            content: ModelOutput::Text(text),
            ..
        } = message
        {
            answer.push_str(&text.content);
        }
    }
    answer
}

/// The nudge sent when a turn produced no usable answer.
///
/// Names the actual problem rather than reusing the loop wording — a model told
/// "you are repeating yourself" when it in fact said nothing has been given the
/// wrong correction.
fn vacuous_answer_redirect() -> Messages {
    Messages::User {
        id: foundation_compact::ids::new_scru128(),
        role: MessageRole::System,
        content: UserModelContent::Text(TextContent {
            content: "Your last reply contained no answer - only punctuation or \
                      whitespace. Answer the user's question directly, in plain \
                      words."
                .into(),
            signature: None,
        }),
        signature: None,
    }
}

impl<D: DocumentStore, M: MemoryStore> AgentLoop<D, M> {
    #[allow(clippy::too_many_arguments)]
    #[must_use]
    pub fn new(
        session_id: SessionId,
        context_provider: ContextProvider<D, M>,
        tool_manager: ToolCallManager,
        queues: SteeringQueues,
        memory: MemoryHierarchy<M, D>,
        message_api: MessageApi<D>,
        ledger: TokenLedger,
        policy: ErrorPolicy,
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
            policy,
            breaker,
            router,
            state: AgentLoopState::Initializing,
            current_model,
            config,
            inner_iteration: 0,
            outer_iteration: 0,
            last_user_prompt: String::new(),
            message_count: 0,
        }
    }

    /// Persist a user message to session history.
    ///
    /// The message reaches the model through context assembly
    /// (`message_api` -> `ContextProvider` -> `interaction.messages`), so
    /// persisting here is the whole job — an earlier `pending_user_messages`
    /// buffer duplicated it, was never drained (unbounded growth), and was read
    /// only by a debug counter, so it was removed.
    pub fn push_user_message(&mut self, msg: Messages) {
        let _ = self
            .message_api
            .append(SessionRecord::Conversation { message: msg });
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

    fn transition_outer_boundary(
        &mut self,
    ) -> TaskStatus<SessionRecord, AgentProgress, BoxedSendExecutionAction> {
        // A hard abort requested via `AgentSession::abort` (or `steer`'s Abort
        // code) terminates the turn at this boundary. Previously the Abort code
        // existed but nothing set it and the loop never read it — a stubbed
        // cancel path (spec-60 matrix 3.5).
        if self.queues.is_aborted() {
            self.queues.reset_cancel();
            self.state = AgentLoopState::Ending;
            return TaskStatus::Pending(AgentProgress::SessionEnding);
        }

        self.outer_iteration += 1;
        if self.outer_iteration > self.config.max_outer_iterations {
            self.state = AgentLoopState::Ending;
            return TaskStatus::Pending(AgentProgress::SessionEnding);
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
            return TaskStatus::Pending(AgentProgress::Steering {
                source: std::borrow::Cow::Borrowed("priority_queue"),
            });
        }

        // Drain follow-up queue.
        let follow_up_msgs = self.queues.drain_follow_up();
        tracing::trace!(
            drained = follow_up_msgs.len(),
            outer_iteration = self.outer_iteration,
            "outer_boundary: drained follow_up queue"
        );
        if !follow_up_msgs.is_empty() {
            for msg in follow_up_msgs {
                if let Messages::User {
                    role: MessageRole::User,
                    content: UserModelContent::Text(ref text),
                    ..
                } = msg
                {
                    self.last_user_prompt.clone_from(&text.content);
                }
                self.push_user_message(msg);
            }
            self.inner_iteration = 0;
            self.state = AgentLoopState::InnerAssemble;
            return TaskStatus::Pending(AgentProgress::Steering {
                source: std::borrow::Cow::Borrowed("follow_up_queue"),
            });
        }

        // No more work — end the session.
        self.state = AgentLoopState::Ending;
        TaskStatus::Pending(AgentProgress::SessionEnding)
    }

    fn transition_inner_assemble(
        &mut self,
    ) -> TaskStatus<SessionRecord, AgentProgress, BoxedSendExecutionAction> {
        // Honor a hard abort mid-turn (before starting another generation).
        if self.queues.is_aborted() {
            self.queues.reset_cancel();
            self.state = AgentLoopState::Ending;
            return TaskStatus::Pending(AgentProgress::SessionEnding);
        }

        // Check priority queue — front-inject interruption.
        if self.queues.has_priority() {
            let msgs = self.queues.drain_priority();
            self.queues.reset_cancel();
            for msg in msgs {
                self.push_user_message(msg);
            }
            return TaskStatus::Pending(AgentProgress::Steering {
                source: std::borrow::Cow::Borrowed("priority_interrupt"),
            });
        }

        // Check budget exhaustion.
        if self.ledger.is_exhausted() {
            let snapshot = self.ledger.snapshot();
            self.state = AgentLoopState::Ending;
            return TaskStatus::Ready(
                AgenticError::BudgetExhausted { snapshot }.into_failed_action(),
            );
        }

        // Attempt to get the model from the router.
        let model = match self.router.get_model(&self.current_model) {
            Ok(m) => m,
            Err(e) => {
                tracing::error!("router failed to get model: {e:?}");
                let err = AgenticError::from(e);
                return self.handle_error(err);
            }
        };

        // Hydrate memory synchronously and assemble context.
        let memory = self
            .context_provider
            .memory_store()
            .hydrate_sync(&self.session_id)
            .unwrap_or_default();
        let mut ctx = self.context_provider.assemble_from_memory(&memory);

        // Preflight compression: when the assembled context exceeds the
        // compression threshold of the budget, shrink it BEFORE sending — drop
        // the oldest recent messages (keeping the newest) until it fits. This is
        // the heavier counterpart to the context-pressure note (below): pressure
        // warns, compression actually reduces. The threshold field existed but
        // was never applied.
        self.apply_preflight_compression(&mut ctx);

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
        tracing::trace!(
            messages = interaction.messages.len(),
            has_system = interaction.system_prompt.is_some(),
            "Sending interactions to model for generation"
        );
        match model.stream(interaction, Some(params)) {
            Ok(stream) => {
                self.state = AgentLoopState::InnerGenerate {
                    stream: Box::new(stream),
                    collected: Vec::new(),
                };
                TaskStatus::Pending(AgentProgress::Generating {
                    model: self.current_model.clone(),
                    tokens_so_far: None,
                })
            }
            Err(gen_err) => {
                let err = AgenticError::from_generation(&gen_err, None, 0);
                self.handle_error(err)
            }
        }
    }

    fn transition_inner_generate(
        &mut self,
    ) -> TaskStatus<SessionRecord, AgentProgress, BoxedSendExecutionAction> {
        // We need to take ownership of the state to pump the stream.
        let previous = std::mem::replace(&mut self.state, AgentLoopState::Done);
        let actual = previous.variant_name();
        let AgentLoopState::InnerGenerate {
            mut stream,
            mut collected,
        } = previous
        else {
            panic!("transition_inner_generate requires AgentLoopState::InnerGenerate, found {actual}")
        };

        // Pump one item from the stream.
        let Some(item) = stream.next() else {
            // Stream finished — run loop detection, extract tool calls.
            self.breaker.on_success();
            return self.on_generation_complete(&collected);
        };

        // Check for mid-generation steering interruption (OD-19-3).
        if self.queues.has_priority() {
            self.queues.reset_cancel();
            let msgs = self.queues.drain_priority();
            for msg in msgs {
                self.push_user_message(msg);
            }
            // Discard current generation, re-assemble with new priority.
            self.state = AgentLoopState::InnerAssemble;
            return TaskStatus::Pending(AgentProgress::Steering {
                source: std::borrow::Cow::Borrowed("mid_gen_priority"),
            });
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
                TaskStatus::Ready(record)
            }
            Stream::Pending(progress) => {
                self.state = AgentLoopState::InnerGenerate { stream, collected };
                TaskStatus::Pending(progress)
            }
            Stream::Init | Stream::Ignore | Stream::Wait => {
                self.state = AgentLoopState::InnerGenerate { stream, collected };
                TaskStatus::Ignore
            }
            Stream::Delayed(d) => {
                self.state = AgentLoopState::InnerGenerate { stream, collected };
                TaskStatus::Delayed(d)
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
                        StreamSpread::Done(rec) => TaskStatus::Ready(rec),
                        StreamSpread::Pending(p) => TaskStatus::Pending(p),
                    }
                } else {
                    TaskStatus::Ignore
                }
            }
        }
    }

    fn on_generation_complete(
        &mut self,
        collected: &[Messages],
    ) -> TaskStatus<SessionRecord, AgentProgress, BoxedSendExecutionAction> {
        // Run loop detection (F17) synchronously on the model outputs.
        for msg in collected {
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
                            return TaskStatus::Pending(AgentProgress::Steering {
                                source: std::borrow::Cow::Borrowed("loop_redirect"),
                            });
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
                            return TaskStatus::Pending(AgentProgress::Steering {
                                source: std::borrow::Cow::Borrowed("loop_model_switch"),
                            });
                        }
                        Escalation::Terminate => {
                            // A model repeating the SAME empty answer is the
                            // vacuous case wearing a loop's clothes, and the
                            // agreed remedy there is to hand the weak answer
                            // over, not to replace it with an error. Failing
                            // here would turn `.` into "loop detected", which
                            // is strictly worse for whoever asked.
                            //
                            // A loop of substantive text still terminates —
                            // that is real token burn and stopping is the point.
                            if is_vacuous_answer(&assembled_answer(collected)) {
                                tracing::debug!(
                                    "agent: repeated empty answer, passing it \
                                     through rather than failing the turn"
                                );
                                break;
                            }

                            // redirect_count is small (bounded by max_redirects).
                            #[allow(clippy::cast_possible_truncation)]
                            let occurrences = self.detector.redirect_count() as u32;
                            let err =
                                AgenticError::LoopDetected(crate::agentic::errors::LoopDetection {
                                    kind: format!("{detection:?}"),
                                    occurrences,
                                });
                            self.state = AgentLoopState::Ending;
                            return TaskStatus::Ready(err.into_failed_action());
                        }
                    }
                }
            }
        }

        // A turn can finish having said nothing usable — a lone `.` or `,` and
        // little else. That is not a loop, but it is just as useless to the
        // caller, so it earns the same remedy: say what was wrong and let the
        // model have another go.
        //
        // Checked on the ASSEMBLED turn, not per message: a streaming model
        // emits one `Messages::Assistant` per token, and almost every single
        // token looks vacuous on its own.
        // A turn that called a tool is exempt: it legitimately produces no text,
        // and the tool result is the progress. Everything else — including a
        // turn that emitted nothing at all — is judged on its text, because an
        // empty answer is exactly as useless as a bare `.`.
        let called_a_tool = collected.iter().any(|message| {
            matches!(
                message,
                Messages::Assistant {
                    content: ModelOutput::ToolCall { .. },
                    ..
                }
            )
        });
        // Set when the ladder ran out on a vacuous answer, so the reset below
        // can tell "this turn was fine" from "this turn was junk we gave up on".
        let mut gave_up_on_a_bad_answer = false;

        if !called_a_tool {
            let answer = assembled_answer(collected);
            if self.detector.check_answer(&answer, &self.last_user_prompt) != LoopDetection::NoLoop
            {
                match self.detector.escalate() {
                    Escalation::Redirect | Escalation::SwitchModelOrTemperature { .. } => {
                        tracing::debug!(
                            answer = %answer,
                            attempt = self.detector.redirect_count(),
                            "agent: turn produced no usable answer, asking again"
                        );
                        self.push_user_message(vacuous_answer_redirect());
                        self.state = AgentLoopState::InnerAssemble;
                        // Streaming already handed the caller every token of the
                        // turn being thrown away, so tell it to drop them —
                        // otherwise the retry's answer is appended to the junk
                        // it was meant to replace.
                        return TaskStatus::Ready(SessionRecord::Retracted {
                            id: foundation_compact::ids::new_scru128(),
                            reason: format!(
                                "turn produced no usable answer ({answer:?}), retrying"
                            ),
                            timestamp: foundation_compact::SystemTime::now(),
                        });
                    }
                    // Deliberately NOT a failure. Out of retries, the best thing
                    // left is the weak answer itself — terminating would turn a
                    // poor reply into no reply, which is strictly worse for the
                    // caller. A real loop still terminates, because there the
                    // point is to stop burning tokens.
                    Escalation::Terminate => {
                        gave_up_on_a_bad_answer = true;
                        tracing::debug!(
                            answer = %answer,
                            "agent: still no usable answer after retries, passing it through"
                        );
                    }
                }
            }
        }

        // A turn that came back good earns a fresh allowance: one bad patch
        // early in a session must not leave it one hiccup from giving up hours
        // later, which is what happened when nothing ever reset this.
        //
        // Giving up on a bad answer is NOT that. Resetting there hands the next
        // outer pass a full ladder to spend on the same junk, so a model stuck
        // producing `0` burns max_redirects retries per outer iteration instead
        // of max_redirects in total. The count stands until something works.
        if !gave_up_on_a_bad_answer {
            self.detector.reset();
        }

        // Persist the accepted assistant turn to session history.
        //
        // The streaming path emits each message to the caller via
        // `Stream::Next` but never wrote it to `message_api` — only user
        // messages and tool results were persisted. So a resumed session (or
        // `message_api.recent()` on the next turn) saw the user's side of the
        // conversation but not the assistant's, silently losing multi-turn
        // context. Persist here — once per turn with the complete `collected`
        // set, NOT per `Stream::Next`, so token-streaming models do not write a
        // fragment per token. Loop redirect/terminate return above, so only an
        // accepted generation reaches this point.
        for msg in collected {
            if matches!(msg, Messages::Assistant { .. }) {
                let _ = self.message_api.append(SessionRecord::Conversation {
                    message: msg.clone(),
                });
            }
        }

        // Record the model's reported usage into the session ledger.
        // `TokenLedger::record` existed but the loop never called it, so budget
        // tracking, usage snapshots and cost accounting stayed at zero
        // regardless of real token consumption. Streaming models report
        // CUMULATIVE usage on each token message, so record only the LAST
        // assistant message's usage (recording every one would multiply-count).
        if let Some(Messages::Assistant { usage, .. }) = collected
            .iter()
            .rev()
            .find(|m| matches!(m, Messages::Assistant { .. }))
        {
            self.ledger.record(usage);
        }

        // Extract tool calls from assistant messages.
        let tool_calls = Self::extract_tool_calls(collected);

        if tool_calls.is_empty() {
            // No tools — go to output processing.
            self.state = AgentLoopState::OutputProcessing;
            TaskStatus::Ignore
        } else {
            let total = tool_calls.len();
            self.state = AgentLoopState::InnerToolCalls { calls: tool_calls };
            TaskStatus::Pending(AgentProgress::ExecutingTools {
                total,
                completed: 0,
            })
        }
    }

    fn extract_tool_calls(messages: &[Messages]) -> Vec<ToolCallRequest> {
        let mut calls = Vec::new();
        for msg in messages {
            if let Messages::Assistant {
                content:
                    ModelOutput::ToolCall {
                        id,
                        name,
                        arguments,
                        depends_on,
                        execution_hint,
                        ..
                    },
                ..
            } = msg
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
        calls
    }

    fn transition_inner_tool_calls(
        &mut self,
    ) -> TaskStatus<SessionRecord, AgentProgress, BoxedSendExecutionAction> {
        let previous = std::mem::replace(&mut self.state, AgentLoopState::Done);
        let actual = previous.variant_name();
        let AgentLoopState::InnerToolCalls { calls } = previous else {
            panic!(
                "transition_inner_tool_calls requires AgentLoopState::InnerToolCalls, found {actual}"
            )
        };

        // Build the workflow (topological sort by depends_on).
        let workflow = match self.tool_manager.build_workflow(&calls) {
            Ok(w) => w,
            Err(e) => {
                let err = AgenticError::ToolCall {
                    tool_name: "workflow".into(),
                    reason: e.to_string(),
                };
                self.state = AgentLoopState::OutputProcessing;
                return TaskStatus::Ready(err.into_failed_action());
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
            return TaskStatus::Ignore;
        }

        self.state = AgentLoopState::InnerExecuting {
            calls: flat_calls,
            results: Vec::new(),
            idx: 0,
            active: None,
            cancel_signals: Vec::new(),
        };
        TaskStatus::Ignore
    }

    fn transition_inner_executing(
        &mut self,
    ) -> TaskStatus<SessionRecord, AgentProgress, BoxedSendExecutionAction> {
        let previous = std::mem::replace(&mut self.state, AgentLoopState::Done);
        let actual = previous.variant_name();
        let AgentLoopState::InnerExecuting {
            calls,
            mut results,
            idx,
            mut active,
            mut cancel_signals,
        } = previous
        else {
            panic!(
                "transition_inner_executing requires AgentLoopState::InnerExecuting, found {actual}"
            )
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
            return TaskStatus::Pending(AgentProgress::Steering {
                source: std::borrow::Cow::Borrowed("cancel_executing"),
            });
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
                return TaskStatus::Ignore;
            }

            self.state = AgentLoopState::InnerEmitResults {
                results: result_messages,
                idx: 0,
            };
            return TaskStatus::Ignore;
        }

        // If no active driven iterator, start one via drive_future with CancellableFutureTask.
        if active.is_none() {
            let call = calls[idx].clone();
            let mgr = self.tool_manager.clone();
            let retry_config = mgr.retry_config(&call.name);
            let fut: core::pin::Pin<
                Box<dyn core::future::Future<Output = Result<ToolCallResult, ToolError>> + Send>,
            > = Box::pin(async move { mgr.execute_with_retry(&call, &retry_config).await });
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
                TaskStatus::Pending(AgentProgress::ExecutingTools {
                    total,
                    completed: idx + 1,
                })
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
                TaskStatus::Pending(AgentProgress::ExecutingTools {
                    total,
                    completed: idx + 1,
                })
            }
            Some(TaskStatus::Pending(_)) => {
                self.state = AgentLoopState::InnerExecuting {
                    calls,
                    results,
                    idx,
                    active,
                    cancel_signals,
                };
                TaskStatus::Pending(AgentProgress::ExecutingTools {
                    total,
                    completed: idx,
                })
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
                TaskStatus::Ignore
            }
            _ => {
                self.state = AgentLoopState::InnerExecuting {
                    calls,
                    results,
                    idx,
                    active,
                    cancel_signals,
                };
                TaskStatus::Ignore
            }
        }
    }

    fn transition_inner_emit_results(
        &mut self,
    ) -> TaskStatus<SessionRecord, AgentProgress, BoxedSendExecutionAction> {
        let previous = std::mem::replace(&mut self.state, AgentLoopState::Done);
        let actual = previous.variant_name();
        let AgentLoopState::InnerEmitResults { results, idx } = previous else {
            panic!(
                "transition_inner_emit_results requires AgentLoopState::InnerEmitResults, found {actual}"
            )
        };

        if idx >= results.len() {
            tracing::trace!(
                "Loop index greater tan results len: idx={}, len={}",
                idx,
                results.len()
            );

            // All results emitted — loop back to InnerAssemble for the LLM
            // to process tool results (guarded by max_inner_iterations).
            self.inner_iteration += 1;
            if self.inner_iteration >= self.config.max_inner_iterations {
                tracing::trace!("Max loop iteration method");
                self.state = AgentLoopState::OutputProcessing;
                return TaskStatus::Pending(AgentProgress::SessionEnding);
            }
            self.state = AgentLoopState::InnerAssemble;
            return TaskStatus::Ignore;
        }

        let msg = results[idx].clone();
        tracing::trace!("Adding new message from model: {:?}", &msg);
        let _ = self.message_api.append(SessionRecord::Conversation {
            message: msg.clone(),
        });
        self.message_count += 1;

        self.state = AgentLoopState::InnerEmitResults {
            results,
            idx: idx + 1,
        };
        TaskStatus::Ready(SessionRecord::Conversation { message: msg })
    }

    fn transition_output_processing(
        &mut self,
    ) -> TaskStatus<SessionRecord, AgentProgress, BoxedSendExecutionAction> {
        // Fire memory triggers (F15) — check if observation/reflection needed.
        let action = self.memory.check_triggers();
        self.state = AgentLoopState::OuterBoundary;
        match action {
            MemoryAction::GenerateObservation => {
                TaskStatus::Pending(AgentProgress::ProcessingMemory {
                    kind: MemoryKind::Observation,
                })
            }
            MemoryAction::GenerateReflection => {
                TaskStatus::Pending(AgentProgress::ProcessingMemory {
                    kind: MemoryKind::Reflection,
                })
            }
            MemoryAction::None => TaskStatus::Ignore,
        }
    }

    fn transition_ending(
        &mut self,
    ) -> TaskStatus<SessionRecord, AgentProgress, BoxedSendExecutionAction> {
        let snapshot = self.ledger.snapshot();
        let summary = SessionRecord::Summary {
            message_count: self.message_count,
            usage: snapshot,
        };
        self.state = AgentLoopState::Done;
        TaskStatus::Ready(summary)
    }

    // -----------------------------------------------------------------------
    // Error handling (F02)

    fn handle_error(
        &mut self,
        error: AgenticError,
    ) -> TaskStatus<SessionRecord, AgentProgress, BoxedSendExecutionAction> {
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

    /// Shrink an over-budget context in place by dropping the OLDEST messages
    /// until it fits under `preflight_compression_threshold * budget`.
    ///
    /// WHY: the config threshold existed but nothing applied it. When a context
    /// is assembled that would blow the budget, sending it wastes tokens (or is
    /// rejected by the provider). Truncation-based compression keeps the most
    /// recent turns — the ones that matter most — and drops the oldest.
    ///
    /// A `0.0` threshold (or no budget) disables it. Recomputes `token_estimate`
    /// so downstream (context-pressure) sees the compressed size.
    fn apply_preflight_compression(&self, ctx: &mut AgentContext) {
        if self.config.preflight_compression_threshold <= 0.0 {
            return;
        }
        let Some(budget) = self.ledger.budget() else {
            return; // unlimited budget — nothing to compress against
        };
        if budget == 0 {
            return;
        }

        #[allow(clippy::cast_precision_loss)]
        let limit = f64::from(self.config.preflight_compression_threshold) * budget as f64;

        // Drop oldest messages (front) until under the limit or only one left.
        while ctx.messages.len() > 1 {
            #[allow(clippy::cast_precision_loss)]
            let ratio = ctx.token_estimate as f64;
            if ratio <= limit {
                break;
            }
            let dropped = ctx.messages.remove(0);
            let est = crate::agentic::context::estimate_tokens_pub(&dropped);
            ctx.token_estimate = ctx.token_estimate.saturating_sub(est);
        }
    }

    fn apply_context_pressure(&self, ctx: &AgentContext) -> Option<String> {
        if self.config.context_pressure_threshold <= 0.0 {
            return ctx.system_prompt.clone();
        }

        let budget = self.ledger.budget().unwrap_or(u64::MAX);
        // u64 → f64: precision loss acceptable for ratio computation.
        #[allow(clippy::cast_precision_loss)]
        let usage_ratio = ctx.token_estimate as f64 / budget as f64;

        if usage_ratio >= f64::from(self.config.context_pressure_threshold) {
            // Percentage is in [0, ~100]; truncation/sign loss are safe.
            #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
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
            AgentLoopState::OuterBoundary => Some(self.transition_outer_boundary()),
            AgentLoopState::InnerAssemble => Some(self.transition_inner_assemble()),
            AgentLoopState::InnerGenerate { .. } => Some(self.transition_inner_generate()),
            AgentLoopState::InnerToolCalls { .. } => Some(self.transition_inner_tool_calls()),
            AgentLoopState::InnerExecuting { .. } => Some(self.transition_inner_executing()),
            AgentLoopState::InnerEmitResults { .. } => Some(self.transition_inner_emit_results()),
            AgentLoopState::OutputProcessing => Some(self.transition_output_processing()),
            AgentLoopState::Ending => Some(self.transition_ending()),
            AgentLoopState::Done => None,
        }
    }
}
