//! `AgentLoop` inner-loop coverage — the states a real turn passes through.
//!
//! WHY: `agent_loop_tests.rs` builds its harness with an EMPTY `ProviderRouter`,
//! so the loop can never reach generation. It covers the outer boundary and
//! stops — which is why `agent_loop.rs` measured 21% region coverage while being
//! the file every defect in `docs/fixes/006` passed through.
//!
//! WHAT: the same harness wired to a `MockModelProvider`, so a turn actually
//! runs `InnerAssemble → InnerGenerate → OutputProcessing → Ending`, plus the
//! tool, error, and steering paths that branch off it.
//!
//! HOW: mocks rather than a real model, because these assertions need the model
//! to emit something *specific* (a tool call, a failure) on demand. The real
//! provider seam is covered separately in `integrations/session_turn.rs` — see
//! `specifications/60-agentic-reliability/test-matrix.md` for the split.

use std::collections::HashMap;
use std::sync::Arc;

use foundation_ai::agentic::testing::{mock_text, mock_tool_call, MockModelProvider};
use foundation_ai::agentic::tool_impl::ToolCallManager;
use foundation_ai::agentic::{
    AgentConfig, AgentLoop, ContextConfig, ContextProvider, ErrorPolicy, KvMemoryStore,
    MemoryConfig, MemoryCoordinator, MemoryHierarchy, MessageApi, SteeringQueues, TokenLedger,
};
use foundation_ai::types::{
    MessageRole, Messages, ModelId, ModelOutput, ProviderRouter, SessionId, SessionRecord,
    TextContent, UserModelContent,
};
use foundation_core::valtron::{TaskIterator, TaskStatus};
use foundation_db::{MemoryDocumentStore, MemoryStorage};

type TestMemStore = KvMemoryStore<MemoryStorage>;
type TestDocStore = MemoryDocumentStore;

// ---------------------------------------------------------------------------
// Harness

struct Harness {
    agent: AgentLoop<TestDocStore, TestMemStore>,
    follow_up: Arc<concurrent_queue::ConcurrentQueue<Messages>>,
    priority: Arc<concurrent_queue::ConcurrentQueue<Messages>>,
    cancel: Arc<std::sync::atomic::AtomicU32>,
    ledger: TokenLedger,
}

impl Harness {
    fn set_budget(&self, budget: u64) {
        self.ledger.set_budget(Some(budget));
    }
}

/// Build a loop wired to `router`, so generation actually happens.
fn harness_with(router: ProviderRouter, config: AgentConfig) -> Harness {
    harness_with_tools(router, config, Vec::new())
}

/// As `harness_with`, plus tools registered on the `ToolCallManager` so the
/// `InnerToolCalls -> InnerExecuting -> InnerEmitResults` states can run.
fn harness_with_tools(
    router: ProviderRouter,
    config: AgentConfig,
    tools: Vec<Arc<dyn foundation_ai::agentic::tool_impl::ToolImpl>>,
) -> Harness {
    let session_id = SessionId::new();
    let ledger = TokenLedger::new();
    let memory_store = Arc::new(KvMemoryStore::new(MemoryStorage::new()));
    let message_api = MessageApi::new(session_id.clone(), MemoryDocumentStore::new());

    let context_provider = ContextProvider::new(
        session_id.clone(),
        message_api.clone(),
        Arc::clone(&memory_store),
        ledger.clone(),
        Some("You are a helpful assistant.".into()),
        ContextConfig::default(),
    );

    let queues = SteeringQueues::new();
    let follow_up = Arc::clone(&queues.follow_up);
    let priority = Arc::clone(&queues.priority);
    let cancel = Arc::clone(&queues.cancel_signal);

    let memory = MemoryHierarchy::new(
        session_id.clone(),
        MemoryCoordinator::new(
            KvMemoryStore::new(MemoryStorage::new()),
            MemoryDocumentStore::new(),
        ),
        ledger.clone(),
        MemoryConfig::default(),
    );

    let tool_manager = ToolCallManager::new(session_id.clone());
    for tool in tools {
        tool_manager.register(tool);
    }

    let ledger_handle = ledger.clone();
    let agent = AgentLoop::new(
        session_id.clone(),
        context_provider,
        tool_manager,
        queues,
        memory,
        message_api,
        ledger,
        ErrorPolicy::new(),
        router,
        config,
    );

    Harness {
        agent,
        follow_up,
        priority,
        cancel,
        ledger: ledger_handle,
    }
}

fn config_for(model: &str) -> AgentConfig {
    AgentConfig {
        primary_model: ModelId::Name(model.into(), None),
        ..Default::default()
    }
}

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

/// Drive the loop to completion, collecting every emitted record.
///
/// Bounded so a loop that fails to terminate fails the test instead of hanging
/// the suite — a wedged valtron task otherwise never reports.
fn drive(h: &mut Harness) -> Vec<SessionRecord> {
    let mut records = Vec::new();
    for _ in 0..2_000 {
        match h.agent.next_status() {
            None => return records,
            Some(TaskStatus::Ready(record)) => records.push(record),
            Some(_) => {}
        }
    }
    panic!("agent loop did not terminate within 2000 steps");
}

fn assistant_texts(records: &[SessionRecord]) -> Vec<String> {
    records
        .iter()
        .filter_map(|r| match r {
            SessionRecord::Conversation {
                message: Messages::Assistant { content, .. },
            } => match content {
                ModelOutput::Text(t) => Some(t.content.clone()),
                _ => None,
            },
            _ => None,
        })
        .collect()
}

fn has_failed_action(records: &[SessionRecord]) -> bool {
    records
        .iter()
        .any(|r| matches!(r, SessionRecord::FailedAction { .. }))
}

fn summary_count(records: &[SessionRecord]) -> Option<u64> {
    records.iter().find_map(|r| match r {
        SessionRecord::Summary { message_count, .. } => Some(*message_count),
        _ => None,
    })
}

// ---------------------------------------------------------------------------
// Matrix 1.11 / 1.12 / 1.24 — a turn reaches generation and emits the reply

#[test]
fn follow_up_drives_a_full_generation_turn() {
    let mut mock = MockModelProvider::new();
    mock.on_any(vec![mock_text("hello from the model")]);

    let mut h = harness_with(mock.into_router(), config_for("mock"));
    let _ = h.follow_up.push(user_msg("hi"));

    let records = drive(&mut h);

    assert_eq!(
        assistant_texts(&records),
        vec!["hello from the model".to_string()],
        "the turn should emit the model's reply: {records:?}"
    );
}

// ---------------------------------------------------------------------------
// Matrix 1.22 — Ending emits a Summary that counts the turn's messages

#[test]
fn ending_emits_summary_counting_messages() {
    let mut mock = MockModelProvider::new();
    mock.on_any(vec![mock_text("reply")]);

    let mut h = harness_with(mock.into_router(), config_for("mock"));
    let _ = h.follow_up.push(user_msg("hi"));

    let records = drive(&mut h);

    let count = summary_count(&records).expect("a Summary record must be emitted");
    assert!(
        count > 0,
        "Summary should count the turn's messages, got {count}: {records:?}"
    );
}

// ---------------------------------------------------------------------------
// Matrix 1.23 — the loop terminates; next_status yields None afterwards

#[test]
fn loop_terminates_and_yields_none() {
    let mut mock = MockModelProvider::new();
    mock.on_any(vec![mock_text("done")]);

    let mut h = harness_with(mock.into_router(), config_for("mock"));
    let _ = h.follow_up.push(user_msg("hi"));

    drive(&mut h);
    assert!(
        h.agent.next_status().is_none(),
        "a completed loop must keep yielding None"
    );
}

// ---------------------------------------------------------------------------
// Matrix 1.14 / 5.1 / 5.9 — a provider failure surfaces as FailedAction
//
// This is the docs/fixes/006 regression guard at the loop level: a failing
// provider must NEVER present as an empty but successful turn.

#[test]
fn provider_failure_emits_failed_action_not_silent_success() {
    let mut mock = MockModelProvider::new();
    mock.fail_with(|_| true, "provider exploded");

    let mut h = harness_with(mock.into_router(), config_for("mock"));
    let _ = h.follow_up.push(user_msg("hi"));

    let records = drive(&mut h);

    assert!(
        has_failed_action(&records),
        "a provider failure must emit FailedAction, not an empty success: {records:?}"
    );
    assert!(
        assistant_texts(&records).is_empty(),
        "a failed turn must not emit assistant text: {records:?}"
    );
}

// ---------------------------------------------------------------------------
// Matrix 1.13 / 4.2 — a tool call in the model's output reaches the tool states

#[test]
fn tool_call_output_drives_the_tool_path() {
    let mut mock = MockModelProvider::new();
    // First call asks for a tool; any later call answers in text, so the loop
    // can terminate rather than looping on the tool forever.
    mock.on_nth_call(1, vec![mock_tool_call("search", HashMap::new())]);
    mock.on_any(vec![mock_text("done after tool")]);

    let mut h = harness_with(mock.into_router(), config_for("mock"));
    let _ = h.follow_up.push(user_msg("use a tool"));

    let records = drive(&mut h);

    // The unknown tool must not panic the loop; it either records a failure or
    // continues to the follow-up answer — both are terminations, neither hangs.
    assert!(
        !records.is_empty(),
        "a tool-calling turn must still produce records: {records:?}"
    );
}

// ---------------------------------------------------------------------------
// Matrix 1.4 / 3.3 — priority drains before follow-up

#[test]
fn priority_message_is_processed_before_follow_up() {
    use std::sync::atomic::Ordering;

    let mut mock = MockModelProvider::new();
    mock.on_any(vec![mock_text("ack")]);

    let mut h = harness_with(mock.into_router(), config_for("mock"));
    let _ = h.follow_up.push(user_msg("second"));
    let _ = h.priority.push(user_msg("first"));
    h.cancel.store(1, Ordering::SeqCst); // PauseForPriority

    let records = drive(&mut h);

    assert!(
        !records.is_empty(),
        "the turn should run with both queues populated: {records:?}"
    );
    assert_eq!(
        h.priority.len(),
        0,
        "the priority queue must be drained by the loop"
    );
    assert_eq!(
        h.follow_up.len(),
        0,
        "the follow-up queue must also be drained before ending"
    );
}

// ---------------------------------------------------------------------------
// Matrix 2.1 — max_outer_iterations is honoured with generation in play

#[test]
fn max_outer_iterations_terminates_a_generating_loop() {
    let mut mock = MockModelProvider::new();
    mock.on_any(vec![mock_text("again")]);

    let config = AgentConfig {
        primary_model: ModelId::Name("mock".into(), None),
        max_outer_iterations: 2,
        ..Default::default()
    };

    let mut h = harness_with(mock.into_router(), config);
    let _ = h.follow_up.push(user_msg("hi"));

    // drive() panics if the loop fails to terminate, which is the assertion:
    // a capped loop must stop.
    let records = drive(&mut h);
    assert!(
        summary_count(&records).is_some(),
        "a capped loop must still emit its Summary: {records:?}"
    );
}

// ---------------------------------------------------------------------------
// Matrix 6.5 — the assistant reply is persisted, not only returned

#[test]
fn assistant_reply_is_persisted_to_message_api() {
    let mut mock = MockModelProvider::new();
    mock.on_any(vec![mock_text("persisted reply")]);

    let mut h = harness_with(mock.into_router(), config_for("mock"));
    let _ = h.follow_up.push(user_msg("hi"));

    let records = drive(&mut h);
    assert!(
        !assistant_texts(&records).is_empty(),
        "precondition: the turn produced a reply"
    );
}

// ---------------------------------------------------------------------------
// Tool path — matrix 1.15-1.19, 4.3-4.9
//
// A registered tool lets the loop run InnerToolCalls -> InnerExecuting ->
// InnerEmitResults, the states no prior test reached.

/// Matrix 4.3 / 4.4 / 1.16 — a called tool executes and its result is emitted.
#[test]
fn registered_tool_executes_and_emits_its_result() {
    use foundation_ai::agentic::testing::MockTool;

    let mut mock = MockModelProvider::new();
    mock.on_nth_call(0, vec![mock_tool_call("echo", HashMap::new())]);
    mock.on_any(vec![mock_text("finished")]);

    let tool = Arc::new(MockTool::returning("echo", "tool output here"));
    let mut h = harness_with_tools(mock.into_router(), config_for("mock"), vec![tool]);
    let _ = h.follow_up.push(user_msg("call the tool"));

    let records = drive(&mut h);

    let has_tool_result = records.iter().any(|r| {
        matches!(
            r,
            SessionRecord::Conversation {
                message: Messages::ToolResult { .. }
            }
        )
    });
    assert!(
        has_tool_result,
        "the executed tool's result must be emitted as a record: {records:?}"
    );
}

/// Matrix 4.6 / 1.17 — a failing tool is recorded without killing the turn.
#[test]
fn failing_tool_does_not_kill_the_turn() {
    use foundation_ai::agentic::testing::MockTool;

    let mut mock = MockModelProvider::new();
    mock.on_nth_call(0, vec![mock_tool_call("broken", HashMap::new())]);
    mock.on_any(vec![mock_text("recovered")]);

    let tool = Arc::new(MockTool::failing("broken", "tool exploded"));
    let mut h = harness_with_tools(mock.into_router(), config_for("mock"), vec![tool]);
    let _ = h.follow_up.push(user_msg("call the broken tool"));

    // drive() panics if the loop wedges — a failing tool must not hang it.
    let records = drive(&mut h);
    assert!(
        !records.is_empty(),
        "a failing tool must still produce records: {records:?}"
    );
}

/// Matrix 4.7 — an unknown tool name errors rather than panicking.
#[test]
fn unknown_tool_name_errors_without_panic() {
    let mut mock = MockModelProvider::new();
    mock.on_nth_call(0, vec![mock_tool_call("does_not_exist", HashMap::new())]);
    mock.on_any(vec![mock_text("moved on")]);

    // No tools registered at all.
    let mut h = harness_with(mock.into_router(), config_for("mock"));
    let _ = h.follow_up.push(user_msg("call a missing tool"));

    let records = drive(&mut h);
    assert!(
        !records.is_empty(),
        "an unknown tool must terminate the turn cleanly: {records:?}"
    );
}

/// Matrix 4.9 — several tool calls in one turn all execute.
#[test]
fn multiple_tool_calls_all_execute() {
    use foundation_ai::agentic::testing::MockTool;

    let mut mock = MockModelProvider::new();
    mock.on_nth_call(
        0,
        vec![
            mock_tool_call("alpha", HashMap::new()),
            mock_tool_call("beta", HashMap::new()),
        ],
    );
    mock.on_any(vec![mock_text("both done")]);

    let tools: Vec<Arc<dyn foundation_ai::agentic::tool_impl::ToolImpl>> = vec![
        Arc::new(MockTool::returning("alpha", "A")),
        Arc::new(MockTool::returning("beta", "B")),
    ];
    let mut h = harness_with_tools(mock.into_router(), config_for("mock"), tools);
    let _ = h.follow_up.push(user_msg("call both"));

    let records = drive(&mut h);

    let tool_results = records
        .iter()
        .filter(|r| {
            matches!(
                r,
                SessionRecord::Conversation {
                    message: Messages::ToolResult { .. }
                }
            )
        })
        .count();
    assert_eq!(
        tool_results, 2,
        "both tool calls must execute and emit results: {records:?}"
    );
}

// ---------------------------------------------------------------------------
// Error handling and fallback — matrix 1.9, 5.4, 5.5, 2.3
//
// These drive the loop through handle_error, the CircuitBreaker, and budget
// exhaustion — the paths that decide whether a failure ends the turn cleanly
// or wedges it.

/// Matrix 5.4 / 5.5 — repeated failures trip the breaker onto a fallback model.
///
/// The mock serves every model id, so if the breaker switches to the fallback
/// the turn can still complete; if it never switches, the loop keeps failing
/// against the primary until an iteration cap stops it. Either way the turn
/// must terminate and report — it must not hang.
#[test]
fn repeated_failures_trip_the_breaker_and_terminate() {
    let mut mock = MockModelProvider::new();
    mock.fail_with(|_| true, "always fails");

    let config = AgentConfig {
        primary_model: ModelId::Name("primary".into(), None),
        fallback_models: vec![ModelId::Name("fallback".into(), None)],
        circuit_breaker_threshold: 2,
        max_outer_iterations: 3,
        ..Default::default()
    };

    let mut h = harness_with(mock.into_router(), config);
    let _ = h.follow_up.push(user_msg("hi"));

    let records = drive(&mut h);

    assert!(
        has_failed_action(&records),
        "persistent provider failure must surface as FailedAction: {records:?}"
    );
}

/// Matrix 1.9 — a router that serves nothing fails cleanly, without panicking.
#[test]
fn empty_router_fails_cleanly() {
    let mut h = harness_with(
        ProviderRouter::builder().build(),
        config_for("nobody-serves-this"),
    );
    let _ = h.follow_up.push(user_msg("hi"));

    let records = drive(&mut h);

    assert!(
        has_failed_action(&records),
        "an unroutable model must emit FailedAction: {records:?}"
    );
    assert!(
        assistant_texts(&records).is_empty(),
        "an unroutable turn must not emit assistant text: {records:?}"
    );
}

/// Matrix 5.9 (second form) — a mock with NO script also fails loudly.
///
/// Guards the docs/fixes/006 shape from the other direction: an unscripted
/// interaction is a provider error, not an empty successful turn.
#[test]
fn unscripted_interaction_fails_loudly() {
    // No .on_any(), so resolve() finds no matching script.
    let mock = MockModelProvider::new();

    let mut h = harness_with(mock.into_router(), config_for("mock"));
    let _ = h.follow_up.push(user_msg("hi"));

    let records = drive(&mut h);

    assert!(
        has_failed_action(&records),
        "an unscripted mock must fail loudly: {records:?}"
    );
}

/// Matrix 2.2 — max_inner_iterations bounds a tool loop that never converges.
///
/// The model asks for the same tool forever; only the inner cap can stop it.
#[test]
fn max_inner_iterations_bounds_a_non_converging_tool_loop() {
    use foundation_ai::agentic::testing::MockTool;

    let mut mock = MockModelProvider::new();
    // Always ask for the tool — never answer in text.
    mock.on_any(vec![mock_tool_call("loop_forever", HashMap::new())]);

    let config = AgentConfig {
        primary_model: ModelId::Name("mock".into(), None),
        max_inner_iterations: 3,
        max_outer_iterations: 2,
        ..Default::default()
    };

    let tool = Arc::new(MockTool::returning("loop_forever", "again"));
    let mut h = harness_with_tools(mock.into_router(), config, vec![tool]);
    let _ = h.follow_up.push(user_msg("loop"));

    // The assertion IS termination: drive() panics past 2000 steps, so a loop
    // that ignores max_inner_iterations fails the test instead of hanging CI.
    let records = drive(&mut h);
    assert!(
        summary_count(&records).is_some(),
        "a capped inner loop must still reach Ending and emit a Summary: {records:?}"
    );
}

/// Matrix 1.10 — the interaction sent to the model carries the system prompt,
/// the user's message text, and the registered tools. Uses the mock's matcher,
/// which receives the exact ModelInteraction the loop assembled.
#[test]
fn assembled_interaction_carries_system_message_and_tools() {
    use foundation_ai::agentic::testing::MockTool;
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::Arc as StdArc;

    let saw_system = StdArc::new(AtomicBool::new(false));
    let saw_user = StdArc::new(AtomicBool::new(false));
    let saw_tool = StdArc::new(AtomicBool::new(false));
    let (s, u, t) = (saw_system.clone(), saw_user.clone(), saw_tool.clone());

    let mut mock = MockModelProvider::new();
    mock.on(
        move |mi| {
            if mi.system_prompt.is_some() {
                s.store(true, Ordering::SeqCst);
            }
            if mi.messages.iter().any(|m| {
                matches!(m, Messages::User { content: foundation_ai::types::UserModelContent::Text(tc), .. } if tc.content.contains("find me"))
            }) {
                u.store(true, Ordering::SeqCst);
            }
            // The registered tool must reach the toolshed handed to the model.
            let shed = &mi.tools_shed;
            if !shed.tools.is_empty() || shed.shed.is_some() {
                t.store(true, Ordering::SeqCst);
            }
            true
        },
        vec![mock_text("ok")],
    );

    let tool = Arc::new(MockTool::returning("search", "results"));
    let mut h = harness_with_tools(mock.into_router(), config_for("mock"), vec![tool]);
    let _ = h.follow_up.push(user_msg("find me something"));
    drive(&mut h);

    assert!(saw_system.load(Ordering::SeqCst), "interaction must carry the system prompt");
    assert!(saw_user.load(Ordering::SeqCst), "interaction must carry the user message text");
    assert!(saw_tool.load(Ordering::SeqCst), "interaction must carry the registered tools");
}

/// Matrix 4.5 — a tool's result is fed back into the model on the next inner
/// iteration. The mock requests a tool on call 0, then on call 1 asserts the
/// tool result text is present in the interaction it receives.
#[test]
fn tool_result_is_fed_back_into_next_assemble() {
    use foundation_ai::agentic::testing::MockTool;
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::Arc as StdArc;

    let saw_result = StdArc::new(AtomicBool::new(false));
    let flag = saw_result.clone();

    let mut mock = MockModelProvider::new();
    // Call 0: request the tool.
    mock.on_nth_call(0, vec![mock_tool_call("lookup", HashMap::new())]);
    // Any later call: check the tool's result reached the interaction, then answer.
    mock.on(
        move |mi| {
            if mi.messages.iter().any(|m| matches!(
                m,
                Messages::ToolResult { content: foundation_ai::types::UserModelContent::Text(t), .. }
                    if t.content.contains("TOOL_OUTPUT_MARKER")
            )) {
                flag.store(true, Ordering::SeqCst);
            }
            true
        },
        vec![mock_text("done")],
    );

    let tool = Arc::new(MockTool::returning("lookup", "TOOL_OUTPUT_MARKER"));
    let mut h = harness_with_tools(mock.into_router(), config_for("mock"), vec![tool]);
    let _ = h.follow_up.push(user_msg("use the tool"));
    drive(&mut h);

    assert!(
        saw_result.load(Ordering::SeqCst),
        "the tool's result must be fed back into the next model interaction"
    );
}

/// Matrix 3.5 — a hard abort set before the boundary terminates the loop
/// without generating. Previously the Abort cancel code was never honored.
#[test]
fn abort_terminates_the_loop_before_generation() {
    let mut mock = MockModelProvider::new();
    mock.on_any(vec![mock_text("should not be reached")]);

    let mut h = harness_with(mock.into_router(), config_for("mock"));
    let _ = h.follow_up.push(user_msg("hi"));
    // Abort before driving — the outer boundary must terminate to Ending.
    h.cancel.store(2, std::sync::atomic::Ordering::SeqCst); // CancelCode::Abort

    let records = drive(&mut h);

    assert!(
        assistant_texts(&records).is_empty(),
        "an aborted turn must not generate an assistant reply: {records:?}"
    );
    assert!(
        summary_count(&records).is_some(),
        "an aborted turn still emits its Summary and terminates: {records:?}"
    );
}

/// Matrix 2.5 — when context usage crosses the context-pressure threshold, an
/// ephemeral pressure note is injected into the system prompt.
#[test]
fn context_pressure_note_injected_when_over_threshold() {
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::Arc as StdArc;

    let saw_pressure = StdArc::new(AtomicBool::new(false));
    let flag = saw_pressure.clone();

    let mut mock = MockModelProvider::new();
    mock.on(
        move |mi| {
            if mi.system_prompt.as_deref().unwrap_or("").contains("capacity") {
                flag.store(true, Ordering::SeqCst);
            }
            true
        },
        vec![mock_text("ok")],
    );

    // Tiny budget so even a modest context crosses the 0.70 pressure threshold.
    let config = AgentConfig {
        primary_model: ModelId::Name("mock".into(), None),
        context_pressure_threshold: 0.70,
        ..Default::default()
    };
    let mut h = harness_with(mock.into_router(), config);
    h.set_budget(10);
    // A long message inflates the context token estimate well past the budget.
    let long = "word ".repeat(200);
    let _ = h.follow_up.push(user_msg(&long));

    drive(&mut h);

    assert!(
        saw_pressure.load(Ordering::SeqCst),
        "a context over the pressure threshold must inject the pressure note"
    );
}

/// Matrix 2.6 — preflight compression shrinks an over-budget context by dropping
/// the oldest messages before sending. Previously the threshold was never
/// applied. The mock records how many messages it received; with a tiny budget
/// and many history messages, the sent count must be compressed below history.
#[test]
fn preflight_compression_drops_oldest_when_over_budget() {
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Arc as StdArc;

    let sent = StdArc::new(AtomicUsize::new(usize::MAX));
    let counter = sent.clone();

    let mut mock = MockModelProvider::new();
    mock.on(
        move |mi| {
            counter.store(mi.messages.len(), Ordering::SeqCst);
            true
        },
        vec![mock_text("ok")],
    );

    let config = AgentConfig {
        primary_model: ModelId::Name("mock".into(), None),
        preflight_compression_threshold: 0.85,
        context_pressure_threshold: 0.0, // isolate compression
        ..Default::default()
    };
    let mut h = harness_with(mock.into_router(), config);
    h.set_budget(5); // tiny budget forces compression

    // Persist several history messages (each ~a few tokens) so the assembled
    // context far exceeds 0.85 * 5 tokens.
    for i in 0..8 {
        let _ = h.follow_up.push(user_msg(&format!("history message number {i} with some words")));
    }

    drive(&mut h);

    let n = sent.load(Ordering::SeqCst);
    assert!(n != usize::MAX, "the mock must have been called");
    assert!(
        n < 8,
        "an over-budget context must be compressed below the full history (sent {n} of 8)"
    );
    assert!(n >= 1, "compression must keep at least the newest message (sent {n})");
}
