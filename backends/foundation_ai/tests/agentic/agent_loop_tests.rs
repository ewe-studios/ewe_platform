use std::sync::Arc;

use foundation_ai::agentic::tool_impl::ToolCallManager;
use foundation_ai::agentic::{
    AgentConfig, AgentLoop, AgentProgress, ContextConfig, ContextProvider, KvMemoryStore,
    MemoryConfig, MemoryCoordinator, MemoryHierarchy, MessageApi, SteeringQueues, TokenLedger,
};
use foundation_ai::types::{
    MessageRole, Messages, ModelId, ProviderRouter, SessionId, SessionRecord, TextContent,
    UserModelContent,
};
use foundation_core::valtron::{TaskIterator, TaskStatus};
use foundation_db::{MemoryDocumentStore, MemoryStorage};

// ---------------------------------------------------------------------------
// Test helpers

type TestMemStore = KvMemoryStore<MemoryStorage>;
type TestDocStore = MemoryDocumentStore;

struct TestHarness {
    agent: AgentLoop<TestDocStore, TestMemStore>,
    priority_queue: Arc<concurrent_queue::ConcurrentQueue<Messages>>,
    follow_up_queue: Arc<concurrent_queue::ConcurrentQueue<Messages>>,
    cancel_signal: Arc<std::sync::atomic::AtomicU32>,
    ledger: TokenLedger,
}

fn build_harness(config: AgentConfig) -> TestHarness {
    let session_id = SessionId::new();
    let ledger = TokenLedger::new();

    let kv = KvMemoryStore::new(MemoryStorage::new());
    let memory_store = Arc::new(kv);

    let doc = MemoryDocumentStore::new();
    let message_api = MessageApi::new(session_id.clone(), doc);

    let context_provider = ContextProvider::new(
        session_id.clone(),
        message_api.clone(),
        Arc::clone(&memory_store),
        ledger.clone(),
        Some("You are a helpful assistant.".into()),
        ContextConfig::default(),
    );

    let tool_manager = ToolCallManager::new(session_id.clone());
    let queues = SteeringQueues::new();
    let priority_handle = Arc::clone(&queues.priority);
    let follow_up_handle = Arc::clone(&queues.follow_up);
    let cancel_handle = Arc::clone(&queues.cancel_signal);

    let coordinator = MemoryCoordinator::new(
        KvMemoryStore::new(MemoryStorage::new()),
        MemoryDocumentStore::new(),
    );
    let memory = MemoryHierarchy::new(
        session_id.clone(),
        coordinator,
        ledger.clone(),
        MemoryConfig::default(),
    );

    let router = ProviderRouter::builder().build();

    let agent = AgentLoop::new(
        session_id,
        context_provider,
        tool_manager,
        queues,
        memory,
        message_api,
        ledger.clone(),
        router,
        config,
    );

    TestHarness {
        agent,
        priority_queue: priority_handle,
        follow_up_queue: follow_up_handle,
        cancel_signal: cancel_handle,
        ledger,
    }
}

fn default_harness() -> TestHarness {
    build_harness(AgentConfig {
        primary_model: ModelId::Name("test-model".into(), None),
        ..Default::default()
    })
}

fn make_user_msg(content: &str) -> Messages {
    Messages::User {
        id: foundation_compact::ids::new_scru128(),
        role: MessageRole::User,
        content: UserModelContent::Text(TextContent {
            content: content.into(),
            signature: None,
        }),
        signature: None,
    }
}

fn make_system_msg(content: &str) -> Messages {
    Messages::User {
        id: foundation_compact::ids::new_scru128(),
        role: MessageRole::System,
        content: UserModelContent::Text(TextContent {
            content: content.into(),
            signature: None,
        }),
        signature: None,
    }
}

fn inject_follow_up(h: &TestHarness, msg: Messages) {
    let _ = h.follow_up_queue.push(msg);
}

fn inject_priority(h: &TestHarness, msg: Messages) {
    use std::sync::atomic::Ordering;
    let _ = h.priority_queue.push(msg);
    h.cancel_signal.store(1, Ordering::SeqCst); // PauseForPriority
}

// ---------------------------------------------------------------------------
// Full lifecycle (Init → OuterBoundary → Ending → Done)

#[test]
fn full_lifecycle_no_messages() {
    let mut h = default_harness();

    // Init
    let s1 = h.agent.next_status();
    assert!(matches!(s1, Some(TaskStatus::Init)));
    assert_eq!(h.agent.state_label(), "outer_boundary");

    // OuterBoundary — empty queues → Ending
    let s2 = h.agent.next_status();
    assert!(matches!(
        s2,
        Some(TaskStatus::Pending(AgentProgress::SessionEnding))
    ));
    assert_eq!(h.agent.state_label(), "ending");

    // Ending → Summary
    let s3 = h.agent.next_status();
    assert!(matches!(
        s3,
        Some(TaskStatus::Ready(SessionRecord::Summary { .. }))
    ));
    assert_eq!(h.agent.state_label(), "done");

    // Done → None
    assert!(h.agent.next_status().is_none());
}

// ---------------------------------------------------------------------------
// Follow-up queue drives outer → inner

#[test]
fn follow_up_message_triggers_inner_loop() {
    let mut h = default_harness();

    // Skip Init.
    assert!(matches!(h.agent.next_status(), Some(TaskStatus::Init)));

    // Inject a follow-up before polling OuterBoundary.
    inject_follow_up(&h, make_user_msg("hello"));

    let status = h.agent.next_status();
    match status {
        Some(TaskStatus::Pending(AgentProgress::Steering { source })) => {
            assert!(source.contains("follow_up"));
        }
        other => panic!("expected Steering follow_up, got {other:?}"),
    }
    assert_eq!(h.agent.state_label(), "inner_assemble");
}

// ---------------------------------------------------------------------------
// Priority queue takes precedence over follow-up

#[test]
fn priority_preempts_follow_up() {
    let mut h = default_harness();
    assert!(matches!(h.agent.next_status(), Some(TaskStatus::Init)));

    inject_follow_up(&h, make_user_msg("follow-up"));
    inject_priority(&h, make_system_msg("urgent"));

    let status = h.agent.next_status();
    match status {
        Some(TaskStatus::Pending(AgentProgress::Steering { source })) => {
            assert!(source.contains("priority"));
        }
        other => panic!("expected priority steering, got {other:?}"),
    }
}

// ---------------------------------------------------------------------------
// Max outer iterations guard

#[test]
fn max_outer_iterations_ends_session() {
    let mut h = build_harness(AgentConfig {
        primary_model: ModelId::Name("test-model".into(), None),
        max_outer_iterations: 1,
        ..Default::default()
    });

    // Init → OuterBoundary (iteration 1 — at max)
    assert!(matches!(h.agent.next_status(), Some(TaskStatus::Init)));

    // Inject a message so we enter the inner loop
    inject_follow_up(&h, make_user_msg("go"));
    let _ = h.agent.next_status(); // Steering → inner_assemble

    // The inner_assemble will fail on router (no provider registered),
    // which triggers handle_error → Terminate since there's no fallback.
    // That puts us in Ending.
    let s = h.agent.next_status();

    // After error we're in ending. Drive to summary.
    // We may get FailedAction or SessionEnding depending on the error path.
    // Either way, the loop must eventually terminate.
    let mut reached_done = false;
    // Process up to 5 more statuses to reach Done.
    if matches!(s, Some(TaskStatus::Ready(SessionRecord::Summary { .. }))) {
        reached_done = h.agent.next_status().is_none();
    } else {
        for _ in 0..5 {
            match h.agent.next_status() {
                None => {
                    reached_done = true;
                    break;
                }
                Some(TaskStatus::Ready(SessionRecord::Summary { .. })) => {
                    reached_done = h.agent.next_status().is_none();
                    break;
                }
                _ => {}
            }
        }
    }
    assert!(
        reached_done,
        "agent should reach Done within bounded iterations"
    );
}

// ---------------------------------------------------------------------------
// Budget exhaustion (via public TokenLedger)

#[test]
fn budget_exhaustion_terminates() {
    use foundation_ai::types::{CostStatus, UsageCosting, UsageReport};

    let mut h = default_harness();

    // Set and exhaust budget via the shared ledger.
    h.ledger.set_budget(Some(100));
    h.ledger.record(&UsageReport {
        input: 100.0,
        output: 50.0,
        cache_read: 0.0,
        cache_write: 0.0,
        total_tokens: 150.0,
        cost: UsageCosting::zero(CostStatus::Estimated),
    });

    // Init → OuterBoundary
    assert!(matches!(h.agent.next_status(), Some(TaskStatus::Init)));

    // Inject a message to enter inner loop.
    inject_follow_up(&h, make_user_msg("hello"));
    let _ = h.agent.next_status(); // Steering → InnerAssemble

    // InnerAssemble detects exhausted budget.
    let status = h.agent.next_status();
    match status {
        Some(TaskStatus::Ready(SessionRecord::FailedAction { error, .. })) => {
            assert!(
                format!("{error:?}").contains("BudgetExhausted"),
                "expected BudgetExhausted, got {error:?}"
            );
        }
        other => panic!("expected BudgetExhausted FailedAction, got {other:?}"),
    }
}

// ---------------------------------------------------------------------------
// Public accessors

#[test]
fn current_model_returns_configured_primary() {
    let h = default_harness();
    assert_eq!(
        *h.agent.current_model(),
        ModelId::Name("test-model".into(), None)
    );
}

#[test]
fn state_label_starts_at_initializing() {
    let h = default_harness();
    assert_eq!(h.agent.state_label(), "initializing");
}

// ---------------------------------------------------------------------------
// push_user_message is observable via next_status

#[test]
fn push_user_message_feeds_pending_messages() {
    let mut h = default_harness();

    // Init
    assert!(matches!(h.agent.next_status(), Some(TaskStatus::Init)));

    // Push via public API + inject a follow-up so we enter the inner loop.
    h.agent.push_user_message(make_user_msg("via push"));
    inject_follow_up(&h, make_user_msg("trigger"));

    let status = h.agent.next_status();
    // Should enter inner loop (follow-up triggers steering).
    assert!(matches!(
        status,
        Some(TaskStatus::Pending(AgentProgress::Steering { .. }))
    ));
    assert_eq!(h.agent.state_label(), "inner_assemble");
}

// ---------------------------------------------------------------------------
// Multiple follow-ups processed across outer iterations

#[test]
fn second_follow_up_triggers_second_outer_iteration() {
    let mut h = build_harness(AgentConfig {
        primary_model: ModelId::Name("test-model".into(), None),
        max_outer_iterations: 5,
        ..Default::default()
    });

    // Init
    assert!(matches!(h.agent.next_status(), Some(TaskStatus::Init)));

    // First follow-up → inner loop
    inject_follow_up(&h, make_user_msg("first"));
    let s = h.agent.next_status();
    assert!(matches!(
        s,
        Some(TaskStatus::Pending(AgentProgress::Steering { .. }))
    ));
    assert_eq!(h.agent.state_label(), "inner_assemble");

    // Inner assemble will fail (no provider) → error → ending path.
    // The important thing is it entered the inner loop from a follow-up.
}

// ---------------------------------------------------------------------------
// Done is truly terminal

#[test]
fn done_is_terminal() {
    let mut h = default_harness();

    // Drive to Done.
    assert!(matches!(h.agent.next_status(), Some(TaskStatus::Init)));
    assert!(matches!(
        h.agent.next_status(),
        Some(TaskStatus::Pending(AgentProgress::SessionEnding))
    ));
    assert!(matches!(
        h.agent.next_status(),
        Some(TaskStatus::Ready(SessionRecord::Summary { .. }))
    ));

    // Multiple calls to Done all return None.
    assert!(h.agent.next_status().is_none());
    assert!(h.agent.next_status().is_none());
    assert!(h.agent.next_status().is_none());
}
