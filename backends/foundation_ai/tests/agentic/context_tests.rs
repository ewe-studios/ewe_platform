//! Public-API coverage for `ContextProvider::assemble_from_memory` — the
//! Decision-03 context assembly order (matrix 6.1/6.2). Synchronous and
//! deterministic; context.rs was 78% region coverage with this branch logic
//! (working/reflection/observation injection, recent messages) untested.

use std::sync::Arc;

use foundation_ai::agentic::memory_store::SessionMemory;
use foundation_ai::agentic::{
    ContextConfig, ContextProvider, KvMemoryStore, MessageApi, TokenLedger,
};
use foundation_ai::types::{
    MemoryFact, MessageRole, Messages, SessionId, SessionRecord, TextContent, UserModelContent,
};
use foundation_compact::ids::Id;
use foundation_compact::SystemTime;
use foundation_db::{MemoryDocumentStore, MemoryStorage};

type M = KvMemoryStore<MemoryStorage>;

fn provider(config: ContextConfig) -> ContextProvider<MemoryDocumentStore, M> {
    let session_id = SessionId::new();
    let ledger = TokenLedger::new();
    let message_api = MessageApi::new(session_id.clone(), MemoryDocumentStore::new());
    let store = Arc::new(KvMemoryStore::new(MemoryStorage::new()));
    ContextProvider::new(
        session_id,
        message_api,
        store,
        ledger,
        Some("SYS".into()),
        config,
    )
}

fn working_memory(fact: &str) -> SessionRecord {
    SessionRecord::WorkingMemory {
        id: Id::default(),
        facts: vec![MemoryFact {
            fact: fact.into(),
            asserted_at: SystemTime::UNIX_EPOCH,
            source_message_id: Id::default(),
            confidence: 1.0,
        }],
        version: 1,
        timestamp: SystemTime::UNIX_EPOCH,
    }
}

fn user_record(text: &str) -> SessionRecord {
    SessionRecord::Conversation {
        message: Messages::User {
            id: foundation_compact::ids::new_scru128(),
            role: MessageRole::User,
            content: UserModelContent::Text(TextContent {
                content: text.into(),
                signature: None,
            }),
            signature: None,
        },
    }
}

// ---------------------------------------------------------------------------

#[test]
fn empty_memory_yields_system_prompt_only() {
    let ctx = provider(ContextConfig::default()).assemble_from_memory(&SessionMemory::default());
    assert_eq!(ctx.system_prompt.as_deref(), Some("SYS"));
    assert!(
        ctx.messages.is_empty(),
        "no memory and no recent messages => no context messages: {:?}",
        ctx.messages
    );
}

#[test]
fn working_memory_is_injected() {
    let memory = SessionMemory {
        working: Some(working_memory("the sky is blue")),
        ..SessionMemory::default()
    };
    let ctx = provider(ContextConfig::default()).assemble_from_memory(&memory);
    assert!(
        !ctx.messages.is_empty(),
        "working memory must appear in the assembled context"
    );
    assert!(
        ctx.token_estimate > 0,
        "a non-empty context must estimate some tokens"
    );
}

#[test]
fn recent_messages_are_included() {
    let p = provider(ContextConfig::default());
    // Persist two user turns, then assemble — they must come back as recent.
    let _ = p.message_api().append(user_record("first"));
    let _ = p.message_api().append(user_record("second"));

    let ctx = p.assemble_from_memory(&SessionMemory::default());
    let count = ctx
        .messages
        .iter()
        .filter(|m| matches!(m, Messages::User { .. }))
        .count();
    assert_eq!(count, 2, "both recent user messages must be assembled");
}

#[test]
fn recent_message_count_is_capped_by_config() {
    let config = ContextConfig {
        recent_message_count: 1,
        ..ContextConfig::default()
    };
    let p = provider(config);
    let _ = p.message_api().append(user_record("older"));
    let _ = p.message_api().append(user_record("newer"));

    let ctx = p.assemble_from_memory(&SessionMemory::default());
    let count = ctx
        .messages
        .iter()
        .filter(|m| matches!(m, Messages::User { .. }))
        .count();
    assert_eq!(count, 1, "recent_message_count must cap the included messages");
}
