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
    /// Tools provisioned on every child sub-agent session (e.g. write, read).
    child_tools: Vec<Arc<dyn ToolImpl>>,
}

impl<D, M> AgentTool<D, M>
where
    D: DocumentStore + Default + 'static,
    M: MemoryStore + Default + 'static,
{
    /// Create a new agent tool.
    ///
    /// `child_tools` are registered on every sub-agent session so the child
    /// can produce output (at minimum: `write` and `read`).
    #[must_use]
    pub fn new(
        router: ProviderRouter,
        depth: u32,
        max_depth: u32,
        default_model: ModelId,
        output_base: String,
        user: UserId,
        child_tools: Vec<Arc<dyn ToolImpl>>,
    ) -> Self {
        Self {
            router,
            runs: Arc::new(Mutex::new(HashMap::new())),
            depth,
            max_depth,
            default_model,
            output_base,
            user,
            child_tools,
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
    fn start(&self, args: &HashMap<String, ArgType>) -> Result<ToolCallResult, ToolError> {
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

        // Runaway guard. Models emit integers inconsistently (u64 / i64 /
        // stringified), so accept all three spellings. `0` is rejected rather
        // than silently creating a sub-agent that can never take a step.
        //
        // Converted with `try_from` rather than `as`: on a 32-bit target an
        // `as` cast silently wraps, so a caller asking for a huge cap could be
        // handed a tiny one and the sub-agent would stop early for no visible
        // reason. An out-of-range value is rejected instead.
        let too_large = || ToolError::InvalidArguments {
            tool: TOOL.into(),
            reason: "'max_iterations' is larger than this platform supports".into(),
        };
        let max_iterations: Option<usize> = match args.get("max_iterations") {
            Some(ArgType::U64(n)) => Some(usize::try_from(*n).map_err(|_| too_large())?),
            Some(ArgType::I64(n)) if *n > 0 => Some(usize::try_from(*n).map_err(|_| too_large())?),
            Some(ArgType::I64(_)) => {
                return Err(ToolError::InvalidArguments {
                    tool: TOOL.into(),
                    reason: "'max_iterations' must be a positive integer".into(),
                })
            }
            Some(ArgType::Text(s)) => match s.parse::<usize>() {
                Ok(n) => Some(n),
                Err(_) => {
                    return Err(ToolError::InvalidArguments {
                        tool: TOOL.into(),
                        reason: format!("'max_iterations' is not a valid integer: {s:?}"),
                    })
                }
            },
            _ => None,
        };
        if max_iterations == Some(0) {
            return Err(ToolError::InvalidArguments {
                tool: TOOL.into(),
                reason: "'max_iterations' must be greater than zero".into(),
            });
        }

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

        // Apply the runaway cap to the child's agent loop. `max_inner_iterations`
        // is the per-turn step ceiling the loop actually enforces.
        if let Some(cap) = max_iterations {
            let mut cfg = crate::agentic::AgentConfig {
                primary_model: model.clone(),
                ..Default::default()
            };
            cfg.max_inner_iterations = cap;
            builder = builder.with_config(cfg);
        }

        // Instruct the sub-agent where to write its result.
        let prompt_text = format!(
            "{task}\n\nWrite your final result to: {output_location}\nWhen finished, include a brief 1-2 line summary in your final message."
        );

        let child_session = builder.build().map_err(|e| ToolError::Execution {
            tool: TOOL.into(),
            reason: format!("failed to build child session: {e}"),
        })?;

        // Provision the child session with the tools it needs to produce output
        // (at minimum: write, read — supplied by the caller at construction).
        for tool in &self.child_tools {
            child_session.tool_manager().register(Arc::clone(tool));
        }

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
    fn check(&self, args: &HashMap<String, ArgType>) -> Result<ToolCallResult, ToolError> {
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
    fn result(&self, args: &HashMap<String, ArgType>) -> Result<ToolCallResult, ToolError> {
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
    fn pause(&self, args: &HashMap<String, ArgType>) -> Result<ToolCallResult, ToolError> {
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
    fn resume(&self, args: &HashMap<String, ArgType>) -> Result<ToolCallResult, ToolError> {
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
    fn stop(&self, args: &HashMap<String, ArgType>) -> Result<ToolCallResult, ToolError> {
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
                        if let Messages::Assistant {
                            content: crate::types::ModelOutput::Text(ref tc),
                            ..
                        } = message
                        {
                            run.summary = Some(truncate_summary(&tc.content));
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
                Some(Stream::Init | Stream::Ignore | Stream::Wait | Stream::Delayed(_)) => {
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
            "start" => self.start(&arguments),
            "check" => self.check(&arguments),
            "result" => self.result(&arguments),
            "pause" => self.pause(&arguments),
            "resume" => self.resume(&arguments),
            "stop" => self.stop(&arguments),
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

/// Schema for the commands whose only argument is a delegation id.
///
/// `check`, `result` and `stop` differ solely in the `command` constant, so the
/// shape is built once rather than repeated three times.
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
/// `child_tools` are provisioned on every sub-agent session the tool spawns.
/// At minimum this should include `write` and `read` so the sub-agent can
/// produce output.
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
    child_tools: Vec<Arc<dyn ToolImpl>>,
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
        child_tools,
    );
    manager.register(Arc::new(tool));
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use std::collections::HashMap;
    use std::sync::Arc;

    use foundation_db::{MemoryDocumentStore, MemoryStorage};

    use crate::agentic::memory_store::KvMemoryStore;
    use crate::agentic::testing::{mock_text, MockModelProvider, MockTool};
    use crate::agentic::tool_impl::{ToolCallManager, ToolError, ToolImpl};
    use crate::agentic::UserId;
    use crate::types::routable_provider::ProviderRouter;
    use crate::types::{
        ArgType, ModelId, SessionId, Tool, UserModelContent,
    };

    use super::*;

    type D = MemoryDocumentStore;
    type M = KvMemoryStore<MemoryStorage>;

    // ------------------------------------------------------------------
    // Helpers
    // ------------------------------------------------------------------

    fn cmd(command: &str, pairs: &[(&str, &str)]) -> HashMap<String, ArgType> {
        let mut m = HashMap::new();
        m.insert("command".to_string(), ArgType::Text(command.to_string()));
        for (k, v) in pairs {
            m.insert((*k).to_string(), ArgType::Text((*v).to_string()));
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
        result: &crate::agentic::tool_impl::ToolCallResult,
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
            let check = futures_lite::future::block_on(
                t.execute(cmd("check", &[("id", id)])),
            )
            .expect("check");
            let check_json = as_json(&check);
            let state = check_json["state"].as_str().unwrap_or("unknown");
            match state {
                "Done" => return Some(check_json),
                "Failed" => return Some(check_json),
                _ => std::thread::sleep(std::time::Duration::from_millis(10)),
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
        futures_lite::future::block_on(async {
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
        futures_lite::future::block_on(async {
            let err = test_tool().execute(cmd("start", &[])).await.unwrap_err();
            match err {
                ToolError::InvalidArguments { reason, .. } => {
                    assert!(reason.contains("'task'"));
                }
                other => panic!("expected InvalidArguments, got {other:?}"),
            }
        });
    }

    #[test]
    fn start_empty_task_errors() {
        futures_lite::future::block_on(async {
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
        futures_lite::future::block_on(async {
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
        futures_lite::future::block_on(async {
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

    #[foundation_core::valtron::valtron_test]
    fn start_returns_immediately_with_id() {
        let mut mock = MockModelProvider::new();
        mock.on_any(vec![mock_text("sub-agent result")]);
        let t = tool_router(mock.into_router());

        let result = futures_lite::future::block_on(
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

    #[foundation_core::valtron::valtron_test]
    fn full_cycle_start_check_result() {
        let mut mock = MockModelProvider::new();
        mock.on_any(vec![mock_text("task completed successfully")]);
        let t = tool_router(mock.into_router());

        let start = futures_lite::future::block_on(
            t.execute(cmd("start", &[("task", "write hello")])),
        )
        .expect("start");
        let id = as_json(&start)["id"].as_str().unwrap().to_string();

        let done = poll_until_done(&t, &id, 200);
        assert!(done.is_some(), "sub-agent never completed after 2s");
        let done_json = done.unwrap();
        assert_eq!(done_json["state"], "Done");
        assert!(!done_json["location"].as_str().unwrap().is_empty());

        let result = futures_lite::future::block_on(
            t.execute(cmd("result", &[("id", &id)])),
        )
        .expect("result");
        let rj = as_json(&result);
        assert!(!rj["location"].as_str().unwrap().is_empty());
        assert!(rj["summary"].as_str().unwrap().len() > 0);

        // Cleanup: after result, id is freed → check fails
        let err = futures_lite::future::block_on(
            t.execute(cmd("check", &[("id", &id)])),
        )
        .unwrap_err();
        assert!(matches!(err, ToolError::InvalidArguments { .. }));
    }

    // ------------------------------------------------------------------
    // pause / resume
    // ------------------------------------------------------------------

    #[foundation_core::valtron::valtron_test]
    fn pause_reports_paused_on_check() {
        let mut mock = MockModelProvider::new();
        mock.on_any(vec![mock_text("ok")]);
        let t = tool_router(mock.into_router());

        let start = futures_lite::future::block_on(
            t.execute(cmd("start", &[("task", "stuff")])),
        )
        .expect("start");
        let id = as_json(&start)["id"].as_str().unwrap().to_string();

        // Pause
        let pause = futures_lite::future::block_on(
            t.execute(cmd("pause", &[("id", &id)])),
        )
        .expect("pause");
        assert_eq!(as_json(&pause)["status"], "paused");

        // Check → Paused
        let check = futures_lite::future::block_on(
            t.execute(cmd("check", &[("id", &id)])),
        )
        .expect("check");
        assert_eq!(as_json(&check)["state"], "Paused");

        // Resume
        let resume = futures_lite::future::block_on(
            t.execute(cmd("resume", &[("id", &id)])),
        )
        .expect("resume");
        assert_eq!(as_json(&resume)["status"], "resumed");
    }

    #[foundation_core::valtron::valtron_test]
    fn resume_idempotent_when_not_paused() {
        let mut mock = MockModelProvider::new();
        mock.on_any(vec![mock_text("ok")]);
        let t = tool_router(mock.into_router());

        let start = futures_lite::future::block_on(
            t.execute(cmd("start", &[("task", "stuff")])),
        )
        .expect("start");
        let id = as_json(&start)["id"].as_str().unwrap().to_string();

        // Resume without prior pause — succeeds
        let resume = futures_lite::future::block_on(
            t.execute(cmd("resume", &[("id", &id)])),
        )
        .expect("resume");
        assert_eq!(as_json(&resume)["status"], "resumed");
    }

    // ------------------------------------------------------------------
    // stop
    // ------------------------------------------------------------------

    #[foundation_core::valtron::valtron_test]
    fn stop_removes_run_without_keep_session() {
        let mut mock = MockModelProvider::new();
        mock.on_any(vec![mock_text("reply")]);
        let t = tool_router(mock.into_router());

        let start = futures_lite::future::block_on(
            t.execute(cmd("start", &[("task", "stuff")])),
        )
        .expect("start");
        let id = as_json(&start)["id"].as_str().unwrap().to_string();

        let stop = futures_lite::future::block_on(
            t.execute(cmd("stop", &[("id", &id)])),
        )
        .expect("stop");
        assert_eq!(as_json(&stop)["status"], "stopped");

        // Run removed → check errors
        let err = futures_lite::future::block_on(
            t.execute(cmd("check", &[("id", &id)])),
        )
        .unwrap_err();
        assert!(matches!(err, ToolError::InvalidArguments { .. }));
    }

    #[foundation_core::valtron::valtron_test]
    fn stop_with_keep_session_preserves_run() {
        let mut mock = MockModelProvider::new();
        mock.on_any(vec![mock_text("reply")]);
        let t = tool_router(mock.into_router());

        let start = futures_lite::future::block_on(
            t.execute(cmd(
                "start",
                &[("task", "stuff"), ("keep_session", "true")],
            )),
        )
        .expect("start");
        let id = as_json(&start)["id"].as_str().unwrap().to_string();

        // Stop with keep_session
        futures_lite::future::block_on(
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
        let check = futures_lite::future::block_on(
            t.execute(cmd("check", &[("id", &id)])),
        )
        .expect("keep_session=true must preserve the run, so check resolves the id");
        let state = as_json(&check)["state"].as_str().unwrap_or("").to_string();
        assert!(
            !state.is_empty(),
            "check must report some state for a preserved run, got: {state:?}"
        );
    }

    #[foundation_core::valtron::valtron_test]
    fn stop_without_keep_session_drops_the_run() {
        // The contrast case that gives the test above its meaning: without
        // `keep_session`, `stop` removes the run and the id stops resolving.
        let mut mock = MockModelProvider::new();
        mock.on_any(vec![mock_text("reply")]);
        let t = tool_router(mock.into_router());

        let start = futures_lite::future::block_on(
            t.execute(cmd("start", &[("task", "stuff")])),
        )
        .expect("start");
        let id = as_json(&start)["id"].as_str().unwrap().to_string();

        futures_lite::future::block_on(t.execute(cmd("stop", &[("id", &id)])))
            .expect("stop");

        let err = futures_lite::future::block_on(
            t.execute(cmd("check", &[("id", &id)])),
        )
        .expect_err("without keep_session the run must be gone");
        assert!(matches!(err, ToolError::InvalidArguments { .. }));
    }

    // ------------------------------------------------------------------
    // child tool provisioning
    // ------------------------------------------------------------------

    #[foundation_core::valtron::valtron_test]
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

        let result = futures_lite::future::block_on(
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

        let shed = manager.build_toolshed();
        assert!(
            shed.tools.iter().any(|t| t.name() == "agent"),
            "toolshed should contain agent: {shed:?}"
        );
    }

    // ------------------------------------------------------------------
    // start() — optional argument branches
    // ------------------------------------------------------------------

    /// Build a `start` arg map with arbitrary extra typed args.
    fn start_args(pairs: Vec<(&str, ArgType)>) -> HashMap<String, ArgType> {
        let mut m = HashMap::new();
        m.insert("command".to_string(), ArgType::Text("start".into()));
        m.insert("task".to_string(), ArgType::Text("do stuff".into()));
        for (k, v) in pairs {
            m.insert(k.to_string(), v);
        }
        m
    }

    #[foundation_core::valtron::valtron_test]
    fn start_accepts_an_explicit_model() {
        let mut mock = MockModelProvider::new();
        mock.on_any(vec![mock_text("ok")]);
        let t = tool_router(mock.into_router());

        // Exercises the `Some(Text)` arm of the `model` lookup rather than
        // falling back to the parent session's default.
        let out = futures_lite::future::block_on(t.execute(start_args(vec![(
            "model",
            ArgType::Text("mock".into()),
        )])))
        .expect("start with explicit model");
        assert_eq!(as_json(&out)["status"], "running");
    }

    #[foundation_core::valtron::valtron_test]
    fn start_ignores_an_empty_model_and_uses_the_default() {
        let mut mock = MockModelProvider::new();
        mock.on_any(vec![mock_text("ok")]);
        let t = tool_router(mock.into_router());

        // An empty string must NOT become ModelId::Name("") — the guard falls
        // through to the parent model, which is known to resolve.
        let out = futures_lite::future::block_on(
            t.execute(start_args(vec![("model", ArgType::Text(String::new()))])),
        )
        .expect("empty model falls back to the default");
        assert_eq!(as_json(&out)["status"], "running");
    }

    #[foundation_core::valtron::valtron_test]
    fn start_accepts_a_system_prompt() {
        let mut mock = MockModelProvider::new();
        mock.on_any(vec![mock_text("ok")]);
        let t = tool_router(mock.into_router());

        let out = futures_lite::future::block_on(t.execute(start_args(vec![(
            "system",
            ArgType::Text("be terse".into()),
        )])))
        .expect("start with system prompt");
        assert_eq!(as_json(&out)["status"], "running");
    }

    #[foundation_core::valtron::valtron_test]
    fn start_accepts_max_iterations_in_every_numeric_spelling() {
        // Models emit integers inconsistently (u64 / i64 / stringified), so all
        // three must parse rather than silently dropping the runaway guard.
        for arg in [
            ArgType::U64(7),
            ArgType::I64(7),
            ArgType::Text("7".into()),
        ] {
            let mut mock = MockModelProvider::new();
            mock.on_any(vec![mock_text("ok")]);
            let t = tool_router(mock.into_router());

            let out = futures_lite::future::block_on(
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
        futures_lite::future::block_on(async {
            for bad in [ArgType::U64(0), ArgType::I64(0), ArgType::I64(-3)] {
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
        futures_lite::future::block_on(async {
            let t = test_tool();
            let err = t
                .execute(start_args(vec![(
                    "max_iterations",
                    ArgType::Text("lots".into()),
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

    #[foundation_core::valtron::valtron_test]
    fn start_returns_distinct_ids_per_delegation() {
        // Two delegations must not collide in the runs map, or `check` on one
        // would report the other.
        let mut mock = MockModelProvider::new();
        mock.on_any(vec![mock_text("ok")]);
        let t = tool_router(mock.into_router());

        let a = futures_lite::future::block_on(
            t.execute(cmd("start", &[("task", "first")])),
        )
        .expect("start a");
        let b = futures_lite::future::block_on(
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

    #[foundation_core::valtron::valtron_test]
    fn result_while_paused_reports_not_done() {
        // Pausing pins the status at Paused, which makes the "not done yet"
        // branch of `result` deterministic (no scheduler race).
        let mut mock = MockModelProvider::new();
        mock.on_any(vec![mock_text("ok")]);
        let t = tool_router(mock.into_router());

        let start = futures_lite::future::block_on(
            t.execute(cmd("start", &[("task", "stuff")])),
        )
        .expect("start");
        let id = as_json(&start)["id"].as_str().unwrap().to_string();

        futures_lite::future::block_on(t.execute(cmd("pause", &[("id", &id)])))
            .expect("pause");

        let err = futures_lite::future::block_on(
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

    #[foundation_core::valtron::valtron_test]
    fn paused_run_is_not_advanced_by_check() {
        // While paused, drain_run must return early without pulling the stream,
        // so repeated checks keep reporting Paused rather than drifting.
        let mut mock = MockModelProvider::new();
        mock.on_any(vec![mock_text("ok")]);
        let t = tool_router(mock.into_router());

        let start = futures_lite::future::block_on(
            t.execute(cmd("start", &[("task", "stuff")])),
        )
        .expect("start");
        let id = as_json(&start)["id"].as_str().unwrap().to_string();

        futures_lite::future::block_on(t.execute(cmd("pause", &[("id", &id)])))
            .expect("pause");

        for _ in 0..3 {
            let c = futures_lite::future::block_on(
                t.execute(cmd("check", &[("id", &id)])),
            )
            .expect("check");
            assert_eq!(as_json(&c)["state"], "Paused");
        }
    }

    #[foundation_core::valtron::valtron_test]
    fn resume_after_pause_lets_the_run_progress_again() {
        let mut mock = MockModelProvider::new();
        mock.on_any(vec![mock_text("done text")]);
        let t = tool_router(mock.into_router());

        let start = futures_lite::future::block_on(
            t.execute(cmd("start", &[("task", "stuff")])),
        )
        .expect("start");
        let id = as_json(&start)["id"].as_str().unwrap().to_string();

        futures_lite::future::block_on(t.execute(cmd("pause", &[("id", &id)])))
            .expect("pause");
        futures_lite::future::block_on(t.execute(cmd("resume", &[("id", &id)])))
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

    #[foundation_core::valtron::valtron_test]
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
        let out = futures_lite::future::block_on(
            t.execute(cmd("start", &[("task", "stuff")])),
        )
        .expect("depth 4 of max 5 must be allowed");
        assert_eq!(as_json(&out)["status"], "running");
    }

    #[test]
    fn depth_cap_of_zero_refuses_all_delegation() {
        futures_lite::future::block_on(async {
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
}
