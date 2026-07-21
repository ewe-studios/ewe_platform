//! Public-API coverage for `ToolCallManager::build_workflow` — the F11 tool-call
//! dependency DAG that groups calls into ordered parallel/sequential stages.
//! Pure and deterministic; tool_impl.rs was 75% region coverage and this path
//! (topo-sort, cycle + unknown-dep detection, stage grouping) was untested.

use std::collections::HashMap;

use foundation_ai::agentic::tool_impl::{ToolCallManager, ToolCallRequest, ToolCallStage};
use foundation_ai::types::{ExecutionHint, SessionId};

fn call(id: &str, deps: &[&str], hint: ExecutionHint) -> ToolCallRequest {
    ToolCallRequest {
        id: id.into(),
        name: format!("tool_{id}"),
        arguments: HashMap::new(),
        depends_on: deps.iter().map(|s| (*s).to_string()).collect(),
        execution_hint: hint,
    }
}

fn stage_ids(stage: &ToolCallStage) -> Vec<String> {
    let calls = match stage {
        ToolCallStage::Parallel { calls, .. } | ToolCallStage::Sequential { calls, .. } => calls,
    };
    let mut ids: Vec<String> = calls.iter().map(|c| c.id.clone()).collect();
    ids.sort();
    ids
}

fn mgr() -> ToolCallManager {
    ToolCallManager::new(SessionId::new())
}

#[test]
fn empty_calls_produce_no_stages() {
    let wf = mgr().build_workflow(&[]).expect("empty is valid");
    assert!(wf.stages.is_empty());
}

#[test]
fn independent_calls_land_in_one_stage() {
    let calls = vec![
        call("a", &[], ExecutionHint::Unspecified),
        call("b", &[], ExecutionHint::Unspecified),
    ];
    let wf = mgr().build_workflow(&calls).expect("valid");
    assert_eq!(wf.stages.len(), 1, "no dependencies => a single stage");
    assert_eq!(stage_ids(&wf.stages[0]), vec!["a", "b"]);
}

#[test]
fn dependency_creates_ordered_stages() {
    // b depends on a, c depends on b => three sequential depth levels.
    let calls = vec![
        call("a", &[], ExecutionHint::Unspecified),
        call("b", &["a"], ExecutionHint::Unspecified),
        call("c", &["b"], ExecutionHint::Unspecified),
    ];
    let wf = mgr().build_workflow(&calls).expect("valid");
    assert_eq!(wf.stages.len(), 3, "a -> b -> c is three depth stages");
    assert_eq!(stage_ids(&wf.stages[0]), vec!["a"]);
    assert_eq!(stage_ids(&wf.stages[1]), vec!["b"]);
    assert_eq!(stage_ids(&wf.stages[2]), vec!["c"]);
}

#[test]
fn diamond_dependency_groups_by_depth() {
    // a; b,c depend on a; d depends on b and c => depths 0 / 1 / 2.
    let calls = vec![
        call("a", &[], ExecutionHint::Unspecified),
        call("b", &["a"], ExecutionHint::Unspecified),
        call("c", &["a"], ExecutionHint::Unspecified),
        call("d", &["b", "c"], ExecutionHint::Unspecified),
    ];
    let wf = mgr().build_workflow(&calls).expect("valid");
    assert_eq!(wf.stages.len(), 3);
    assert_eq!(stage_ids(&wf.stages[0]), vec!["a"]);
    assert_eq!(stage_ids(&wf.stages[1]), vec!["b", "c"], "b and c run together");
    assert_eq!(stage_ids(&wf.stages[2]), vec!["d"]);
}

#[test]
fn cyclic_dependency_is_rejected() {
    // a -> b -> a
    let calls = vec![
        call("a", &["b"], ExecutionHint::Unspecified),
        call("b", &["a"], ExecutionHint::Unspecified),
    ];
    let err = mgr()
        .build_workflow(&calls)
        .expect_err("a cycle must be rejected");
    assert!(
        format!("{err:?}").to_lowercase().contains("cyclic"),
        "error should name the cyclic dependency: {err:?}"
    );
}

#[test]
fn unknown_dependency_is_rejected() {
    let calls = vec![call("a", &["ghost"], ExecutionHint::Unspecified)];
    let err = mgr()
        .build_workflow(&calls)
        .expect_err("a dep on a non-existent call must be rejected");
    assert!(
        format!("{err:?}").to_lowercase().contains("unknown"),
        "error should name the unknown dependency: {err:?}"
    );
}

#[test]
fn sequential_hint_makes_its_stage_sequential() {
    let calls = vec![
        call("a", &[], ExecutionHint::Sequential),
        call("b", &[], ExecutionHint::Unspecified),
    ];
    let wf = mgr().build_workflow(&calls).expect("valid");
    assert_eq!(wf.stages.len(), 1);
    assert!(
        matches!(wf.stages[0], ToolCallStage::Sequential { .. }),
        "a Sequential hint in the stage forces the whole stage sequential"
    );
}
