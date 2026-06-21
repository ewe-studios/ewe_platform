//! `ContextProvider` — deterministic context assembly for each LLM turn (F16).
//!
//! WHY: Each model call needs context assembled in a fixed, replayable order
//! from the memory hierarchy + recall, within the token budget.
//!
//! WHAT: `ContextProvider` reads working/reflection/observation memory from
//! `MemoryStore` (F07), recent messages from `MessageApi` (F08), and builds
//! the `messages` field of a `ModelInteraction`. INCON-03: observations are
//! injected only when newer than the latest reflection.
//!
//! HOW: Deterministic Decision 03 order — system prompt, working memory,
//! reflections, (conditional) observations, recent messages, semantic recall.

use std::sync::Arc;

use crate::agentic::memory_store::{MemoryStore, SessionMemory};
use crate::agentic::message_api::MessageApi;
use crate::agentic::token_ledger::TokenLedger;
use crate::types::{
    MessageRole, Messages, SessionId, SessionRecord, TextContent, UserModelContent,
};
use foundation_db::traits::DocumentStore;

// ---------------------------------------------------------------------------
// SearchMode

/// What kind of search to perform via `ContextProvider::search`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SearchMode {
    /// Semantic vector recall over session messages.
    Semantic,
    /// Memory-tier vector recall (observation/reflection embeddings).
    Memory,
    /// Code-graph structural search (F27 — deferred).
    Graph,
    /// Hybrid fusion of vector + BM25 (F26 — deferred).
    Hybrid,
}

// ---------------------------------------------------------------------------
// AgentContext

/// The assembled context for a single LLM turn.
///
/// WHY: Model calls need a complete, token-budgeted payload — system
/// prompt, ordered messages, and a token estimate for budget checks. Passing
/// these as a struct guarantees the caller cannot forget a field and lets
/// downstream layers (context pressure, preflight compression) inspect the
/// whole payload before it reaches the model.
///
/// WHAT: System prompt (optional), ordered message list, and a rough token
/// estimate for the assembled payload.
///
/// HOW: Built by `ContextProvider::assemble_from_memory` in Decision 03
/// order. The `token_estimate` is a heuristic (character count / 4); the
/// agent loop feeds it to budget checks and context-pressure injection.
#[derive(Debug, Clone)]
pub struct AgentContext {
    pub system_prompt: Option<String>,
    pub messages: Vec<Messages>,
    pub token_estimate: u64,
}

// ---------------------------------------------------------------------------
// ContextConfig

/// Tuning knobs for `ContextProvider` assembly.
///
/// WHY: The right number of recent messages and recall budget depends on
/// the model's context window and the session's communication pattern.
/// Externalising these lets callers tune context composition without
/// touching assembly logic.
///
/// WHAT: `recent_message_count` — how many raw messages to include;
/// `recall_budget_tokens` — token budget allocated for semantic recall
/// results; `inject_newer_observations` — whether to include observations
/// that are newer than the latest reflection (INCON-03).
///
/// HOW: Passed to `ContextProvider::new`; read during `assemble_from_memory`.
#[derive(Debug, Clone)]
pub struct ContextConfig {
    /// Max recent raw messages to include.
    pub recent_message_count: usize,
    /// Max tokens to allocate for semantic recall (fills remaining budget).
    pub recall_budget_tokens: u64,
    /// Whether to inject observations when newer than reflections (INCON-03).
    pub inject_newer_observations: bool,
}

impl Default for ContextConfig {
    fn default() -> Self {
        Self {
            recent_message_count: 20,
            recall_budget_tokens: 4096,
            inject_newer_observations: true,
        }
    }
}

// ---------------------------------------------------------------------------
// ContextProvider

/// Assembles the LLM context in Decision 03's deterministic order.
///
/// Reads memory snapshots from `MemoryStore` and recent messages from
/// `MessageApi`. Does NOT own the memory generation logic (that's F15).
pub struct ContextProvider<D, M> {
    session_id: SessionId,
    message_api: MessageApi<D>,
    memory_store: Arc<M>,
    ledger: TokenLedger,
    system_prompt: Option<String>,
    config: ContextConfig,
}

impl<D, M> Clone for ContextProvider<D, M> {
    fn clone(&self) -> Self {
        Self {
            session_id: self.session_id.clone(),
            message_api: self.message_api.clone(),
            memory_store: self.memory_store.clone(),
            ledger: self.ledger.clone(),
            system_prompt: self.system_prompt.clone(),
            config: self.config.clone(),
        }
    }
}

impl<D: DocumentStore, M: MemoryStore> ContextProvider<D, M> {
    #[must_use]
    pub fn new(
        session_id: SessionId,
        message_api: MessageApi<D>,
        memory_store: Arc<M>,
        ledger: TokenLedger,
        system_prompt: Option<String>,
        config: ContextConfig,
    ) -> Self {
        Self {
            session_id,
            message_api,
            memory_store,
            ledger,
            system_prompt,
            config,
        }
    }

    /// Build the context for a single LLM turn in Decision 03's order:
    /// 1. System prompt
    /// 2. Working memory (always, ~500 tokens)
    /// 3. Reflection memory (summarized observations)
    /// 4. Observations (only if newer than latest reflection — INCON-03)
    /// 5. Recent raw messages
    /// 6. Semantic recall (fills remaining budget — deferred to F31 wiring)
    pub async fn assemble(&self) -> AgentContext {
        let memory = self.hydrate_memory().await;
        self.assemble_from_memory(&memory)
    }

    /// Build context from a pre-hydrated `SessionMemory` — synchronous,
    /// suitable for the `AgentLoop`'s `TaskIterator::next_status` (F19).
    #[must_use]
    pub fn assemble_from_memory(&self, memory: &SessionMemory) -> AgentContext {
        let mut messages = Vec::new();
        let mut token_estimate: u64 = 0;

        // 1. Working memory — curated facts, always present.
        if let Some(ref working) = memory.working {
            if let Some(msg) = memory_record_to_message(working, "working_memory") {
                token_estimate += estimate_tokens(&msg);
                messages.push(msg);
            }
        }

        // 2. Reflection memory — condensed summaries.
        if let Some(ref reflection) = memory.reflection {
            if let Some(msg) = memory_record_to_message(reflection, "reflection") {
                token_estimate += estimate_tokens(&msg);
                messages.push(msg);
            }
        }

        // 3. INCON-03: inject observation only if newer than latest reflection.
        if self.config.inject_newer_observations {
            if let Some(ref obs) = memory.observation {
                let should_inject = match &memory.reflection {
                    None => true,
                    Some(refl) => observation_is_newer(obs, refl),
                };
                if should_inject {
                    if let Some(msg) = memory_record_to_message(obs, "observation") {
                        token_estimate += estimate_tokens(&msg);
                        messages.push(msg);
                    }
                }
            }
        }

        // 4. Recent raw messages.
        if let Ok(recent) = self.message_api.recent(self.config.recent_message_count) {
            for record in recent {
                if let SessionRecord::Conversation { message } = record {
                    token_estimate += estimate_tokens(&message);
                    messages.push(message);
                }
            }
        }

        // 5. Semantic recall — deferred to F31 (EmbeddingProvider) wiring.

        AgentContext {
            system_prompt: self.system_prompt.clone(),
            messages,
            token_estimate,
        }
    }

    /// Hydrate memory from the `MemoryStore` fast path.
    async fn hydrate_memory(&self) -> SessionMemory {
        self.memory_store
            .hydrate_async(&self.session_id)
            .await
            .unwrap_or_default()
    }

    /// The memory store (for sync hydrate in F19).
    #[must_use]
    pub fn memory_store(&self) -> &Arc<M> {
        &self.memory_store
    }

    /// The session's token ledger.
    #[must_use]
    pub fn ledger(&self) -> &TokenLedger {
        &self.ledger
    }

    /// The current context configuration.
    #[must_use]
    pub fn config(&self) -> &ContextConfig {
        &self.config
    }

    /// Update the context configuration.
    pub fn set_config(&mut self, config: ContextConfig) {
        self.config = config;
    }
}

// ---------------------------------------------------------------------------
// Helpers

fn memory_record_to_message(record: &SessionRecord, label: &str) -> Option<Messages> {
    let text = match record {
        SessionRecord::WorkingMemory { facts, .. } => {
            let lines: Vec<String> = facts.iter().map(|f| format!("- {}", f.fact)).collect();
            if lines.is_empty() {
                return None;
            }
            format!("[{label}]\n{}", lines.join("\n"))
        }
        SessionRecord::Observation { observations, .. } => {
            let lines: Vec<String> = observations
                .iter()
                .map(|o| format!("- {}", o.content))
                .collect();
            if lines.is_empty() {
                return None;
            }
            format!("[{label}]\n{}", lines.join("\n"))
        }
        SessionRecord::Reflection { reflections, .. } => {
            let lines: Vec<String> = reflections.iter().map(|r| r.summary.clone()).collect();
            if lines.is_empty() {
                return None;
            }
            format!("[{label}]\n{}", lines.join("\n"))
        }
        _ => return None,
    };

    Some(Messages::User {
        id: foundation_compact::ids::new_scru128(),
        role: MessageRole::System,
        content: UserModelContent::Text(TextContent {
            content: text,
            signature: None,
        }),
        signature: None,
    })
}

fn observation_is_newer(obs: &SessionRecord, refl: &SessionRecord) -> bool {
    let SessionRecord::Observation { timestamp, .. } = obs else {
        return false;
    };
    let SessionRecord::Reflection { generated_at, .. } = refl else {
        return true;
    };
    timestamp > generated_at
}

fn estimate_tokens(msg: &Messages) -> u64 {
    let text_len = match msg {
        Messages::User { content, .. } | Messages::ToolResult { content, .. } => match content {
            UserModelContent::Text(tc) => tc.content.len(),
            UserModelContent::Image(_) => 200,
        },
        Messages::Assistant { content, .. } => match content {
            crate::types::ModelOutput::Text(tc) => tc.content.len(),
            _ => 50,
        },
    };
    (text_len as u64) / 4
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agentic::memory_store::KvMemoryStore;
    use crate::types::{MemoryFact, ObservationEntry, ObservationKind, ReflectionEntry};
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
            facts: vec![MemoryFact {
                fact: fact.into(),
                asserted_at: SystemTime::UNIX_EPOCH,
                source_message_id: None,
                confidence: 0.9,
            }],
            version: 1,
            timestamp: SystemTime::UNIX_EPOCH,
        }
    }

    fn observation_record(content: &str, ts: SystemTime) -> SessionRecord {
        SessionRecord::Observation {
            observations: vec![ObservationEntry {
                kind: ObservationKind::Assertion,
                content: content.into(),
                timestamp: ts,
                source_message_id: None,
                scope: None,
            }],
            token_count: 5,
            timestamp: ts,
        }
    }

    fn reflection_record(summary: &str, ts: SystemTime) -> SessionRecord {
        SessionRecord::Reflection {
            reflections: vec![ReflectionEntry {
                summary: summary.into(),
                time_range: None,
                observation_refs: vec![],
                importance: 0.8,
            }],
            generated_at: ts,
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
                .memory_store
                .set_async(&provider.session_id, &working_record("user likes rust"))
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
                .memory_store
                .set_async(&provider.session_id, &obs)
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
                .memory_store
                .set_async(&provider.session_id, &obs)
                .await
                .unwrap();
            provider
                .memory_store
                .set_async(&provider.session_id, &refl)
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
                .memory_store
                .set_async(&provider.session_id, &refl)
                .await
                .unwrap();
            provider
                .memory_store
                .set_async(&provider.session_id, &obs)
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
                .message_api
                .append(SessionRecord::Conversation { message: msg });
            let _ = provider.message_api.flush();
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
                .memory_store
                .set_async(&provider.session_id, &working_record("fact A"))
                .await
                .unwrap();
            provider
                .memory_store
                .set_async(&provider.session_id, &reflection_record("reflection B", t1))
                .await
                .unwrap();
            provider
                .memory_store
                .set_async(
                    &provider.session_id,
                    &observation_record("observation C", t2),
                )
                .await
                .unwrap();

            let msg = user_message("user message D");
            provider
                .message_api
                .append(SessionRecord::Conversation { message: msg });
            let _ = provider.message_api.flush();

            let ctx = provider.assemble().await;
            // Order: working → reflection → observation (newer) → recent messages
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
                crate::types::ModelOutput::Text(tc) => tc.content.clone(),
                _ => String::new(),
            },
        }
    }
}
