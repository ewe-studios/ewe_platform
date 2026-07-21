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
use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// SearchMode

/// What kind of search to perform via `ContextProvider::search`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
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

/// A single knowledge-recall hit from `ContextProvider::search`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KnowledgeHit {
    /// Where this hit came from (e.g. `message`, `working_memory`, `observation`).
    pub source: String,
    /// Relevance score (0.0–1.0).
    pub score: f32,
    /// The matched content.
    pub content: String,
    /// Optional reference to the source record.
    pub record_ref: Option<String>,
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
    /// Optional embedding capability for real semantic recall (F16). When unset,
    /// `SearchMode::Semantic` falls back to keyword matching.
    embedder: Option<Arc<dyn crate::agentic::embedding::EmbeddingProvider>>,
    /// The embedding model id to request from `embedder`.
    embedding_model: String,
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
            embedder: self.embedder.clone(),
            embedding_model: self.embedding_model.clone(),
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
            embedder: None,
            embedding_model: String::new(),
        }
    }

    /// Attach an embedding capability so `SearchMode::Semantic` performs real
    /// vector recall (cosine similarity) instead of keyword matching (F16).
    #[must_use]
    pub fn with_embedder(
        mut self,
        embedder: Arc<dyn crate::agentic::embedding::EmbeddingProvider>,
        embedding_model: impl Into<String>,
    ) -> Self {
        self.embedder = Some(embedder);
        self.embedding_model = embedding_model.into();
        self
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

    #[must_use]
    pub fn session_id(&self) -> &SessionId {
        &self.session_id
    }

    #[must_use]
    pub fn message_api(&self) -> &MessageApi<D> {
        &self.message_api
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

    /// Search knowledge surfaces: semantic message recall, memory-tier vectors,
    /// code-graph, or hybrid fusion.
    ///
    /// Returns up to `k` hits ranked by relevance. The actual vector search
    /// backends are wired in F31; until then this performs keyword matching
    /// over assembled context as a functional placeholder.
    pub async fn search(&self, query: &str, mode: SearchMode, k: usize) -> Vec<KnowledgeHit> {
        let memory = self.hydrate_memory().await;
        self.search_from_memory(query, mode, k, &memory)
    }

    /// Synchronous search over pre-hydrated memory.
    #[must_use]
    pub fn search_from_memory(
        &self,
        query: &str,
        mode: SearchMode,
        k: usize,
        memory: &SessionMemory,
    ) -> Vec<KnowledgeHit> {
        let query_lower = query.to_lowercase();
        let mut hits = Vec::new();

        match mode {
            SearchMode::Semantic => {
                // Real embedding recall when an embedder is wired (F16); else
                // keyword fallback so a capability-less context still works.
                if !self.semantic_search_messages(query, &mut hits) {
                    self.search_messages(&query_lower, &mut hits);
                }
            }
            SearchMode::Memory => {
                Self::search_memory_tiers(&query_lower, memory, &mut hits);
            }
            SearchMode::Graph => {
                // Code-graph traversal (spec-36 F27) is not wired: the only graph
                // structure available (`foundation_vectors::code_graph`) indexes
                // CODE, not session knowledge, so wiring it here would be wrong.
                // Rather than silently return empty (which reads as "no results"),
                // fall back to hybrid recall so the caller still gets relevant hits.
                tracing::warn!(
                    "SearchMode::Graph is not available (no session knowledge graph); \
                     falling back to hybrid recall"
                );
                if !self.semantic_search_messages(query, &mut hits) {
                    self.search_messages(&query_lower, &mut hits);
                }
                Self::search_memory_tiers(&query_lower, memory, &mut hits);
            }
            SearchMode::Hybrid => {
                if !self.semantic_search_messages(query, &mut hits) {
                    self.search_messages(&query_lower, &mut hits);
                }
                Self::search_memory_tiers(&query_lower, memory, &mut hits);
            }
        }

        hits.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal));
        hits.truncate(k);
        hits
    }

    /// Embedding-based recall over recent messages (F16). Embeds the query and
    /// each candidate message, scoring by cosine similarity. Returns `true` when
    /// it ran (an embedder is wired and the query embedded); `false` signals the
    /// caller to fall back to keyword matching.
    fn semantic_search_messages(&self, query: &str, hits: &mut Vec<KnowledgeHit>) -> bool {
        let Some(embedder) = &self.embedder else {
            return false;
        };
        let query_vec = match embedder.embed(query, &self.embedding_model) {
            Ok(v) => v,
            Err(e) => {
                tracing::warn!(error = ?e, "query embedding failed; falling back to keyword search");
                return false;
            }
        };

        let Ok(recent) = self.message_api.recent(self.config.recent_message_count) else {
            return true; // embedder ran; nothing to search over
        };
        for record in recent {
            if let SessionRecord::Conversation { ref message } = record {
                let text = extract_message_text(message);
                if text.trim().is_empty() {
                    continue;
                }
                let Ok(cand) = embedder.embed(&text, &self.embedding_model) else {
                    continue;
                };
                let score = cosine_similarity(&query_vec.data, &cand.data);
                if score > 0.0 {
                    hits.push(KnowledgeHit {
                        source: "message".into(),
                        score,
                        content: text,
                        record_ref: None,
                    });
                }
            }
        }
        true
    }

    fn search_messages(&self, query_lower: &str, hits: &mut Vec<KnowledgeHit>) {
        if let Ok(recent) = self.message_api.recent(self.config.recent_message_count) {
            for record in recent {
                if let SessionRecord::Conversation { ref message } = record {
                    let text = extract_message_text(message);
                    if let Some(score) = keyword_score(&text, query_lower) {
                        hits.push(KnowledgeHit {
                            source: "message".into(),
                            score,
                            content: text,
                            record_ref: None,
                        });
                    }
                }
            }
        }
    }

    fn search_memory_tiers(
        query_lower: &str,
        memory: &SessionMemory,
        hits: &mut Vec<KnowledgeHit>,
    ) {
        if let Some(SessionRecord::WorkingMemory { facts, .. }) = &memory.working {
            for fact in facts {
                if let Some(score) = keyword_score(&fact.fact, query_lower) {
                    hits.push(KnowledgeHit {
                        source: "working_memory".into(),
                        score,
                        content: fact.fact.clone(),
                        record_ref: None,
                    });
                }
            }
        }
        if let Some(SessionRecord::Observation { observations, .. }) = &memory.observation {
            for entry in observations {
                if let Some(score) = keyword_score(&entry.content, query_lower) {
                    hits.push(KnowledgeHit {
                        source: "observation".into(),
                        score,
                        content: entry.content.clone(),
                        record_ref: None,
                    });
                }
            }
        }
        if let Some(SessionRecord::Reflection { reflections, .. }) = &memory.reflection {
            for entry in reflections {
                if let Some(score) = keyword_score(&entry.summary, query_lower) {
                    hits.push(KnowledgeHit {
                        source: "reflection".into(),
                        score,
                        content: entry.summary.clone(),
                        record_ref: None,
                    });
                }
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Helpers

fn extract_message_text(msg: &Messages) -> String {
    match msg {
        Messages::User { content, .. } | Messages::ToolResult { content, .. } => match content {
            UserModelContent::Text(tc) => tc.content.clone(),
            UserModelContent::Image(_) => String::new(),
        },
        Messages::Assistant { content, .. } => match content {
            crate::types::ModelOutput::Text(tc) => tc.content.clone(),
            _ => String::new(),
        },
    }
}

/// Cosine similarity between two embedding vectors. Returns 0.0 for mismatched
/// lengths or a zero-magnitude vector.
fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
    if a.len() != b.len() || a.is_empty() {
        return 0.0;
    }
    let mut dot = 0.0f32;
    let mut norm_a = 0.0f32;
    let mut norm_b = 0.0f32;
    for (x, y) in a.iter().zip(b.iter()) {
        dot += x * y;
        norm_a += x * x;
        norm_b += y * y;
    }
    if norm_a == 0.0 || norm_b == 0.0 {
        return 0.0;
    }
    dot / (norm_a.sqrt() * norm_b.sqrt())
}

fn keyword_score(text: &str, query_lower: &str) -> Option<f32> {
    let text_lower = text.to_lowercase();
    if !text_lower.contains(query_lower) {
        return None;
    }
    let count = text_lower.matches(query_lower).count();
    #[allow(clippy::cast_precision_loss)]
    let density = (count as f32) / (text.len().max(1) as f32);
    Some((density * 1000.0).min(1.0))
}

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
    let SessionRecord::Observation { id: obs_id, .. } = obs else {
        return false;
    };
    let SessionRecord::Reflection { id: refl_id, .. } = refl else {
        return true;
    };
    obs_id > refl_id
}

/// Crate-visible token estimate for a message — used by the loop's preflight
/// compression to recompute a compressed context's size.
pub(crate) fn estimate_tokens_pub(msg: &Messages) -> u64 {
    estimate_tokens(msg)
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

