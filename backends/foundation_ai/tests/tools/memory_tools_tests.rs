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

// ---------------------------------------------------------------------------
// Joined-name dispatch (`memory_add` -> memory tool, command=add)
// ---------------------------------------------------------------------------

/// Run one tool call through the manager and return the result.
async fn call(
    mgr: &foundation_ai::agentic::tool_impl::ToolCallManager,
    name: &str,
    args: &[(&str, &str)],
) -> Result<foundation_ai::agentic::tool_impl::ToolCallResult, ToolError> {
    use foundation_ai::agentic::tool_impl::ToolCallRequest;
    let mut arguments = HashMap::new();
    for (k, v) in args {
        arguments.insert((*k).to_string(), ArgType::Text((*v).to_string()));
    }
    mgr.execute_one(&ToolCallRequest {
        id: "call-1".to_string(),
        name: name.to_string(),
        arguments,
        depends_on: Vec::new(),
    })
    .await
}

#[valtron_test]
fn the_canonical_group_plus_command_call_works() {
    use foundation_ai::agentic::tool_impl::ToolCallManager;
    let mgr = ToolCallManager::new(SessionId::new());
    register_memory_tool(&mgr, setup());

    let out = futures_lite::future::block_on(call(
        &mgr,
        "memory",
        &[("command", "add"), ("fact", "the user prefers tabs")],
    ));
    assert!(out.is_ok(), "the canonical call shape must work: {out:?}");
}

#[valtron_test]
fn a_joined_name_call_resolves_to_the_group_and_command() {
    // Multi-command tools render as ONE function named for the group (`memory`)
    // with a `command` argument. Models do not reliably call them that way —
    // many have been trained on flattened tool names and emit `memory_add` with
    // no `command` at all. That used to be UnknownTool and fail the turn, even
    // though the request was unambiguous.
    use foundation_ai::agentic::tool_impl::ToolCallManager;
    let mgr = ToolCallManager::new(SessionId::new());
    register_memory_tool(&mgr, setup());

    let out = futures_lite::future::block_on(call(
        &mgr,
        "memory_add",
        &[("fact", "the user prefers spaces")],
    ));
    assert!(
        out.is_ok(),
        "`memory_add` must resolve to the memory tool with command=add: {out:?}"
    );
}

#[valtron_test]
fn an_explicit_command_argument_beats_the_joined_name() {
    // The fallback fills in what the model left out; it must never overwrite an
    // argument the model actually supplied.
    use foundation_ai::agentic::tool_impl::ToolCallManager;
    let hierarchy = setup();
    let mgr = ToolCallManager::new(SessionId::new());
    register_memory_tool(&mgr, hierarchy.clone());

    futures_lite::future::block_on(call(
        &mgr,
        "memory",
        &[("command", "add"), ("fact", "keep me")],
    ))
    .expect("seed");

    // Name says `remove`, explicit argument says `add` — the argument wins.
    let out = futures_lite::future::block_on(call(
        &mgr,
        "memory_remove",
        &[("command", "add"), ("fact", "added not removed")],
    ));
    assert!(out.is_ok(), "explicit command must be honoured: {out:?}");
    let facts = hierarchy.working_memory_facts();
    assert!(
        facts.iter().any(|f| f.contains("added not removed")),
        "the explicit `add` must have run, not the name's `remove`: {facts:?}"
    );
}

#[valtron_test]
fn an_unknown_joined_name_is_still_an_error() {
    // The fallback must not turn every typo into a silent success. A name whose
    // remainder is not one of the tool's commands stays UnknownTool.
    use foundation_ai::agentic::tool_impl::ToolCallManager;
    let mgr = ToolCallManager::new(SessionId::new());
    register_memory_tool(&mgr, setup());

    for bogus in ["memory_frobnicate", "memoryadd", "notmemory_add", "add"] {
        let out = futures_lite::future::block_on(call(&mgr, bogus, &[("fact", "x")]));
        assert!(
            matches!(out, Err(ToolError::UnknownTool(_))),
            "`{bogus}` must stay an unknown tool, got {out:?}"
        );
    }
}
