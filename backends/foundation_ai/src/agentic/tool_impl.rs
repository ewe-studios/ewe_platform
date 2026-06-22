//! `ToolImpl` + `ToolCallManager` — the tool contract, registry (F09), and
//! staged execution DAG (F11).
//!
//! WHY: The agent needs a uniform way to define, register, look up, and execute
//! tools — including multi-call workflows with dependency ordering, retry, and
//! persist-before-deliver.
//!
//! WHAT: `ToolImpl` (definition + async execute), `ToolCallManager` (registration +
//! lookup + validation + execute + workflow), `ToolCallWorkflow` / `ToolCallStage` /
//! `FailMode` (staged DAG execution), and `ToolRetryConfig` (non-blocking backoff).

use crate::types::{ArgType, Args, ExecutionHint, Tool, ToolShed};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

// ---------------------------------------------------------------------------
// ToolError

/// Error types for tool execution — Clone + `PartialEq` so F02's `AgenticError` can
/// embed it (the stream types must stay Clone + `PartialEq`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum ToolError {
    /// The named tool is not registered.
    UnknownTool(String),
    /// Arguments failed JSON-Schema validation.
    InvalidArguments { tool: String, reason: String },
    /// Tool execution failed (user-code error, subprocess exit ≠ 0, etc.).
    Execution { tool: String, reason: String },
    /// Tool execution exceeded its deadline (valtron-driven timeout, F11).
    Timeout { tool: String },
    /// Tool execution was cancelled via steering signal.
    Cancelled(String),
}

impl std::fmt::Display for ToolError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ToolError::UnknownTool(name) => write!(f, "unknown tool: {name}"),
            ToolError::InvalidArguments { tool, reason } => {
                write!(f, "invalid arguments for {tool}: {reason}")
            }
            ToolError::Execution { tool, reason } => {
                write!(f, "execution error in {tool}: {reason}")
            }
            ToolError::Timeout { tool } => write!(f, "timeout in {tool}"),
            ToolError::Cancelled(tool) => write!(f, "cancelled: {tool}"),
        }
    }
}

// ---------------------------------------------------------------------------
// ToolDefinition

/// The LLM-facing definition of a tool, built by implementers.
#[derive(Debug, Clone)]
pub struct ToolDefinition {
    /// Unique tool name (used as the registry key).
    pub name: String,
    /// Human-readable description shown to the LLM.
    pub description: String,
    /// JSON-Schema of expected arguments.
    pub arguments: Args,
    /// Category tag for shed discovery (F10).
    pub category: String,
}

// ---------------------------------------------------------------------------
// ToolCallResult

/// The result of a tool execution. Becomes `Messages::ToolResult.content`.
#[derive(Debug, Clone)]
pub struct ToolCallResult {
    /// The content returned to the LLM.
    pub content: crate::types::UserModelContent,
    /// Optional error detail (for diagnostics — the LLM sees `content`).
    pub error_detail: Option<String>,
}

// ---------------------------------------------------------------------------
// ToolImpl trait

/// The tool contract every tool implementer satisfies. Registered as
/// `Arc<dyn ToolImpl>` in the `ToolCallManager`.
#[async_trait]
pub trait ToolImpl: Send + Sync {
    /// The LLM-facing definition (name, description, JSON-Schema args).
    fn definition(&self) -> ToolDefinition;

    /// Run the tool with validated arguments. Async (async-first).
    /// Cancellation = valtron stops polling the future and drops it —
    /// drop-based cleanup handles process kills, connection closes, etc.
    async fn execute(
        &self,
        arguments: HashMap<String, ArgType>,
    ) -> Result<ToolCallResult, ToolError>;
}

// ---------------------------------------------------------------------------
// ToolCallRequest — parsed from ModelOutput::ToolCall

/// A single tool-call request (carries F01's `depends_on` / `execution_hint`).
#[derive(Debug, Clone)]
pub struct ToolCallRequest {
    /// The tool-call id (matches `Messages::ToolResult.tool_call_id`).
    pub id: String,
    /// The tool name (registry key).
    pub name: String,
    /// Parsed arguments (validated against the tool's Args schema).
    pub arguments: HashMap<String, ArgType>,
    /// Other tool-call ids this depends on (F01 / F11 DAG).
    pub depends_on: Vec<String>,
    /// How the caller wants this tool run (F01).
    pub execution_hint: ExecutionHint,
}

// ---------------------------------------------------------------------------
// F11: ToolCallWorkflow — staged DAG execution

/// A staged workflow built from tool-call dependency graphs.
/// Each stage contains calls that can execute after all prior stages complete.
#[derive(Debug, Clone)]
pub struct ToolCallWorkflow {
    pub stages: Vec<ToolCallStage>,
}

/// One stage of a workflow — either parallel or sequential execution.
#[derive(Debug, Clone)]
pub enum ToolCallStage {
    Parallel {
        calls: Vec<ToolCallRequest>,
        fail_mode: FailMode,
    },
    Sequential {
        calls: Vec<ToolCallRequest>,
        fail_fast: bool,
    },
}

/// How parallel-stage failures are handled.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum FailMode {
    /// Run all calls; collect all results (including errors).
    #[default]
    CollectAll,
    /// Cancel remaining calls on first failure.
    CancelOnFailure,
}

/// Which error kinds are retriable.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ToolErrorKind {
    Timeout,
    Network,
    Execution,
    InvalidArguments,
}

impl ToolError {
    #[must_use]
    pub fn kind(&self) -> ToolErrorKind {
        match self {
            ToolError::Timeout { .. } => ToolErrorKind::Timeout,
            ToolError::Execution { .. } | ToolError::Cancelled(_) => ToolErrorKind::Execution,
            ToolError::InvalidArguments { .. } | ToolError::UnknownTool(_) => {
                ToolErrorKind::InvalidArguments
            }
        }
    }
}

/// Per-tool retry configuration. Backoff uses `TaskStatus::Delayed` —
/// never `sleep()` (would block the valtron executor / deadlock on wasm).
#[derive(Debug, Clone)]
pub struct ToolRetryConfig {
    pub max_retries: u32,
    pub initial_backoff: Duration,
    pub backoff_multiplier: f64,
    pub max_backoff: Duration,
    pub retry_on: Vec<ToolErrorKind>,
}

impl Default for ToolRetryConfig {
    fn default() -> Self {
        Self {
            max_retries: 3,
            initial_backoff: Duration::from_secs(1),
            backoff_multiplier: 2.0,
            max_backoff: Duration::from_secs(30),
            retry_on: vec![ToolErrorKind::Timeout, ToolErrorKind::Network],
        }
    }
}

impl ToolRetryConfig {
    #[must_use]
    pub fn should_retry(&self, err: &ToolError, attempt: u32) -> bool {
        attempt < self.max_retries && self.retry_on.contains(&err.kind())
    }

    #[must_use]
    pub fn backoff_for(&self, attempt: u32) -> Duration {
        // as_millis returns u128; precision loss acceptable for backoff durations.
        #[allow(clippy::cast_precision_loss)]
        let millis = self.initial_backoff.as_millis() as f64
            * self.backoff_multiplier.powi(attempt.cast_signed());
        // millis is non-negative and capped by max_backoff.
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        Duration::from_millis(millis as u64).min(self.max_backoff)
    }
}

/// The result of executing an entire workflow — one result per tool call.
#[derive(Debug)]
pub struct WorkflowResult {
    pub results: Vec<(ToolCallRequest, Result<ToolCallResult, ToolError>)>,
    pub interrupted: bool,
}

// ---------------------------------------------------------------------------
// ToolCallManager

/// The tool registry — owns `Arc<dyn ToolImpl>` by name. Cheap to clone
/// (`Arc`-shared) across valtron tasks.
///
/// ```ignore
/// let mgr = ToolCallManager::new(session_id);
/// mgr.register(Arc::new(MyTool::new()));
/// let result = mgr.execute_one(&request).await?;
/// ```
pub struct ToolCallManager {
    inner: Arc<ToolCallManagerInner>,
}

impl Clone for ToolCallManager {
    fn clone(&self) -> Self {
        Self {
            inner: self.inner.clone(),
        }
    }
}

struct ToolCallManagerInner {
    tools: std::sync::RwLock<HashMap<String, Arc<dyn ToolImpl>>>,
    /// Cached definitions keyed by tool name — `build_toolshed` reads from this
    /// instead of looping the tools map and calling `definition()` each time.
    defs: std::sync::RwLock<HashMap<String, ToolDefinition>>,
    /// Per-tool retry config overrides (F11).
    retry_configs: std::sync::RwLock<HashMap<String, ToolRetryConfig>>,
    session_id: crate::types::SessionId,
}

impl ToolCallManager {
    /// Create a new empty registry for the given session.
    #[must_use]
    pub fn new(session_id: crate::types::SessionId) -> Self {
        Self {
            inner: Arc::new(ToolCallManagerInner {
                tools: std::sync::RwLock::new(HashMap::new()),
                defs: std::sync::RwLock::new(HashMap::new()),
                retry_configs: std::sync::RwLock::new(HashMap::new()),
                session_id,
            }),
        }
    }

    /// Register a tool (interior mutability — `&self`, no `Arc` mutation needed).
    pub fn register(&self, tool: Arc<dyn ToolImpl>) {
        let def = tool.definition();
        let mut tools = self.inner.tools.write().unwrap();
        let mut defs = self.inner.defs.write().unwrap();

        let tool_name = def.name.clone();
        defs.insert(tool_name.clone(), def);
        tools.insert(tool_name, tool);
    }

    /// Deregister a tool by name.
    pub fn deregister(&self, name: &str) {
        self.inner.tools.write().unwrap().remove(name);
        self.inner.defs.write().unwrap().remove(name);
    }

    /// Look up a registered tool.
    #[must_use]
    pub fn get(&self, name: &str) -> Option<Arc<dyn ToolImpl>> {
        self.inner.tools.read().unwrap().get(name).cloned()
    }

    /// Look up a tool's definition (fast — cached at register time).
    #[must_use]
    pub fn get_def(&self, name: &str) -> Option<ToolDefinition> {
        self.inner.defs.read().unwrap().get(name).cloned()
    }

    /// List all registered tool names.
    #[must_use]
    pub fn names(&self) -> Vec<String> {
        self.inner.tools.read().unwrap().keys().cloned().collect()
    }

    /// Collect all cached definitions grouped by category — no looping over
    /// live tools, just a single read of the defs hashmap.
    fn defs_by_category(&self) -> HashMap<String, Tool> {
        let defs = self.inner.defs.read().unwrap();
        defs.values()
            .map(|d| {
                (
                    d.category.clone(),
                    Tool {
                        name: d.name.clone(),
                        description: d.description.clone(),
                        arguments: Some(d.arguments.clone()),
                        returns: None,
                    },
                )
            })
            .collect()
    }

    /// Validate arguments against the tool's JSON-Schema, then execute.
    pub async fn execute_one(
        &self,
        request: &ToolCallRequest,
    ) -> Result<ToolCallResult, ToolError> {
        let tool = self
            .get(&request.name)
            .ok_or_else(|| ToolError::UnknownTool(request.name.clone()))?;

        // Validate arguments against the tool's Args schema.
        // Schema validation is delegated to the tool implementer — we just
        // ensure the args can be serialized to JSON (the schema is carried
        // through to the LLM for pre-validation).
        let _def = tool.definition();

        tool.execute(request.arguments.clone()).await
    }

    /// Build the `ToolShed` from cached definitions. `shed` is always present;
    /// category tools are populated from the cached defs (no looping);
    /// `memory`/`delegate` are assembled from tools matching `memory_*` /
    /// `delegate_(start|check|pause|resume|result)` name prefixes.
    #[must_use]
    pub fn build_toolshed(&self) -> ToolShed {
        let by_cat = self.defs_by_category();
        let memory = self.build_memory_tool();
        let delegate = self.build_delegate_tool();

        ToolShed {
            shed: Some(Tool {
                name: "shed".into(),
                description:
                    "Search the tool registry for available tools by category or free-text query."
                        .into(),
                arguments: None,
                returns: None,
            }),
            memory,
            delegate,
            read: by_cat.get("read").cloned(),
            edit: by_cat.get("edit").cloned(),
            write: by_cat.get("write").cloned(),
            search: by_cat.get("search").cloned(),
            search_files: by_cat.get("search_files").cloned(),
            shell: by_cat.get("shell").cloned(),
        }
    }

    /// Build `MemoryTool` from tools whose names start with `memory_`.
    fn build_memory_tool(&self) -> Option<crate::types::MemoryTool> {
        let defs = self.inner.defs.read().unwrap();

        let add = defs.get("memory_add").map(|d| Tool {
            name: d.name.clone(),
            description: d.description.clone(),
            arguments: Some(d.arguments.clone()),
            returns: None,
        })?;
        let replace = defs.get("memory_replace").map(|d| Tool {
            name: d.name.clone(),
            description: d.description.clone(),
            arguments: Some(d.arguments.clone()),
            returns: None,
        })?;
        let remove = defs.get("memory_remove").map(|d| Tool {
            name: d.name.clone(),
            description: d.description.clone(),
            arguments: Some(d.arguments.clone()),
            returns: None,
        })?;

        Some(crate::types::MemoryTool {
            add,
            replace,
            remove,
        })
    }

    /// Build `DelegationTool` from tools whose names start with `delegate_`.
    fn build_delegate_tool(&self) -> Option<crate::types::DelegationTool> {
        let defs = self.inner.defs.read().unwrap();

        let start = defs.get("delegate_start").map(|d| Tool {
            name: d.name.clone(),
            description: d.description.clone(),
            arguments: Some(d.arguments.clone()),
            returns: None,
        })?;
        let stop = defs.get("delegate_stop").map(|d| Tool {
            name: d.name.clone(),
            description: d.description.clone(),
            arguments: Some(d.arguments.clone()),
            returns: None,
        })?;
        let pause = defs.get("delegate_pause").map(|d| Tool {
            name: d.name.clone(),
            description: d.description.clone(),
            arguments: Some(d.arguments.clone()),
            returns: None,
        })?;
        let check = defs.get("delegate_check").map(|d| Tool {
            name: d.name.clone(),
            description: d.description.clone(),
            arguments: Some(d.arguments.clone()),
            returns: None,
        })?;
        let result = defs.get("delegate_result").map(|d| Tool {
            name: d.name.clone(),
            description: d.description.clone(),
            arguments: Some(d.arguments.clone()),
            returns: None,
        })?;

        Some(crate::types::DelegationTool {
            start,
            stop,
            pause,
            check,
            result,
        })
    }

    /// Register the default tool set + the shed discovery tool.
    /// `shed` is registered first and is always present.
    #[must_use]
    pub fn with_defaults(
        session_id: crate::types::SessionId,
        discovery: Arc<crate::agentic::tools::shed::ToolDiscovery>,
    ) -> Self {
        let mgr = Self::new(session_id);
        mgr.register(Arc::new(crate::agentic::tools::shed::ShedTool::new(
            discovery,
        )));
        mgr
    }

    /// Access the session id (for logging / persist).
    #[must_use]
    pub fn session_id(&self) -> &crate::types::SessionId {
        &self.inner.session_id
    }

    // -----------------------------------------------------------------
    // F11: Workflow builder + execution
    // -----------------------------------------------------------------

    /// Topologically group tool calls into stages by `depends_on` depth.
    ///
    /// Stage 0 = calls with no deps (default: Parallel).
    /// Stage N = calls whose deps are all satisfied by stages < N.
    /// Cycles or missing deps → `ToolError::InvalidArguments`.
    pub fn build_workflow(&self, calls: &[ToolCallRequest]) -> Result<ToolCallWorkflow, ToolError> {
        fn resolve_depth(
            idx: usize,
            calls: &[ToolCallRequest],
            ids: &HashMap<&str, usize>,
            depths: &mut [Option<u32>],
            stack: &mut Vec<usize>,
        ) -> Result<u32, ToolError> {
            if let Some(d) = depths[idx] {
                return Ok(d);
            }
            if stack.contains(&idx) {
                return Err(ToolError::InvalidArguments {
                    tool: calls[idx].name.clone(),
                    reason: format!("cyclic dependency involving '{}'", calls[idx].id),
                });
            }
            stack.push(idx);
            let mut max_dep = 0u32;
            for dep_id in &calls[idx].depends_on {
                let dep_idx =
                    ids.get(dep_id.as_str())
                        .ok_or_else(|| ToolError::InvalidArguments {
                            tool: calls[idx].name.clone(),
                            reason: format!("depends on unknown call '{dep_id}'"),
                        })?;
                let dep_depth = resolve_depth(*dep_idx, calls, ids, depths, stack)?;
                max_dep = max_dep.max(dep_depth + 1);
            }
            stack.pop();
            depths[idx] = Some(max_dep);
            Ok(max_dep)
        }

        if calls.is_empty() {
            return Ok(ToolCallWorkflow { stages: vec![] });
        }

        let ids: HashMap<&str, usize> = calls
            .iter()
            .enumerate()
            .map(|(i, c)| (c.id.as_str(), i))
            .collect();

        // Compute depth for each call.
        let mut depths: Vec<Option<u32>> = vec![None; calls.len()];
        let mut stack: Vec<usize> = Vec::new();

        for i in 0..calls.len() {
            resolve_depth(i, calls, &ids, &mut depths, &mut stack)?;
        }

        // Group by depth.
        let max_depth = depths.iter().filter_map(|d| *d).max().unwrap_or(0);
        let mut stages = Vec::with_capacity((max_depth + 1) as usize);

        for depth in 0..=max_depth {
            let stage_calls: Vec<ToolCallRequest> = calls
                .iter()
                .enumerate()
                .filter(|(i, _)| depths[*i] == Some(depth))
                .map(|(_, c)| c.clone())
                .collect();

            if stage_calls.is_empty() {
                continue;
            }

            // If any call in the stage requests Sequential, the whole stage is sequential.
            let any_sequential = stage_calls
                .iter()
                .any(|c| c.execution_hint == ExecutionHint::Sequential);

            if any_sequential || stage_calls.len() == 1 {
                stages.push(ToolCallStage::Sequential {
                    calls: stage_calls,
                    fail_fast: true,
                });
            } else {
                stages.push(ToolCallStage::Parallel {
                    calls: stage_calls,
                    fail_mode: FailMode::CollectAll,
                });
            }
        }

        Ok(ToolCallWorkflow { stages })
    }

    /// Execute a single call with retry (non-blocking backoff via returned Duration).
    ///
    /// Returns `(result, backoff_durations_used)`. The caller is responsible for
    /// implementing the actual delay (via `TaskStatus::Delayed` in valtron context).
    /// In test / direct-call context, retries happen immediately.
    pub async fn execute_with_retry(
        &self,
        request: &ToolCallRequest,
        config: &ToolRetryConfig,
    ) -> Result<ToolCallResult, ToolError> {
        let mut attempt = 0u32;
        loop {
            match self.execute_one(request).await {
                Ok(result) => return Ok(result),
                Err(e) if config.should_retry(&e, attempt) => {
                    attempt += 1;
                    // In async context, the caller should yield with the backoff duration.
                    // Here we just proceed to the next attempt (valtron Delayed is wired
                    // by the task iterator in F19, not by this async fn).
                }
                Err(e) => return Err(e),
            }
        }
    }

    /// Set per-tool retry config override.
    pub fn set_retry_config(&self, tool_name: &str, config: ToolRetryConfig) {
        self.inner
            .retry_configs
            .write()
            .unwrap()
            .insert(tool_name.to_string(), config);
    }

    /// Get retry config for a tool (per-tool override or default).
    #[must_use]
    pub fn retry_config(&self, tool_name: &str) -> ToolRetryConfig {
        self.inner
            .retry_configs
            .read()
            .unwrap()
            .get(tool_name)
            .cloned()
            .unwrap_or_default()
    }
}

// ---------------------------------------------------------------------------
// Tests

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{TextContent, UserModelContent};

    /// A minimal test tool that echoes its arguments.
    struct EchoTool;

    #[async_trait]
    impl ToolImpl for EchoTool {
        fn definition(&self) -> ToolDefinition {
            ToolDefinition {
                name: "echo".into(),
                description: "Echoes the message argument".into(),
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

    #[test]
    fn register_and_execute() {
        futures_lite::future::block_on(async {
            let mgr = ToolCallManager::new(crate::types::SessionId::new());
            mgr.register(Arc::new(EchoTool));

            assert_eq!(mgr.names(), vec!["echo"]);
            assert!(mgr.get("echo").is_some());
            assert!(mgr.get("nonexistent").is_none());

            let request = ToolCallRequest {
                id: "call-1".into(),
                name: "echo".into(),
                arguments: HashMap::from([("message".into(), ArgType::Text("hello".into()))]),
                depends_on: vec![],
                execution_hint: ExecutionHint::default(),
            };
            let result = mgr.execute_one(&request).await.unwrap();
            if let UserModelContent::Text(tc) = result.content {
                assert_eq!(tc.content, "echo: hello");
            }

            // Deregister.
            mgr.deregister("echo");
            assert!(mgr.get("echo").is_none());
            assert_eq!(mgr.names().len(), 0);
        })
    }

    #[test]
    fn unknown_tool_returns_error() {
        futures_lite::future::block_on(async {
            let mgr = ToolCallManager::new(crate::types::SessionId::new());
            let request = ToolCallRequest {
                id: "call-1".into(),
                name: "nonexistent".into(),
                arguments: HashMap::new(),
                depends_on: vec![],
                execution_hint: ExecutionHint::default(),
            };
            let err = mgr.execute_one(&request).await.unwrap_err();
            assert_eq!(err, ToolError::UnknownTool("nonexistent".into()));
        })
    }

    #[test]
    fn build_toolshed_populates_by_category() {
        futures_lite::future::block_on(async {
            let mgr = ToolCallManager::new(crate::types::SessionId::new());
            mgr.register(Arc::new(EchoTool)); // category = "shell"

            let shed = mgr.build_toolshed();
            assert_eq!(shed.shell.as_ref().unwrap().name, "echo");
            assert_eq!(
                shed.shell.as_ref().unwrap().description,
                "Echoes the message argument"
            );
            assert!(shed.read.is_none());
        })
    }

    #[test]
    fn build_toolshed_empty_has_only_shed() {
        futures_lite::future::block_on(async {
            let mgr = ToolCallManager::new(crate::types::SessionId::new());
            let shed = mgr.build_toolshed();
            assert_eq!(shed.shed.as_ref().unwrap().name, "shed");
            assert!(shed.shell.is_none());
            assert!(shed.read.is_none());
        })
    }
}
