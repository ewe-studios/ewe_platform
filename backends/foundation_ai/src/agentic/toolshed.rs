//! `ToolShed` — the one explicit list of tools an agent session can call.
//!
//! WHY: Tools used to be configured in two places — declarations on the
//! builder's shed, implementations on the session's `ToolCallManager` after
//! `build()` — and preflight failed whenever the two disagreed. Tools that
//! need the session (recall over its stores, its memory hierarchy) could only
//! be added after `build()`, by hand.
//!
//! WHAT: [`ToolShed`] holds one [`ToolConstructor`] per tool name. A
//! ready-made `ToolImpl` converts into a constructor that ignores the session;
//! [`tool_fn`] (and `ToolPreset::search_context` / `ToolPreset::session_memory`)
//! build tools from the session being built, through [`SessionParts`].
//! [`ToolShed::build`] constructs every tool and returns the populated
//! `ToolCallManager`; `AgentSessionBuilder::build` just calls it.
//!
//! HOW: `SessionParts` carries the session's shared parts behind object-safe
//! traits ([`ContextSearch`], [`MemoryAccess`]) so constructors don't depend on
//! the session's store types — the session builder can change its store types
//! after the shed is set.

use std::collections::hash_map::Entry;
use std::collections::HashMap;
use std::sync::Arc;

use async_trait::async_trait;
use foundation_db::traits::DocumentStore;
use foundation_db::StorageResult;

use crate::agentic::context::{ContextProvider, KnowledgeHit, SearchMode};
use crate::agentic::embedding::EmbeddingProvider;
use crate::agentic::errors::AgenticError;
use crate::agentic::memory::MemoryHierarchy;
use crate::agentic::memory_store::{MemoryStore, SessionMemory};
use crate::agentic::tool_impl::{ToolCallManager, ToolImpl};
use crate::agentic::tools::shed::{ToolDiscovery, SHED_TOOL_NAME};
use crate::types::{MemoryFact, SessionId};

// ---------------------------------------------------------------------------
// Session parts a tool can use
// ---------------------------------------------------------------------------

/// Recall over a session's history and memory — what `search_context` needs.
/// Implemented by every `ContextProvider<D, M>`.
#[async_trait]
pub trait ContextSearch: Send + Sync {
    /// The session searched.
    fn session_id(&self) -> &SessionId;
    /// Up to `k` hits for `query`, ranked by relevance.
    async fn search(&self, query: &str, mode: SearchMode, k: usize) -> Vec<KnowledgeHit>;
}

#[async_trait]
impl<D: DocumentStore + 'static, M: MemoryStore + 'static> ContextSearch for ContextProvider<D, M> {
    fn session_id(&self) -> &SessionId {
        ContextProvider::session_id(self)
    }

    async fn search(&self, query: &str, mode: SearchMode, k: usize) -> Vec<KnowledgeHit> {
        ContextProvider::search(self, query, mode, k).await
    }
}

/// Read and write a session's memory tiers — what the `memory` tool needs.
/// Implemented by every `MemoryHierarchy<M, D>`.
#[async_trait]
pub trait MemoryAccess: Send + Sync {
    /// The session whose memory this is.
    fn session_id(&self) -> &SessionId;
    /// The latest working memory, observation and reflection records.
    async fn hydrate(&self) -> StorageResult<SessionMemory>;
    /// Write a new working-memory version (`prev_version + 1`) holding `facts`.
    async fn update_working_memory(
        &self,
        facts: Vec<MemoryFact>,
        prev_version: u64,
    ) -> StorageResult<()>;
}

#[async_trait]
impl<M: MemoryStore + 'static, D: DocumentStore + 'static> MemoryAccess for MemoryHierarchy<M, D> {
    fn session_id(&self) -> &SessionId {
        MemoryHierarchy::session_id(self)
    }

    async fn hydrate(&self) -> StorageResult<SessionMemory> {
        self.coordinator()
            .hydrate_async(MemoryHierarchy::session_id(self))
            .await
    }

    async fn update_working_memory(
        &self,
        facts: Vec<MemoryFact>,
        prev_version: u64,
    ) -> StorageResult<()> {
        MemoryHierarchy::update_working_memory(self, facts, prev_version).await
    }
}

/// What a [`ToolConstructor`] can use from the session being built.
///
/// Shared `Arc`s — the same ones the session holds — so a tool can keep what
/// it takes for the life of the session, and cloning one is a refcount bump.
#[derive(Clone)]
pub struct SessionParts {
    /// The session's id.
    pub session_id: SessionId,
    /// The session's context provider (history + memory recall, with the
    /// session's embedder when it has one).
    pub context: Arc<dyn ContextSearch>,
    /// The session's memory hierarchy.
    pub memory: Arc<dyn MemoryAccess>,
    /// The session's embedder and embedding model id, when it has one. With
    /// one, `shed` searches tools by embedding.
    pub embedder: Option<(Arc<dyn EmbeddingProvider>, String)>,
}

// ---------------------------------------------------------------------------
// ToolConstructor
// ---------------------------------------------------------------------------

/// Makes a tool for a session. Called once, inside `AgentSession::build()`.
pub trait ToolConstructor: Send + Sync {
    /// The tool's name: the shed's key, known before construction. The built
    /// tool's `definition().name()` must match it.
    fn name(&self) -> &str;

    /// Build the tool for `session`.
    fn construct(&self, session: &SessionParts) -> Arc<dyn ToolImpl>;

    /// The tool itself, when the constructor doesn't need a session (a
    /// ready-made tool). `None` for session-dependent constructors.
    fn prebuilt(&self) -> Option<Arc<dyn ToolImpl>> {
        None
    }
}

/// A ready-made tool: its constructor ignores the session.
struct Prebuilt {
    name: String,
    tool: Arc<dyn ToolImpl>,
}

impl Prebuilt {
    fn new(tool: Arc<dyn ToolImpl>) -> Self {
        Self {
            name: tool.definition().name().to_string(),
            tool,
        }
    }
}

impl ToolConstructor for Prebuilt {
    fn name(&self) -> &str {
        &self.name
    }

    fn construct(&self, _session: &SessionParts) -> Arc<dyn ToolImpl> {
        Arc::clone(&self.tool)
    }

    fn prebuilt(&self) -> Option<Arc<dyn ToolImpl>> {
        Some(Arc::clone(&self.tool))
    }
}

impl<T: ToolImpl + 'static> From<T> for Box<dyn ToolConstructor> {
    fn from(tool: T) -> Self {
        Box::new(Prebuilt::new(Arc::new(tool)))
    }
}

impl<T: ToolImpl + 'static> From<Arc<T>> for Box<dyn ToolConstructor> {
    fn from(tool: Arc<T>) -> Self {
        Box::new(Prebuilt::new(tool))
    }
}

impl From<Arc<dyn ToolImpl>> for Box<dyn ToolConstructor> {
    fn from(tool: Arc<dyn ToolImpl>) -> Self {
        Box::new(Prebuilt::new(tool))
    }
}

/// A closure constructor.
struct FnConstructor<F> {
    name: String,
    make: F,
}

impl<F> ToolConstructor for FnConstructor<F>
where
    F: Fn(&SessionParts) -> Arc<dyn ToolImpl> + Send + Sync,
{
    fn name(&self) -> &str {
        &self.name
    }

    fn construct(&self, session: &SessionParts) -> Arc<dyn ToolImpl> {
        (self.make)(session)
    }
}

/// A closure constructor, for tools that need the session.
///
/// ```ignore
/// ToolShed::new().tool(tool_fn("notes", |s| Arc::new(NotesTool::new(Arc::clone(&s.memory)))))
/// ```
pub fn tool_fn<F>(name: &str, f: F) -> Box<dyn ToolConstructor>
where
    F: Fn(&SessionParts) -> Arc<dyn ToolImpl> + Send + Sync + 'static,
{
    Box::new(FnConstructor {
        name: name.to_string(),
        make: f,
    })
}

// ---------------------------------------------------------------------------
// ToolShed
// ---------------------------------------------------------------------------

/// Why a [`ToolShed`] (or a `ToolPreset`) couldn't produce its tools.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum ToolShedError {
    /// Two tools were added under this name.
    DuplicateTool(String),
    /// A tool was added under a reserved name (`shed`).
    ReservedName(String),
    /// A constructor built a tool whose definition has a different name.
    NameMismatch { expected: String, actual: String },
    /// A session-dependent tool was asked for without a session (e.g.
    /// `ToolPreset::register_all` on a preset holding `search_context`).
    NeedsSession(String),
    /// Indexing the tools for `shed`'s embedding search failed.
    Discovery(String),
}

impl std::fmt::Display for ToolShedError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ToolShedError::DuplicateTool(name) => {
                write!(f, "tool '{name}' was added to the toolshed twice")
            }
            ToolShedError::ReservedName(name) => {
                write!(f, "'{name}' is reserved for the built-in discovery tool")
            }
            ToolShedError::NameMismatch { expected, actual } => write!(
                f,
                "constructor for '{expected}' built a tool named '{actual}'"
            ),
            ToolShedError::NeedsSession(name) => write!(
                f,
                "tool '{name}' is built from a session; add it to a ToolShed instead"
            ),
            ToolShedError::Discovery(reason) => {
                write!(f, "indexing tools for discovery failed: {reason}")
            }
        }
    }
}

impl std::error::Error for ToolShedError {}

impl From<ToolShedError> for AgenticError {
    fn from(e: ToolShedError) -> Self {
        AgenticError::Session(format!("toolshed: {e}"))
    }
}

/// The tools an agent session can call, implementations included.
///
/// ```ignore
/// let tools = ToolShed::new()
///     .tool(GreetTool)                         // ready-made
///     .tools(ToolPreset::files(fs))            // a preset
///     .tools(ToolPreset::search_context());    // built from the session
///
/// let agent = AgentSession::builder(router).with_toolshed(tools).build()?;
/// ```
///
/// The model is offered only the built-in `shed` meta-tool up front; the tools
/// it returns become active and are declared on later requests (see
/// `ToolCallManager::offered_tools`).
#[derive(Default)]
pub struct ToolShed {
    /// Keyed by tool name: O(1) lookup.
    tools: HashMap<String, Box<dyn ToolConstructor>>,
    /// Problems found while adding tools (duplicate or reserved names),
    /// reported by [`build`](Self::build).
    problems: Vec<ToolShedError>,
}

impl std::fmt::Debug for ToolShed {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ToolShed")
            .field("tools", &self.names())
            .field("problems", &self.problems)
            .finish()
    }
}

impl ToolShed {
    /// An empty shed.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Add one tool: a ready-made `ToolImpl`, or any `ToolConstructor`.
    #[must_use]
    pub fn tool(mut self, tool: impl Into<Box<dyn ToolConstructor>>) -> Self {
        self.insert(tool.into());
        self
    }

    /// Add a list: a `ToolPreset`, a `Vec<Box<dyn ToolConstructor>>`, …
    #[must_use]
    pub fn tools(mut self, tools: impl IntoIterator<Item = Box<dyn ToolConstructor>>) -> Self {
        for tool in tools {
            self.insert(tool);
        }
        self
    }

    fn insert(&mut self, constructor: Box<dyn ToolConstructor>) {
        let name = constructor.name().to_string();
        if name == SHED_TOOL_NAME {
            self.problems.push(ToolShedError::ReservedName(name));
            return;
        }
        match self.tools.entry(name) {
            Entry::Occupied(taken) => self
                .problems
                .push(ToolShedError::DuplicateTool(taken.key().clone())),
            Entry::Vacant(slot) => {
                slot.insert(constructor);
            }
        }
    }

    /// The constructor for `name`.
    #[must_use]
    pub fn get(&self, name: &str) -> Option<&dyn ToolConstructor> {
        self.tools.get(name).map(AsRef::as_ref)
    }

    /// True when a tool named `name` was added.
    #[must_use]
    pub fn contains(&self, name: &str) -> bool {
        self.tools.contains_key(name)
    }

    /// The tool names, sorted.
    #[must_use]
    pub fn names(&self) -> Vec<&str> {
        let mut names: Vec<&str> = self.tools.keys().map(String::as_str).collect();
        names.sort_unstable();
        names
    }

    /// Number of tools.
    #[must_use]
    pub fn len(&self) -> usize {
        self.tools.len()
    }

    /// True when no tools were added.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.tools.is_empty()
    }

    /// Construct every tool for this session and return the populated
    /// `ToolCallManager`. Called by `AgentSession::build()`. When the session
    /// has an embedder, the tools are indexed for `shed`'s embedding search.
    ///
    /// # Errors
    /// [`ToolShedError::DuplicateTool`] if a name was added twice,
    /// [`ToolShedError::ReservedName`] for a tool named `shed`,
    /// [`ToolShedError::NameMismatch`] if a constructor built a tool under a
    /// different name, [`ToolShedError::Discovery`] if indexing fails.
    pub fn build(self, session: &SessionParts) -> Result<ToolCallManager, ToolShedError> {
        if let Some(problem) = self.problems.into_iter().next() {
            return Err(problem);
        }

        let manager = ToolCallManager::new(session.session_id.clone());
        let mut constructors: Vec<(String, Box<dyn ToolConstructor>)> =
            self.tools.into_iter().collect();
        constructors.sort_by(|(a, _), (b, _)| a.cmp(b));
        for (name, constructor) in constructors {
            let tool = constructor.construct(session);
            let built = tool.definition().name().to_string();
            if built != name {
                return Err(ToolShedError::NameMismatch {
                    expected: name,
                    actual: built,
                });
            }
            manager.register(tool);
        }

        if let Some((embedder, model)) = &session.embedder {
            let discovery = ToolDiscovery::in_memory(Arc::clone(embedder), model.clone());
            manager
                .enable_discovery(Arc::new(discovery))
                .map_err(|e| ToolShedError::Discovery(e.to_string()))?;
        }

        Ok(manager)
    }
}
