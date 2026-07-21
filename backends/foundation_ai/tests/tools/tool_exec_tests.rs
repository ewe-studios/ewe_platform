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

// ---------------------------------------------------------------------------
// Additional execution coverage (tool_impl.rs was 75%): retry-then-succeed,
// per-tool retry config round-trip, and get_def lookup.

/// A tool that fails `fail_n` times (Timeout, retriable) then succeeds.
struct FlakyTool {
    fail_n: u32,
    attempt: std::sync::atomic::AtomicU32,
}

#[async_trait]
impl ToolImpl for FlakyTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "flaky".into(),
            description: "Fails then succeeds".into(),
            arguments: Args::from_value(serde_json::json!({})),
            category: "shell".into(),
        }
    }

    async fn execute(
        &self,
        _arguments: HashMap<String, ArgType>,
    ) -> Result<ToolCallResult, ToolError> {
        use std::sync::atomic::Ordering;
        let n = self.attempt.fetch_add(1, Ordering::SeqCst);
        if n < self.fail_n {
            Err(ToolError::Timeout {
                tool: "flaky".into(),
            })
        } else {
            Ok(ToolCallResult {
                content: foundation_ai::types::UserModelContent::Text(
                    foundation_ai::types::TextContent {
                        content: "ok".into(),
                        signature: None,
                    },
                ),
                error_detail: None,
            })
        }
    }
}

#[test]
fn execute_with_retry_succeeds_after_transient_failures() {
    futures_lite::future::block_on(async {
        let m = ToolCallManager::new(SessionId::new());
        m.register(Arc::new(FlakyTool {
            fail_n: 2,
            attempt: std::sync::atomic::AtomicU32::new(0),
        }));
        // Default retries Timeout up to its max — 2 failures then success.
        let request = req("a", "flaky", vec![], ExecutionHint::Unspecified);
        let cfg = ToolRetryConfig::default();
        let result = m.execute_with_retry(&request, &cfg).await;
        assert!(
            result.is_ok(),
            "retriable failures below the cap must eventually succeed: {result:?}"
        );
    });
}

#[test]
fn per_tool_retry_config_round_trips() {
    let m = mgr();
    let custom = ToolRetryConfig {
        max_retries: 7,
        ..ToolRetryConfig::default()
    };
    m.set_retry_config("echo", custom);
    assert_eq!(
        m.retry_config("echo").max_retries,
        7,
        "a set per-tool retry config must be read back"
    );
    // An unset tool falls back to the default.
    assert_eq!(
        m.retry_config("unknown").max_retries,
        ToolRetryConfig::default().max_retries
    );
}

#[test]
fn get_def_returns_registered_definition() {
    let m = mgr();
    let def = m.get_def("echo").expect("echo is registered");
    assert_eq!(def.name, "echo");
    assert!(m.get_def("nope").is_none(), "unknown tool has no definition");
}

/// Matrix 10.6 — a panicking tool is contained at the boundary: execute_one
/// returns a ToolError::Execution rather than unwinding into the caller.
struct PanicTool;

#[async_trait]
impl ToolImpl for PanicTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "boom".into(),
            description: "Panics".into(),
            arguments: Args::from_value(serde_json::json!({})),
            category: "shell".into(),
        }
    }
    async fn execute(
        &self,
        _arguments: HashMap<String, ArgType>,
    ) -> Result<ToolCallResult, ToolError> {
        panic!("intentional tool panic");
    }
}

#[test]
fn panicking_tool_is_contained_as_error() {
    futures_lite::future::block_on(async {
        let m = ToolCallManager::new(SessionId::new());
        m.register(Arc::new(PanicTool));
        let request = req("a", "boom", vec![], ExecutionHint::Unspecified);
        let result = m.execute_one(&request).await;
        assert!(
            matches!(result, Err(ToolError::Execution { .. })),
            "a panicking tool must be contained as a ToolError, not unwind: {result:?}"
        );
    });
}

/// Matrix 4.8 — malformed tool arguments produce a clean error, not a panic.
/// The framework delegates schema validation to the tool; a tool that requires
/// a specific argument rejects a call missing it with InvalidArguments.
struct StrictTool;

#[async_trait]
impl ToolImpl for StrictTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "strict".into(),
            description: "Requires a 'path' argument".into(),
            arguments: Args::from_value(serde_json::json!({
                "type": "object",
                "properties": { "path": { "type": "string" } },
                "required": ["path"]
            })),
            category: "read".into(),
        }
    }
    async fn execute(
        &self,
        arguments: HashMap<String, ArgType>,
    ) -> Result<ToolCallResult, ToolError> {
        if !arguments.contains_key("path") {
            return Err(ToolError::InvalidArguments {
                tool: "strict".into(),
                reason: "missing required argument 'path'".into(),
            });
        }
        Ok(ToolCallResult {
            content: foundation_ai::types::UserModelContent::Text(
                foundation_ai::types::TextContent { content: "ok".into(), signature: None },
            ),
            error_detail: None,
        })
    }
}

#[test]
fn malformed_tool_arguments_error_cleanly() {
    futures_lite::future::block_on(async {
        let m = ToolCallManager::new(SessionId::new());
        m.register(Arc::new(StrictTool));
        // Call with EMPTY args — the required 'path' is missing.
        let request = req("a", "strict", vec![], ExecutionHint::Unspecified);
        let result = m.execute_one(&request).await;
        assert!(
            matches!(result, Err(ToolError::InvalidArguments { .. })),
            "missing required args must be a clean InvalidArguments error: {result:?}"
        );
    });
}
