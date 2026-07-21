//! Delegation tools — `delegate_{start,stop,pause,check,result}` (spec-60 F15).
//!
//! WHY: the `ToolShed.delegate` slot was declared but shipped no `ToolImpl`, so
//! the agent could not hand a sub-task to a child agent.
//!
//! WHAT: a bounded [`DelegationManager`] + the five verb `ToolImpl`s the
//! `build_toolshed` delegate assembler looks for. `delegate_start` spawns a
//! child `AgentSession` (via an injected spawner) and runs one turn to
//! completion, storing the result under a delegation id; the other verbs query
//! or annotate that handle. A depth cap prevents unbounded recursion.
//!
//! HOW: the tools are generic over the session's `D: DocumentStore` + `M:
//! MemoryStore` and share one `Arc<DelegationManager<D, M>>`. The spawner
//! captures whatever a child needs (router, stores, config), so this module
//! stays decoupled from session construction.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use foundation_db::traits::DocumentStore;

use crate::agentic::memory_store::MemoryStore;
use crate::agentic::session::AgentSession;
use crate::agentic::tool_impl::{ToolCallResult, ToolDefinition, ToolError, ToolImpl};
use crate::types::base_types::Args;
use crate::types::{
    ArgType, MessageRole, Messages, ModelOutput, SessionId, SessionRecord, TextContent,
    UserModelContent,
};

// ---------------------------------------------------------------------------
// Shared helpers
// ---------------------------------------------------------------------------

fn text_arg(args: &HashMap<String, ArgType>, key: &str, tool: &str) -> Result<String, ToolError> {
    match args.get(key) {
        Some(ArgType::Text(s)) => Ok(s.clone()),
        _ => Err(ToolError::InvalidArguments {
            tool: tool.into(),
            reason: format!("missing or invalid '{key}' argument"),
        }),
    }
}

fn text_result(content: String) -> ToolCallResult {
    ToolCallResult {
        content: UserModelContent::Text(TextContent {
            content,
            signature: None,
        }),
        error_detail: None,
    }
}

fn exec_err(tool: &str, reason: impl Into<String>) -> ToolError {
    ToolError::Execution {
        tool: tool.into(),
        reason: reason.into(),
    }
}

fn user_msg(text: &str) -> Messages {
    Messages::User {
        id: foundation_compact::ids::new_scru128(),
        role: MessageRole::User,
        content: UserModelContent::Text(TextContent {
            content: text.to_string(),
            signature: None,
        }),
        signature: None,
    }
}

fn assistant_text(records: &[SessionRecord]) -> String {
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
        .collect::<Vec<_>>()
        .join("\n")
}

// ---------------------------------------------------------------------------
// DelegationManager
// ---------------------------------------------------------------------------

/// Lifecycle state of a delegated sub-task.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DelegationStatus {
    /// The child turn finished successfully.
    Completed,
    /// The child turn returned an error.
    Failed,
    /// Marked stopped by `delegate_stop` (best-effort; run-to-completion model).
    Stopped,
}

impl DelegationStatus {
    fn as_str(&self) -> &'static str {
        match self {
            Self::Completed => "completed",
            Self::Failed => "failed",
            Self::Stopped => "stopped",
        }
    }
}

/// A completed (or failed) delegation's record.
#[derive(Debug, Clone)]
pub struct DelegationHandle {
    pub task: String,
    pub status: DelegationStatus,
    pub result: String,
}

/// Spawns a child `AgentSession` for a sub-task at a given depth.
type Spawner<D, M> = Arc<dyn Fn(SessionId, u32) -> AgentSession<D, M> + Send + Sync>;

/// Bounded manager for delegated sub-tasks. Holds a spawner (captures the
/// router/stores/config a child needs), a depth cap, and the completed handles.
pub struct DelegationManager<D, M> {
    spawner: Spawner<D, M>,
    depth: u32,
    max_depth: u32,
    handles: Mutex<HashMap<String, DelegationHandle>>,
}

impl<D: DocumentStore + 'static, M: MemoryStore + 'static> DelegationManager<D, M> {
    /// Create a manager at `depth` with children capped at `max_depth`.
    #[must_use]
    pub fn new(spawner: Spawner<D, M>, depth: u32, max_depth: u32) -> Self {
        Self {
            spawner,
            depth,
            max_depth,
            handles: Mutex::new(HashMap::new()),
        }
    }

    fn get(&self, id: &str) -> Option<DelegationHandle> {
        self.handles.lock().expect("handles poisoned").get(id).cloned()
    }
}

// ---------------------------------------------------------------------------
// delegate_start
// ---------------------------------------------------------------------------

/// Hand a sub-task to a child agent and run it to completion.
pub struct DelegateStartTool<D, M> {
    manager: Arc<DelegationManager<D, M>>,
}

impl<D: DocumentStore + 'static, M: MemoryStore + 'static> DelegateStartTool<D, M> {
    #[must_use]
    pub fn new(manager: Arc<DelegationManager<D, M>>) -> Self {
        Self { manager }
    }
}

#[async_trait]
impl<D: DocumentStore + 'static, M: MemoryStore + 'static> ToolImpl for DelegateStartTool<D, M> {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "delegate_start".into(),
            description: "Hand a self-contained sub-task to a child agent, which runs it to \
                          completion. Returns a delegation id. Args: task (required)."
                .into(),
            arguments: Args::new(
                foundation_jsonschema::scheme::object()
                    .required("task", foundation_jsonschema::scheme::string().min_len(1))
                    .build(),
            ),
            category: "delegate".into(),
        }
    }

    async fn execute(
        &self,
        arguments: HashMap<String, ArgType>,
    ) -> Result<ToolCallResult, ToolError> {
        let task = text_arg(&arguments, "task", "delegate_start")?;

        // Depth cap: a child at max_depth cannot delegate further.
        if self.manager.depth >= self.manager.max_depth {
            return Err(exec_err(
                "delegate_start",
                format!(
                    "delegation depth cap reached ({}/{})",
                    self.manager.depth, self.manager.max_depth
                ),
            ));
        }

        let id = foundation_compact::ids::new_scru128().to_string();
        let child = (self.manager.spawner)(SessionId::new(), self.manager.depth + 1);

        let handle = match child.run_turn(user_msg(&task)) {
            Ok(records) => {
                let _ = child.end();
                DelegationHandle {
                    task: task.clone(),
                    status: DelegationStatus::Completed,
                    result: assistant_text(&records),
                }
            }
            Err(e) => DelegationHandle {
                task: task.clone(),
                status: DelegationStatus::Failed,
                result: e.to_string(),
            },
        };

        let status = handle.status.as_str();
        self.manager
            .handles
            .lock()
            .expect("handles poisoned")
            .insert(id.clone(), handle);

        Ok(text_result(format!("delegation {id} {status}")))
    }
}

// ---------------------------------------------------------------------------
// delegate_check
// ---------------------------------------------------------------------------

/// Report a delegation's status.
pub struct DelegateCheckTool<D, M> {
    manager: Arc<DelegationManager<D, M>>,
}

impl<D: DocumentStore + 'static, M: MemoryStore + 'static> DelegateCheckTool<D, M> {
    #[must_use]
    pub fn new(manager: Arc<DelegationManager<D, M>>) -> Self {
        Self { manager }
    }
}

#[async_trait]
impl<D: DocumentStore + 'static, M: MemoryStore + 'static> ToolImpl for DelegateCheckTool<D, M> {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "delegate_check".into(),
            description: "Check a delegation's status. Args: id (required).".into(),
            arguments: Args::new(
                foundation_jsonschema::scheme::object()
                    .required("id", foundation_jsonschema::scheme::string().min_len(1))
                    .build(),
            ),
            category: "delegate".into(),
        }
    }

    async fn execute(
        &self,
        arguments: HashMap<String, ArgType>,
    ) -> Result<ToolCallResult, ToolError> {
        let id = text_arg(&arguments, "id", "delegate_check")?;
        match self.manager.get(&id) {
            Some(h) => Ok(text_result(format!("delegation {id} {}", h.status.as_str()))),
            None => Err(exec_err("delegate_check", format!("unknown delegation id '{id}'"))),
        }
    }
}

// ---------------------------------------------------------------------------
// delegate_result
// ---------------------------------------------------------------------------

/// Return a completed delegation's output text.
pub struct DelegateResultTool<D, M> {
    manager: Arc<DelegationManager<D, M>>,
}

impl<D: DocumentStore + 'static, M: MemoryStore + 'static> DelegateResultTool<D, M> {
    #[must_use]
    pub fn new(manager: Arc<DelegationManager<D, M>>) -> Self {
        Self { manager }
    }
}

#[async_trait]
impl<D: DocumentStore + 'static, M: MemoryStore + 'static> ToolImpl for DelegateResultTool<D, M> {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "delegate_result".into(),
            description: "Retrieve a completed delegation's result. Args: id (required).".into(),
            arguments: Args::new(
                foundation_jsonschema::scheme::object()
                    .required("id", foundation_jsonschema::scheme::string().min_len(1))
                    .build(),
            ),
            category: "delegate".into(),
        }
    }

    async fn execute(
        &self,
        arguments: HashMap<String, ArgType>,
    ) -> Result<ToolCallResult, ToolError> {
        let id = text_arg(&arguments, "id", "delegate_result")?;
        match self.manager.get(&id) {
            Some(h) => Ok(text_result(h.result)),
            None => Err(exec_err("delegate_result", format!("unknown delegation id '{id}'"))),
        }
    }
}

// ---------------------------------------------------------------------------
// delegate_stop / delegate_pause (best-effort; run-to-completion model)
// ---------------------------------------------------------------------------

/// Mark a delegation stopped. Since `delegate_start` runs to completion, this is
/// best-effort: it annotates the handle (a no-op if already finished).
pub struct DelegateStopTool<D, M> {
    manager: Arc<DelegationManager<D, M>>,
}

impl<D: DocumentStore + 'static, M: MemoryStore + 'static> DelegateStopTool<D, M> {
    #[must_use]
    pub fn new(manager: Arc<DelegationManager<D, M>>) -> Self {
        Self { manager }
    }
}

#[async_trait]
impl<D: DocumentStore + 'static, M: MemoryStore + 'static> ToolImpl for DelegateStopTool<D, M> {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "delegate_stop".into(),
            description: "Stop a delegation. Args: id (required).".into(),
            arguments: Args::new(
                foundation_jsonschema::scheme::object()
                    .required("id", foundation_jsonschema::scheme::string().min_len(1))
                    .build(),
            ),
            category: "delegate".into(),
        }
    }

    async fn execute(
        &self,
        arguments: HashMap<String, ArgType>,
    ) -> Result<ToolCallResult, ToolError> {
        let id = text_arg(&arguments, "id", "delegate_stop")?;
        let mut handles = self.manager.handles.lock().expect("handles poisoned");
        match handles.get_mut(&id) {
            Some(h) => {
                h.status = DelegationStatus::Stopped;
                Ok(text_result(format!("delegation {id} stopped")))
            }
            None => Err(exec_err("delegate_stop", format!("unknown delegation id '{id}'"))),
        }
    }
}

/// Pause a delegation. Run-to-completion means there is nothing to suspend, so
/// this reports that clearly rather than silently pretending to pause.
pub struct DelegatePauseTool<D, M> {
    manager: Arc<DelegationManager<D, M>>,
}

impl<D: DocumentStore + 'static, M: MemoryStore + 'static> DelegatePauseTool<D, M> {
    #[must_use]
    pub fn new(manager: Arc<DelegationManager<D, M>>) -> Self {
        Self { manager }
    }
}

#[async_trait]
impl<D: DocumentStore + 'static, M: MemoryStore + 'static> ToolImpl for DelegatePauseTool<D, M> {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "delegate_pause".into(),
            description: "Pause a delegation. Args: id (required).".into(),
            arguments: Args::new(
                foundation_jsonschema::scheme::object()
                    .required("id", foundation_jsonschema::scheme::string().min_len(1))
                    .build(),
            ),
            category: "delegate".into(),
        }
    }

    async fn execute(
        &self,
        arguments: HashMap<String, ArgType>,
    ) -> Result<ToolCallResult, ToolError> {
        let id = text_arg(&arguments, "id", "delegate_pause")?;
        match self.manager.get(&id) {
            Some(_) => Ok(text_result(format!(
                "delegation {id} runs to completion; pause is a no-op"
            ))),
            None => Err(exec_err("delegate_pause", format!("unknown delegation id '{id}'"))),
        }
    }
}

// ---------------------------------------------------------------------------
// Registration
// ---------------------------------------------------------------------------

/// Register the five delegation verbs onto a [`ToolCallManager`]; `build_toolshed`
/// assembles the `delegate` `ToolShed` slot from their `delegate_*` names.
///
/// [`ToolCallManager`]: crate::agentic::tool_impl::ToolCallManager
pub fn register_delegate_tools<D, M>(
    tool_manager: &crate::agentic::tool_impl::ToolCallManager,
    manager: Arc<DelegationManager<D, M>>,
) where
    D: DocumentStore + 'static,
    M: MemoryStore + 'static,
{
    tool_manager.register(Arc::new(DelegateStartTool::new(manager.clone())));
    tool_manager.register(Arc::new(DelegateStopTool::new(manager.clone())));
    tool_manager.register(Arc::new(DelegatePauseTool::new(manager.clone())));
    tool_manager.register(Arc::new(DelegateCheckTool::new(manager.clone())));
    tool_manager.register(Arc::new(DelegateResultTool::new(manager)));
}
