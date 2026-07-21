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

// ---------------------------------------------------------------------------
// Reflection + observation injection (INCON-03) — matrix 6.2 full.

use foundation_ai::types::{ObservationEntry, ObservationKind, ReflectionEntry};
type CId = Id;

fn observation(seq: u8) -> SessionRecord {
    // A larger scru128 id => "newer". Encode seq into the first byte.
    let mut bytes = [0u8; 16];
    bytes[0] = seq;
    SessionRecord::Observation {
        id: CId::from(bytes),
        observations: vec![ObservationEntry {
            kind: ObservationKind::Assertion,
            content: "user likes tea".into(),
            timestamp: SystemTime::UNIX_EPOCH,
            source_message_id: CId::default(),
            scope: None,
        }],
        token_count: 4,
        timestamp: SystemTime::UNIX_EPOCH,
    }
}

fn reflection(seq: u8) -> SessionRecord {
    let mut bytes = [0u8; 16];
    bytes[0] = seq;
    SessionRecord::Reflection {
        id: CId::from(bytes),
        reflections: vec![ReflectionEntry {
            summary: "prefers tea over coffee".into(),
            time_range: None,
            observation_refs: vec![],
            importance: 0.8,
        }],
        generated_at: SystemTime::UNIX_EPOCH,
        observation_token_count_before: 10,
        reflection_token_count_after: 4,
    }
}

fn assistant_texts_of(ctx: &foundation_ai::agentic::AgentContext) -> String {
    ctx.messages
        .iter()
        .map(|m| format!("{m:?}"))
        .collect::<Vec<_>>()
        .join(" ")
}

#[test]
fn reflection_memory_is_injected() {
    let memory = SessionMemory {
        reflection: Some(reflection(1)),
        ..SessionMemory::default()
    };
    let ctx = provider(ContextConfig::default()).assemble_from_memory(&memory);
    assert!(
        assistant_texts_of(&ctx).contains("prefers tea"),
        "reflection memory must be injected: {ctx:?}"
    );
}

#[test]
fn observation_injected_when_newer_than_reflection() {
    // obs seq 5 > refl seq 2 => observation is newer, so it IS injected.
    let memory = SessionMemory {
        reflection: Some(reflection(2)),
        observation: Some(observation(5)),
        ..SessionMemory::default()
    };
    let ctx = provider(ContextConfig::default()).assemble_from_memory(&memory);
    assert!(
        assistant_texts_of(&ctx).contains("likes tea"),
        "a newer observation must be injected alongside the reflection: {ctx:?}"
    );
}

#[test]
fn observation_skipped_when_older_than_reflection() {
    // obs seq 1 < refl seq 9 => observation is older, so it is NOT injected.
    let memory = SessionMemory {
        reflection: Some(reflection(9)),
        observation: Some(observation(1)),
        ..SessionMemory::default()
    };
    let ctx = provider(ContextConfig::default()).assemble_from_memory(&memory);
    assert!(
        !assistant_texts_of(&ctx).contains("likes tea"),
        "an observation older than the latest reflection must be skipped (INCON-03): {ctx:?}"
    );
}

#[test]
fn observation_injected_when_no_reflection() {
    let memory = SessionMemory {
        observation: Some(observation(1)),
        ..SessionMemory::default()
    };
    let ctx = provider(ContextConfig::default()).assemble_from_memory(&memory);
    assert!(
        assistant_texts_of(&ctx).contains("likes tea"),
        "with no reflection, the observation is always injected: {ctx:?}"
    );
}

// ===========================================================================
// F16 — real embedding-based semantic recall (SearchMode::Semantic)
// ===========================================================================

use foundation_ai::agentic::{
    CacheStats, EmbeddingError, EmbeddingProvider, EmbeddingVector, SearchMode,
};

/// A deterministic topic embedder: text maps to a 3-dim [animal, tech, food]
/// vector by topic keywords. Crucially, semantically-related words that share NO
/// substring (e.g. "feline" and "cat") map to the SAME vector — so a hit proves
/// embedding recall, not keyword matching.
struct TopicEmbedder;

impl TopicEmbedder {
    fn vec_for(text: &str) -> Vec<f32> {
        let t = text.to_lowercase();
        let any = |ws: &[&str]| f32::from(u8::from(ws.iter().any(|w| t.contains(w))));
        vec![
            any(&["cat", "feline", "kitten", "pet", "whiskers", "purr"]),
            any(&["laptop", "computer", "cpu", "code", "software"]),
            any(&["pizza", "food", "meal", "eat"]),
        ]
    }
}

impl EmbeddingProvider for TopicEmbedder {
    fn embed(&self, text: &str, _model_id: &str) -> Result<EmbeddingVector, EmbeddingError> {
        Ok(EmbeddingVector {
            dimensions: 3,
            data: Self::vec_for(text),
            model_id: "topic".into(),
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

/// Build a provider whose message log is pre-seeded (the api is seeded before it
/// is moved into the provider, so `search` sees the records via `recent`).
fn provider_seeded(texts: &[&str]) -> ContextProvider<MemoryDocumentStore, M> {
    let session_id = SessionId::new();
    let api = MessageApi::new(session_id.clone(), MemoryDocumentStore::new());
    for t in texts {
        api.append(user_record(t));
    }
    api.flush().expect("flush");
    let store = Arc::new(KvMemoryStore::new(MemoryStorage::new()));
    ContextProvider::new(
        session_id,
        api,
        store,
        TokenLedger::new(),
        Some("SYS".into()),
        ContextConfig::default(),
    )
}

#[test]
fn semantic_search_uses_embeddings_not_keywords() {
    futures_lite::future::block_on(async {
        let p = provider_seeded(&[
            "I have a cat named Whiskers",
            "My laptop is very fast",
            "I love pizza",
        ])
        .with_embedder(Arc::new(TopicEmbedder), "topic");

        // "feline" shares NO substring with "cat" — only an embedder can match it.
        let hits = p.search("feline companion", SearchMode::Semantic, 5).await;
        assert!(!hits.is_empty(), "semantic search should return hits");
        let top = &hits[0];
        assert!(
            top.content.contains("cat"),
            "semantic top hit should be the cat message, got: {:?}",
            top.content
        );
        assert!(top.score > 0.9, "cosine to the animal message should be high: {}", top.score);
    });
}

#[test]
fn semantic_search_without_embedder_falls_back_to_keyword() {
    futures_lite::future::block_on(async {
        // No embedder → keyword fallback: "feline" won't match "cat".
        let p = provider_seeded(&["I have a cat named Whiskers", "My laptop is fast"]);

        let none = p.search("feline", SearchMode::Semantic, 5).await;
        assert!(none.is_empty(), "keyword fallback can't match feline→cat");

        // But a shared keyword does match under the fallback.
        let some = p.search("laptop", SearchMode::Semantic, 5).await;
        assert!(some.iter().any(|h| h.content.contains("laptop")), "keyword hit");
    });
}

#[test]
fn graph_search_falls_back_to_hybrid_not_silently_empty() {
    futures_lite::future::block_on(async {
        // F17: Graph mode has no session knowledge graph, so instead of silently
        // returning empty it falls back to hybrid recall (keyword here, no embedder).
        let p = provider_seeded(&["the laptop is on the desk", "unrelated note"]);
        let hits = p.search("laptop", SearchMode::Graph, 5).await;
        assert!(
            hits.iter().any(|h| h.content.contains("laptop")),
            "Graph mode must fall back to useful recall, got: {hits:?}"
        );
    });
}
