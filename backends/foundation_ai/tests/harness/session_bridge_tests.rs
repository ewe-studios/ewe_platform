//! Harness → `AgentSession` bridge tests.
//!
//! WHY: A preset is only "super easy" if the builder it hands back actually
//! produces a working session. This verifies the bridge end-to-end: a preset's
//! `into_agent_builder` (and the `*_session` convenience wrappers) yields a
//! builder that passes preflight and builds a real `AgentSession` with the
//! given id and the wired primary model — all offline.
//!
//! WHAT: builds sessions from the cloud presets (which construct without
//! network) using the in-memory document + memory stores, then asserts the
//! session id round-trips and the router carries the expected models.
//!
//! HOW: uses the same store aliases as the agentic session tests
//! (`MemoryDocumentStore`, `KvMemoryStore<MemoryStorage>`); cloud preflight
//! (`AllowAllAccess`, empty toolshed, default budget) needs no network.

use foundation_ai::agentic::KvMemoryStore;
use foundation_ai::harness::{self, CLAUDE_OPUS};
use foundation_ai::types::{ModelId, SessionId};
use foundation_db::{MemoryDocumentStore, MemoryStorage};

type TestMemStore = KvMemoryStore<MemoryStorage>;
type TestDocStore = MemoryDocumentStore;

fn named(id: &str) -> ModelId {
    ModelId::Name(id.to_string(), None)
}

#[test]
fn into_agent_builder_builds_session_with_id() {
    let id = SessionId::from_name("harness-bridge-preset");
    let preset = harness::claude_router("test-key").expect("router builds");

    let session = preset
        .into_agent_builder::<TestDocStore, TestMemStore>(id.clone())
        .build()
        .expect("session builds from preset");

    assert_eq!(*session.session_id(), id);
    // The wired router is reachable and still resolves the primary model.
    assert!(session.router().resolve(&named(CLAUDE_OPUS)).is_ok());
}

#[test]
fn session_convenience_wrapper_builds_and_accepts_customisation() {
    let id = SessionId::from_name("harness-bridge-wrapper");

    let session = harness::claude_session::<TestDocStore, TestMemStore>(id.clone(), "test-key")
        .expect("builder returned")
        .with_system_prompt("You are a helpful assistant.")
        .build()
        .expect("session builds from convenience wrapper");

    assert_eq!(*session.session_id(), id);
}
