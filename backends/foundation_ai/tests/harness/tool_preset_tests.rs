//! `harness::ToolPreset` — pre-built tool collections (spec-60 F15).
//!
//! WHY: `ToolPreset` is the one-call way to provision an agent's tools and the
//! source of `child_tools` for the F15 agent tool. Its per-tool constructors
//! (`files`, `memory`, `shed`) and the composites (`minimal_sub_agent`,
//! `standard`) were untested — only `shell()` and `agent()` had coverage — so a
//! preset could silently ship the wrong tool set.
//!
//! WHAT: asserts each constructor produces exactly the tools it claims (by
//! registry name, not just count), that composition is additive, and that the
//! two ways of consuming a preset (`register_all` / `into_manager` /
//! `as_child_tools`) agree.
//!
//! HOW: everything is in-memory — `MemoryFs` for the VFS, `KvMemoryStore` +
//! `MemoryDocumentStore` for memory. Offline and deterministic.

use std::sync::Arc;

use foundation_ai::agentic::memory::{MemoryConfig, MemoryHierarchy};
use foundation_ai::agentic::memory_coordinator::MemoryCoordinator;
use foundation_ai::agentic::memory_store::KvMemoryStore;
use foundation_ai::agentic::token_ledger::TokenLedger;
use foundation_ai::agentic::tool_impl::ToolCallManager;
use foundation_ai::agentic::{
    CacheStats, EmbeddingError, EmbeddingProvider, EmbeddingVector, ToolDiscovery, UserId,
};
use foundation_ai::harness::ToolPreset;
use foundation_ai::types::routable_provider::ProviderRouter;
use foundation_ai::types::{ModelId, SessionId, Tool};
use foundation_db::{MemoryDocumentStore, MemoryStorage};
use foundation_nativeapis::MemoryFs;
use foundation_vectors::metric::DistanceMetric;
use foundation_vectors::store::{InMemoryVectorStore, VectorStoreConfig};

type Doc = MemoryDocumentStore;
type Mem = KvMemoryStore<MemoryStorage>;
type Hierarchy = MemoryHierarchy<Mem, Doc>;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn fs() -> Arc<MemoryFs> {
    Arc::new(MemoryFs::new())
}

fn hierarchy() -> Arc<Hierarchy> {
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

fn empty_router() -> ProviderRouter {
    ProviderRouter::builder().build()
}

/// A deterministic stand-in embedder: `ToolDiscovery` only needs *an*
/// `EmbeddingProvider` to construct, and these tests assert on which tools a
/// preset contains, never on semantic search quality.
struct StubEmbedder;

impl EmbeddingProvider for StubEmbedder {
    fn embed(&self, _text: &str, _model_id: &str) -> Result<EmbeddingVector, EmbeddingError> {
        Ok(EmbeddingVector {
            data: vec![0.0; 4],
            dimensions: 4,
            model_id: "stub".into(),
        })
    }
    fn embed_batch(
        &self,
        texts: &[String],
        model_id: &str,
    ) -> Result<Vec<EmbeddingVector>, EmbeddingError> {
        texts.iter().map(|t| self.embed(t, model_id)).collect()
    }
    fn register_model(&self, _model_id: &str, _dimensions: u16) {}
    fn cache_stats(&self) -> CacheStats {
        CacheStats::default()
    }
    fn clear_cache(&self) {}
}

fn discovery() -> Arc<ToolDiscovery> {
    let store = Arc::new(InMemoryVectorStore::new(VectorStoreConfig::new(4, DistanceMetric::Cosine)));
    Arc::new(ToolDiscovery::new(
        store,
        Arc::new(StubEmbedder),
        "stub".into(),
    ))
}

/// Registry names of every tool in a preset, sorted for stable comparison.
fn names(preset: &ToolPreset) -> Vec<String> {
    let mut n: Vec<String> = preset
        .tools()
        .iter()
        .map(|t| t.definition().name().to_string())
        .collect();
    n.sort();
    n
}

// ---------------------------------------------------------------------------
// Individual constructors
// ---------------------------------------------------------------------------

#[test]
fn files_preset_provides_read_write_edit() {
    let p = ToolPreset::files(fs());
    assert_eq!(p.len(), 3);
    assert_eq!(names(&p), vec!["edit", "read", "write"]);
}

#[test]
fn shell_preset_provides_bash() {
    let p = ToolPreset::shell();
    assert_eq!(names(&p), vec!["bash"]);
}

#[test]
fn memory_preset_provides_one_multicommand_tool() {
    let p = ToolPreset::memory(hierarchy());
    assert_eq!(names(&p), vec!["memory"]);

    // memory is a MultiCommands tool exposing add/remove/replace (F14/F19).
    match p.tools()[0].definition() {
        Tool::MultiCommands(name, cmds) => {
            assert_eq!(name, "memory");
            let mut sub: Vec<&str> = cmds.iter().map(|c| c.name.as_str()).collect();
            sub.sort_unstable();
            assert_eq!(sub, vec!["add", "remove", "replace"]);
        }
        other => panic!("memory must be MultiCommands, got {other:?}"),
    }
}

#[test]
fn agent_preset_provides_the_six_command_agent_tool() {
    let p = ToolPreset::agent::<Doc, Mem>(
        empty_router(),
        0,
        5,
        ModelId::Name("mock".into(), None),
        "/tmp/test-delegation",
        UserId("test".into()),
        vec![],
    );
    assert_eq!(names(&p), vec!["agent"]);

    match p.tools()[0].definition() {
        Tool::MultiCommands(name, cmds) => {
            assert_eq!(name, "agent");
            let mut sub: Vec<&str> = cmds.iter().map(|c| c.name.as_str()).collect();
            sub.sort_unstable();
            assert_eq!(
                sub,
                vec!["check", "pause", "result", "resume", "start", "stop"]
            );
        }
        other => panic!("agent must be MultiCommands, got {other:?}"),
    }
}

// ---------------------------------------------------------------------------
// Composites
// ---------------------------------------------------------------------------

#[test]
fn minimal_sub_agent_is_files_plus_shell() {
    let p = ToolPreset::minimal_sub_agent(fs());
    assert_eq!(names(&p), vec!["bash", "edit", "read", "write"]);
}

#[test]
fn minimal_sub_agent_excludes_delegation_and_memory() {
    // A sub-agent that could itself delegate would allow unbounded chains, so
    // the minimal preset must not carry the agent tool (nor memory).
    let n = names(&ToolPreset::minimal_sub_agent(fs()));
    assert!(!n.contains(&"agent".to_string()), "got: {n:?}");
    assert!(!n.contains(&"memory".to_string()), "got: {n:?}");
}

#[test]
fn shed_preset_provides_the_discovery_metatool() {
    let p = ToolPreset::shed(discovery());
    assert_eq!(names(&p), vec!["shed"]);
}

#[test]
fn standard_is_files_shell_memory_and_shed() {
    let p = ToolPreset::standard(fs(), hierarchy(), discovery());
    assert_eq!(
        names(&p),
        vec!["bash", "edit", "memory", "read", "shed", "write"]
    );
}

#[test]
fn standard_excludes_the_agent_tool() {
    // Delegation is opt-in — `standard` must not silently grant it.
    let n = names(&ToolPreset::standard(fs(), hierarchy(), discovery()));
    assert!(!n.contains(&"agent".to_string()), "got: {n:?}");
}

// ---------------------------------------------------------------------------
// Composition + consumption
// ---------------------------------------------------------------------------

#[test]
fn merge_is_additive() {
    let combined = ToolPreset::files(fs()).merge(ToolPreset::shell());
    assert_eq!(combined.len(), 4);
}

#[test]
fn add_operator_matches_merge() {
    let via_op = ToolPreset::files(fs()) + ToolPreset::shell();
    let via_merge = ToolPreset::files(fs()).merge(ToolPreset::shell());
    assert_eq!(names(&via_op), names(&via_merge));
}

#[test]
fn empty_and_default_are_empty() {
    assert!(ToolPreset::empty().is_empty());
    assert_eq!(ToolPreset::empty().len(), 0);
    assert!(ToolPreset::default().is_empty());
}

#[test]
fn from_tools_round_trips() {
    let src = ToolPreset::files(fs());
    let expected = names(&src);
    let rebuilt = ToolPreset::from_tools(src.tools().to_vec());
    assert_eq!(names(&rebuilt), expected);
}

#[test]
fn register_all_puts_every_tool_on_the_manager() {
    let mgr = ToolCallManager::new(SessionId::new());
    ToolPreset::minimal_sub_agent(fs()).register_all(&mgr);

    let mut registered = mgr.names();
    registered.sort();
    assert_eq!(registered, vec!["bash", "edit", "read", "write"]);
}

#[test]
fn into_manager_matches_register_all() {
    let mgr = ToolPreset::minimal_sub_agent(fs()).into_manager(SessionId::new());
    let mut registered = mgr.names();
    registered.sort();
    assert_eq!(registered, vec!["bash", "edit", "read", "write"]);
}

#[test]
fn as_child_tools_clones_without_consuming() {
    let p = ToolPreset::files(fs());
    let children = p.as_child_tools();
    assert_eq!(children.len(), 3);
    // The preset is untouched — Arc clone, not a move.
    assert_eq!(p.len(), 3);
}

#[test]
fn preset_tools_survive_into_a_toolshed() {
    // End-to-end: preset → manager → ToolShed is what an agent session actually
    // hands the model, so verify the tools arrive there.
    let mgr = ToolPreset::minimal_sub_agent(fs()).into_manager(SessionId::new());
    let shed = mgr.build_toolshed();

    let mut shed_names: Vec<String> =
        shed.tools.iter().map(|t| t.name().to_string()).collect();
    shed_names.sort();
    assert_eq!(shed_names, vec!["bash", "edit", "read", "write"]);
}
