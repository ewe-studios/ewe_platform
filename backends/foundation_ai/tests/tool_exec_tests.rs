//! ToolCall Execution DAG tests (F11).
//!
//! Tests workflow building (topological staging), retry config, and error classification.

use async_trait::async_trait;
use foundation_ai::agentic::{
    FailMode, ToolCallManager, ToolCallRequest, ToolCallResult, ToolCallStage, ToolDefinition,
    ToolError, ToolErrorKind, ToolImpl, ToolRetryConfig,
};
use foundation_ai::types::{
    ArgType, Args, ExecutionHint, SessionId, TextContent, UserModelContent,
};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

// ---------------------------------------------------------------------------
// Test tools

struct EchoTool;

#[async_trait]
impl ToolImpl for EchoTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "echo".into(),
            description: "Echoes input".into(),
            arguments: Args::from_value(serde_json::json!({})),
            category: "shell".into(),
        }
    }

    async fn execute(
        &self,
        arguments: HashMap<String, ArgType>,
    ) -> Result<ToolCallResult, ToolError> {
        let msg = arguments
            .get("message")
            .and_then(|v| match v {
                ArgType::Text(s) => Some(s.clone()),
                _ => None,
            })
            .unwrap_or_default();
        Ok(ToolCallResult {
            content: UserModelContent::Text(TextContent {
                content: format!("echo: {msg}"),
                signature: None,
            }),
            error_detail: None,
        })
    }
}

struct FailingTool {
    kind: ToolErrorKind,
}

#[async_trait]
impl ToolImpl for FailingTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "fail".into(),
            description: "Always fails".into(),
            arguments: Args::from_value(serde_json::json!({})),
            category: "shell".into(),
        }
    }

    async fn execute(
        &self,
        _arguments: HashMap<String, ArgType>,
    ) -> Result<ToolCallResult, ToolError> {
        match self.kind {
            ToolErrorKind::Timeout => Err(ToolError::Timeout {
                tool: "fail".into(),
            }),
            ToolErrorKind::Execution => Err(ToolError::Execution {
                tool: "fail".into(),
                reason: "boom".into(),
            }),
            _ => Err(ToolError::Execution {
                tool: "fail".into(),
                reason: "generic".into(),
            }),
        }
    }
}

fn req(id: &str, name: &str, deps: Vec<&str>, hint: ExecutionHint) -> ToolCallRequest {
    ToolCallRequest {
        id: id.into(),
        name: name.into(),
        arguments: HashMap::new(),
        depends_on: deps.into_iter().map(String::from).collect(),
        execution_hint: hint,
    }
}

fn mgr() -> ToolCallManager {
    let m = ToolCallManager::new(SessionId::new());
    m.register(Arc::new(EchoTool));
    m
}

// ---------------------------------------------------------------------------
// Workflow builder tests

#[test]
fn empty_calls_produces_empty_workflow() {
    let wf = mgr().build_workflow(&[]).unwrap();
    assert!(wf.stages.is_empty());
}

#[test]
fn single_call_produces_one_sequential_stage() {
    let wf = mgr()
        .build_workflow(&[req("a", "echo", vec![], ExecutionHint::Unspecified)])
        .unwrap();
    assert_eq!(wf.stages.len(), 1);
    assert!(matches!(&wf.stages[0], ToolCallStage::Sequential { calls, .. } if calls.len() == 1));
}

#[test]
fn independent_calls_produce_one_parallel_stage() {
    let wf = mgr()
        .build_workflow(&[
            req("a", "echo", vec![], ExecutionHint::Unspecified),
            req("b", "echo", vec![], ExecutionHint::Unspecified),
            req("c", "echo", vec![], ExecutionHint::Unspecified),
        ])
        .unwrap();
    assert_eq!(wf.stages.len(), 1);
    assert!(matches!(
        &wf.stages[0],
        ToolCallStage::Parallel { calls, fail_mode }
            if calls.len() == 3 && *fail_mode == FailMode::CollectAll
    ));
}

#[test]
fn linear_chain_produces_multiple_stages() {
    // a -> b -> c
    let wf = mgr()
        .build_workflow(&[
            req("a", "echo", vec![], ExecutionHint::Unspecified),
            req("b", "echo", vec!["a"], ExecutionHint::Unspecified),
            req("c", "echo", vec!["b"], ExecutionHint::Unspecified),
        ])
        .unwrap();
    assert_eq!(wf.stages.len(), 3);
}

#[test]
fn diamond_dependency_produces_three_stages() {
    // a -> b, a -> c, b+c -> d
    let wf = mgr()
        .build_workflow(&[
            req("a", "echo", vec![], ExecutionHint::Unspecified),
            req("b", "echo", vec!["a"], ExecutionHint::Unspecified),
            req("c", "echo", vec!["a"], ExecutionHint::Unspecified),
            req("d", "echo", vec!["b", "c"], ExecutionHint::Unspecified),
        ])
        .unwrap();
    // Stage 0: a (sequential, single)
    // Stage 1: b, c (parallel, both depth 1)
    // Stage 2: d (sequential, single)
    assert_eq!(wf.stages.len(), 3);
    assert!(matches!(
        &wf.stages[1],
        ToolCallStage::Parallel { calls, .. } if calls.len() == 2
    ));
}

#[test]
fn decision_04_five_call_example() {
    // Decision 04's canonical example:
    // read_a (depth 0), read_b (depth 0), read_c (depth 0) → parallel stage 0
    // analyze (depends on read_a, read_b, read_c, depth 1) → sequential stage 1
    // summarize (depends on analyze, depth 2) → sequential stage 2
    let wf = mgr()
        .build_workflow(&[
            req("read_a", "echo", vec![], ExecutionHint::Parallel),
            req("read_b", "echo", vec![], ExecutionHint::Parallel),
            req("read_c", "echo", vec![], ExecutionHint::Parallel),
            req(
                "analyze",
                "echo",
                vec!["read_a", "read_b", "read_c"],
                ExecutionHint::Sequential,
            ),
            req(
                "summarize",
                "echo",
                vec!["analyze"],
                ExecutionHint::Sequential,
            ),
        ])
        .unwrap();
    assert_eq!(wf.stages.len(), 3);
    assert!(matches!(
        &wf.stages[0],
        ToolCallStage::Parallel { calls, .. } if calls.len() == 3
    ));
    assert!(matches!(
        &wf.stages[1],
        ToolCallStage::Sequential { calls, .. } if calls.len() == 1
    ));
    assert!(matches!(
        &wf.stages[2],
        ToolCallStage::Sequential { calls, .. } if calls.len() == 1
    ));
}

#[test]
fn sequential_hint_forces_sequential_stage() {
    let wf = mgr()
        .build_workflow(&[
            req("a", "echo", vec![], ExecutionHint::Sequential),
            req("b", "echo", vec![], ExecutionHint::Parallel),
        ])
        .unwrap();
    assert_eq!(wf.stages.len(), 1);
    // One Sequential call in the stage forces the whole stage to Sequential.
    assert!(matches!(&wf.stages[0], ToolCallStage::Sequential { .. }));
}

#[test]
fn cyclic_dependency_rejected() {
    let result = mgr().build_workflow(&[
        req("a", "echo", vec!["b"], ExecutionHint::Unspecified),
        req("b", "echo", vec!["a"], ExecutionHint::Unspecified),
    ]);
    assert!(result.is_err());
    let err = result.unwrap_err();
    assert!(matches!(err, ToolError::InvalidArguments { reason, .. } if reason.contains("cyclic")));
}

#[test]
fn missing_dependency_rejected() {
    let result = mgr().build_workflow(&[req(
        "a",
        "echo",
        vec!["nonexistent"],
        ExecutionHint::Unspecified,
    )]);
    assert!(result.is_err());
    let err = result.unwrap_err();
    assert!(
        matches!(err, ToolError::InvalidArguments { reason, .. } if reason.contains("unknown call"))
    );
}

// ---------------------------------------------------------------------------
// ToolError classification

#[test]
fn tool_error_kind_classification() {
    assert_eq!(
        ToolError::Timeout { tool: "t".into() }.kind(),
        ToolErrorKind::Timeout
    );
    assert_eq!(
        ToolError::Execution {
            tool: "t".into(),
            reason: "x".into()
        }
        .kind(),
        ToolErrorKind::Execution
    );
    assert_eq!(
        ToolError::InvalidArguments {
            tool: "t".into(),
            reason: "x".into()
        }
        .kind(),
        ToolErrorKind::InvalidArguments
    );
    assert_eq!(
        ToolError::UnknownTool("t".into()).kind(),
        ToolErrorKind::InvalidArguments
    );
}

// ---------------------------------------------------------------------------
// ToolRetryConfig

#[test]
fn default_retry_config_retries_timeout_and_network() {
    let cfg = ToolRetryConfig::default();
    assert_eq!(cfg.max_retries, 3);

    let timeout_err = ToolError::Timeout { tool: "t".into() };
    assert!(cfg.should_retry(&timeout_err, 0));
    assert!(cfg.should_retry(&timeout_err, 2));
    assert!(!cfg.should_retry(&timeout_err, 3)); // at max

    let exec_err = ToolError::Execution {
        tool: "t".into(),
        reason: "x".into(),
    };
    assert!(!cfg.should_retry(&exec_err, 0)); // Execution not in retry_on
}

#[test]
fn backoff_doubles_and_caps() {
    let cfg = ToolRetryConfig {
        initial_backoff: Duration::from_millis(100),
        backoff_multiplier: 2.0,
        max_backoff: Duration::from_millis(500),
        ..Default::default()
    };
    assert_eq!(cfg.backoff_for(0), Duration::from_millis(100));
    assert_eq!(cfg.backoff_for(1), Duration::from_millis(200));
    assert_eq!(cfg.backoff_for(2), Duration::from_millis(400));
    assert_eq!(cfg.backoff_for(3), Duration::from_millis(500)); // capped
    assert_eq!(cfg.backoff_for(10), Duration::from_millis(500)); // still capped
}

#[test]
fn per_tool_retry_config_override() {
    let m = mgr();
    let custom = ToolRetryConfig {
        max_retries: 5,
        ..Default::default()
    };
    m.set_retry_config("echo", custom);

    assert_eq!(m.retry_config("echo").max_retries, 5);
    assert_eq!(m.retry_config("other").max_retries, 3); // default
}

// ---------------------------------------------------------------------------
// Execute with retry

#[test]
fn execute_with_retry_succeeds_on_first_try() {
    futures_lite::future::block_on(async {
        let m = mgr();
        let request = req("a", "echo", vec![], ExecutionHint::Unspecified);
        let cfg = ToolRetryConfig::default();
        let result = m.execute_with_retry(&request, &cfg).await;
        assert!(result.is_ok());
    });
}

#[test]
fn execute_non_retriable_fails_immediately() {
    futures_lite::future::block_on(async {
        let m = ToolCallManager::new(SessionId::new());
        m.register(Arc::new(FailingTool {
            kind: ToolErrorKind::Execution,
        }));
        let request = req("a", "fail", vec![], ExecutionHint::Unspecified);
        let cfg = ToolRetryConfig::default(); // only retries Timeout/Network
        let result = m.execute_with_retry(&request, &cfg).await;
        assert!(matches!(result, Err(ToolError::Execution { .. })));
    });
}

// ---------------------------------------------------------------------------
// FailMode / WorkflowResult

#[test]
fn fail_mode_default_is_collect_all() {
    assert_eq!(FailMode::default(), FailMode::CollectAll);
}
