//! Memory tool (single multi-command `memory`: add/remove/replace) over a real
//! in-memory `MemoryHierarchy` — spec-60 F14 / F19. Offline and deterministic.

use std::collections::HashMap;
use std::sync::Arc;

use foundation_ai::agentic::memory::{MemoryConfig, MemoryHierarchy};
use foundation_ai::agentic::memory_coordinator::MemoryCoordinator;
use foundation_ai::agentic::memory_store::KvMemoryStore;
use foundation_ai::agentic::token_ledger::TokenLedger;
use foundation_ai::agentic::tool_impl::{ToolError, ToolImpl};
use foundation_ai::agentic::tools::memory::{register_memory_tool, MemoryTool};
use foundation_ai::types::{ArgType, SessionId, SessionRecord, Tool};
use foundation_db::{MemoryDocumentStore, MemoryStorage};

type Hierarchy = MemoryHierarchy<KvMemoryStore<MemoryStorage>, MemoryDocumentStore>;

fn setup() -> Arc<Hierarchy> {
    let coordinator = MemoryCoordinator::new(
        KvMemoryStore::new(MemoryStorage::new()),
        MemoryDocumentStore::new(),
    );
    Arc::new(MemoryHierarchy::new(
        SessionId::new(),
        coordinator,
        TokenLedger::new(),
        MemoryConfig::default(),
    ))
}

/// Build a `command`-bearing arg map.
fn cmd(command: &str, pairs: &[(&str, &str)]) -> HashMap<String, ArgType> {
    let mut m: HashMap<String, ArgType> = HashMap::new();
    m.insert("command".to_string(), ArgType::Text(command.to_string()));
    for (k, v) in pairs {
        m.insert((*k).to_string(), ArgType::Text((*v).to_string()));
    }
    m
}

async fn facts_of(h: &Hierarchy) -> Vec<String> {
    let mem = h
        .coordinator()
        .hydrate_async(h.session_id())
        .await
        .expect("hydrate");
    match mem.working {
        Some(SessionRecord::WorkingMemory { facts, .. }) => {
            facts.into_iter().map(|f| f.fact).collect()
        }
        _ => Vec::new(),
    }
}

#[test]
fn memory_add_appends_facts() {
    futures_lite::future::block_on(async {
        let h = setup();
        let tool = MemoryTool::new(h.clone());
        tool.execute(cmd("add", &[("fact", "user prefers dark mode")]))
            .await
            .expect("add 1");
        tool.execute(cmd("add", &[("fact", "user is in GMT")]))
            .await
            .expect("add 2");

        let facts = facts_of(&h).await;
        assert_eq!(facts.len(), 2);
        assert!(facts.contains(&"user prefers dark mode".to_string()));
    });
}

#[test]
fn memory_missing_command_errors() {
    futures_lite::future::block_on(async {
        let tool = MemoryTool::new(setup());
        let err = tool.execute(HashMap::new()).await.unwrap_err();
        assert!(matches!(err, ToolError::InvalidArguments { .. }));
    });
}

#[test]
fn memory_unknown_command_errors() {
    futures_lite::future::block_on(async {
        let tool = MemoryTool::new(setup());
        let err = tool.execute(cmd("frobnicate", &[])).await.unwrap_err();
        assert!(matches!(err, ToolError::InvalidArguments { .. }));
    });
}

#[test]
fn memory_remove_deletes_matching_fact() {
    futures_lite::future::block_on(async {
        let h = setup();
        let tool = MemoryTool::new(h.clone());
        tool.execute(cmd("add", &[("fact", "keep me")])).await.unwrap();
        tool.execute(cmd("add", &[("fact", "delete me")])).await.unwrap();
        tool.execute(cmd("remove", &[("fact", "delete me")]))
            .await
            .expect("remove");

        assert_eq!(facts_of(&h).await, vec!["keep me".to_string()]);
    });
}

#[test]
fn memory_remove_absent_fact_errors() {
    futures_lite::future::block_on(async {
        let tool = MemoryTool::new(setup());
        tool.execute(cmd("add", &[("fact", "present")])).await.unwrap();
        let err = tool
            .execute(cmd("remove", &[("fact", "not there")]))
            .await
            .unwrap_err();
        assert!(matches!(err, ToolError::Execution { .. }));
    });
}

#[test]
fn memory_replace_updates_fact_text() {
    futures_lite::future::block_on(async {
        let h = setup();
        let tool = MemoryTool::new(h.clone());
        tool.execute(cmd("add", &[("fact", "user likes tea")])).await.unwrap();
        tool.execute(cmd("replace", &[("old", "user likes tea"), ("new", "user likes coffee")]))
            .await
            .expect("replace");

        assert_eq!(facts_of(&h).await, vec!["user likes coffee".to_string()]);
    });
}

#[test]
fn memory_replace_absent_fact_errors() {
    futures_lite::future::block_on(async {
        let err = MemoryTool::new(setup())
            .execute(cmd("replace", &[("old", "nope"), ("new", "x")]))
            .await
            .unwrap_err();
        assert!(matches!(err, ToolError::Execution { .. }));
    });
}

#[test]
fn memory_registers_as_one_multicommand_tool() {
    use foundation_ai::agentic::tool_impl::ToolCallManager;

    let mgr = ToolCallManager::new(SessionId::new());
    register_memory_tool(&mgr, setup());

    let shed = mgr.build_toolshed();
    let memory = shed
        .tools
        .iter()
        .find(|t| t.name() == "memory")
        .expect("memory tool present");
    match memory {
        Tool::MultiCommands(name, cmds) => {
            assert_eq!(name, "memory");
            let names: Vec<&str> = cmds.iter().map(|c| c.name.as_str()).collect();
            assert!(names.contains(&"add"));
            assert!(names.contains(&"remove"));
            assert!(names.contains(&"replace"));
        }
        Tool::SingleCommand(_) => panic!("memory should be a MultiCommands tool"),
    }
}
