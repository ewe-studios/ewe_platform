//! Search tools (F32) — `SearchContextTool` (knowledge recall) and
//! `SearchFileTool` (filesystem grep/find).
//!
//! Two distinct `ToolImpl`s per Decision 14 TODO #6: `search_context` wraps
//! `ContextProvider::search` for semantic/memory/graph recall, while
//! `search_file` wraps a `FileSearch` backend delegating to
//! `foundation_nativeapis::VfsSearcher` for the actual filesystem walk.

use std::collections::HashMap;
use std::sync::Arc;

use async_trait::async_trait;
use serde::{Deserialize, Serialize};

use crate::agentic::context::{ContextProvider, SearchMode};
use crate::agentic::memory_store::MemoryStore;
use crate::agentic::tool_impl::{ToolCallResult, ToolDefinition, ToolError, ToolImpl};
use crate::types::{ArgType, Args, TextContent, UserModelContent};
use foundation_db::traits::DocumentStore;
use foundation_nativeapis::{VfsFileSystem, VfsSearchMatch, VfsSearcher};

// ---------------------------------------------------------------------------
// FileSearch types (re-exported from foundation_nativeapis)
// ---------------------------------------------------------------------------

pub use foundation_nativeapis::VfsSearchKind as FileSearchKind;

/// A single file-search hit.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileMatch {
    pub path: String,
    pub line_number: u32,
    pub content: String,
    pub score: f32,
}

impl From<VfsSearchMatch> for FileMatch {
    fn from(m: VfsSearchMatch) -> Self {
        Self {
            path: m.path,
            line_number: m.line_number,
            content: m.content,
            score: m.score,
        }
    }
}

// ---------------------------------------------------------------------------
// FileSearch trait
// ---------------------------------------------------------------------------

/// Backend trait for filesystem search — delegates to `VfsSearcher`.
pub trait FileSearch: Send + Sync {
    /// # Errors
    /// Returns `ToolError::InvalidArguments` for invalid regex or `ToolError::Execution` for I/O failures.
    fn search(
        &self,
        query: &str,
        kind: FileSearchKind,
        paths: Option<&[String]>,
    ) -> Result<Vec<FileMatch>, ToolError>;
}

// ---------------------------------------------------------------------------
// VfsSearchBackend — wraps a VfsSearcher from foundation_nativeapis
// ---------------------------------------------------------------------------

/// File search backend delegating to a `VfsSearcher` from `foundation_nativeapis`.
/// The VfsSearcher handles the cascade: CLI tools (rg/grep) → in-code VFS walk.
pub struct VfsSearchBackend {
    searcher: Box<dyn VfsSearcher>,
    root: String,
}

impl VfsSearchBackend {
    #[must_use]
    pub fn new(searcher: Box<dyn VfsSearcher>, root: String) -> Self {
        Self { searcher, root }
    }

    #[must_use]
    pub fn from_vfs<F: VfsFileSystem + 'static>(fs: Arc<F>, root: String) -> Self {
        let searcher = foundation_nativeapis::vfs_searcher(fs);
        Self { searcher, root }
    }
}

impl FileSearch for VfsSearchBackend {
    fn search(
        &self,
        query: &str,
        kind: FileSearchKind,
        paths: Option<&[String]>,
    ) -> Result<Vec<FileMatch>, ToolError> {
        let search_roots: Vec<String> = match paths {
            Some(ps) if !ps.is_empty() => ps
                .iter()
                .map(|p| {
                    if p.starts_with('/') {
                        p.clone()
                    } else if self.root == "/" {
                        format!("/{p}")
                    } else {
                        format!("{}/{p}", self.root)
                    }
                })
                .collect(),
            _ => vec![self.root.clone()],
        };

        let vfs_matches = self
            .searcher
            .search(query, kind, &search_roots)
            .map_err(|e| ToolError::Execution {
                tool: "search_file".into(),
                reason: format!("{e}"),
            })?;

        Ok(vfs_matches.into_iter().map(FileMatch::from).collect())
    }
}

// ---------------------------------------------------------------------------
// SearchContextTool
// ---------------------------------------------------------------------------

/// Tool that searches knowledge surfaces (semantic/memory/graph) via
/// `ContextProvider::search`. Category: `search`.
pub struct SearchContextTool<D, M> {
    context: ContextProvider<D, M>,
}

impl<D, M> SearchContextTool<D, M> {
    #[must_use]
    pub fn new(context: ContextProvider<D, M>) -> Self {
        Self { context }
    }
}

#[async_trait]
impl<D: DocumentStore + 'static, M: MemoryStore + 'static> ToolImpl for SearchContextTool<D, M> {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "search_context".into(),
            description: "Search your knowledge — semantic recall over prior messages, \
                          distilled memory (observations/reflections), and the code-graph. \
                          NOT the live filesystem — use search_file for that."
                .into(),
            arguments: Args::new(
                foundation_jsonschema::scheme::object()
                    .required("query", foundation_jsonschema::scheme::string().min_len(1))
                    .required(
                        "mode",
                        foundation_jsonschema::scheme::string()
                            .enum_values(vec![
                                "Semantic".into(),
                                "Memory".into(),
                                "Graph".into(),
                                "Hybrid".into(),
                            ]),
                    )
                    .optional("k", foundation_jsonschema::scheme::integer().min(1))
                    .build(),
            ),
            category: "search".into(),
        }
    }

    async fn execute(
        &self,
        arguments: HashMap<String, ArgType>,
    ) -> Result<ToolCallResult, ToolError> {
        let query = match arguments.get("query") {
            Some(ArgType::Text(s)) => s.clone(),
            _ => {
                return Err(ToolError::InvalidArguments {
                    tool: "search_context".into(),
                    reason: "missing or invalid 'query' argument".into(),
                })
            }
        };

        let mode = match arguments.get("mode") {
            Some(ArgType::Text(s)) => parse_search_mode(s)?,
            _ => SearchMode::Hybrid,
        };

        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        let k = match arguments.get("k") {
            Some(ArgType::Usize(n)) => *n,
            Some(ArgType::U32(n)) => *n as usize,
            Some(ArgType::U64(n)) => *n as usize,
            Some(ArgType::I32(n)) => *n as usize,
            Some(ArgType::I64(n)) => *n as usize,
            _ => 10,
        };

        let hits = self.context.search(&query, mode, k).await;
        let json =
            serde_json::to_string(&hits).unwrap_or_else(|_| "[]".into());

        Ok(ToolCallResult {
            content: UserModelContent::Text(TextContent {
                content: json,
                signature: None,
            }),
            error_detail: None,
        })
    }
}

pub fn parse_search_mode(s: &str) -> Result<SearchMode, ToolError> {
    match s {
        "Semantic" | "semantic" => Ok(SearchMode::Semantic),
        "Memory" | "memory" => Ok(SearchMode::Memory),
        "Graph" | "graph" => Ok(SearchMode::Graph),
        "Hybrid" | "hybrid" => Ok(SearchMode::Hybrid),
        other => Err(ToolError::InvalidArguments {
            tool: "search_context".into(),
            reason: format!("unknown search mode: {other}"),
        }),
    }
}

// ---------------------------------------------------------------------------
// SearchFileTool
// ---------------------------------------------------------------------------

/// Tool that searches the filesystem via grep/find. Category: `search_files`.
pub struct SearchFileTool {
    backend: Arc<dyn FileSearch>,
}

impl SearchFileTool {
    #[must_use]
    pub fn new(backend: Arc<dyn FileSearch>) -> Self {
        Self { backend }
    }

    #[must_use]
    pub fn from_vfs<F: VfsFileSystem + 'static>(fs: Arc<F>, root: String) -> Self {
        Self {
            backend: Arc::new(VfsSearchBackend::from_vfs(fs, root)),
        }
    }

    #[must_use]
    pub fn from_searcher(searcher: Box<dyn VfsSearcher>, root: String) -> Self {
        Self {
            backend: Arc::new(VfsSearchBackend::new(searcher, root)),
        }
    }
}

#[async_trait]
impl ToolImpl for SearchFileTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "search_file".into(),
            description: "Search files: grep file content, find by path pattern, \
                          multi-file grep. NOT for memory recall — use search_context."
                .into(),
            arguments: Args::new(
                foundation_jsonschema::scheme::object()
                    .required("query", foundation_jsonschema::scheme::string().min_len(1))
                    .optional(
                        "kind",
                        foundation_jsonschema::scheme::string()
                            .enum_values(vec![
                                "Grep".into(),
                                "Find".into(),
                                "MultiGrep".into(),
                            ]),
                    )
                    .optional(
                        "paths",
                        foundation_jsonschema::scheme::array_of(
                            foundation_jsonschema::scheme::string().build_schema(),
                        ),
                    )
                    .build(),
            ),
            category: "search_files".into(),
        }
    }

    async fn execute(
        &self,
        arguments: HashMap<String, ArgType>,
    ) -> Result<ToolCallResult, ToolError> {
        let query = match arguments.get("query") {
            Some(ArgType::Text(s)) => s.clone(),
            _ => {
                return Err(ToolError::InvalidArguments {
                    tool: "search_file".into(),
                    reason: "missing or invalid 'query' argument".into(),
                })
            }
        };

        let kind = match arguments.get("kind") {
            Some(ArgType::Text(s)) => parse_file_search_kind(s)?,
            _ => FileSearchKind::Grep,
        };

        let paths: Option<Vec<String>> = arguments.get("paths").and_then(|v| match v {
            ArgType::JSON(json_str) => serde_json::from_str(json_str).ok(),
            _ => None,
        });

        let matches = self
            .backend
            .search(&query, kind, paths.as_deref())?;

        let json =
            serde_json::to_string(&matches).unwrap_or_else(|_| "[]".into());

        Ok(ToolCallResult {
            content: UserModelContent::Text(TextContent {
                content: json,
                signature: None,
            }),
            error_detail: None,
        })
    }
}

pub fn parse_file_search_kind(s: &str) -> Result<FileSearchKind, ToolError> {
    match s {
        "Grep" | "grep" => Ok(FileSearchKind::Grep),
        "Find" | "find" => Ok(FileSearchKind::Find),
        "MultiGrep" | "multi_grep" | "multigrep" => Ok(FileSearchKind::MultiGrep),
        other => Err(ToolError::InvalidArguments {
            tool: "search_file".into(),
            reason: format!("unknown file search kind: {other}"),
        }),
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

