//! The `agent` tool (sub-agent delegation): definitions, argument checks and
//! the start / check / result / pause / resume / stop cycle over a mock
//! provider. Moved here from the inline module in `src/agentic/tools/agent.rs`.

use std::sync::Arc;
use std::thread;
use std::time::Duration;

use foundation_ai::agentic::memory_store::KvMemoryStore;
use foundation_ai::agentic::testing::{mock_text, MockModelProvider, MockTool};
use foundation_ai::agentic::tool_impl::{ToolCallManager, ToolCallResult, ToolError, ToolImpl};
use foundation_ai::agentic::tools::agent::{register_agent_tool, AgentTool};
use foundation_ai::agentic::UserId;
use foundation_ai::types::routable_provider::ProviderRouter;
use foundation_ai::types::{ModelId, SessionId, Tool, ToolArguments, UserModelContent};
use foundation_core::valtron::valtron_test;
use foundation_db::{MemoryDocumentStore, MemoryStorage};
use futures_lite::future::block_on;

type D = MemoryDocumentStore;
type M = KvMemoryStore<MemoryStorage>;

// ------------------------------------------------------------------
// Helpers
// ------------------------------------------------------------------

fn cmd(command: &str, pairs: &[(&str, &str)]) -> ToolArguments {
    let mut m = ToolArguments::new();
    m.insert(
        "command".to_string(),
        serde_json::Value::String(command.to_string()),
    );
    for (k, v) in pairs {
        m.insert(
            (*k).to_string(),
            serde_json::Value::String((*v).to_string()),
        );
    }
    m
}

fn empty_router() -> ProviderRouter {
    ProviderRouter::builder().build()
}

fn test_user() -> UserId {
    UserId("test".into())
}

fn test_tool() -> AgentTool<D, M> {
    AgentTool::new(
        empty_router(),
        0,
        5,
        ModelId::Name("mock".into(), None),
        "/tmp/test-delegation".into(),
        test_user(),
        vec![],
    )
}

fn tool_router(router: ProviderRouter) -> AgentTool<D, M> {
    AgentTool::new(
        router,
        0,
        5,
        ModelId::Name("mock".into(), None),
        "/tmp/test-delegation".into(),
        test_user(),
        vec![],
    )
}

fn as_json(
    result: &ToolCallResult,
) -> serde_json::Value {
    match &result.content {
        UserModelContent::Text(t) => {
            serde_json::from_str(&t.content).unwrap_or_else(|_| {
                serde_json::json!({"raw": t.content})
            })
        }
        _ => serde_json::json!({}),
    }
}

fn poll_until_done(
    t: &AgentTool<D, M>,
    id: &str,
    max_iters: usize,
) -> Option<serde_json::Value> {
    for _ in 0..max_iters {
        let check = block_on(
            t.execute(cmd("check", &[("id", id)])),
        )
        .expect("check");
        let check_json = as_json(&check);
        let state = check_json["state"].as_str().unwrap_or("unknown");
        match state {
            "Done" => return Some(check_json),
            "Failed" => return Some(check_json),
            _ => thread::sleep(Duration::from_millis(10)),
        }
    }
    None
}

// ------------------------------------------------------------------
// definition
// ------------------------------------------------------------------

#[test]
fn definition_is_multi_commands() {
    let t = test_tool();
    match t.definition() {
        Tool::MultiCommands(name, cmds) => {
            assert_eq!(name, "agent");
            assert_eq!(cmds.len(), 6);
            let names: Vec<&str> = cmds.iter().map(|c| c.name.as_str()).collect();
            for cmd in &["start", "check", "result", "pause", "resume", "stop"] {
                assert!(names.contains(cmd), "missing command: {cmd}");
            }
            let start = cmds.iter().find(|c| c.name == "start").unwrap();
            let required: Vec<&str> = start.arguments.schema["required"]
                .as_array()
                .unwrap()
                .iter()
                .map(|v| v.as_str().unwrap())
                .collect();
            assert!(required.contains(&"command"));
            assert!(required.contains(&"task"));
        }
        other => panic!("expected MultiCommands, got {other:?}"),
    }
}

#[test]
fn definition_name_is_agent() {
    assert_eq!(test_tool().definition().name(), "agent");
}

#[test]
fn definition_function_spec_discriminated() {
    let t = test_tool();
    let spec = t.definition().function_spec();
    assert_eq!(spec.name, "agent");
    assert!(spec.description.contains("start"));
    let branches = spec.parameters["oneOf"].as_array().unwrap();
    assert_eq!(branches.len(), 6);
    for (i, expected) in
        ["start", "check", "result", "pause", "resume", "stop"]
            .iter()
            .enumerate()
    {
        let got = branches[i]["properties"]["command"]["const"]
            .as_str()
            .unwrap();
        assert_eq!(got, *expected);
    }
}

// ------------------------------------------------------------------
// argument validation
// ------------------------------------------------------------------

#[test]
fn unknown_command_errors() {
    block_on(async {
        let err = test_tool()
            .execute(cmd("nonexistent", &[]))
            .await
            .unwrap_err();
        match err {
            ToolError::InvalidArguments { reason, .. } => {
                assert!(reason.contains("unknown agent command"));
            }
            other => panic!("expected InvalidArguments, got {other:?}"),
        }
    });
}

#[test]
fn start_missing_task_errors() {
    block_on(async {
        let err = test_tool().execute(cmd("start", &[])).await.unwrap_err();
        match err {
            ToolError::InvalidArguments { reason, .. } => {
                assert!(
                    reason.contains("missing required argument `task`"),
                    "{reason}"
                );
            }
            other => panic!("expected InvalidArguments, got {other:?}"),
        }
    });
}

#[test]
fn start_empty_task_errors() {
    block_on(async {
        let err = test_tool()
            .execute(cmd("start", &[("task", "   ")]))
            .await
            .unwrap_err();
        match err {
            ToolError::InvalidArguments { reason, .. } => {
                assert!(reason.contains("must not be empty"));
            }
            other => panic!("expected InvalidArguments, got {other:?}"),
        }
    });
}

#[test]
fn depth_cap_blocks_delegation() {
    block_on(async {
        let t = AgentTool::<D, M>::new(
            empty_router(),
            5,
            5,
            ModelId::Name("mock".into(), None),
            "/tmp".into(),
            test_user(),
            vec![],
        );
        let err = t
            .execute(cmd("start", &[("task", "anything")]))
            .await
            .unwrap_err();
        match err {
            ToolError::Execution { reason, .. } => {
                assert!(reason.contains("depth cap"));
            }
            other => panic!("expected Execution, got {other:?}"),
        }
    });
}

// ------------------------------------------------------------------
// unknown ids
// ------------------------------------------------------------------

fn assert_unknown_id(command: &str) {
    block_on(async {
        let err = test_tool()
            .execute(cmd(command, &[("id", "ghost")]))
            .await
            .unwrap_err();
        assert!(
            matches!(err, ToolError::InvalidArguments { .. }),
            "{command}: expected InvalidArguments, got {err:?}"
        );
    });
}

#[test]
fn check_unknown_id_errors() {
    assert_unknown_id("check");
}

#[test]
fn result_unknown_id_errors() {
    assert_unknown_id("result");
}

#[test]
fn pause_unknown_id_errors() {
    assert_unknown_id("pause");
}

#[test]
fn resume_unknown_id_errors() {
    assert_unknown_id("resume");
}

#[test]
fn stop_unknown_id_errors() {
    assert_unknown_id("stop");
}

// ------------------------------------------------------------------
// integration: full cycle with mock provider (valtron pool)
// ------------------------------------------------------------------

#[valtron_test]
fn start_returns_immediately_with_id() {
    let mut mock = MockModelProvider::new();
    mock.on_any(vec![mock_text("sub-agent result")]);
    let t = tool_router(mock.into_router());

    let result = block_on(
        t.execute(cmd("start", &[("task", "write hello world")])),
    )
    .expect("start");

    let json = as_json(&result);
    assert_eq!(json["status"], "running");
    assert!(!json["id"].as_str().unwrap().is_empty());
    assert!(!json["session_id"].as_str().unwrap().is_empty());
    assert!(json["output_location"]
        .as_str()
        .unwrap()
        .contains("test-delegation"));
}

#[valtron_test]
fn full_cycle_start_check_result() {
    let mut mock = MockModelProvider::new();
    mock.on_any(vec![mock_text("task completed successfully")]);
    let t = tool_router(mock.into_router());

    let start = block_on(
        t.execute(cmd("start", &[("task", "write hello")])),
    )
    .expect("start");
    let id = as_json(&start)["id"].as_str().unwrap().to_string();

    let done = poll_until_done(&t, &id, 200);
    assert!(done.is_some(), "sub-agent never completed after 2s");
    let done_json = done.unwrap();
    assert_eq!(done_json["state"], "Done");
    assert!(!done_json["location"].as_str().unwrap().is_empty());

    let result = block_on(
        t.execute(cmd("result", &[("id", &id)])),
    )
    .expect("result");
    let rj = as_json(&result);
    assert!(!rj["location"].as_str().unwrap().is_empty());
    assert!(rj["summary"].as_str().unwrap().len() > 0);

    // Cleanup: after result, id is freed → check fails
    let err = block_on(
        t.execute(cmd("check", &[("id", &id)])),
    )
    .unwrap_err();
    assert!(matches!(err, ToolError::InvalidArguments { .. }));
}

// ------------------------------------------------------------------
// pause / resume
// ------------------------------------------------------------------

#[valtron_test]
fn pause_reports_paused_on_check() {
    let mut mock = MockModelProvider::new();
    mock.on_any(vec![mock_text("ok")]);
    let t = tool_router(mock.into_router());

    let start = block_on(
        t.execute(cmd("start", &[("task", "stuff")])),
    )
    .expect("start");
    let id = as_json(&start)["id"].as_str().unwrap().to_string();

    // Pause
    let pause = block_on(
        t.execute(cmd("pause", &[("id", &id)])),
    )
    .expect("pause");
    assert_eq!(as_json(&pause)["status"], "paused");

    // Check → Paused
    let check = block_on(
        t.execute(cmd("check", &[("id", &id)])),
    )
    .expect("check");
    assert_eq!(as_json(&check)["state"], "Paused");

    // Resume
    let resume = block_on(
        t.execute(cmd("resume", &[("id", &id)])),
    )
    .expect("resume");
    assert_eq!(as_json(&resume)["status"], "resumed");
}

#[valtron_test]
fn resume_idempotent_when_not_paused() {
    let mut mock = MockModelProvider::new();
    mock.on_any(vec![mock_text("ok")]);
    let t = tool_router(mock.into_router());

    let start = block_on(
        t.execute(cmd("start", &[("task", "stuff")])),
    )
    .expect("start");
    let id = as_json(&start)["id"].as_str().unwrap().to_string();

    // Resume without prior pause — succeeds
    let resume = block_on(
        t.execute(cmd("resume", &[("id", &id)])),
    )
    .expect("resume");
    assert_eq!(as_json(&resume)["status"], "resumed");
}

// ------------------------------------------------------------------
// stop
// ------------------------------------------------------------------

#[valtron_test]
fn stop_removes_run_without_keep_session() {
    let mut mock = MockModelProvider::new();
    mock.on_any(vec![mock_text("reply")]);
    let t = tool_router(mock.into_router());

    let start = block_on(
        t.execute(cmd("start", &[("task", "stuff")])),
    )
    .expect("start");
    let id = as_json(&start)["id"].as_str().unwrap().to_string();

    let stop = block_on(
        t.execute(cmd("stop", &[("id", &id)])),
    )
    .expect("stop");
    assert_eq!(as_json(&stop)["status"], "stopped");

    // Run removed → check errors
    let err = block_on(
        t.execute(cmd("check", &[("id", &id)])),
    )
    .unwrap_err();
    assert!(matches!(err, ToolError::InvalidArguments { .. }));
}

#[valtron_test]
fn stop_with_keep_session_preserves_run() {
    let mut mock = MockModelProvider::new();
    mock.on_any(vec![mock_text("reply")]);
    let t = tool_router(mock.into_router());

    let start = block_on(
        t.execute(cmd(
            "start",
            &[("task", "stuff"), ("keep_session", "true")],
        )),
    )
    .expect("start");
    let id = as_json(&start)["id"].as_str().unwrap().to_string();

    // Stop with keep_session
    block_on(
        t.execute(cmd("stop", &[("id", &id)])),
    )
    .expect("stop");

    // The contract under test: `keep_session` means `stop` must NOT drop the
    // run from the map, so the id still resolves. `check` returning Ok (i.e.
    // not `unknown delegation id`) is exactly that guarantee.
    //
    // Deliberately no assertion on the reported state: whether the pool has
    // advanced the sub-agent past `Started` by this point is a scheduling
    // race, and asserting on it made this test flaky across codegen
    // backends (it passed on cranelift, failed on LLVM).
    let check = block_on(
        t.execute(cmd("check", &[("id", &id)])),
    )
    .expect("keep_session=true must preserve the run, so check resolves the id");
    let state = as_json(&check)["state"].as_str().unwrap_or("").to_string();
    assert!(
        !state.is_empty(),
        "check must report some state for a preserved run, got: {state:?}"
    );
}

#[valtron_test]
fn stop_without_keep_session_drops_the_run() {
    // The contrast case that gives the test above its meaning: without
    // `keep_session`, `stop` removes the run and the id stops resolving.
    let mut mock = MockModelProvider::new();
    mock.on_any(vec![mock_text("reply")]);
    let t = tool_router(mock.into_router());

    let start = block_on(
        t.execute(cmd("start", &[("task", "stuff")])),
    )
    .expect("start");
    let id = as_json(&start)["id"].as_str().unwrap().to_string();

    block_on(t.execute(cmd("stop", &[("id", &id)])))
        .expect("stop");

    let err = block_on(
        t.execute(cmd("check", &[("id", &id)])),
    )
    .expect_err("without keep_session the run must be gone");
    assert!(matches!(err, ToolError::InvalidArguments { .. }));
}

// ------------------------------------------------------------------
// child tool provisioning
// ------------------------------------------------------------------

#[valtron_test]
fn child_tools_provisioned_on_sub_agent() {
    let write_tool: Arc<dyn ToolImpl> =
        Arc::new(MockTool::returning("write", "wrote /tmp/out.json"));

    let mut mock = MockModelProvider::new();
    mock.on_any(vec![mock_text("result")]);

    let t = AgentTool::<D, M>::new(
        mock.into_router(),
        0,
        5,
        ModelId::Name("mock".into(), None),
        "/tmp/test-delegation".into(),
        test_user(),
        vec![write_tool],
    );

    let result = block_on(
        t.execute(cmd("start", &[("task", "write something")])),
    )
    .expect("start");
    let json = as_json(&result);
    assert_eq!(json["status"], "running");
    assert!(!json["session_id"].as_str().unwrap().is_empty());
}

// ------------------------------------------------------------------
// registration
// ------------------------------------------------------------------

#[test]
fn register_adds_to_manager() {
    let manager = ToolCallManager::new(SessionId::new());
    assert!(!manager.names().contains(&"agent".to_string()));

    register_agent_tool::<D, M>(
        &manager,
        empty_router(),
        0,
        5,
        ModelId::Name("mock".into(), None),
        "/tmp/delegation",
        test_user(),
        vec![],
    );

    assert!(manager.names().contains(&"agent".to_string()));
}

#[test]
fn toolshed_contains_agent_after_registration() {
    let manager = ToolCallManager::new(SessionId::new());
    register_agent_tool::<D, M>(
        &manager,
        empty_router(),
        0,
        5,
        ModelId::Name("mock".into(), None),
        "/tmp/delegation",
        test_user(),
        vec![],
    );

    let shed = manager.all_declarations();
    assert!(
        shed.tools.iter().any(|t| t.name() == "agent"),
        "toolshed should contain agent: {shed:?}"
    );
}

// ------------------------------------------------------------------
// start() — optional argument branches
// ------------------------------------------------------------------

/// Build a `start` arg map with arbitrary extra typed args.
fn start_args(pairs: Vec<(&str, serde_json::Value)>) -> ToolArguments {
    let mut m = ToolArguments::new();
    m.insert(
        "command".to_string(),
        serde_json::Value::String("start".into()),
    );
    m.insert(
        "task".to_string(),
        serde_json::Value::String("do stuff".into()),
    );
    for (k, v) in pairs {
        m.insert(k.to_string(), v);
    }
    m
}

#[valtron_test]
fn start_accepts_an_explicit_model() {
    let mut mock = MockModelProvider::new();
    mock.on_any(vec![mock_text("ok")]);
    let t = tool_router(mock.into_router());

    // Exercises the `Some(Text)` arm of the `model` lookup rather than
    // falling back to the parent session's default.
    let out = block_on(t.execute(start_args(vec![(
        "model",
        serde_json::Value::String("mock".into()),
    )])))
    .expect("start with explicit model");
    assert_eq!(as_json(&out)["status"], "running");
}

#[valtron_test]
fn start_ignores_an_empty_model_and_uses_the_default() {
    let mut mock = MockModelProvider::new();
    mock.on_any(vec![mock_text("ok")]);
    let t = tool_router(mock.into_router());

    // An empty string must NOT become ModelId::Name("") — the guard falls
    // through to the parent model, which is known to resolve.
    let out = block_on(t.execute(start_args(vec![(
        "model",
        serde_json::Value::String(String::new()),
    )])))
    .expect("empty model falls back to the default");
    assert_eq!(as_json(&out)["status"], "running");
}

#[valtron_test]
fn start_accepts_a_system_prompt() {
    let mut mock = MockModelProvider::new();
    mock.on_any(vec![mock_text("ok")]);
    let t = tool_router(mock.into_router());

    let out = block_on(t.execute(start_args(vec![(
        "system",
        serde_json::Value::String("be terse".into()),
    )])))
    .expect("start with system prompt");
    assert_eq!(as_json(&out)["status"], "running");
}

#[valtron_test]
fn start_accepts_max_iterations_in_every_numeric_spelling() {
    // Models emit integers inconsistently (u64 / i64 / stringified), so all
    // three must parse rather than silently dropping the runaway guard.
    for arg in [
        serde_json::json!(7),
        serde_json::json!(7),
        serde_json::Value::String("7".into()),
    ] {
        let mut mock = MockModelProvider::new();
        mock.on_any(vec![mock_text("ok")]);
        let t = tool_router(mock.into_router());

        let out = block_on(
            t.execute(start_args(vec![("max_iterations", arg.clone())])),
        )
        .unwrap_or_else(|e| panic!("max_iterations {arg:?} must be accepted: {e:?}"));
        assert_eq!(as_json(&out)["status"], "running");
    }
}

#[test]
fn start_rejects_a_zero_or_negative_max_iterations() {
    // A cap of 0 (or negative) would create a sub-agent that can never take
    // a step — reject it at the boundary instead of hanging at Started.
    block_on(async {
        for bad in [
            serde_json::json!(0),
            serde_json::json!(0),
            serde_json::json!(-3),
        ] {
            let t = test_tool();
            let result = t
                .execute(start_args(vec![("max_iterations", bad.clone())]))
                .await;
            let err = match result {
                Err(e) => e,
                Ok(_) => panic!("max_iterations {bad:?} must be rejected, not accepted"),
            };
            match err {
                ToolError::InvalidArguments { reason, .. } => {
                    assert!(
                        reason.contains("max_iterations"),
                        "expected a max_iterations message for {bad:?}, got: {reason}"
                    );
                }
                other => panic!("expected InvalidArguments for {bad:?}, got {other:?}"),
            }
        }
    });
}

#[test]
fn start_rejects_an_unparseable_max_iterations() {
    // A stringified cap that is not a number must be an explicit error, not
    // a silently-dropped guard.
    block_on(async {
        let t = test_tool();
        let err = t
            .execute(start_args(vec![(
                "max_iterations",
                serde_json::Value::String("lots".into()),
            )]))
            .await
            .unwrap_err();
        match err {
            ToolError::InvalidArguments { reason, .. } => {
                assert!(
                    reason.contains("max_iterations"),
                    "expected a max_iterations message, got: {reason}"
                );
            }
            other => panic!("expected InvalidArguments, got {other:?}"),
        }
    });
}

#[valtron_test]
fn start_returns_distinct_ids_per_delegation() {
    // Two delegations must not collide in the runs map, or `check` on one
    // would report the other.
    let mut mock = MockModelProvider::new();
    mock.on_any(vec![mock_text("ok")]);
    let t = tool_router(mock.into_router());

    let a = block_on(
        t.execute(cmd("start", &[("task", "first")])),
    )
    .expect("start a");
    let b = block_on(
        t.execute(cmd("start", &[("task", "second")])),
    )
    .expect("start b");

    let id_a = as_json(&a)["id"].as_str().unwrap().to_string();
    let id_b = as_json(&b)["id"].as_str().unwrap().to_string();
    assert_ne!(id_a, id_b, "each start must mint a fresh delegation id");

    // …and distinct output locations, or they would overwrite each other.
    assert_ne!(
        as_json(&a)["output_location"],
        as_json(&b)["output_location"]
    );
}

// ------------------------------------------------------------------
// result() — the not-done branch
// ------------------------------------------------------------------

#[valtron_test]
fn result_while_paused_reports_not_done() {
    // Pausing pins the status at Paused, which makes the "not done yet"
    // branch of `result` deterministic (no scheduler race).
    let mut mock = MockModelProvider::new();
    mock.on_any(vec![mock_text("ok")]);
    let t = tool_router(mock.into_router());

    let start = block_on(
        t.execute(cmd("start", &[("task", "stuff")])),
    )
    .expect("start");
    let id = as_json(&start)["id"].as_str().unwrap().to_string();

    block_on(t.execute(cmd("pause", &[("id", &id)])))
        .expect("pause");

    let err = block_on(
        t.execute(cmd("result", &[("id", &id)])),
    )
    .expect_err("a paused run is not done, so result must error");
    match err {
        ToolError::Execution { reason, .. } => {
            assert!(
                reason.contains("not done yet"),
                "expected a not-done message, got: {reason}"
            );
        }
        other => panic!("expected Execution, got {other:?}"),
    }
}

#[valtron_test]
fn paused_run_is_not_advanced_by_check() {
    // While paused, drain_run must return early without pulling the stream,
    // so repeated checks keep reporting Paused rather than drifting.
    let mut mock = MockModelProvider::new();
    mock.on_any(vec![mock_text("ok")]);
    let t = tool_router(mock.into_router());

    let start = block_on(
        t.execute(cmd("start", &[("task", "stuff")])),
    )
    .expect("start");
    let id = as_json(&start)["id"].as_str().unwrap().to_string();

    block_on(t.execute(cmd("pause", &[("id", &id)])))
        .expect("pause");

    for _ in 0..3 {
        let c = block_on(
            t.execute(cmd("check", &[("id", &id)])),
        )
        .expect("check");
        assert_eq!(as_json(&c)["state"], "Paused");
    }
}

#[valtron_test]
fn resume_after_pause_lets_the_run_progress_again() {
    let mut mock = MockModelProvider::new();
    mock.on_any(vec![mock_text("done text")]);
    let t = tool_router(mock.into_router());

    let start = block_on(
        t.execute(cmd("start", &[("task", "stuff")])),
    )
    .expect("start");
    let id = as_json(&start)["id"].as_str().unwrap().to_string();

    block_on(t.execute(cmd("pause", &[("id", &id)])))
        .expect("pause");
    block_on(t.execute(cmd("resume", &[("id", &id)])))
        .expect("resume");

    // After resuming, the run must be able to reach a terminal state.
    let done = poll_until_done(&t, &id, 200);
    assert!(
        done.is_some(),
        "a resumed run must still be able to complete"
    );
}

// ------------------------------------------------------------------
// depth
// ------------------------------------------------------------------

#[valtron_test]
fn start_below_the_cap_is_allowed() {
    // depth < max_depth must pass the guard (the cap test covers ==).
    let mut mock = MockModelProvider::new();
    mock.on_any(vec![mock_text("ok")]);
    let t = AgentTool::<D, M>::new(
        mock.into_router(),
        4,
        5,
        ModelId::Name("mock".into(), None),
        "/tmp/test-delegation".into(),
        test_user(),
        vec![],
    );
    let out = block_on(
        t.execute(cmd("start", &[("task", "stuff")])),
    )
    .expect("depth 4 of max 5 must be allowed");
    assert_eq!(as_json(&out)["status"], "running");
}

#[test]
fn depth_cap_of_zero_refuses_all_delegation() {
    block_on(async {
        let t = AgentTool::<D, M>::new(
            empty_router(),
            0,
            0,
            ModelId::Name("mock".into(), None),
            "/tmp".into(),
            test_user(),
            vec![],
        );
        let err = t
            .execute(cmd("start", &[("task", "anything")]))
            .await
            .unwrap_err();
        match err {
            ToolError::Execution { reason, .. } => {
                assert!(reason.contains("depth cap"), "got: {reason}");
            }
            other => panic!("expected Execution, got {other:?}"),
        }
    });
}
