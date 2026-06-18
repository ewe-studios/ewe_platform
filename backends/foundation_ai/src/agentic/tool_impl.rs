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
    session_id: crate::types::SessionId,
}

impl ToolCallManager {
    /// Create a new empty registry for the given session.
    pub fn new(session_id: crate::types::SessionId) -> Self {
        Self {
            inner: Arc::new(ToolCallManagerInner {
                tools: std::sync::RwLock::new(HashMap::new()),
                session_id,
            }),
        }
    }

    /// Register a tool (interior mutability — `&self`, no `Arc` mutation needed).
    pub fn register(&self, tool: Arc<dyn ToolImpl>) {
        let def = tool.definition();
        self.inner.tools.write().unwrap().insert(def.name, tool);
    }

    /// Deregister a tool by name.
    pub fn deregister(&self, name: &str) {
        self.inner.tools.write().unwrap().remove(name);
    }

    /// Look up a registered tool.
    pub fn get(&self, name: &str) -> Option<Arc<dyn ToolImpl>> {
        self.inner.tools.read().unwrap().get(name).cloned()
    }

    /// List all registered tool names.
    pub fn names(&self) -> Vec<String> {
        self.inner.tools.read().unwrap().keys().cloned().collect()
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
        let def = tool.definition();
        let schema = &def.arguments.schema;
        // Skip validation for empty schema.
        if !schema.is_null() && schema.get("type").is_some() {
            if let Ok(validator) = foundation_jsonschema::Validator::compile(schema) {
                // Convert ArgType map to serde_json::Value for validation.
                let args_json =
                    serde_json::to_value(&request.arguments).map_err(|e| ToolError::Execution {
                        tool: request.name.clone(),
                        reason: format!("failed to serialize args: {e}"),
                    })?;
                if let Err(errs) = validator.validate(&args_json) {
                    return Err(ToolError::InvalidArguments {
                        tool: request.name.clone(),
                        reason: errs
                            .into_iter()
                            .map(|e| e.message.unwrap_or_default())
                            .collect::<Vec<_>>()
                            .join("; "),
                    });
                }
            }
        }

        tool.execute(request.arguments.clone()).await
    }

    /// Build the `ToolShed` from registered tools. Returns `None` if no tools
    /// are registered (the caller passes `None` as `tools_shed`).
    pub fn build_toolshed(&self) -> Option<ToolShed> {
        let tools = self.inner.tools.read().unwrap();
        if tools.is_empty() {
            return None;
        }

        let mut read = None;
        let mut edit = None;
        let mut write = None;
        let mut search = None;
        let mut search_files = None;
        let mut shell = None;

        for (_name, tool) in tools.iter() {
            let def = tool.definition();
            let t = Tool {
                name: def.name.clone(),
                description: def.description.clone(),
                arguments: Some(def.arguments.clone()),
                returns: None,
            };
            match def.category.as_str() {
                "read" => read = Some(t),
                "edit" => edit = Some(t),
                "write" => write = Some(t),
                "search" => search = Some(t),
                "search_files" => search_files = Some(t),
                "shell" => shell = Some(t),
                _ => {}
            }
        }

        Some(
            ToolShed::default()
                .with_read(read)
                .with_edit(edit)
                .with_write(write)
                .with_search(search)
                .with_search_files(search_files)
                .with_shell(shell),
        )
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
                    ArgType::String(s) => Some(s.clone()),
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

    #[futures_lite::future::block_on]
    async fn register_and_execute() {
        let mgr = ToolCallManager::new(crate::types::SessionId::new());
        mgr.register(Arc::new(EchoTool));

        assert_eq!(mgr.names(), vec!["echo"]);
        assert!(mgr.get("echo").is_some());
        assert!(mgr.get("nonexistent").is_none());

        let request = ToolCallRequest {
            id: "call-1".into(),
            name: "echo".into(),
            arguments: HashMap::from([("message".into(), ArgType::String("hello".into()))]),
            depends_on: vec![],
            execution_hint: ExecutionHint::default(),
        };
        let result = mgr.execute_one(&request).await.unwrap();
        let UserModelContent::Text(tc) = result.content;
        assert_eq!(tc.content, "echo: hello");

        // Deregister.
        mgr.deregister("echo");
        assert!(mgr.get("echo").is_none());
        assert_eq!(mgr.names().len(), 0);
    }

    #[test]
    fn tool_impl_register_and_execute() {
        register_and_execute();
    }

    #[futures_lite::future::block_on]
    async fn unknown_tool_returns_error() {
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
    }

    #[test]
    fn unknown_tool_error() {
        unknown_tool_returns_error();
    }

    #[futures_lite::future::block_on]
    async fn build_toolshed_populates_by_category() {
        let mgr = ToolCallManager::new(crate::types::SessionId::new());
        mgr.register(Arc::new(EchoTool)); // category = "shell"

        let shed = mgr.build_toolshed().expect("should have tools");
        assert_eq!(shed.shell.name, "echo");
        assert_eq!(shed.shell.description, "Echoes the message argument");
        // Default stub for unregistered categories.
        assert!(shed.read.description.contains("stub"));
    }

    #[test]
    fn toolshed_from_registry() {
        build_toolshed_populates_by_category();
    }

    #[futures_lite::future::block_on]
    async fn build_toolshed_empty_returns_none() {
        let mgr = ToolCallManager::new(crate::types::SessionId::new());
        assert!(mgr.build_toolshed().is_none());
    }

    #[test]
    fn toolshed_empty_is_none() {
        build_toolshed_empty_returns_none();
    }
}
