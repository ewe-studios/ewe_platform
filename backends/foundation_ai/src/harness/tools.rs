//! Tool presets — pre-built tool collections for quick registration (spec-60 F15).
//!
//! WHY: wiring every `register_*` function by hand is boilerplate, and the F15
//! agent tool needs `Vec<Arc<dyn ToolImpl>>` of child tools to provision on
//! spawned sub-agents. A preset bundles the common configurations into one call
//! and doubles as the child-tool source.
//!
//! WHAT: [`ToolPreset`] — a collection of `Arc<dyn ToolImpl>` built by named
//! constructors matching the corresponding `register_*` functions. Presets
//! compose via `merge()`. [`ToolPreset::register_all()`] stamps every tool
//! onto a [`ToolCallManager`]; [`ToolPreset::as_child_tools()`] returns clones
//! for the agent tool's `child_tools` parameter.

use std::sync::Arc;

use foundation_db::traits::DocumentStore;
use foundation_nativeapis::shared::vfs::AsyncVfsFileSystem;

use crate::agentic::memory::MemoryHierarchy;
use crate::agentic::memory_store::MemoryStore;
use crate::agentic::tool_impl::{ToolCallManager, ToolImpl};
use crate::agentic::UserId;
use crate::types::routable_provider::ProviderRouter;
use crate::types::{ModelId, SessionId};

// ---------------------------------------------------------------------------
// ToolPreset
// ---------------------------------------------------------------------------

/// A pre-built collection of tool implementations, ready to register.
///
/// ```ignore
/// let preset = ToolPreset::files(my_fs)
///     .merge(ToolPreset::memory(my_hierarchy))
///     .merge(ToolPreset::shell());
///
/// // Register on a session's tool manager:
/// preset.register_all(session.tool_manager());
///
/// // Or use as child tools for the agent tool:
/// let child_tools = preset.as_child_tools();
/// ```
pub struct ToolPreset {
    tools: Vec<Arc<dyn ToolImpl>>,
}

impl Default for ToolPreset {
    fn default() -> Self {
        Self { tools: Vec::new() }
    }
}

impl ToolPreset {
    /// An empty preset.
    #[must_use]
    pub fn empty() -> Self {
        Self::default()
    }

    /// Shortcut: build a preset from a raw list of tool impls.
    #[must_use]
    pub fn from_tools(tools: Vec<Arc<dyn ToolImpl>>) -> Self {
        Self { tools }
    }

    /// Register every tool onto a [`ToolCallManager`].
    pub fn register_all(&self, manager: &ToolCallManager) {
        for tool in &self.tools {
            manager.register(Arc::clone(tool));
        }
    }

    /// Build a fresh [`ToolCallManager`] and register every tool on it.
    #[must_use]
    pub fn into_manager(self, session_id: SessionId) -> ToolCallManager {
        let mgr = ToolCallManager::new(session_id);
        self.register_all(&mgr);
        mgr
    }

    /// Clone each tool (cheap — `Arc` ref-count bump) as a `Vec<Arc<dyn
    /// ToolImpl>>` suitable for the F15 agent tool's `child_tools` parameter.
    #[must_use]
    pub fn as_child_tools(&self) -> Vec<Arc<dyn ToolImpl>> {
        self.tools.iter().map(Arc::clone).collect()
    }

    /// Number of tools in this preset.
    #[must_use]
    pub fn len(&self) -> usize {
        self.tools.len()
    }

    /// True when this preset has no tools.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.tools.is_empty()
    }

    /// Merge another preset into this one, returning the combined set.
    #[must_use]
    pub fn merge(mut self, other: Self) -> Self {
        self.tools.extend(other.tools);
        self
    }

    /// Access the underlying tool list.
    #[must_use]
    pub fn tools(&self) -> &[Arc<dyn ToolImpl>] {
        &self.tools
    }

    // ------------------------------------------------------------------
    // Standard presets — mirrors the register_* functions in tools/
    // ------------------------------------------------------------------

    /// File tools: `read`, `write`, `edit` over a VFS filesystem.
    ///
    /// Same three tools that [`register_file_tools`] registers.
    ///
    /// [`register_file_tools`]: crate::agentic::tools::files::register_file_tools
    #[must_use]
    pub fn files<F: AsyncVfsFileSystem + 'static>(fs: Arc<F>) -> Self {
        use crate::agentic::tools::files::{EditTool, ReadTool, WriteTool};
        Self {
            tools: vec![
                Arc::new(ReadTool::new(Arc::clone(&fs) as Arc<_>)),
                Arc::new(WriteTool::new(Arc::clone(&fs) as Arc<_>)),
                Arc::new(EditTool::new(fs)),
            ],
        }
    }

    /// Shell tool: `bash`.
    #[must_use]
    pub fn shell() -> Self {
        use crate::agentic::tools::files::BashTool;
        Self {
            tools: vec![Arc::new(BashTool::new())],
        }
    }

    /// Memory tool: `memory add` / `memory remove` / `memory replace` over
    /// a [`MemoryHierarchy`] (MultiCommands, F14/F19).
    #[must_use]
    pub fn memory<M, D>(hierarchy: Arc<MemoryHierarchy<M, D>>) -> Self
    where
        M: MemoryStore + 'static,
        D: DocumentStore + 'static,
    {
        use crate::agentic::tools::memory::MemoryTool;
        Self {
            tools: vec![Arc::new(MemoryTool::new(hierarchy))],
        }
    }

    /// Shed meta-tool: tool discovery via the vector store.
    #[must_use]
    pub fn shed(discovery: Arc<crate::agentic::tools::shed::ToolDiscovery>) -> Self {
        use crate::agentic::tools::shed::ShedTool;
        Self {
            tools: vec![Arc::new(ShedTool::new(discovery))],
        }
    }

    /// Agent tool: background sub-agent delegation (F15).
    ///
    /// `child_tools` are provisioned on every sub-agent session the tool spawns.
    /// At minimum this should include file tools so the sub-agent can produce
    /// output.  The returned tool requires the same generics as the `AgentSession`
    /// that will own it.
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
        Self {
            tools: vec![Arc::new(AgentTool::<D, M>::new(
                router,
                depth,
                max_depth,
                default_model,
                output_base.to_string(),
                user,
                child_tools,
            ))],
        }
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

    /// Standard set for a general-purpose agent: `files`, `shell`, `memory`,
    /// and `shed`. No `agent` — add it explicitly when delegation is desired.
    #[must_use]
    pub fn standard<F, M, D>(
        fs: Arc<F>,
        hierarchy: Arc<MemoryHierarchy<M, D>>,
        discovery: Arc<crate::agentic::tools::shed::ToolDiscovery>,
    ) -> Self
    where
        F: AsyncVfsFileSystem + 'static,
        M: MemoryStore + 'static,
        D: DocumentStore + 'static,
    {
        Self::files(fs)
            .merge(Self::shell())
            .merge(Self::memory(hierarchy))
            .merge(Self::shed(discovery))
    }
}

impl std::ops::Add for ToolPreset {
    type Output = Self;

    fn add(self, rhs: Self) -> Self::Output {
        self.merge(rhs)
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_is_empty() {
        assert!(ToolPreset::empty().is_empty());
        assert_eq!(ToolPreset::empty().len(), 0);
    }

    #[test]
    fn shell_is_nonempty() {
        assert_eq!(ToolPreset::shell().len(), 1);
    }

    #[test]
    fn merge_combines() {
        assert_eq!(ToolPreset::shell().merge(ToolPreset::shell()).len(), 2);
    }

    #[test]
    fn as_child_tools_clones() {
        let p = ToolPreset::shell();
        let children = p.as_child_tools();
        assert_eq!(children.len(), 1);
        assert_eq!(p.len(), 1); // original intact
    }

    #[test]
    fn into_manager_registers_all() {
        use crate::agentic::tool_impl::ToolCallManager;
        let mgr = ToolCallManager::new(SessionId::new());
        ToolPreset::shell().register_all(&mgr);
        assert!(mgr.names().contains(&"bash".to_string()));
    }

    #[test]
    fn into_manager_builds_and_registers() {
        let mgr = ToolPreset::shell().into_manager(SessionId::new());
        assert!(mgr.names().contains(&"bash".to_string()));
    }

    #[test]
    fn default_is_empty() {
        assert!(ToolPreset::default().is_empty());
    }
}
