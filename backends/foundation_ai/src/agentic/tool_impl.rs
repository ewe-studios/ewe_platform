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

use crate::agentic::tools::shed::{
    shed_definition, ShedResult, ToolDiscovery, ToolSummary, DEFAULT_SHED_LIMIT, SHED_TOOL_NAME,
};
use crate::types::{ArgType, ExecutionHint, TextContent, Tool, ToolDeclarations, UserModelContent};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use std::time::Duration;

// ---------------------------------------------------------------------------
// ToolDefinition — the single, shared descriptor (F19). Defined in the types
// layer and re-exported here so `ToolImpl::definition() -> ToolDefinition` and
// every `impl ToolImpl` keep referring to `tool_impl::ToolDefinition` unchanged.
pub use crate::types::base_types::ToolDefinition;

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
    /// The tool is registered but `shed` hasn't returned it yet, so the model
    /// may not call it.
    NotActive(String),
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
            ToolError::NotActive(tool) => write!(
                f,
                "tool '{tool}' is not active yet: call `{SHED_TOOL_NAME}` with a description \
                 of what you need to discover it first"
            ),
        }
    }
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

impl ToolCallResult {
    /// A plain-text result with no error detail.
    #[must_use]
    pub fn text(content: impl Into<String>) -> Self {
        Self {
            content: UserModelContent::Text(TextContent {
                content: content.into(),
                signature: None,
            }),
            error_detail: None,
        }
    }
}

// ---------------------------------------------------------------------------
// ToolArgs — typed reads of a call's arguments

/// Typed access to a tool call's arguments, whichever way the backend spelled
/// them.
///
/// Every getter fails with [`ToolError::InvalidArguments`] naming the tool and
/// the key; the `opt_*` getters return `Ok(None)` for a missing key but still
/// fail on a present value of the wrong type. Numbers are accepted as any
/// integer variant or numeric text (some models quote numbers); booleans as a
/// JSON boolean or the text `"true"` / `"false"`.
///
/// ```ignore
/// let args = ToolArgs::new("edit", &arguments);
/// let path = args.str("path")?;
/// let replace_all = args.opt_bool("replace_all")?.unwrap_or(false);
///
/// #[derive(Deserialize)]
/// struct EditArgs { path: String, #[serde(default)] replace_all: bool }
/// let EditArgs { path, replace_all } = args.parse()?;
/// ```
#[derive(Debug, Clone, Copy)]
pub struct ToolArgs<'a> {
    tool: &'a str,
    args: &'a HashMap<String, ArgType>,
}

impl<'a> ToolArgs<'a> {
    /// Read `args`, reporting errors against `tool`.
    #[must_use]
    #[allow(clippy::implicit_hasher)]
    pub fn new(tool: &'a str, args: &'a HashMap<String, ArgType>) -> Self {
        Self { tool, args }
    }

    /// The tool name errors are reported against.
    #[must_use]
    pub fn tool(&self) -> &'a str {
        self.tool
    }

    /// True when `key` was passed.
    #[must_use]
    pub fn contains(&self, key: &str) -> bool {
        self.args.contains_key(key)
    }

    fn invalid(&self, reason: String) -> ToolError {
        ToolError::InvalidArguments {
            tool: self.tool.to_string(),
            reason,
        }
    }

    fn missing(&self, key: &str) -> ToolError {
        self.invalid(format!("missing required argument `{key}`"))
    }

    fn wrong_type(&self, key: &str, expected: &str) -> ToolError {
        self.invalid(format!("`{key}` must be {expected}"))
    }

    fn required<T>(&self, key: &str, value: Result<Option<T>, ToolError>) -> Result<T, ToolError> {
        value?.ok_or_else(|| self.missing(key))
    }

    /// A required string.
    ///
    /// # Errors
    /// [`ToolError::InvalidArguments`] when missing or not a string.
    pub fn str(&self, key: &str) -> Result<&'a str, ToolError> {
        self.required(key, self.opt_str(key))
    }

    /// An optional string.
    ///
    /// # Errors
    /// [`ToolError::InvalidArguments`] when present but not a string.
    pub fn opt_str(&self, key: &str) -> Result<Option<&'a str>, ToolError> {
        match self.args.get(key) {
            None => Ok(None),
            Some(ArgType::Text(s)) => Ok(Some(s.as_str())),
            Some(_) => Err(self.wrong_type(key, "a string")),
        }
    }

    /// A required integer.
    ///
    /// # Errors
    /// [`ToolError::InvalidArguments`] when missing, not an integer, or out
    /// of `i64` range.
    pub fn i64(&self, key: &str) -> Result<i64, ToolError> {
        self.required(key, self.opt_i64(key))
    }

    /// An optional integer.
    ///
    /// # Errors
    /// [`ToolError::InvalidArguments`] when present but not an integer in
    /// `i64` range.
    pub fn opt_i64(&self, key: &str) -> Result<Option<i64>, ToolError> {
        let Some(value) = self.args.get(key) else {
            return Ok(None);
        };
        let n = match value {
            ArgType::I8(n) => Some(i64::from(*n)),
            ArgType::I16(n) => Some(i64::from(*n)),
            ArgType::I32(n) => Some(i64::from(*n)),
            ArgType::I64(n) => Some(*n),
            ArgType::I128(n) => i64::try_from(*n).ok(),
            ArgType::Isize(n) => i64::try_from(*n).ok(),
            ArgType::U8(n) => Some(i64::from(*n)),
            ArgType::U16(n) => Some(i64::from(*n)),
            ArgType::U32(n) => Some(i64::from(*n)),
            ArgType::U64(n) => i64::try_from(*n).ok(),
            ArgType::U128(n) => i64::try_from(*n).ok(),
            ArgType::Usize(n) => i64::try_from(*n).ok(),
            ArgType::Text(s) | ArgType::JSON(s) => s.trim().parse().ok(),
            _ => None,
        };
        n.map(Some)
            .ok_or_else(|| self.wrong_type(key, "an integer"))
    }

    /// A required non-negative integer (counts, limits, offsets).
    ///
    /// # Errors
    /// [`ToolError::InvalidArguments`] when missing, not an integer, or
    /// negative / too large for `usize`.
    pub fn usize(&self, key: &str) -> Result<usize, ToolError> {
        self.required(key, self.opt_usize(key))
    }

    /// An optional non-negative integer.
    ///
    /// # Errors
    /// [`ToolError::InvalidArguments`] when present but not a non-negative
    /// integer that fits `usize`.
    pub fn opt_usize(&self, key: &str) -> Result<Option<usize>, ToolError> {
        match self.opt_i64(key)? {
            None => Ok(None),
            Some(n) => usize::try_from(n)
                .map(Some)
                .map_err(|_| self.wrong_type(key, "a non-negative integer")),
        }
    }

    /// A required number.
    ///
    /// # Errors
    /// [`ToolError::InvalidArguments`] when missing or not a number.
    pub fn f64(&self, key: &str) -> Result<f64, ToolError> {
        self.required(key, self.opt_f64(key))
    }

    /// An optional number.
    ///
    /// # Errors
    /// [`ToolError::InvalidArguments`] when present but not a number.
    pub fn opt_f64(&self, key: &str) -> Result<Option<f64>, ToolError> {
        let Some(value) = self.args.get(key) else {
            return Ok(None);
        };
        let n = match value {
            ArgType::Float64(n) => Some(*n),
            ArgType::Float32(n) => Some(f64::from(*n)),
            ArgType::Text(s) | ArgType::JSON(s) => s.trim().parse().ok(),
            _ => match self.opt_i64(key) {
                // Integers are numbers too; precision loss past 2^53 is accepted.
                Ok(Some(i)) => Some(i as f64),
                _ => None,
            },
        };
        n.map(Some).ok_or_else(|| self.wrong_type(key, "a number"))
    }

    /// A required boolean.
    ///
    /// # Errors
    /// [`ToolError::InvalidArguments`] when missing or not a boolean.
    pub fn bool(&self, key: &str) -> Result<bool, ToolError> {
        self.required(key, self.opt_bool(key))
    }

    /// An optional boolean: a JSON boolean, or the text `"true"` / `"false"`.
    ///
    /// # Errors
    /// [`ToolError::InvalidArguments`] when present but not a boolean.
    pub fn opt_bool(&self, key: &str) -> Result<Option<bool>, ToolError> {
        match self.args.get(key) {
            None => Ok(None),
            Some(ArgType::JSON(s) | ArgType::Text(s)) => match s.trim() {
                "true" => Ok(Some(true)),
                "false" => Ok(Some(false)),
                _ => Err(self.wrong_type(key, "a boolean")),
            },
            Some(_) => Err(self.wrong_type(key, "a boolean")),
        }
    }

    /// The argument as plain JSON (arrays, objects, anything), if passed.
    #[must_use]
    pub fn opt_value(&self, key: &str) -> Option<serde_json::Value> {
        self.args.get(key).map(ArgType::to_json_value)
    }

    /// All arguments as one struct.
    ///
    /// # Errors
    /// [`ToolError::InvalidArguments`] with the deserializer's message when
    /// the arguments don't fit `T`.
    pub fn parse<T: serde::de::DeserializeOwned>(&self) -> Result<T, ToolError> {
        let object = serde_json::Value::Object(
            self.args
                .iter()
                .map(|(key, value)| (key.clone(), value.to_json_value()))
                .collect(),
        );
        serde_json::from_value(object).map_err(|e| self.invalid(e.to_string()))
    }
}

/// Check a call's arguments against the tool's declared JSON Schema.
///
/// A `SingleCommand` tool is checked against its `arguments` schema. A
/// `MultiCommands` tool is dispatched on the `command` argument: the command
/// must exist, and the remaining arguments are checked against that command's
/// schema.
///
/// # Errors
/// [`ToolError::InvalidArguments`] naming the first violation, an unknown
/// command, or a schema that does not compile.
#[allow(clippy::implicit_hasher)]
pub fn validate_arguments(
    definition: &Tool,
    tool_name: &str,
    arguments: &HashMap<String, ArgType>,
) -> Result<(), ToolError> {
    let invalid = |reason: String| ToolError::InvalidArguments {
        tool: tool_name.to_string(),
        reason,
    };

    let (schema, instance) = match definition {
        Tool::SingleCommand(def) => (&def.arguments.schema, arguments.clone()),
        Tool::MultiCommands(_, commands) => {
            let command = match arguments.get("command") {
                Some(ArgType::Text(command)) => command.as_str(),
                _ => return Err(invalid("missing 'command' argument".into())),
            };
            let Some(def) = commands.iter().find(|def| def.name == command) else {
                let known: Vec<&str> = commands.iter().map(|def| def.name.as_str()).collect();
                return Err(invalid(format!(
                    "unknown command '{command}' (expected one of: {})",
                    known.join(", ")
                )));
            };
            let mut rest = arguments.clone();
            rest.remove("command");
            (&def.arguments.schema, rest)
        }
    };

    let instance = serde_json::Value::Object(
        instance
            .iter()
            .map(|(key, value)| (key.clone(), value.to_json_value()))
            .collect(),
    );
    foundation_jsonschema::validate(schema, &instance).map_err(|err| invalid(err.to_string()))
}

// ---------------------------------------------------------------------------
// ToolImpl trait

/// The tool contract every tool implementer satisfies. Registered as
/// `Arc<dyn ToolImpl>` in the `ToolCallManager`.
#[async_trait]
pub trait ToolImpl: Send + Sync {
    /// The tool's own declaration of its shape (F19): `Tool::SingleCommand` for a
    /// leaf capability, or `Tool::MultiCommands(name, commands)` for a tool that
    /// exposes several commands (dispatched on the `command` argument). The
    /// registry keys the tool by `Tool::name()`.
    fn definition(&self) -> Tool;

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
            ToolError::InvalidArguments { .. }
            | ToolError::UnknownTool(_)
            | ToolError::NotActive(_) => ToolErrorKind::InvalidArguments,
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
    defs: std::sync::RwLock<HashMap<String, Tool>>,
    /// Per-tool retry config overrides (F11).
    retry_configs: std::sync::RwLock<HashMap<String, ToolRetryConfig>>,
    /// Tools `shed` has returned: declared to the model on every later request.
    active: std::sync::RwLock<HashSet<String>>,
    /// Embedding search for `shed`, when the session has an embedder.
    discovery: std::sync::RwLock<Option<Arc<ToolDiscovery>>>,
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
                active: std::sync::RwLock::new(HashSet::new()),
                discovery: std::sync::RwLock::new(None),
                session_id,
            }),
        }
    }

    /// Register a tool (interior mutability — `&self`, no `Arc` mutation needed).
    ///
    /// The tool is discoverable through `shed` straight away, but isn't
    /// declared to the model until `shed` returns it (or it is
    /// [`activate`](Self::activate)d). The name `shed` is reserved for the
    /// built-in discovery tool: a tool registered under it is never offered or
    /// run.
    ///
    /// When discovery is enabled the tool is also indexed for embedding search;
    /// if that fails it is logged and the tool stays findable by name and
    /// description.
    /// # Panics
    /// Panics if a registry lock is poisoned.
    pub fn register(&self, tool: Arc<dyn ToolImpl>) {
        let def = tool.definition();
        let tool_name = def.name().to_string();
        if tool_name == SHED_TOOL_NAME {
            tracing::warn!(
                "a tool named `{SHED_TOOL_NAME}` was registered; the name is reserved for the \
                 built-in discovery tool, so it will never be offered or run"
            );
        }

        let discovery = self.inner.discovery.read().unwrap().clone();
        if let Some(discovery) = discovery {
            if let Err(err) = discovery.index_tool(&def) {
                tracing::warn!(
                    tool = %tool_name,
                    "indexing the tool for `{SHED_TOOL_NAME}` failed ({err}); it stays \
                     findable by name and description"
                );
            }
        }

        let mut tools = self.inner.tools.write().unwrap();
        let mut defs = self.inner.defs.write().unwrap();
        defs.insert(tool_name.clone(), def);
        tools.insert(tool_name, tool);
    }

    /// Deregister a tool by name. It is also dropped from the active set and
    /// from the discovery index.
    /// # Panics
    /// Panics if a registry lock is poisoned.
    pub fn deregister(&self, name: &str) {
        self.inner.tools.write().unwrap().remove(name);
        self.inner.defs.write().unwrap().remove(name);
        self.inner.active.write().unwrap().remove(name);
        let discovery = self.inner.discovery.read().unwrap().clone();
        if let Some(discovery) = discovery {
            if let Err(err) = discovery.remove(name) {
                tracing::warn!(
                    tool = %name,
                    "removing the tool from the discovery index failed: {err}"
                );
            }
        }
    }

    // -----------------------------------------------------------------
    // What the model sees: `shed`, plus the tools `shed` has activated.
    // -----------------------------------------------------------------

    /// The `shed` meta-tool's declaration — always offered while any tool is
    /// registered.
    #[must_use]
    pub fn shed_definition(&self) -> Tool {
        shed_definition()
    }

    /// Declarations of the active tools (those `shed` has returned), sorted by
    /// name.
    #[must_use]
    pub fn active_definitions(&self) -> Vec<Tool> {
        let active = self.inner.active.read().unwrap();
        let defs = self.inner.defs.read().unwrap();
        let mut tools: Vec<Tool> = active.iter().filter_map(|n| defs.get(n).cloned()).collect();
        tools.sort_by(|a, b| a.name().cmp(b.name()));
        tools
    }

    /// Mark registered tools as active, so they are declared on the following
    /// requests and the model may call them. Called by `shed` with its hits;
    /// public for custom loops and for hosts that want a tool callable from
    /// the first request. Names that aren't registered are ignored.
    pub fn activate(&self, names: &[String]) {
        let defs = self.inner.defs.read().unwrap();
        let mut active = self.inner.active.write().unwrap();
        for name in names {
            if name != SHED_TOOL_NAME && defs.contains_key(name) {
                active.insert(name.clone());
            }
        }
    }

    /// True when `name` is registered and `shed` has activated it.
    #[must_use]
    pub fn is_active(&self, name: &str) -> bool {
        self.inner.active.read().unwrap().contains(name)
    }

    /// The tools to declare on the next model request: `shed` plus the active
    /// tools. With no tools registered nothing is declared — offering `shed` to
    /// a tool-less agent only invites a pointless call (docs/fixes/007).
    #[must_use]
    pub fn offered_tools(&self) -> ToolDeclarations {
        if self.inner.tools.read().unwrap().is_empty() {
            return ToolDeclarations {
                shed: None,
                tools: Vec::new(),
            };
        }
        ToolDeclarations {
            shed: Some(self.shed_definition()),
            tools: self.active_definitions(),
        }
    }

    /// Whether the model may call `name` right now: `shed` always; any other
    /// tool only once it is registered and active.
    /// # Errors
    /// [`ToolError::UnknownTool`] for an unregistered name,
    /// [`ToolError::NotActive`] for a registered tool `shed` hasn't returned.
    pub fn check_offered(&self, name: &str) -> Result<(), ToolError> {
        if name == SHED_TOOL_NAME || self.is_active(name) {
            return Ok(());
        }
        if self.inner.tools.read().unwrap().contains_key(name) {
            Err(ToolError::NotActive(name.to_string()))
        } else {
            Err(ToolError::UnknownTool(name.to_string()))
        }
    }

    /// Search the registered tools for `shed`.
    ///
    /// With discovery enabled the embedding hits come first; the rest of the
    /// `limit` is filled by name/description match (which is the whole search
    /// without discovery). Each query word that appears in a tool's name,
    /// description or category scores a point; ties sort by name. An empty
    /// description lists every tool.
    /// # Errors
    /// [`ToolError::Execution`] if the embedding search fails.
    pub fn search_tools(
        &self,
        description: &str,
        limit: usize,
    ) -> Result<Vec<ToolSummary>, ToolError> {
        let limit = limit.max(1);
        let mut hits: Vec<ToolSummary> = Vec::new();

        let discovery = self.inner.discovery.read().unwrap().clone();
        if let Some(discovery) = discovery {
            let found = discovery.search(description, limit)?;
            let registered = self.inner.defs.read().unwrap();
            hits.extend(
                found
                    .into_iter()
                    .filter(|hit| registered.contains_key(&hit.name)),
            );
        }

        if hits.len() < limit {
            let words: Vec<String> = description
                .split(|c: char| !c.is_alphanumeric() && c != '_')
                .filter(|w| !w.is_empty())
                .map(str::to_lowercase)
                .collect();
            let defs = self.inner.defs.read().unwrap();
            let mut scored: Vec<(usize, ToolSummary)> = defs
                .iter()
                .filter(|(name, _)| name.as_str() != SHED_TOOL_NAME)
                .map(|(_, def)| ToolSummary::of(def))
                .filter(|s| !hits.iter().any(|h| h.name == s.name))
                .filter_map(|s| {
                    let haystack =
                        format!("{} {} {}", s.name, s.description, s.category).to_lowercase();
                    let score = words
                        .iter()
                        .filter(|w| haystack.contains(w.as_str()))
                        .count();
                    (words.is_empty() || score > 0).then_some((score, s))
                })
                .collect();
            scored.sort_by(|(sa, a), (sb, b)| sb.cmp(sa).then_with(|| a.name.cmp(&b.name)));
            let room = limit - hits.len();
            hits.extend(scored.into_iter().map(|(_, s)| s).take(room));
        }

        hits.truncate(limit);
        Ok(hits)
    }

    /// Search with an embedder from now on: index every registered tool (and,
    /// via [`register`](Self::register), every later one) in `discovery`.
    /// # Errors
    /// [`ToolError::Execution`] if indexing a registered tool fails; discovery
    /// is then left disabled.
    pub fn enable_discovery(&self, discovery: Arc<ToolDiscovery>) -> Result<(), ToolError> {
        let defs: Vec<Tool> = self
            .inner
            .defs
            .read()
            .unwrap()
            .iter()
            .filter(|(name, _)| name.as_str() != SHED_TOOL_NAME)
            .map(|(_, def)| def.clone())
            .collect();
        for def in &defs {
            discovery.index_tool(def)?;
        }
        *self.inner.discovery.write().unwrap() = Some(discovery);
        Ok(())
    }

    /// Run the built-in `shed`: search, activate the hits, return them.
    fn run_shed(&self, arguments: &HashMap<String, ArgType>) -> Result<ToolCallResult, ToolError> {
        validate_arguments(&shed_definition(), SHED_TOOL_NAME, arguments)?;
        let args = ToolArgs::new(SHED_TOOL_NAME, arguments);
        let description = args.str("description")?;
        let limit = args.opt_usize("limit")?.unwrap_or(DEFAULT_SHED_LIMIT);

        let hits = self.search_tools(description, limit)?;
        let names: Vec<String> = hits.iter().map(|h| h.name.clone()).collect();
        self.activate(&names);

        let json = serde_json::to_string(&ShedResult { tools: hits }).map_err(|e| {
            ToolError::Execution {
                tool: SHED_TOOL_NAME.into(),
                reason: format!("serialization failed: {e}"),
            }
        })?;
        Ok(ToolCallResult::text(json))
    }

    /// Look up a registered tool.
    #[must_use]
    /// # Errors
    /// Returns [`ToolError`] if execution fails.
    pub fn get(&self, name: &str) -> Option<Arc<dyn ToolImpl>> {
        self.inner.tools.read().unwrap().get(name).cloned()
    }

    /// Look up a tool's definition (fast — cached at register time).
    #[must_use]
    /// # Errors
    /// Returns [`ToolError`] if a dependency fails.
    pub fn get_def(&self, name: &str) -> Option<Tool> {
        self.inner.defs.read().unwrap().get(name).cloned()
    }

    /// List all registered tool names.
    #[must_use]
    /// # Errors
    /// Returns [`ToolError`] if execution fails.
    pub fn names(&self) -> Vec<String> {
        self.inner.tools.read().unwrap().keys().cloned().collect()
    }

    /// The registered tools' own `Tool` declarations, name-sorted, leaving
    /// out anything registered under the reserved `shed` name.
    fn collected_tools(&self) -> Vec<Tool> {
        let defs = self.inner.defs.read().unwrap();
        let mut tools: Vec<Tool> = defs
            .iter()
            .filter(|(name, _)| name.as_str() != SHED_TOOL_NAME)
            .map(|(_, def)| def.clone())
            .collect();
        tools.sort_by(|a, b| a.name().cmp(b.name()));
        tools
    }

    /// Validate arguments against the tool's JSON-Schema, then execute.
    /// # Errors
    /// Returns [`ToolError`] if validation fails.
    pub async fn execute_one(
        &self,
        request: &ToolCallRequest,
    ) -> Result<ToolCallResult, ToolError> {
        if request.name == SHED_TOOL_NAME {
            return self.run_shed(&request.arguments);
        }

        let tool = self
            .get(&request.name)
            .ok_or_else(|| ToolError::UnknownTool(request.name.clone()))?;

        // Validate the arguments against the tool's JSON Schema before running
        // it. The schema used to be advisory only (sent to the model, never
        // checked), so every tool had to re-validate — and most didn't.
        validate_arguments(&tool.definition(), &request.name, &request.arguments)?;

        // Contain a panicking tool at the boundary: a buggy tool must not take
        // down the agent (or wedge the driving valtron task). A panic becomes a
        // ToolError::Execution the loop handles like any other tool failure.
        use futures_lite::FutureExt;
        let name = request.name.clone();
        match std::panic::AssertUnwindSafe(tool.execute(request.arguments.clone()))
            .catch_unwind()
            .await
        {
            Ok(result) => result,
            Err(_panic) => Err(ToolError::Execution {
                tool: name,
                reason: "tool panicked during execution".into(),
            }),
        }
    }

    /// Every registered tool's declaration, name-sorted, plus `shed` (when any
    /// tool is registered). What the model *could* reach through `shed` — not
    /// what a request declares; see [`offered_tools`](Self::offered_tools).
    #[must_use]
    pub fn all_declarations(&self) -> ToolDeclarations {
        let tools = self.collected_tools();
        let shed = (!tools.is_empty()).then(shed_definition);
        ToolDeclarations { shed, tools }
    }

    /// Every registered tool's declaration plus `shed`.
    #[deprecated(note = "use all_declarations(); a request declares offered_tools()")]
    #[must_use]
    pub fn build_toolshed(&self) -> ToolDeclarations {
        self.all_declarations()
    }

    /// A manager whose `shed` searches through `discovery` (embedding search).
    #[must_use]
    pub fn with_defaults(
        session_id: crate::types::SessionId,
        discovery: Arc<ToolDiscovery>,
    ) -> Self {
        let mgr = Self::new(session_id);
        *mgr.inner.discovery.write().unwrap() = Some(discovery);
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
    /// # Errors
    /// Returns [`ToolError`] if the tool is not found.
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
    /// # Errors
    /// Returns [`ToolError`] if execution fails.
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
    /// # Errors
    /// Returns [`ToolError`] if the tool is not found.
    pub fn set_retry_config(&self, tool_name: &str, config: ToolRetryConfig) {
        self.inner
            .retry_configs
            .write()
            .unwrap()
            .insert(tool_name.to_string(), config);
    }

    /// Get retry config for a tool (per-tool override or default).
    #[must_use]
    /// # Errors
    /// Returns [`ToolError`] if a dependency fails.
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
