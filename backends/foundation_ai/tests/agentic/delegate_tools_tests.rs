//! Delegation tools (delegate_start/check/result/stop/pause) — spec-60 F15.
//! Drives real child `AgentSession`s (MockModelProvider) via a `DelegationManager`.
//! The `#[valtron_test]`s also prove a child `run_turn` runs safely from within a
//! tool's async `execute` (no nested-executor deadlock).

use std::collections::HashMap;
use std::sync::Arc;

use foundation_ai::agentic::testing::{mock_text, MockModelProvider};
use foundation_ai::agentic::tool_impl::{ToolCallManager, ToolError, ToolImpl};
use foundation_ai::agentic::tools::delegate::{
    register_delegate_tools, DelegateCheckTool, DelegateResultTool, DelegateStartTool,
    DelegateStopTool, DelegationManager,
};
use foundation_ai::agentic::{AgentConfig, AgentSession, KvMemoryStore};
use foundation_ai::types::{ArgType, ModelId, SessionId, UserModelContent};
use foundation_core::valtron::valtron_test;
use foundation_db::{MemoryDocumentStore, MemoryStorage};

type D = MemoryDocumentStore;
type M = KvMemoryStore<MemoryStorage>;
type Manager = DelegationManager<D, M>;

/// Build a child session backed by a mock that always replies `reply`.
fn child_session(session_id: SessionId, reply: &'static str) -> AgentSession<D, M> {
    let mut mock = MockModelProvider::new();
    mock.on_any(vec![mock_text(reply)]);
    let model_id = ModelId::Name("mock".into(), None);
    AgentSession::builder(session_id, mock.into_router())
        .with_system_prompt("child")
        .with_model(model_id.clone())
        .with_config(AgentConfig {
            primary_model: model_id,
            ..Default::default()
        })
        .build()
        .expect("child session builds")
}

fn manager(depth: u32, max_depth: u32, reply: &'static str) -> Arc<Manager> {
    Arc::new(DelegationManager::new(
        Arc::new(move |sid, _d| child_session(sid, reply)),
        depth,
        max_depth,
    ))
}

fn args(pairs: &[(&str, &str)]) -> HashMap<String, ArgType> {
    pairs
        .iter()
        .map(|(k, v)| ((*k).to_string(), ArgType::Text((*v).to_string())))
        .collect()
}

fn text_of(r: &foundation_ai::agentic::tool_impl::ToolCallResult) -> String {
    match &r.content {
        UserModelContent::Text(t) => t.content.clone(),
        UserModelContent::Image(_) => String::new(),
    }
}

/// Parse the delegation id out of "delegation <id> completed".
fn id_from(msg: &str) -> String {
    msg.split_whitespace().nth(1).unwrap_or_default().to_string()
}

#[valtron_test]
fn delegate_start_runs_child_turn_to_completion() {
    let m = manager(0, 3, "child did the work");
    let start = DelegateStartTool::new(m.clone());

    // A child run_turn executed from within a tool's async execute — this is the
    // nested-execution path; it must complete, not deadlock.
    let out = futures_lite::future::block_on(
        start.execute(args(&[("task", "summarize the logs")])),
    )
    .expect("delegate_start succeeds");
    let msg = text_of(&out);
    assert!(msg.contains("completed"), "delegation should complete: {msg}");

    // The result is retrievable and carries the child's assistant output.
    let id = id_from(&msg);
    let result = futures_lite::future::block_on(
        DelegateResultTool::new(m.clone()).execute(args(&[("id", &id)])),
    )
    .expect("result available");
    assert_eq!(text_of(&result), "child did the work");

    // check reports completed.
    let check = futures_lite::future::block_on(
        DelegateCheckTool::new(m).execute(args(&[("id", &id)])),
    )
    .expect("check ok");
    assert!(text_of(&check).contains("completed"));
}

#[valtron_test]
fn delegate_depth_cap_is_enforced() {
    // A manager already at max depth must refuse to start a child.
    let m = manager(3, 3, "unused");
    let err = futures_lite::future::block_on(
        DelegateStartTool::new(m).execute(args(&[("task", "recurse forever")])),
    )
    .unwrap_err();
    assert!(
        matches!(err, ToolError::Execution { ref reason, .. } if reason.contains("depth cap")),
        "depth cap must block delegation: {err:?}"
    );
}

#[valtron_test]
fn delegate_stop_marks_handle_and_check_unknown_errors() {
    let m = manager(0, 2, "done");
    let start = DelegateStartTool::new(m.clone());
    let out = futures_lite::future::block_on(start.execute(args(&[("task", "t")]))).unwrap();
    let id = id_from(&text_of(&out));

    // stop annotates the handle.
    let stopped = futures_lite::future::block_on(
        DelegateStopTool::new(m.clone()).execute(args(&[("id", &id)])),
    )
    .expect("stop ok");
    assert!(text_of(&stopped).contains("stopped"));
    let check = futures_lite::future::block_on(
        DelegateCheckTool::new(m.clone()).execute(args(&[("id", &id)])),
    )
    .unwrap();
    assert!(text_of(&check).contains("stopped"));

    // Unknown ids error rather than silently succeeding.
    let err = futures_lite::future::block_on(
        DelegateCheckTool::new(m).execute(args(&[("id", "no-such-id")])),
    )
    .unwrap_err();
    assert!(matches!(err, ToolError::Execution { .. }));
}

#[valtron_test]
fn registering_delegate_tools_fills_shed_slot() {
    let m = manager(0, 2, "x");
    let mgr = ToolCallManager::new(SessionId::new());
    register_delegate_tools(&mgr, m);

    let shed = mgr.build_toolshed();
    let del = shed.delegate.expect("delegate slot filled");
    assert_eq!(del.start.name, "delegate_start");
    assert_eq!(del.stop.name, "delegate_stop");
    assert_eq!(del.pause.name, "delegate_pause");
    assert_eq!(del.check.name, "delegate_check");
    assert_eq!(del.result.name, "delegate_result");
}
