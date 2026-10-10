//! Tool presets — pre-built tool collections (spec-60 F15).
//!
//! WHY: wiring every tool by hand is boilerplate, and the F15 agent tool needs
//! `Vec<Arc<dyn ToolImpl>>` of child tools to provision on spawned sub-agents.
//! A preset bundles the common configurations into one value.
//!
//! WHAT: [`ToolPreset`] — a list of [`ToolConstructor`]s built by named
//! constructors. Ready-made tools (`files`, `shell`, `memory(h)`, `agent`)
//! ignore the session; session-dependent ones (`search_context`,
//! `session_memory`) are built from the session inside
//! `AgentSession::build()`. A preset is an `IntoIterator` of constructors, so it
//! goes straight into [`ToolShed::tools`](crate::agentic::ToolShed::tools).
//! Presets compose via `merge()` / `+`.
//!
//! HOW: the normal path is `ToolShed::new().tools(preset)`. Presets of
//! ready-made tools can also be registered on an existing manager
//! (`register_all`) or handed to the agent tool (`as_child_tools`); those fail
//! with [`ToolShedError::NeedsSession`] for a session-dependent tool.

use std::sync::Arc;

use foundation_db::traits::DocumentStore;
use foundation_nativeapis::shared::vfs::AsyncVfsFileSystem;

use crate::agentic::memory::MemoryHierarchy;
use crate::agentic::memory_store::MemoryStore;
use crate::agentic::tool_impl::{ToolCallManager, ToolImpl};
use crate::agentic::toolshed::{tool_fn, ToolConstructor, ToolShedError};
use crate::agentic::UserId;
use crate::types::routable_provider::ProviderRouter;
use crate::types::{ModelId, SessionId};

// ---------------------------------------------------------------------------
// ToolPreset
// ---------------------------------------------------------------------------

/// A pre-built collection of tool constructors.
///
/// ```ignore
/// let tools = ToolShed::new()
///     .tools(ToolPreset::files(my_fs))
///     .tools(ToolPreset::shell())
///     .tools(ToolPreset::search_context()); // built from the session
///
/// let agent = AgentSession::builder(router).with_model("my-model").with_toolshed(tools).build()?;
///
/// // Ready-made tools only: hand them to the agent tool.
/// let child_tools = ToolPreset::minimal_sub_agent(fs).as_child_tools()?;
/// ```
#[derive(Default)]
pub struct ToolPreset {
    constructors: Vec<Box<dyn ToolConstructor>>,
}

impl ToolPreset {
    /// An empty preset.
    #[must_use]
    pub fn empty() -> Self {
        Self::default()
    }

    /// A preset of ready-made tools.
    #[must_use]
    pub fn from_tools(tools: Vec<Arc<dyn ToolImpl>>) -> Self {
        Self {
            constructors: tools.into_iter().map(Into::into).collect(),
        }
    }

    /// A preset of constructors (ready-made or session-dependent).
    #[must_use]
    pub fn from_constructors(constructors: Vec<Box<dyn ToolConstructor>>) -> Self {
        Self { constructors }
    }

    /// The tool names, in the order they were added.
    #[must_use]
    pub fn names(&self) -> Vec<&str> {
        self.constructors.iter().map(|c| c.name()).collect()
    }

    /// The ready-made tools, or [`ToolShedError::NeedsSession`] naming the
    /// first tool that has to be built from a session.
    fn prebuilt_tools(&self) -> Result<Vec<Arc<dyn ToolImpl>>, ToolShedError> {
        self.constructors
            .iter()
            .map(|c| {
                c.prebuilt()
                    .ok_or_else(|| ToolShedError::NeedsSession(c.name().to_string()))
            })
            .collect()
    }

    /// Register every tool onto an existing [`ToolCallManager`] — e.g. adding
    /// tools to a running session. Registers nothing if any tool needs a
    /// session.
    ///
    /// # Errors
    /// [`ToolShedError::NeedsSession`] for a session-dependent tool; put those
    /// in the session's `ToolShed` instead.
    pub fn register_all(&self, manager: &ToolCallManager) -> Result<(), ToolShedError> {
        for tool in self.prebuilt_tools()? {
            manager.register(tool);
        }
        Ok(())
    }

    /// Build a fresh [`ToolCallManager`] and register every tool on it.
    ///
    /// # Errors
    /// [`ToolShedError::NeedsSession`] for a session-dependent tool.
    pub fn into_manager(self, session_id: SessionId) -> Result<ToolCallManager, ToolShedError> {
        let mgr = ToolCallManager::new(session_id);
        self.register_all(&mgr)?;
        Ok(mgr)
    }

    /// The tools (cheap `Arc` clones) for the F15 agent tool's `child_tools`
    /// parameter.
    ///
    /// # Errors
    /// [`ToolShedError::NeedsSession`] for a session-dependent tool.
    pub fn as_child_tools(&self) -> Result<Vec<Arc<dyn ToolImpl>>, ToolShedError> {
        self.prebuilt_tools()
    }

    /// Number of tools in this preset.
    #[must_use]
    pub fn len(&self) -> usize {
        self.constructors.len()
    }

    /// True when this preset has no tools.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.constructors.is_empty()
    }

    /// Merge another preset into this one, returning the combined set.
    #[must_use]
    pub fn merge(mut self, other: Self) -> Self {
        self.constructors.extend(other.constructors);
        self
    }

    // ------------------------------------------------------------------
    // Ready-made presets
    // ------------------------------------------------------------------

    /// File tools: `read`, `write`, `edit` over a VFS filesystem.
    #[must_use]
    pub fn files<F: AsyncVfsFileSystem + 'static>(fs: Arc<F>) -> Self {
        use crate::agentic::tools::files::{EditTool, ReadTool, WriteTool};
        Self::from_tools(vec![
            Arc::new(ReadTool::new(Arc::clone(&fs) as Arc<_>)),
            Arc::new(WriteTool::new(Arc::clone(&fs) as Arc<_>)),
            Arc::new(EditTool::new(fs)),
        ])
    }

    /// Shell tool: `bash`.
    #[must_use]
    pub fn shell() -> Self {
        use crate::agentic::tools::files::BashTool;
        Self::from_tools(vec![Arc::new(BashTool::new())])
    }

    /// Memory tool (`memory add` / `remove` / `replace`, `MultiCommands`) over
    /// a given [`MemoryHierarchy`]. For the session's own memory use
    /// [`session_memory`](Self::session_memory).
    #[must_use]
    pub fn memory<M, D>(hierarchy: Arc<MemoryHierarchy<M, D>>) -> Self
    where
        M: MemoryStore + 'static,
        D: DocumentStore + 'static,
    {
        use crate::agentic::tools::memory::MemoryTool;
        Self::from_tools(vec![Arc::new(MemoryTool::new(hierarchy))])
    }

    /// Agent tool: background sub-agent delegation (F15).
    ///
    /// `child_tools` are provisioned on every sub-agent session the tool spawns.
    /// At minimum this should include file tools so the sub-agent can produce
    /// output. `D` / `M` are the sub-agents' store types (default-constructed
    /// for each sub-agent).
    #[must_use]
    pub fn agent<D, M>(
        router: ProviderRouter,
        depth: u32,
        max_depth: u32,
        default_model: ModelId,
        output_base: &str,
        user: UserId,
        child_tools: Vec<Arc<dyn ToolImpl>>,
    ) -> Self
    where
        D: DocumentStore + Default + 'static,
        M: MemoryStore + Default + 'static,
    {
        use crate::agentic::tools::agent::AgentTool;
        Self::from_tools(vec![Arc::new(AgentTool::<D, M>::new(
            router,
            depth,
            max_depth,
            default_model,
            output_base.to_string(),
            user,
            child_tools,
        ))])
    }

    // ------------------------------------------------------------------
    // Session-dependent presets — built inside `AgentSession::build()`
    // ------------------------------------------------------------------

    /// `search_context` over the session's own stores and embedder: a
    /// constructor that reads the session's context provider.
    #[must_use]
    pub fn search_context() -> Self {
        use crate::agentic::tools::search::SearchContextTool;
        Self::from_constructors(vec![tool_fn("search_context", |s| {
            Arc::new(SearchContextTool::new(Arc::clone(&s.context))) as Arc<dyn ToolImpl>
        })])
    }

    /// The `memory` tool over the session's own memory hierarchy.
    #[must_use]
    pub fn session_memory() -> Self {
        use crate::agentic::tools::memory::MemoryTool;
        Self::from_constructors(vec![tool_fn("memory", |s| {
            Arc::new(MemoryTool::from_shared(Arc::clone(&s.memory))) as Arc<dyn ToolImpl>
        })])
    }

    // ------------------------------------------------------------------
    // Composite presets
    // ------------------------------------------------------------------

    /// Minimal set for a sub-agent: `read` + `write` + `edit` + `bash`.
    ///
    /// No memory or agent tool — prevents unbounded delegation chains.
    #[must_use]
    pub fn minimal_sub_agent<F: AsyncVfsFileSystem + 'static>(fs: Arc<F>) -> Self {
        Self::files(fs).merge(Self::shell())
    }

    /// Standard set for a general-purpose agent: `files`, `shell`, and
    /// `memory` over the session's own memory. No `agent` — add it explicitly
    /// when delegation is desired. (`shed` is built in to every session.)
    #[must_use]
    pub fn standard<F: AsyncVfsFileSystem + 'static>(fs: Arc<F>) -> Self {
        Self::files(fs)
            .merge(Self::shell())
            .merge(Self::session_memory())
    }
}

impl std::ops::Add for ToolPreset {
    type Output = Self;

    fn add(self, rhs: Self) -> Self::Output {
        self.merge(rhs)
    }
}

impl IntoIterator for ToolPreset {
    type Item = Box<dyn ToolConstructor>;
    type IntoIter = std::vec::IntoIter<Box<dyn ToolConstructor>>;

    fn into_iter(self) -> Self::IntoIter {
        self.constructors.into_iter()
    }
}
