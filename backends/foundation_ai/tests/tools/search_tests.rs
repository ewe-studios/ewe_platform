use std::collections::HashMap;
use std::sync::Arc;

use foundation_ai::agentic::context::{ContextConfig, ContextProvider, KnowledgeHit, SearchMode};
use foundation_ai::agentic::memory_store::{KvMemoryStore, MemoryStore};
use foundation_ai::agentic::message_api::MessageApi;
use foundation_ai::agentic::token_ledger::TokenLedger;
use foundation_ai::agentic::tool_impl::ToolImpl;
use foundation_ai::agentic::tools::search::*;
use foundation_ai::types::{
    ArgType, MemoryFact, MessageRole, Messages, SessionId, SessionRecord, TextContent,
    UserModelContent,
};
use foundation_db::{MemoryDocumentStore, MemoryStorage};
use foundation_nativeapis::VfsFileSystem;

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
            facts: vec![MemoryFact {
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

fn make_vfs_tool() -> SearchFileTool {
    let fs = Arc::new(foundation_nativeapis::MemoryFs::new());
    SearchFileTool::from_vfs(fs, "/".into())
}

fn make_vfs_tool_with_files() -> SearchFileTool {
    let fs = foundation_nativeapis::MemoryFs::new();
    fs.write_file("/hello.txt", b"hello world\ngoodbye world\n")
        .unwrap();
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

#[test]
fn search_tools_register_in_toolshed() {
    let mgr = foundation_ai::agentic::tool_impl::ToolCallManager::new(SessionId::new());

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
