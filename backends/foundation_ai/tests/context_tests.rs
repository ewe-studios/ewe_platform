use std::sync::Arc;

use foundation_ai::agentic::context::ContextProvider;
use foundation_ai::agentic::context::ContextConfig;
use foundation_ai::agentic::memory_store::{KvMemoryStore, MemoryStore};
use foundation_ai::agentic::message_api::MessageApi;
use foundation_ai::agentic::token_ledger::TokenLedger;
use foundation_ai::types::{
    MemoryFact, MessageRole, Messages, ModelOutput, ObservationEntry, ObservationKind,
    ReflectionEntry, SessionId, SessionRecord, TextContent, UserModelContent,
};
use foundation_compact::SystemTime;
use foundation_db::{MemoryDocumentStore, MemoryStorage};

fn make_provider() -> ContextProvider<MemoryDocumentStore, KvMemoryStore<MemoryStorage>> {
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
        Some("You are a helpful assistant.".into()),
        ContextConfig::default(),
    )
}

fn working_record(fact: &str) -> SessionRecord {
    SessionRecord::WorkingMemory {
        id: foundation_compact::ids::new_scru128(),
        facts: vec![MemoryFact {
            fact: fact.into(),
            asserted_at: SystemTime::UNIX_EPOCH,
            source_message_id: foundation_compact::ids::new_scru128(),
            confidence: 0.9,
        }],
        version: 1,
        timestamp: SystemTime::UNIX_EPOCH,
    }
}

fn observation_record(content: &str, _ts: SystemTime) -> SessionRecord {
    SessionRecord::Observation {
        id: foundation_compact::ids::new_scru128(),
        observations: vec![ObservationEntry {
            kind: ObservationKind::Assertion,
            content: content.into(),
            timestamp: _ts,
            source_message_id: foundation_compact::ids::new_scru128(),
            scope: None,
        }],
        token_count: 5,
        timestamp: _ts,
    }
}

fn reflection_record(summary: &str, _ts: SystemTime) -> SessionRecord {
    SessionRecord::Reflection {
        id: foundation_compact::ids::new_scru128(),
        reflections: vec![ReflectionEntry {
            summary: summary.into(),
            time_range: None,
            observation_refs: vec![],
            importance: 0.8,
        }],
        generated_at: _ts,
        observation_token_count_before: 5,
        reflection_token_count_after: 2,
    }
}

fn user_message(text: &str) -> Messages {
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
fn empty_context_has_system_prompt_only() {
    use futures_lite::future::block_on;
    let provider = make_provider();
    let ctx = block_on(provider.assemble());
    assert_eq!(
        ctx.system_prompt.as_deref(),
        Some("You are a helpful assistant.")
    );
    assert!(ctx.messages.is_empty());
}

#[test]
fn working_memory_injected() {
    use futures_lite::future::block_on;
    let provider = make_provider();
    block_on(async {
        provider
            .memory_store()
            .set_async(provider.session_id(), &working_record("user likes rust"))
            .await
            .unwrap();
        let ctx = provider.assemble().await;
        assert_eq!(ctx.messages.len(), 1);
        let text = extract_text(&ctx.messages[0]);
        assert!(text.contains("user likes rust"));
    });
}

#[test]
fn incon03_observation_injected_when_no_reflection() {
    use futures_lite::future::block_on;
    let provider = make_provider();
    block_on(async {
        let obs = observation_record("saw something", SystemTime::UNIX_EPOCH);
        provider
            .memory_store()
            .set_async(provider.session_id(), &obs)
            .await
            .unwrap();
        let ctx = provider.assemble().await;
        assert_eq!(ctx.messages.len(), 1);
        let text = extract_text(&ctx.messages[0]);
        assert!(text.contains("saw something"));
    });
}

#[test]
fn incon03_observation_suppressed_when_reflection_newer() {
    use futures_lite::future::block_on;
    let provider = make_provider();
    block_on(async {
        let t1 = SystemTime::UNIX_EPOCH;
        let t2 = SystemTime::UNIX_EPOCH + std::time::Duration::from_secs(100);
        let obs = observation_record("old observation", t1);
        let refl = reflection_record("newer reflection", t2);
        provider
            .memory_store()
            .set_async(provider.session_id(), &obs)
            .await
            .unwrap();
        provider
            .memory_store()
            .set_async(provider.session_id(), &refl)
            .await
            .unwrap();
        let ctx = provider.assemble().await;
        // Should have reflection but NOT observation (reflection is newer).
        let texts: Vec<String> = ctx.messages.iter().map(|m| extract_text(m)).collect();
        assert!(
            texts.iter().any(|t| t.contains("newer reflection")),
            "reflection should be present"
        );
        assert!(
            !texts.iter().any(|t| t.contains("old observation")),
            "old observation should be suppressed"
        );
    });
}

#[test]
fn incon03_observation_injected_when_newer_than_reflection() {
    use futures_lite::future::block_on;
    let provider = make_provider();
    block_on(async {
        let t1 = SystemTime::UNIX_EPOCH;
        let t2 = SystemTime::UNIX_EPOCH + std::time::Duration::from_secs(100);
        let refl = reflection_record("old reflection", t1);
        let obs = observation_record("newer observation", t2);
        provider
            .memory_store()
            .set_async(provider.session_id(), &refl)
            .await
            .unwrap();
        provider
            .memory_store()
            .set_async(provider.session_id(), &obs)
            .await
            .unwrap();
        let ctx = provider.assemble().await;
        let texts: Vec<String> = ctx.messages.iter().map(|m| extract_text(m)).collect();
        assert!(texts.iter().any(|t| t.contains("old reflection")));
        assert!(texts.iter().any(|t| t.contains("newer observation")));
    });
}

#[test]
fn recent_messages_included() {
    use futures_lite::future::block_on;
    let provider = make_provider();
    block_on(async {
        let msg = user_message("hello world");
        provider
            .message_api()
            .append(SessionRecord::Conversation { message: msg });
        let _ = provider.message_api().flush();
        let ctx = provider.assemble().await;
        assert_eq!(ctx.messages.len(), 1);
        let text = extract_text(&ctx.messages[0]);
        assert!(text.contains("hello world"));
    });
}

#[test]
fn assembly_order_is_deterministic() {
    use futures_lite::future::block_on;
    let provider = make_provider();
    block_on(async {
        let t1 = SystemTime::UNIX_EPOCH;
        let t2 = SystemTime::UNIX_EPOCH + std::time::Duration::from_secs(100);

        provider
            .memory_store()
            .set_async(provider.session_id(), &working_record("fact A"))
            .await
            .unwrap();
        provider
            .memory_store()
            .set_async(provider.session_id(), &reflection_record("reflection B", t1))
            .await
            .unwrap();
        provider
            .memory_store()
            .set_async(
                provider.session_id(),
                &observation_record("observation C", t2),
            )
            .await
            .unwrap();

        let msg = user_message("user message D");
        provider
            .message_api()
            .append(SessionRecord::Conversation { message: msg });
        let _ = provider.message_api().flush();

        let ctx = provider.assemble().await;
        // Order: working -> reflection -> observation (newer) -> recent messages
        assert_eq!(ctx.messages.len(), 4);
        let texts: Vec<String> = ctx.messages.iter().map(|m| extract_text(m)).collect();
        assert!(
            texts[0].contains("fact A"),
            "first should be working memory"
        );
        assert!(
            texts[1].contains("reflection B"),
            "second should be reflection"
        );
        assert!(
            texts[2].contains("observation C"),
            "third should be observation (newer than reflection)"
        );
        assert!(
            texts[3].contains("user message D"),
            "fourth should be recent message"
        );
    });
}

#[test]
fn clone_shares_state() {
    let provider = make_provider();
    let _cloned = provider.clone();
}

fn extract_text(msg: &Messages) -> String {
    match msg {
        Messages::User { content, .. } | Messages::ToolResult { content, .. } => {
            match content {
                UserModelContent::Text(tc) => tc.content.clone(),
                _ => String::new(),
            }
        }
        Messages::Assistant { content, .. } => match content {
            ModelOutput::Text(tc) => tc.content.clone(),
            _ => String::new(),
        },
    }
}
