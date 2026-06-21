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

fn parse_search_mode(s: &str) -> Result<SearchMode, ToolError> {
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

fn parse_file_search_kind(s: &str) -> Result<FileSearchKind, ToolError> {
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agentic::context::{ContextConfig, KnowledgeHit};
    use crate::agentic::memory_store::KvMemoryStore;
    use crate::agentic::message_api::MessageApi;
    use crate::agentic::token_ledger::TokenLedger;
    use crate::types::{MessageRole, Messages, SessionId, SessionRecord, TextContent};
    use foundation_db::{MemoryDocumentStore, MemoryStorage};
    use std::sync::Arc;

    type TestCtxProvider = ContextProvider<MemoryDocumentStore, KvMemoryStore<MemoryStorage>>;

    fn make_context_provider() -> TestCtxProvider {
        let doc_store = MemoryDocumentStore::new();
        let kv_store = KvMemoryStore::new(MemoryStorage::new());
        let session_id = SessionId::new();
        let message_api = MessageApi::new(session_id.clone(), doc_store);
        let ledger = TokenLedger::new();

        ContextProvider::new(
            session_id,
            message_api,
            Arc::new(kv_store),
            ledger,
            Some("test".into()),
            ContextConfig::default(),
        )
    }

    fn user_msg(text: &str) -> Messages {
        Messages::User {
            id: foundation_compact::ids::new_scru128(),
            role: MessageRole::User,
            content: UserModelContent::Text(TextContent {
                content: text.into(),
                signature: None,
            }),
            signature: None,
        }
    }

    // -- SearchContextTool --

    #[test]
    fn search_context_tool_definition() {
        let ctx = make_context_provider();
        let tool = SearchContextTool::new(ctx);
        let def = tool.definition();
        assert_eq!(def.name, "search_context");
        assert_eq!(def.category, "search");
        assert!(def.description.contains("knowledge"));
    }

    #[test]
    fn search_context_requires_query() {
        futures_lite::future::block_on(async {
            let ctx = make_context_provider();
            let tool = SearchContextTool::new(ctx);
            let result = tool.execute(HashMap::new()).await;
            assert!(result.is_err());
            let err = result.unwrap_err();
            assert!(matches!(err, ToolError::InvalidArguments { .. }));
        });
    }

    #[test]
    fn search_context_returns_empty_on_no_match() {
        futures_lite::future::block_on(async {
            let ctx = make_context_provider();
            let tool = SearchContextTool::new(ctx);
            let args = HashMap::from([
                ("query".into(), ArgType::Text("nonexistent_xyz".into())),
                ("mode".into(), ArgType::Text("Semantic".into())),
            ]);
            let result = tool.execute(args).await.unwrap();
            if let UserModelContent::Text(tc) = &result.content {
                let hits: Vec<KnowledgeHit> = serde_json::from_str(&tc.content).unwrap();
                assert!(hits.is_empty());
            }
        });
    }

    #[test]
    fn search_context_finds_matching_messages() {
        futures_lite::future::block_on(async {
            let ctx = make_context_provider();
            ctx.message_api().append(SessionRecord::Conversation {
                message: user_msg("the authentication module handles JWT tokens"),
            });
            let _ = ctx.message_api().flush();

            let tool = SearchContextTool::new(ctx);
            let args = HashMap::from([
                ("query".into(), ArgType::Text("authentication".into())),
                ("mode".into(), ArgType::Text("Semantic".into())),
            ]);
            let result = tool.execute(args).await.unwrap();
            if let UserModelContent::Text(tc) = &result.content {
                let hits: Vec<KnowledgeHit> = serde_json::from_str(&tc.content).unwrap();
                assert!(!hits.is_empty());
                assert_eq!(hits[0].source, "message");
                assert!(hits[0].content.contains("authentication"));
            }
        });
    }

    #[test]
    fn search_context_hybrid_searches_messages_and_memory() {
        futures_lite::future::block_on(async {
            let ctx = make_context_provider();
            ctx.message_api().append(SessionRecord::Conversation {
                message: user_msg("the database uses postgres"),
            });
            let _ = ctx.message_api().flush();

            let working = SessionRecord::WorkingMemory {
                id: foundation_compact::ids::new_scru128(),
                facts: vec![crate::types::MemoryFact {
                    fact: "database backend is postgres".into(),
                    asserted_at: foundation_compact::SystemTime::UNIX_EPOCH,
                    source_message_id: foundation_compact::ids::new_scru128(),
                    confidence: 0.9,
                }],
                version: 1,
                timestamp: foundation_compact::SystemTime::UNIX_EPOCH,
            };
            ctx.memory_store()
                .set_async(&SessionId::new(), &working)
                .await
                .unwrap();

            let tool = SearchContextTool::new(ctx);
            let args = HashMap::from([
                ("query".into(), ArgType::Text("database".into())),
                ("mode".into(), ArgType::Text("Hybrid".into())),
            ]);
            let result = tool.execute(args).await.unwrap();
            if let UserModelContent::Text(tc) = &result.content {
                let hits: Vec<KnowledgeHit> = serde_json::from_str(&tc.content).unwrap();
                assert!(
                    hits.iter().any(|h| h.source == "message"),
                    "should have message hits"
                );
            }
        });
    }

    #[test]
    fn search_context_invalid_mode_errors() {
        futures_lite::future::block_on(async {
            let ctx = make_context_provider();
            let tool = SearchContextTool::new(ctx);
            let args = HashMap::from([
                ("query".into(), ArgType::Text("test".into())),
                ("mode".into(), ArgType::Text("BadMode".into())),
            ]);
            let result = tool.execute(args).await;
            assert!(result.is_err());
        });
    }

    #[test]
    fn search_context_graph_returns_empty() {
        futures_lite::future::block_on(async {
            let ctx = make_context_provider();
            let tool = SearchContextTool::new(ctx);
            let args = HashMap::from([
                ("query".into(), ArgType::Text("test".into())),
                ("mode".into(), ArgType::Text("Graph".into())),
            ]);
            let result = tool.execute(args).await.unwrap();
            if let UserModelContent::Text(tc) = &result.content {
                let hits: Vec<KnowledgeHit> = serde_json::from_str(&tc.content).unwrap();
                assert!(hits.is_empty());
            }
        });
    }

    // -- SearchFileTool --

    fn make_vfs_tool() -> SearchFileTool {
        let fs = Arc::new(foundation_nativeapis::MemoryFs::new());
        SearchFileTool::from_vfs(fs, "/".into())
    }

    fn make_vfs_tool_with_files() -> SearchFileTool {
        let fs = foundation_nativeapis::MemoryFs::new();
        fs.write_file("/hello.txt", b"hello world\ngoodbye world\n").unwrap();
        fs.write_file("/foo.rs", b"fn main() {}").unwrap();
        fs.write_file("/bar.txt", b"text content").unwrap();
        let fs = Arc::new(fs);
        SearchFileTool::from_vfs(fs, "/".into())
    }

    #[test]
    fn search_file_tool_definition() {
        let tool = make_vfs_tool();
        let def = tool.definition();
        assert_eq!(def.name, "search_file");
        assert_eq!(def.category, "search_files");
        assert!(def.description.contains("grep"));
    }

    #[test]
    fn search_file_requires_query() {
        futures_lite::future::block_on(async {
            let tool = make_vfs_tool();
            let result = tool.execute(HashMap::new()).await;
            assert!(result.is_err());
        });
    }

    #[test]
    fn search_file_grep_vfs() {
        futures_lite::future::block_on(async {
            let tool = make_vfs_tool_with_files();
            let args = HashMap::from([
                ("query".into(), ArgType::Text("hello".into())),
                ("kind".into(), ArgType::Text("Grep".into())),
            ]);
            let result = tool.execute(args).await.unwrap();
            if let UserModelContent::Text(tc) = &result.content {
                let matches: Vec<FileMatch> = serde_json::from_str(&tc.content).unwrap();
                assert!(!matches.is_empty());
                assert_eq!(matches[0].line_number, 1);
                assert!(matches[0].content.contains("hello"));
            }
        });
    }

    #[test]
    fn search_file_find_vfs() {
        futures_lite::future::block_on(async {
            let tool = make_vfs_tool_with_files();
            let args = HashMap::from([
                ("query".into(), ArgType::Text(r"\.rs$".into())),
                ("kind".into(), ArgType::Text("Find".into())),
            ]);
            let result = tool.execute(args).await.unwrap();
            if let UserModelContent::Text(tc) = &result.content {
                let matches: Vec<FileMatch> = serde_json::from_str(&tc.content).unwrap();
                assert_eq!(matches.len(), 1);
                assert!(matches[0].path.contains("foo.rs"));
            }
        });
    }

    #[test]
    fn search_file_invalid_regex_errors() {
        futures_lite::future::block_on(async {
            let tool = make_vfs_tool();
            let args = HashMap::from([
                ("query".into(), ArgType::Text("[invalid".into())),
                ("kind".into(), ArgType::Text("Grep".into())),
            ]);
            let result = tool.execute(args).await;
            assert!(result.is_err());
        });
    }

    #[test]
    fn search_file_invalid_kind_errors() {
        futures_lite::future::block_on(async {
            let tool = make_vfs_tool();
            let args = HashMap::from([
                ("query".into(), ArgType::Text("test".into())),
                ("kind".into(), ArgType::Text("BadKind".into())),
            ]);
            let result = tool.execute(args).await;
            assert!(result.is_err());
        });
    }

    // -- ToolCallManager integration --

    #[test]
    fn search_tools_register_in_toolshed() {
        let mgr =
            crate::agentic::tool_impl::ToolCallManager::new(SessionId::new());

        let ctx = make_context_provider();
        mgr.register(Arc::new(SearchContextTool::new(ctx)));
        mgr.register(Arc::new(make_vfs_tool()));

        let shed = mgr.build_toolshed();
        assert!(shed.search.is_some(), "search slot should be populated");
        assert_eq!(shed.search.as_ref().unwrap().name, "search_context");

        assert!(
            shed.search_files.is_some(),
            "search_files slot should be populated"
        );
        assert_eq!(shed.search_files.as_ref().unwrap().name, "search_file");
    }

    // -- Parse helpers --

    #[test]
    fn parse_search_mode_variants() {
        assert_eq!(parse_search_mode("Semantic").unwrap(), SearchMode::Semantic);
        assert_eq!(parse_search_mode("semantic").unwrap(), SearchMode::Semantic);
        assert_eq!(parse_search_mode("Memory").unwrap(), SearchMode::Memory);
        assert_eq!(parse_search_mode("Graph").unwrap(), SearchMode::Graph);
        assert_eq!(parse_search_mode("Hybrid").unwrap(), SearchMode::Hybrid);
        assert!(parse_search_mode("unknown").is_err());
    }

    #[test]
    fn parse_file_search_kind_variants() {
        assert_eq!(
            parse_file_search_kind("Grep").unwrap(),
            FileSearchKind::Grep
        );
        assert_eq!(
            parse_file_search_kind("Find").unwrap(),
            FileSearchKind::Find
        );
        assert_eq!(
            parse_file_search_kind("MultiGrep").unwrap(),
            FileSearchKind::MultiGrep
        );
        assert_eq!(
            parse_file_search_kind("multi_grep").unwrap(),
            FileSearchKind::MultiGrep
        );
        assert!(parse_file_search_kind("bad").is_err());
    }

    // -- KnowledgeHit / FileMatch serialization --

    #[test]
    fn knowledge_hit_roundtrip() {
        let hit = KnowledgeHit {
            source: "message".into(),
            score: 0.95,
            content: "test content".into(),
            record_ref: Some("ref-1".into()),
        };
        let json = serde_json::to_string(&hit).unwrap();
        let back: KnowledgeHit = serde_json::from_str(&json).unwrap();
        assert_eq!(back.source, "message");
        assert_eq!(back.score, 0.95);
    }

    #[test]
    fn file_match_roundtrip() {
        let m = FileMatch {
            path: "/src/main.rs".into(),
            line_number: 42,
            content: "fn main()".into(),
            score: 1.0,
        };
        let json = serde_json::to_string(&m).unwrap();
        let back: FileMatch = serde_json::from_str(&json).unwrap();
        assert_eq!(back.path, "/src/main.rs");
        assert_eq!(back.line_number, 42);
    }
}
