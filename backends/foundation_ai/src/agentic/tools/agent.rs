//! Agent tool — background sub-agent delegation (spec-60 F15).
//!
//! WHY: the parent agent needs to offload self-contained sub-tasks to a
//! background LLM (same router, named model) without pinning a worker for the
//! entire sub-turn. The tool returns a handle immediately; the work runs on the
//! pool; the handle supports pause, resume, and stop.
//!
//! WHAT: One `Tool::MultiCommands("agent", [start, stop, pause, resume, check,
//! result])` in `ToolDeclarations.tools`. `start` builds a child `AgentSession`, calls
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
use std::future::Future;
use std::pin::Pin;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use foundation_core::valtron::Stream;
use foundation_db::traits::AsyncDocumentStore;
use foundation_db::StorageResult;
use serde::{Deserialize, Serialize};

use crate::agentic::errors::UserId;
use crate::agentic::memory_store::MemoryStore;
use crate::agentic::session::AgentSession;
use crate::agentic::tool_impl::{ToolArgs, ToolCallResult, ToolError, ToolImpl};
use crate::agentic::toolshed::ToolShed;
use crate::types::agentic::{SessionId, SessionRecord};
use crate::types::base_types::{
    Args, MessageRole, Messages, ModelId, Tool, ToolArguments, ToolDefinition,
};
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
// SessionStoreFactory — fresh stores for each sub-agent
// ---------------------------------------------------------------------------

/// The future a [`SessionStoreFactory`] returns.
pub type StoreFuture<D, M> = Pin<Box<dyn Future<Output = StorageResult<(D, M)>> + Send + 'static>>;

/// Builds a fresh document store and memory store for one sub-agent.
///
/// Each `start` gets its own stores. Building them is async, so a store
/// that opens a database (Turso, libsql) awaits its constructor instead of
/// blocking in a `Default` impl.
pub trait SessionStoreFactory<D, M>: Send + Sync + 'static {
    /// Build the stores for one sub-agent session.
    fn create(&self) -> StoreFuture<D, M>;
}

/// Factory for stores whose `Default` is cheap and doesn't block (the
/// in-memory ones: `MemoryDocumentStore`, `KvMemoryStore<MemoryStorage>`).
#[derive(Debug, Clone, Copy, Default)]
pub struct DefaultStores;

impl<D, M> SessionStoreFactory<D, M> for DefaultStores
where
    D: Default + Send + 'static,
    M: Default + Send + 'static,
{
    fn create(&self) -> StoreFuture<D, M> {
        Box::pin(async { Ok((D::default(), M::default())) })
    }
}

/// Any `Fn() -> impl Future<Output = StorageResult<(D, M)>>` is a factory,
/// e.g. `|| async { Ok((open_doc_store().await?, open_memory().await?)) }`.
impl<D, M, F, Fut> SessionStoreFactory<D, M> for F
where
    F: Fn() -> Fut + Send + Sync + 'static,
    Fut: Future<Output = StorageResult<(D, M)>> + Send + 'static,
{
    fn create(&self) -> StoreFuture<D, M> {
        Box::pin(self())
    }
}

// ---------------------------------------------------------------------------
// AgentRun — one in-flight delegation
// ---------------------------------------------------------------------------

/// A single in-flight delegation. Lives in the `AgentTool`'s runs map.
#[allow(dead_code)]
struct AgentRun<D, M>
where
    D: AsyncDocumentStore + 'static,
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
    stream: crate::agentic::turn::TurnStream<D, M>,
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
    D: AsyncDocumentStore + 'static,
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
    /// Builds each sub-agent's stores.
    stores: Arc<dyn SessionStoreFactory<D, M>>,
}

impl<D, M> AgentTool<D, M>
where
    D: AsyncDocumentStore + Default + 'static,
    M: MemoryStore + Default + 'static,
{
    /// Create a new agent tool whose sub-agents get default-constructed stores
    /// ([`DefaultStores`]). Use it for in-memory stores; for stores that open
    /// a database use [`with_store_factory`](Self::with_store_factory).
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
        Self::with_store_factory(
            router,
            depth,
            max_depth,
            default_model,
            output_base,
            user,
            child_tools,
            Arc::new(DefaultStores),
        )
    }
}

impl<D, M> AgentTool<D, M>
where
    D: AsyncDocumentStore + 'static,
    M: MemoryStore + 'static,
{
    /// Create a new agent tool whose sub-agents get their stores from
    /// `stores`.
    ///
    /// `child_tools` are registered on every sub-agent session so the child
    /// can produce output (at minimum: `write` and `read`).
    #[must_use]
    #[allow(clippy::too_many_arguments)]
    pub fn with_store_factory(
        router: ProviderRouter,
        depth: u32,
        max_depth: u32,
        default_model: ModelId,
        output_base: String,
        user: UserId,
        child_tools: Vec<Arc<dyn ToolImpl>>,
        stores: Arc<dyn SessionStoreFactory<D, M>>,
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
            stores,
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
    async fn start(&self, args: &ToolArguments) -> Result<ToolCallResult, ToolError> {
        if self.depth >= self.max_depth {
            return Err(ToolError::Execution {
                tool: TOOL.into(),
                reason: format!(
                    "delegation depth cap reached (depth={}, max={})",
                    self.depth, self.max_depth
                ),
            });
        }

        let args = ToolArgs::new(TOOL, args);
        let task = args.str("task")?.to_string();
        if task.trim().is_empty() {
            return Err(ToolError::InvalidArguments {
                tool: TOOL.into(),
                reason: "'task' must not be empty".into(),
            });
        }

        let model: ModelId = match args.opt_str("model")? {
            Some(name) if !name.is_empty() => ModelId::from(name),
            _ => self.default_model.clone(),
        };

        let system: Option<String> = args
            .opt_str("system")?
            .filter(|s| !s.is_empty())
            .map(str::to_owned);

        let keep_session = args.opt_bool("keep_session")?.unwrap_or(false);

        // Runaway guard. Models emit integers inconsistently (u64 / i64 /
        // stringified); ToolArgs accepts every spelling and rejects negative,
        // unparseable or out-of-range values. `0` is rejected below rather than
        // silently creating a sub-agent that can never take a step.
        let max_iterations = args.opt_usize("max_iterations")?;
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

        let (doc_store, memory_store) = self.stores.create().await.map_err(|e| ToolError::Execution {
            tool: TOOL.into(),
            reason: format!("failed to create the sub-agent's stores: {e}"),
        })?;

        let mut builder = AgentSession::builder(self.router.clone())
            .with_session_id(child_session_id.clone())
            .with_doc_store(doc_store)
            .with_memory_store(memory_store)
            // Provision the child session with the tools it needs to produce
            // output (at minimum: write, read — supplied at construction).
            .with_toolshed(
                ToolShed::new().tools(self.child_tools.iter().map(|t| Arc::clone(t).into())),
            )
            .with_user(self.user.clone())
            .with_model(model.clone());

        if let Some(sys) = system {
            builder = builder.with_system_prompt(sys);
        }

        // Apply the runaway cap to the child's agent loop. `max_inner_iterations`
        // is the per-turn step ceiling the loop actually enforces.
        if let Some(cap) = max_iterations {
            builder = builder.with_config(crate::agentic::AgentConfig {
                max_inner_iterations: cap,
                ..Default::default()
            });
        }

        // Instruct the sub-agent where to write its result.
        let prompt_text = format!(
            "{task}\n\nWrite your final result to: {output_location}\nWhen finished, include a brief 1-2 line summary in your final message."
        );

        let child_session = builder.build().await.map_err(|e| ToolError::Execution {
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
    fn check(&self, args: &ToolArguments) -> Result<ToolCallResult, ToolError> {
        let id = ToolArgs::new(TOOL, args).str("id")?.to_string();
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
    fn result(&self, args: &ToolArguments) -> Result<ToolCallResult, ToolError> {
        let id = ToolArgs::new(TOOL, args).str("id")?.to_string();

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
    fn pause(&self, args: &ToolArguments) -> Result<ToolCallResult, ToolError> {
        let id = ToolArgs::new(TOOL, args).str("id")?.to_string();
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
    fn resume(&self, args: &ToolArguments) -> Result<ToolCallResult, ToolError> {
        let id = ToolArgs::new(TOOL, args).str("id")?.to_string();
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
    fn stop(&self, args: &ToolArguments) -> Result<ToolCallResult, ToolError> {
        let id = ToolArgs::new(TOOL, args).str("id")?.to_string();
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
    D: AsyncDocumentStore + 'static,
    M: MemoryStore + 'static,
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

    async fn execute(&self, arguments: ToolArguments) -> Result<ToolCallResult, ToolError> {
        let command = ToolArgs::new(TOOL, &arguments).str("command")?;
        match command {
            "start" => self.start(&arguments).await,
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
    D: AsyncDocumentStore + Default + 'static,
    M: MemoryStore + Default + 'static,
{
    register_agent_tool_with_stores::<D, M>(
        manager,
        router,
        depth,
        max_depth,
        default_model,
        output_base,
        user,
        child_tools,
        Arc::new(DefaultStores),
    );
}

/// Register the `agent` tool, building each sub-agent's stores with
/// `stores` (see [`SessionStoreFactory`]).
#[allow(clippy::too_many_arguments)]
pub fn register_agent_tool_with_stores<D, M>(
    manager: &crate::agentic::tool_impl::ToolCallManager,
    router: ProviderRouter,
    depth: u32,
    max_depth: u32,
    default_model: ModelId,
    output_base: &str,
    user: UserId,
    child_tools: Vec<Arc<dyn ToolImpl>>,
    stores: Arc<dyn SessionStoreFactory<D, M>>,
) where
    D: AsyncDocumentStore + 'static,
    M: MemoryStore + 'static,
{
    let tool = AgentTool::<D, M>::with_store_factory(
        router,
        depth,
        max_depth,
        default_model,
        output_base.to_string(),
        user,
        child_tools,
        stores,
    );
    manager.register(Arc::new(tool));
}
