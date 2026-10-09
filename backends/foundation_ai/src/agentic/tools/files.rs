//! Standard file tools — `read`, `write`, `edit` — over the VFS (spec-60 F05–F07).
//!
//! WHY: the `ToolShed` declares `read`/`edit`/`write` slots but shipped no
//! implementations, so the agent could not touch the filesystem. These are the
//! implementations.
//!
//! WHAT: three `ToolImpl`s, each generic over `F: AsyncVfsFileSystem` and holding
//! an `Arc<F>`. Going through the VFS (rather than `std::fs`) makes the FS base
//! swappable — a real FS, an in-memory `MemoryFs` (tests), an overlay, or a
//! remote FS — and keeps the tools usable on wasm.
//!
//! HOW: the tool is generic; registration monomorphizes it before boxing into
//! `Arc<dyn ToolImpl>`. Args are parsed from the `ArgType` map; results are text.

use std::collections::HashMap;
use std::sync::Arc;

use async_trait::async_trait;
use foundation_nativeapis::shared::vfs::AsyncVfsFileSystem;

use crate::agentic::tool_impl::{ToolCallResult, ToolDefinition, ToolError, ToolImpl};
use crate::types::Tool;
use crate::types::{ArgType, TextContent, UserModelContent};
use crate::types::base_types::Args;

/// Pull a required text argument or return a clean `InvalidArguments` error.
fn text_arg(args: &HashMap<String, ArgType>, key: &str, tool: &str) -> Result<String, ToolError> {
    match args.get(key) {
        Some(ArgType::Text(s)) => Ok(s.clone()),
        _ => Err(ToolError::InvalidArguments {
            tool: tool.into(),
            reason: format!("missing or invalid '{key}' argument"),
        }),
    }
}

/// Optional unsigned integer argument (accepts any of the unsigned `ArgType`s).
fn opt_usize(args: &HashMap<String, ArgType>, key: &str) -> Option<usize> {
    match args.get(key)? {
        ArgType::Usize(n) => Some(*n),
        ArgType::U8(n) => Some(*n as usize),
        ArgType::U16(n) => Some(*n as usize),
        ArgType::U32(n) => Some(*n as usize),
        ArgType::U64(n) => usize::try_from(*n).ok(),
        _ => None,
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

// ---------------------------------------------------------------------------
// ReadTool (F05)
// ---------------------------------------------------------------------------

/// Read a file's contents via the VFS. Optional `offset`/`limit` select a line
/// range (1-indexed `offset`), else the whole file.
pub struct ReadTool<F> {
    fs: Arc<F>,
}

impl<F: AsyncVfsFileSystem + 'static> ReadTool<F> {
    #[must_use]
    pub fn new(fs: Arc<F>) -> Self {
        Self { fs }
    }
}

#[async_trait]
impl<F: AsyncVfsFileSystem + 'static> ToolImpl for ReadTool<F> {
    fn definition(&self) -> Tool {
        Tool::SingleCommand(ToolDefinition {
            name: "read".into(),
            description: "Read a text file. Args: path (required), optional offset (1-indexed \
                          start line) and limit (max lines)."
                .into(),
            arguments: Args::new(
                foundation_jsonschema::scheme::object()
                    .required("path", foundation_jsonschema::scheme::string().min_len(1))
                    .optional("offset", foundation_jsonschema::scheme::integer())
                    .optional("limit", foundation_jsonschema::scheme::integer())
                    .build(),
            ),
            category: "read".into(),
            returns: None,
        })
    }

    async fn execute(
        &self,
        arguments: HashMap<String, ArgType>,
    ) -> Result<ToolCallResult, ToolError> {
        let path = text_arg(&arguments, "path", "read")?;
        let bytes = self
            .fs
            .read_file_async(path.clone())
            .await
            .map_err(|e| exec_err("read", format!("cannot read '{path}': {e}")))?;
        let text = String::from_utf8(bytes)
            .map_err(|_| exec_err("read", format!("'{path}' is not valid UTF-8 text")))?;

        let offset = opt_usize(&arguments, "offset");
        let limit = opt_usize(&arguments, "limit");
        if offset.is_none() && limit.is_none() {
            return Ok(text_result(text));
        }

        // 1-indexed offset; default whole file from the start.
        let start = offset.unwrap_or(1).saturating_sub(1);
        let selected: String = text
            .lines()
            .skip(start)
            .take(limit.unwrap_or(usize::MAX))
            .collect::<Vec<_>>()
            .join("\n");
        Ok(text_result(selected))
    }
}

// ---------------------------------------------------------------------------
// WriteTool (F06)
// ---------------------------------------------------------------------------

/// Create or overwrite a file via the VFS.
pub struct WriteTool<F> {
    fs: Arc<F>,
}

impl<F: AsyncVfsFileSystem + 'static> WriteTool<F> {
    #[must_use]
    pub fn new(fs: Arc<F>) -> Self {
        Self { fs }
    }
}

#[async_trait]
impl<F: AsyncVfsFileSystem + 'static> ToolImpl for WriteTool<F> {
    fn definition(&self) -> Tool {
        Tool::SingleCommand(ToolDefinition {
            name: "write".into(),
            description: "Create or overwrite a text file. Args: path (required), content \
                          (required). Returns the number of bytes written."
                .into(),
            arguments: Args::new(
                foundation_jsonschema::scheme::object()
                    .required("path", foundation_jsonschema::scheme::string().min_len(1))
                    .required("content", foundation_jsonschema::scheme::string())
                    .build(),
            ),
            category: "write".into(),
            returns: None,
        })
    }

    async fn execute(
        &self,
        arguments: HashMap<String, ArgType>,
    ) -> Result<ToolCallResult, ToolError> {
        let path = text_arg(&arguments, "path", "write")?;
        let content = text_arg(&arguments, "content", "write")?;
        let n = content.len();
        self.fs
            .write_file_async(path.clone(), content.into_bytes())
            .await
            .map_err(|e| exec_err("write", format!("cannot write '{path}': {e}")))?;
        Ok(text_result(format!("wrote {n} bytes to {path}")))
    }
}

// ---------------------------------------------------------------------------
// EditTool (F07)
// ---------------------------------------------------------------------------

/// Replace an exact string in a file via the VFS. `old_string` must be unique
/// unless `replace_all` is set — mirrors the editor contract.
pub struct EditTool<F> {
    fs: Arc<F>,
}

impl<F: AsyncVfsFileSystem + 'static> EditTool<F> {
    #[must_use]
    pub fn new(fs: Arc<F>) -> Self {
        Self { fs }
    }
}

#[async_trait]
impl<F: AsyncVfsFileSystem + 'static> ToolImpl for EditTool<F> {
    fn definition(&self) -> Tool {
        Tool::SingleCommand(ToolDefinition {
            name: "edit".into(),
            description: "Replace an exact string in a file. Args: path, old_string, new_string, \
                          optional replace_all (default false). old_string must be unique unless \
                          replace_all is set."
                .into(),
            arguments: Args::new(
                foundation_jsonschema::scheme::object()
                    .required("path", foundation_jsonschema::scheme::string().min_len(1))
                    .required("old_string", foundation_jsonschema::scheme::string())
                    .required("new_string", foundation_jsonschema::scheme::string())
                    .optional("replace_all", foundation_jsonschema::scheme::boolean())
                    .build(),
            ),
            category: "edit".into(),
            returns: None,
        })
    }

    async fn execute(
        &self,
        arguments: HashMap<String, ArgType>,
    ) -> Result<ToolCallResult, ToolError> {
        let path = text_arg(&arguments, "path", "edit")?;
        let old = text_arg(&arguments, "old_string", "edit")?;
        let new = text_arg(&arguments, "new_string", "edit")?;
        let replace_all = matches!(
            arguments.get("replace_all"),
            Some(ArgType::Text(t)) if t == "true"
        );

        let bytes = self
            .fs
            .read_file_async(path.clone())
            .await
            .map_err(|e| exec_err("edit", format!("cannot read '{path}': {e}")))?;
        let text = String::from_utf8(bytes)
            .map_err(|_| exec_err("edit", format!("'{path}' is not valid UTF-8 text")))?;

        let count = text.matches(&old).count();
        if count == 0 {
            return Err(exec_err("edit", format!("old_string not found in '{path}'")));
        }
        if count > 1 && !replace_all {
            return Err(exec_err(
                "edit",
                format!("old_string is not unique in '{path}' ({count} matches); set replace_all"),
            ));
        }

        let updated = if replace_all {
            text.replace(&old, &new)
        } else {
            text.replacen(&old, &new, 1)
        };
        let replaced = if replace_all { count } else { 1 };

        self.fs
            .write_file_async(path.clone(), updated.into_bytes())
            .await
            .map_err(|e| exec_err("edit", format!("cannot write '{path}': {e}")))?;
        Ok(text_result(format!("replaced {replaced} occurrence(s) in {path}")))
    }
}

// ---------------------------------------------------------------------------
// BashTool (F08) — native only
// ---------------------------------------------------------------------------

/// Run a shell command and capture stdout/stderr/exit. Native only; on wasm the
/// tool returns a clear "unsupported on this target" error.
///
/// WHY exec is separate from the VFS: file tools swap the FS base; command
/// execution is a distinct capability that only makes sense on a native host.
pub struct BashTool {
    /// Max wall-clock time before the command is killed.
    timeout: std::time::Duration,
}

impl Default for BashTool {
    fn default() -> Self {
        Self {
            timeout: std::time::Duration::from_secs(120),
        }
    }
}

impl BashTool {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    #[must_use]
    pub fn with_timeout(timeout: std::time::Duration) -> Self {
        Self { timeout }
    }
}

#[async_trait]
impl ToolImpl for BashTool {
    fn definition(&self) -> Tool {
        Tool::SingleCommand(ToolDefinition {
            name: "bash".into(),
            description: "Run a shell command. Args: command (required), optional cwd. Returns \
                          stdout, stderr and the exit code."
                .into(),
            arguments: Args::new(
                foundation_jsonschema::scheme::object()
                    .required("command", foundation_jsonschema::scheme::string().min_len(1))
                    .optional("cwd", foundation_jsonschema::scheme::string())
                    .build(),
            ),
            category: "shell".into(),
            returns: None,
        })
    }

    #[cfg(not(target_family = "wasm"))]
    async fn execute(
        &self,
        arguments: HashMap<String, ArgType>,
    ) -> Result<ToolCallResult, ToolError> {
        let command = text_arg(&arguments, "command", "bash")?;
        let cwd = match arguments.get("cwd") {
            Some(ArgType::Text(s)) => Some(s.clone()),
            _ => None,
        };
        let timeout = self.timeout;

        // Blocking process spawn on a background thread; bounded by `timeout`.
        let (tx, rx) = std::sync::mpsc::channel();
        std::thread::spawn(move || {
            let mut cmd = std::process::Command::new("sh");
            cmd.arg("-c").arg(&command);
            if let Some(dir) = cwd {
                cmd.current_dir(dir);
            }
            let _ = tx.send(cmd.output());
        });

        let output = match rx.recv_timeout(timeout) {
            Ok(Ok(o)) => o,
            Ok(Err(e)) => return Err(exec_err("bash", format!("spawn failed: {e}"))),
            Err(_) => {
                return Err(exec_err(
                    "bash",
                    format!("command timed out after {}s", timeout.as_secs()),
                ))
            }
        };

        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);
        let code = output.status.code().unwrap_or(-1);
        Ok(text_result(format!(
            "exit_code: {code}\nstdout:\n{stdout}\nstderr:\n{stderr}"
        )))
    }

    #[cfg(target_family = "wasm")]
    async fn execute(
        &self,
        _arguments: HashMap<String, ArgType>,
    ) -> Result<ToolCallResult, ToolError> {
        Err(exec_err("bash", "shell execution is not supported on this target"))
    }
}

// ---------------------------------------------------------------------------
// Registration helper (F09) — fill the ToolShed slots from a VFS
// ---------------------------------------------------------------------------

/// Register the standard filesystem tools (`read`/`write`/`edit`) — all backed
/// by the supplied VFS — onto a [`ToolCallManager`]. The manager's
/// [`ToolCallManager::build_toolshed`] then populates the matching `ToolShed`
/// slots automatically (the tools carry `read`/`write`/`edit` categories).
///
/// WHY a VFS parameter: the FS base is swappable — pass a real FS in production,
/// a `MemoryFs` in tests, or an overlay/remote FS — without touching the tools.
pub fn register_file_tools<F: AsyncVfsFileSystem + 'static>(
    manager: &crate::agentic::tool_impl::ToolCallManager,
    fs: Arc<F>,
) {
    manager.register(Arc::new(ReadTool::new(fs.clone())));
    manager.register(Arc::new(WriteTool::new(fs.clone())));
    manager.register(Arc::new(EditTool::new(fs)));
}

/// Register the native shell tool (`bash`). Fills the `shell` `ToolShed` slot.
/// Native only — on wasm the tool exists but returns an "unsupported" error.
pub fn register_shell_tool(manager: &crate::agentic::tool_impl::ToolCallManager) {
    manager.register(Arc::new(BashTool::new()));
}
