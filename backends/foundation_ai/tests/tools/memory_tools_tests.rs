//! Memory-curation tools (memory_add/remove/replace) over a real in-memory
//! `MemoryHierarchy` — spec-60 F14. Offline and deterministic.

use std::collections::HashMap;
use std::sync::Arc;

use foundation_ai::agentic::memory::{MemoryConfig, MemoryHierarchy};
use foundation_ai::agentic::memory_coordinator::MemoryCoordinator;
use foundation_ai::agentic::memory_store::KvMemoryStore;
use foundation_ai::agentic::token_ledger::TokenLedger;
use foundation_ai::agentic::tool_impl::{ToolError, ToolImpl};
use foundation_ai::agentic::tools::memory::{
    register_memory_tools, MemoryAddTool, MemoryRemoveTool, MemoryReplaceTool,
};
use foundation_ai::types::{ArgType, SessionId, SessionRecord};
use foundation_db::{MemoryDocumentStore, MemoryStorage};

type Hierarchy = MemoryHierarchy<KvMemoryStore<MemoryStorage>, MemoryDocumentStore>;

fn setup() -> Arc<Hierarchy> {
    let coordinator = MemoryCoordinator::new(KvMemoryStore::new(MemoryStorage::new()), MemoryDocumentStore::new());
    Arc::new(MemoryHierarchy::new(
        SessionId::new(),
        coordinator,
        TokenLedger::new(),
        MemoryConfig::default(),
    ))
}

fn args(pairs: &[(&str, &str)]) -> HashMap<String, ArgType> {
    pairs
        .iter()
        .map(|(k, v)| ((*k).to_string(), ArgType::Text((*v).to_string())))
        .collect()
}

/// Read the current working-memory fact strings straight from the coordinator.
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
        let add = MemoryAddTool::new(h.clone());
        add.execute(args(&[("fact", "user prefers dark mode")]))
            .await
            .expect("add 1");
        add.execute(args(&[("fact", "user is in GMT")]))
            .await
            .expect("add 2");

        let facts = facts_of(&h).await;
        assert_eq!(facts.len(), 2);
        assert!(facts.contains(&"user prefers dark mode".to_string()));
        assert!(facts.contains(&"user is in GMT".to_string()));
    });
}

#[test]
fn memory_add_missing_arg_errors() {
    futures_lite::future::block_on(async {
        let add = MemoryAddTool::new(setup());
        let err = add.execute(HashMap::new()).await.unwrap_err();
        assert!(matches!(err, ToolError::InvalidArguments { .. }));
    });
}

#[test]
fn memory_remove_deletes_matching_fact() {
    futures_lite::future::block_on(async {
        let h = setup();
        let add = MemoryAddTool::new(h.clone());
        add.execute(args(&[("fact", "keep me")])).await.unwrap();
        add.execute(args(&[("fact", "delete me")])).await.unwrap();

        let remove = MemoryRemoveTool::new(h.clone());
        remove.execute(args(&[("fact", "delete me")])).await.expect("remove");

        let facts = facts_of(&h).await;
        assert_eq!(facts, vec!["keep me".to_string()]);
    });
}

#[test]
fn memory_remove_absent_fact_errors() {
    futures_lite::future::block_on(async {
        let h = setup();
        MemoryAddTool::new(h.clone())
            .execute(args(&[("fact", "present")]))
            .await
            .unwrap();
        let err = MemoryRemoveTool::new(h)
            .execute(args(&[("fact", "not there")]))
            .await
            .unwrap_err();
        assert!(matches!(err, ToolError::Execution { .. }));
    });
}

#[test]
fn memory_replace_updates_fact_text() {
    futures_lite::future::block_on(async {
        let h = setup();
        MemoryAddTool::new(h.clone())
            .execute(args(&[("fact", "user likes tea")]))
            .await
            .unwrap();

        MemoryReplaceTool::new(h.clone())
            .execute(args(&[("old", "user likes tea"), ("new", "user likes coffee")]))
            .await
            .expect("replace");

        let facts = facts_of(&h).await;
        assert_eq!(facts, vec!["user likes coffee".to_string()]);
    });
}

#[test]
fn memory_replace_absent_fact_errors() {
    futures_lite::future::block_on(async {
        let err = MemoryReplaceTool::new(setup())
            .execute(args(&[("old", "nope"), ("new", "x")]))
            .await
            .unwrap_err();
        assert!(matches!(err, ToolError::Execution { .. }));
    });
}

#[test]
fn registering_memory_tools_fills_shed_slot() {
    use foundation_ai::agentic::tool_impl::ToolCallManager;

    let h = setup();
    let mgr = ToolCallManager::new(SessionId::new());
    register_memory_tools(&mgr, h);

    let shed = mgr.build_toolshed();
    let mem = shed.memory.expect("memory slot filled");
    assert_eq!(mem.add.name, "memory_add");
    assert_eq!(mem.remove.name, "memory_remove");
    assert_eq!(mem.replace.name, "memory_replace");
}
