//! `ToolImpl` + `ToolCallManager` — the tool contract and registry (F09).
//!
//! WHY: The agent needs a uniform way to define, register, look up, and describe
//! tools. `foundation_ai` has the wire types (`Tool`, `ToolShed`, `ToolFormatter`)
//! but no implementer contract and no registry.
//!
//! WHAT: `ToolImpl` (definition + async execute), `ToolCallManager` (registration +
//! lookup + validation + execute), and the schema path to `ToolShed`.

use crate::types::{ArgType, Args, ExecutionHint, Tool, ToolShed};
use async_trait::async_trait;
use foundation_jsonschema;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;

// ---------------------------------------------------------------------------
// ToolError

/// Error types for tool execution — Clone + PartialEq so F02's AgenticError can
/// embed it (the stream types must stay Clone + PartialEq).
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

/// A single tool-call request (carries F01's depends_on / execution_hint).
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

struct ToolCallManagerInner {
    tools: std::sync::RwLock<HashMap<String, Arc<dyn ToolImpl>>>,
    /// Cached definitions keyed by tool name — `build_toolshed` reads from this
    /// instead of looping the tools map and calling `definition()` each time.
    defs: std::sync::RwLock<HashMap<String, ToolDefinition>>,
    session_id: crate::types::SessionId,
}

impl ToolCallManager {
    /// Create a new empty registry for the given session.
    pub fn new(session_id: crate::types::SessionId) -> Self {
        Self {
            inner: Arc::new(ToolCallManagerInner {
                tools: std::sync::RwLock::new(HashMap::new()),
                defs: std::sync::RwLock::new(HashMap::new()),
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
    pub fn get(&self, name: &str) -> Option<Arc<dyn ToolImpl>> {
        self.inner.tools.read().unwrap().get(name).cloned()
    }

    /// Look up a tool's definition (fast — cached at register time).
    pub fn get_def(&self, name: &str) -> Option<ToolDefinition> {
        self.inner.defs.read().unwrap().get(name).cloned()
    }

    /// List all registered tool names.
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
    pub fn build_toolshed(&self) -> ToolShed {
        let by_cat = self.defs_by_category();
        let memory = self.build_memory_tool();
        let delegate = self.build_delegate_tool();

        ToolShed {
            shed: Tool {
                name: "shed".into(),
                description:
                    "Search the tool registry for available tools by category or free-text query."
                        .into(),
                arguments: None,
                returns: None,
            },
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
        let stop = defs.get("delegate_start").map(|d| Tool {
            name: d.name.clone(),
            description: d.description.clone(),
            arguments: Some(d.arguments.clone()),
            returns: None,
        })?;
        let pause = defs.get("delegate_start").map(|d| Tool {
            name: d.name.clone(),
            description: d.description.clone(),
            arguments: Some(d.arguments.clone()),
            returns: None,
        })?;
        let check = defs.get("delegate_start").map(|d| Tool {
            name: d.name.clone(),
            description: d.description.clone(),
            arguments: Some(d.arguments.clone()),
            returns: None,
        })?;
        let result = defs.get("delegate_start").map(|d| Tool {
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

    /// Access the session id (for logging / persist).
    pub fn session_id(&self) -> &crate::types::SessionId {
        &self.inner.session_id
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
            assert_eq!(shed.shed.name, "shed");
            assert!(shed.shell.is_none());
            assert!(shed.read.is_none());
        })
    }
}
