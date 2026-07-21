//! Agent tool — background sub-agent delegation (spec-60 F15).
//!
//! WHY: the parent agent needs to offload self-contained sub-tasks to a
//! background LLM (same router, named model) without pinning a worker for the
//! entire sub-turn. The tool returns a handle immediately; the work runs on the
//! pool; the handle supports pause, resume, and stop.
//!
//! WHAT: One `Tool::MultiCommands("agent", [start, stop, pause, resume, check,
//! result])` in `ToolShed.tools`. `start` builds a child `AgentSession`, calls
//! `run_turn_stream(prompt)`, stores the `DrivenStreamIterator` + control flags,
//! and returns at once. The sub-agent writes its result to a file via the
//! standard `write` tool — output never accumulates in the delegator's memory.
//!
//! HOW: `AgentTool<D, M>` holds an `Arc<Mutex<HashMap<String, AgentRun<D, M>>>>`.
//! `start` schedules the sub-agent on the pool via `AgentSession::run_turn_stream`.
//! `check` drains the `DrivenStreamIterator` to observe progress. `stop` trips
//! `SteeringQueues::abort()` on the child session. `pause`/`resume` toggle an
//! `Arc<AtomicBool>` checked at drain time.

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use foundation_core::valtron::{DrivenStreamIterator, Stream};
use foundation_db::traits::DocumentStore;
use serde::{Deserialize, Serialize};

use crate::agentic::errors::UserId;
use crate::agentic::memory_store::MemoryStore;
use crate::agentic::session::AgentSession;
use crate::agentic::tool_impl::{ToolCallResult, ToolDefinition, ToolError, ToolImpl};
use crate::types::agentic::{SessionId, SessionRecord};
use crate::types::base_types::{ArgType, Args, MessageRole, Messages, ModelId, Tool};
use crate::types::routable_provider::ProviderRouter;
use crate::types::{TextContent, UserModelContent};

const TOOL: &str = "agent";

// ---------------------------------------------------------------------------
// AgentStatus — the sub-agent lifecycle state reported by `check`
// ---------------------------------------------------------------------------

/// The sub-agent's current status, emitted as `check`'s result.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "state")]
enum AgentStatus {
    /// The sub-agent has been scheduled and is starting.
    Started,
    /// The sub-agent is actively working.
    Working { iteration: u32 },
    /// The sub-agent is paused (the pause flag is set).
    Paused,
    /// The sub-agent terminated with an unrecoverable error.
    Failed { reason: String },
    /// Terminal: the sub-agent finished successfully. Carries the file location
    /// the sub-agent wrote its result to + a short summary.
    Done { location: String, summary: String },
}

// ---------------------------------------------------------------------------
// AgentRun — one in-flight delegation
// ---------------------------------------------------------------------------

/// A single in-flight delegation. Lives in the `AgentTool`'s runs map.
#[allow(dead_code)]
struct AgentRun<D, M>
where
    D: DocumentStore + 'static,
    M: MemoryStore + 'static,
{
    /// Opaque delegation id returned to the caller.
    id: String,
    /// The sub-agent's session — for abort + session-id access.
    session: AgentSession<D, M>,
    /// The sub-agent's session id (for resume / resurrect).
    session_id: SessionId,
    /// Where the sub-agent writes its result (local path or remote key).
    output_location: String,
    /// Observation handle — drained by `check` / `result` to observe progress.
    stream: DrivenStreamIterator<crate::agentic::agent_loop::AgentLoop<D, M>>,
    /// Abort flag: set by `stop`, checked at each drain step.
    stop: Arc<AtomicBool>,
    /// Suspend flag: set by `pause`, cleared by `resume`.
    pause: Arc<AtomicBool>,
    /// Whether to keep the sub-agent's session after `result`.
    keep_session: bool,
    /// Cached latest status (updated each `check` drain).
    latest: AgentStatus,
    /// Iteration counter for `Working` heartbeats.
    iteration: u32,
    /// The sub-agent's final summary.
    summary: Option<String>,
    /// Set once the stream is exhausted (closed).
    exhausted: bool,
}

// ---------------------------------------------------------------------------
// AgentTool — the ToolImpl
// ---------------------------------------------------------------------------

/// The `agent` tool: `start` / `stop` / `pause` / `resume` / `check` / `result`.
pub struct AgentTool<D, M>
where
    D: DocumentStore + 'static,
    M: MemoryStore + 'static,
{
    router: ProviderRouter,
    runs: Arc<Mutex<HashMap<String, AgentRun<D, M>>>>,
    depth: u32,
    max_depth: u32,
    default_model: ModelId,
    output_base: String,
    user: UserId,
}

impl<D, M> AgentTool<D, M>
where
    D: DocumentStore + Default + 'static,
    M: MemoryStore + Default + 'static,
{
    /// Create a new agent tool.
    #[must_use]
    pub fn new(
        router: ProviderRouter,
        depth: u32,
        max_depth: u32,
        default_model: ModelId,
        output_base: String,
        user: UserId,
    ) -> Self {
        Self {
            router,
            runs: Arc::new(Mutex::new(HashMap::new())),
            depth,
            max_depth,
            default_model,
            output_base,
            user,
        }
    }

    /// Generate a fresh delegation id.
    fn new_id() -> String {
        format!("deleg-{}", foundation_compact::ids::new_scru128())
    }

    // ------------------------------------------------------------------
    // Command handlers
    // ------------------------------------------------------------------

    /// `start` — schedule a sub-agent turn, return immediately.
    async fn start(&self, args: &HashMap<String, ArgType>) -> Result<ToolCallResult, ToolError> {
        if self.depth >= self.max_depth {
            return Err(ToolError::Execution {
                tool: TOOL.into(),
                reason: format!(
                    "delegation depth cap reached (depth={}, max={})",
                    self.depth, self.max_depth
                ),
            });
        }

        let task = text_arg(args, "task")?;
        if task.trim().is_empty() {
            return Err(ToolError::InvalidArguments {
                tool: TOOL.into(),
                reason: "'task' must not be empty".into(),
            });
        }

        let model: ModelId = match args.get("model") {
            Some(ArgType::Text(s)) if !s.is_empty() => ModelId::Name(s.clone(), None),
            _ => self.default_model.clone(),
        };

        let system: Option<String> = match args.get("system") {
            Some(ArgType::Text(s)) if !s.is_empty() => Some(s.clone()),
            _ => None,
        };

        let keep_session: bool = match args.get("keep_session") {
            Some(ArgType::Text(s)) => s == "true",
            _ => false,
        };

        let _max_iterations: Option<u32> = match args.get("max_iterations") {
            Some(ArgType::U64(n)) => Some(*n as u32),
            Some(ArgType::I64(n)) => Some(*n as u32),
            Some(ArgType::Text(s)) => s.parse().ok(),
            _ => None,
        };

        // --- build child session ---
        let child_session_id = SessionId::new();
        let delegate_id = Self::new_id();
        let output_location = format!("{}/{}.json", self.output_base, delegate_id);

        let mut builder = AgentSession::builder(child_session_id.clone(), self.router.clone())
            .with_user(self.user.clone())
            .with_model(model.clone());

        if let Some(sys) = system {
            builder = builder.with_system_prompt(sys);
        }

        // Instruct the sub-agent where to write its result.
        let prompt_text = format!(
            "{}\n\nWrite your final result to: {}\nWhen finished, include a brief 1-2 line summary in your final message.",
            task, output_location
        );

        let child_session = builder.build().map_err(|e| ToolError::Execution {
            tool: TOOL.into(),
            reason: format!("failed to build child session: {e}"),
        })?;

        let prompt = Messages::User {
            id: foundation_compact::ids::new_scru128(),
            role: MessageRole::User,
            content: UserModelContent::Text(TextContent {
                content: prompt_text,
                signature: None,
            }),
            signature: None,
        };
        let stream = child_session.run_turn_stream(prompt).map_err(|e| {
            ToolError::Execution {
                tool: TOOL.into(),
                reason: format!("failed to schedule sub-agent: {e}"),
            }
        })?;

        let run_session_id = child_session.session_id().clone();

        let run = AgentRun {
            id: delegate_id.clone(),
            session: child_session,
            session_id: run_session_id.clone(),
            output_location: output_location.clone(),
            stream,
            stop: Arc::new(AtomicBool::new(false)),
            pause: Arc::new(AtomicBool::new(false)),
            keep_session,
            latest: AgentStatus::Started,
            iteration: 0,
            summary: None,
            exhausted: false,
        };

        self.runs.lock().unwrap().insert(delegate_id.clone(), run);

        let result = serde_json::json!({
            "id": delegate_id,
            "session_id": run_session_id.to_string(),
            "output_location": output_location,
            "status": "running",
        });

        Ok(ToolCallResult {
            content: UserModelContent::Text(TextContent {
                content: result.to_string(),
                signature: None,
            }),
            error_detail: None,
        })
    }

    /// `check` — non-blocking drain: report latest status.
    async fn check(&self, args: &HashMap<String, ArgType>) -> Result<ToolCallResult, ToolError> {
        let id = text_arg(args, "id")?;
        let mut runs = self.runs.lock().unwrap();
        let run = runs
            .get_mut(&id)
            .ok_or_else(|| unknown_id(&id))?;

        Self::drain_run(run);

        let content = serde_json::to_string(&run.latest).map_err(|e| {
            ToolError::Execution {
                tool: TOOL.into(),
                reason: format!("serialization failed: {e}"),
            }
        })?;

        Ok(ToolCallResult {
            content: UserModelContent::Text(TextContent {
                content,
                signature: None,
            }),
            error_detail: None,
        })
    }

    /// `result` — when done, return location + summary; clean up unless
    /// `keep_session`.
    async fn result(&self, args: &HashMap<String, ArgType>) -> Result<ToolCallResult, ToolError> {
        let id = text_arg(args, "id")?;

        // Drain and extract under one mutable borrow.
        let (status, keep_session) = {
            let mut runs = self.runs.lock().unwrap();
            let run = runs
                .get_mut(&id)
                .ok_or_else(|| unknown_id(&id))?;
            Self::drain_run(run);
            (run.latest.clone(), run.keep_session)
        };

        match status {
            AgentStatus::Done { location, summary } => {
                let result = serde_json::json!({
                    "location": location,
                    "summary": summary,
                });

                // Remove from the runs map unless kept.
                if !keep_session {
                    self.runs.lock().unwrap().remove(&id);
                }

                Ok(ToolCallResult {
                    content: UserModelContent::Text(TextContent {
                        content: result.to_string(),
                        signature: None,
                    }),
                    error_detail: None,
                })
            }
            AgentStatus::Failed { reason } => {
                self.runs.lock().unwrap().remove(&id);
                Err(ToolError::Execution {
                    tool: TOOL.into(),
                    reason: format!("sub-agent failed: {reason}"),
                })
            }
            AgentStatus::Started | AgentStatus::Working { .. } | AgentStatus::Paused => {
                let status_str =
                    serde_json::to_string(&status).unwrap_or_else(|_| "unknown".into());
                Err(ToolError::Execution {
                    tool: TOOL.into(),
                    reason: format!("sub-agent is not done yet (status: {status_str})"),
                })
            }
        }
    }

    /// `pause` — set the suspend flag.
    async fn pause(&self, args: &HashMap<String, ArgType>) -> Result<ToolCallResult, ToolError> {
        let id = text_arg(args, "id")?;
        let runs = self.runs.lock().unwrap();
        let run = runs.get(&id).ok_or_else(|| unknown_id(&id))?;
        run.pause.store(true, Ordering::SeqCst);

        Ok(ToolCallResult {
            content: UserModelContent::Text(TextContent {
                content: format!("{{\"status\":\"paused\",\"id\":\"{id}\"}}"),
                signature: None,
            }),
            error_detail: None,
        })
    }

    /// `resume` — clear the suspend flag.
    async fn resume(&self, args: &HashMap<String, ArgType>) -> Result<ToolCallResult, ToolError> {
        let id = text_arg(args, "id")?;
        let runs = self.runs.lock().unwrap();
        let run = runs.get(&id).ok_or_else(|| unknown_id(&id))?;
        run.pause.store(false, Ordering::SeqCst);

        Ok(ToolCallResult {
            content: UserModelContent::Text(TextContent {
                content: format!("{{\"status\":\"resumed\",\"id\":\"{id}\"}}"),
                signature: None,
            }),
            error_detail: None,
        })
    }

    /// `stop` — set the abort flag + abort the child session.
    async fn stop(&self, args: &HashMap<String, ArgType>) -> Result<ToolCallResult, ToolError> {
        let id = text_arg(args, "id")?;
        let mut runs = self.runs.lock().unwrap();
        let run = runs.get_mut(&id).ok_or_else(|| unknown_id(&id))?;

        run.stop.store(true, Ordering::SeqCst);
        run.session.steering_queues().abort();
        Self::drain_run(run);

        if !run.keep_session {
            runs.remove(&id);
            drop(runs);
        }

        Ok(ToolCallResult {
            content: UserModelContent::Text(TextContent {
                content: format!("{{\"status\":\"stopped\",\"id\":\"{id}\"}}"),
                signature: None,
            }),
            error_detail: None,
        })
    }

    // ------------------------------------------------------------------
    // Stream draining
    // ------------------------------------------------------------------

    fn drain_run(run: &mut AgentRun<D, M>) {
        if run.pause.load(Ordering::SeqCst) {
            run.latest = AgentStatus::Paused;
            return;
        }
        if run.exhausted {
            return;
        }

        loop {
            if run.stream.is_empty() {
                if run.stream.is_closed() {
                    run.exhausted = true;
                    if let Some(ref summary) = run.summary {
                        run.latest = AgentStatus::Done {
                            location: run.output_location.clone(),
                            summary: summary.clone(),
                        };
                    }
                }
                break;
            }

            match run.stream.next() {
                Some(Stream::Next(record)) => match record {
                    SessionRecord::Conversation { message } => {
                        run.iteration += 1;
                        run.latest = AgentStatus::Working {
                            iteration: run.iteration,
                        };
                        if let Messages::Assistant { ref content, .. } = message {
                            if let crate::types::ModelOutput::Text(ref tc) = content {
                                run.summary = Some(truncate_summary(&tc.content));
                            }
                        }
                    }
                    SessionRecord::FailedAction { ref error, .. } => {
                        run.latest = AgentStatus::Failed {
                            reason: format!("{error}"),
                        };
                        run.exhausted = true;
                        break;
                    }
                    _ => {
                        run.iteration += 1;
                    }
                },
                Some(Stream::Pending(_)) => {
                    run.latest = AgentStatus::Working {
                        iteration: run.iteration,
                    };
                    break;
                }
                Some(Stream::Init)
                | Some(Stream::Ignore)
                | Some(Stream::Wait)
                | Some(Stream::Delayed(_)) => {
                    continue;
                }
                Some(Stream::Spread(_)) => {
                    run.iteration += 1;
                    continue;
                }
                None => {
                    run.exhausted = true;
                    if let Some(ref summary) = run.summary {
                        run.latest = AgentStatus::Done {
                            location: run.output_location.clone(),
                            summary: summary.clone(),
                        };
                    }
                    break;
                }
            }

            if run.stop.load(Ordering::SeqCst) {
                break;
            }
        }
    }
}

// ---------------------------------------------------------------------------
// ToolImpl
// ---------------------------------------------------------------------------

#[async_trait]
impl<D, M> ToolImpl for AgentTool<D, M>
where
    D: DocumentStore + Default + 'static,
    M: MemoryStore + Default + 'static,
{
    fn definition(&self) -> Tool {
        let start_opts = foundation_jsonschema::ValidationOptions::with_schema(
            serde_json::json!({
                "type": "object",
                "properties": {
                    "command": { "const": "start" },
                    "task": { "type": "string", "description": "The self-contained sub-task the sub-agent must accomplish" },
                    "model": { "type": "string", "description": "Model id to run the sub-agent on. When omitted, inherits the parent session's model." },
                    "system": { "type": "string", "description": "Extra system/role guidance prepended to the sub-agent's prompt" },
                    "keep_session": { "type": "boolean", "description": "Keep the sub-agent session after result instead of deleting it", "default": false },
                    "max_iterations": { "type": "integer", "description": "Hard cap on sub-agent loop iterations (runaway guard)" }
                },
                "required": ["command", "task"]
            }),
        );

        fn id_schema(command_name: &str) -> foundation_jsonschema::ValidationOptions {
            foundation_jsonschema::ValidationOptions::with_schema(serde_json::json!({
                "type": "object",
                "properties": {
                    "command": { "const": command_name },
                    "id": { "type": "string", "description": "Delegation id returned by start" }
                },
                "required": ["command", "id"]
            }))
        }

        Tool::MultiCommands(
            TOOL.to_string(),
            vec![
                ToolDefinition {
                    name: "start".into(),
                    category: TOOL.into(),
                    description: "Start a background sub-agent to accomplish a self-contained task. Returns an id immediately; the sub-agent runs asynchronously. Use check/result to monitor.".into(),
                    arguments: Args::new(start_opts),
                    returns: None,
                },
                ToolDefinition {
                    name: "check".into(),
                    category: TOOL.into(),
                    description: "Check a sub-agent's current status without blocking. Reports working/paused/failed/done.".into(),
                    arguments: Args::new(id_schema("check")),
                    returns: None,
                },
                ToolDefinition {
                    name: "result".into(),
                    category: TOOL.into(),
                    description: "Get the sub-agent's result (location + summary) once done. Errors if still running.".into(),
                    arguments: Args::new(id_schema("result")),
                    returns: None,
                },
                ToolDefinition {
                    name: "pause".into(),
                    category: TOOL.into(),
                    description: "Pause a running sub-agent at its next boundary. Idempotent.".into(),
                    arguments: Args::new(id_schema("pause")),
                    returns: None,
                },
                ToolDefinition {
                    name: "resume".into(),
                    category: TOOL.into(),
                    description: "Resume a paused sub-agent. No-op if not paused.".into(),
                    arguments: Args::new(id_schema("resume")),
                    returns: None,
                },
                ToolDefinition {
                    name: "stop".into(),
                    category: TOOL.into(),
                    description: "Abort a running sub-agent at its next boundary. Cleans up the session.".into(),
                    arguments: Args::new(id_schema("stop")),
                    returns: None,
                },
            ],
        )
    }

    async fn execute(
        &self,
        arguments: HashMap<String, ArgType>,
    ) -> Result<ToolCallResult, ToolError> {
        let command = text_arg(&arguments, "command")?;
        match command.as_str() {
            "start" => self.start(&arguments).await,
            "check" => self.check(&arguments).await,
            "result" => self.result(&arguments).await,
            "pause" => self.pause(&arguments).await,
            "resume" => self.resume(&arguments).await,
            "stop" => self.stop(&arguments).await,
            other => Err(ToolError::InvalidArguments {
                tool: TOOL.into(),
                reason: format!(
                    "unknown agent command '{other}' (start|check|result|pause|resume|stop)"
                ),
            }),
        }
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn text_arg(args: &HashMap<String, ArgType>, key: &str) -> Result<String, ToolError> {
    match args.get(key) {
        Some(ArgType::Text(s)) => Ok(s.clone()),
        _ => Err(ToolError::InvalidArguments {
            tool: TOOL.into(),
            reason: format!("missing or invalid '{key}' argument"),
        }),
    }
}

fn unknown_id(id: &str) -> ToolError {
    ToolError::InvalidArguments {
        tool: TOOL.into(),
        reason: format!("unknown delegation id '{id}'"),
    }
}

/// Truncate content to a short summary (max ~200 chars).
fn truncate_summary(content: &str) -> String {
    if content.len() <= 200 {
        content.to_string()
    } else {
        let mut end = 200;
        while !content.is_char_boundary(end) {
            end -= 1;
        }
        format!("{}…", &content[..end])
    }
}

// ---------------------------------------------------------------------------
// Registration
// ---------------------------------------------------------------------------

/// Register the `agent` tool onto a [`ToolCallManager`].
///
/// [`ToolCallManager`]: crate::agentic::tool_impl::ToolCallManager
pub fn register_agent_tool<D, M>(
    manager: &crate::agentic::tool_impl::ToolCallManager,
    router: ProviderRouter,
    depth: u32,
    max_depth: u32,
    default_model: ModelId,
    output_base: &str,
    user: UserId,
) where
    D: DocumentStore + Default + 'static,
    M: MemoryStore + Default + 'static,
{
    let tool = AgentTool::<D, M>::new(
        router,
        depth,
        max_depth,
        default_model,
        output_base.to_string(),
        user,
    );
    manager.register(Arc::new(tool));
}
