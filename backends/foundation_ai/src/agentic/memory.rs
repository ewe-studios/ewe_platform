//! `MemoryHierarchy` — the three-tier memory generator (F15).
//!
//! WHY: Long sessions overflow the context window. Progressive distillation
//! turns raw messages into observations → reflections → working memory,
//! never destroying the audit trail.
//!
//! WHAT: `check_triggers` reads F04's rolling counter (30k obs threshold)
//! and the observation store size (40k reflection threshold). On trigger,
//! a generation task calls the memory model (F12), parses into structured
//! entries, and pushes to both F08 (audit) and F07 (fast latest).
//!
//! HOW: Triggers are cheap counter reads. Generation is a background
//! valtron task (F19 schedules it). Reset_rolling fires only after
//! durable append.

use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;

use serde::{Deserialize, Serialize};

use crate::agentic::memory_coordinator::MemoryCoordinator;
use crate::agentic::token_ledger::TokenLedger;
use crate::types::{MemoryFact, ModelId, SessionId, SessionRecord};
use foundation_compact::SystemTime;
use foundation_db::traits::DocumentStore;
use foundation_db::StorageResult;

use super::memory_store::MemoryStore;

// ---------------------------------------------------------------------------
// MemoryAction

/// What kind of memory generation the agent loop should schedule next.
///
/// WHY: The agent loop polls `MemoryHierarchy::check_triggers` every turn
/// but must not block on generation. Returning an enum lets the loop decide
/// *when* to schedule the work (e.g. after tool results, not mid-stream).
///
/// WHAT: Three variants — `None` (thresholds not reached), `GenerateObservation`
/// (rolling token counter hit the observation threshold), `GenerateReflection`
/// (observation store size hit the reflection threshold).
///
/// HOW: `check_triggers` reads two cheap atomics and returns the
/// corresponding variant. The agent loop maps it to a valtron background
/// task via `Spawn`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MemoryAction {
    None,
    GenerateObservation,
    GenerateReflection,
}

// ---------------------------------------------------------------------------
// MemoryParseStrategy

/// How to parse the memory model's output into structured entries.
///
/// WHY: The memory model can produce output in different formats depending
/// on the prompt template. The parser must match the format the model was
/// told to use, and different deployments may prefer one over the other.
///
/// WHAT: `StructuredText` — freeform text parsed with regex/heading
/// conventions; `TwoStepJson` — model emits JSON, parsed with serde.
///
/// HOW: `MemoryHierarchy` passes this to the generation task, which selects
/// the matching prompt template and parser.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum MemoryParseStrategy {
    #[default]
    StructuredText,
    TwoStepJson,
}

// ---------------------------------------------------------------------------
// MemoryConfig

/// Tuning knobs for the three-tier memory generator.
///
/// WHY: Observation and reflection thresholds depend on the model's context
/// window and the desired compression ratio. Hard-coding them would prevent
/// callers from adjusting to different models or session profiles.
///
/// WHAT: Token thresholds for observation and reflection triggers, an
/// optional dedicated memory model id (falls back to the agent's primary
/// model), and the parse strategy for the model's output.
///
/// HOW: Passed to `MemoryHierarchy::new`; `check_triggers` compares the
/// rolling token counter against these thresholds.
#[derive(Debug, Clone)]
pub struct MemoryConfig {
    pub observation_trigger_tokens: u64,
    pub reflection_trigger_tokens: u64,
    pub memory_model: Option<ModelId>,
    pub parse_strategy: MemoryParseStrategy,
}

impl Default for MemoryConfig {
    fn default() -> Self {
        Self {
            observation_trigger_tokens: 30_000,
            reflection_trigger_tokens: 40_000,
            memory_model: None,
            parse_strategy: MemoryParseStrategy::default(),
        }
    }
}

// ---------------------------------------------------------------------------
// MemoryHierarchy

/// Three-tier progressive memory distillation (F15).
///
/// WHY: Long sessions produce more messages than a model's context window
/// can hold. Rather than truncating (losing information), the hierarchy
/// distils raw messages into observations, then observations into
/// reflections, then reflections into working memory — each tier is
/// smaller and more abstract.
///
/// WHAT: Wraps a `MemoryCoordinator` (F07/F08 storage), a `TokenLedger`
/// (rolling counter), and a `MemoryConfig` (trigger thresholds). Exposes
/// `check_triggers` (cheap atomic reads) and `generate_*` (background
/// model calls).
///
/// HOW: `check_triggers` compares the rolling token counter against
/// `MemoryConfig` thresholds and returns a `MemoryAction`. The agent loop
/// schedules the corresponding generation task via `Spawn`. Generation
/// calls the memory model through the router, parses output, and appends
/// to `MemoryCoordinator`. `Arc`-wrapped inner state makes it cheaply
/// clonable for cross-task sharing.
pub struct MemoryHierarchy<M, D> {
    inner: Arc<MemoryInner<M, D>>,
}

impl<M, D> Clone for MemoryHierarchy<M, D> {
    fn clone(&self) -> Self {
        Self {
            inner: self.inner.clone(),
        }
    }
}

struct MemoryInner<M, D> {
    session_id: SessionId,
    coordinator: MemoryCoordinator<M, D>,
    ledger: TokenLedger,
    config: MemoryConfig,
    observation_tokens: AtomicU64,
    is_generating: AtomicBool,
}

impl<M: MemoryStore, D: DocumentStore> MemoryHierarchy<M, D> {
    #[must_use]
    pub fn new(
        session_id: SessionId,
        coordinator: MemoryCoordinator<M, D>,
        ledger: TokenLedger,
        config: MemoryConfig,
    ) -> Self {
        Self {
            inner: Arc::new(MemoryInner {
                session_id,
                coordinator,
                ledger,
                config,
                observation_tokens: AtomicU64::new(0),
                is_generating: AtomicBool::new(false),
            }),
        }
    }

    /// Cheap counter check — called after each turn by the agent loop.
    /// Returns what (if anything) to generate next.
    #[must_use]
    pub fn check_triggers(&self) -> MemoryAction {
        if self.inner.is_generating.load(Ordering::Relaxed) {
            return MemoryAction::None;
        }
        if self.inner.ledger.rolling() >= self.inner.config.observation_trigger_tokens {
            return MemoryAction::GenerateObservation;
        }
        if self.inner.observation_tokens.load(Ordering::Relaxed)
            >= self.inner.config.reflection_trigger_tokens
        {
            return MemoryAction::GenerateReflection;
        }
        MemoryAction::None
    }

    /// Current active observation memory size (tokens).
    #[must_use]
    pub fn observation_memory_tokens(&self) -> u64 {
        self.inner.observation_tokens.load(Ordering::Relaxed)
    }

    /// Persist an observation snapshot.
    /// Appends to audit log + latest cache, then resets rolling.
    ///
    /// # Errors
    /// Returns storage errors from the coordinator.
    pub async fn persist_observation(
        &self,
        observations: Vec<crate::types::ObservationEntry>,
        token_count: u64,
    ) -> StorageResult<()> {
        let record = SessionRecord::Observation {
            id: foundation_compact::ids::new_scru128(),
            observations,
            token_count,
            timestamp: SystemTime::now(),
        };

        self.inner
            .coordinator
            .record_async(&self.inner.session_id, &record)
            .await?;

        self.inner
            .observation_tokens
            .fetch_add(token_count, Ordering::Relaxed);

        self.inner.ledger.reset_rolling();

        Ok(())
    }

    /// Persist a reflection snapshot and reset the observation counter.
    ///
    /// # Errors
    /// Returns storage errors from the coordinator.
    pub async fn persist_reflection(
        &self,
        reflections: Vec<crate::types::ReflectionEntry>,
        observation_token_count_before: u64,
    ) -> StorageResult<()> {
        let reflection_token_count = reflections
            .iter()
            .map(|r| r.summary.len() as u64 / 4)
            .sum::<u64>();

        let record = SessionRecord::Reflection {
            id: foundation_compact::ids::new_scru128(),
            reflections,
            generated_at: SystemTime::now(),
            observation_token_count_before,
            reflection_token_count_after: reflection_token_count,
        };

        self.inner
            .coordinator
            .record_async(&self.inner.session_id, &record)
            .await?;

        self.inner.observation_tokens.store(0, Ordering::Relaxed);

        Ok(())
    }

    /// Update working memory with new facts (version bump).
    ///
    /// # Errors
    /// Returns storage errors from the coordinator.
    pub async fn update_working_memory(
        &self,
        facts: Vec<MemoryFact>,
        prev_version: u64,
    ) -> StorageResult<()> {
        let record = SessionRecord::WorkingMemory {
            id: foundation_compact::ids::new_scru128(),
            facts,
            version: prev_version + 1,
            timestamp: SystemTime::now(),
        };
        self.inner
            .coordinator
            .record_async(&self.inner.session_id, &record)
            .await
    }

    /// Mark generation as in-progress (prevents re-triggering).
    #[must_use]
    pub fn begin_generation(&self) -> bool {
        self.inner
            .is_generating
            .compare_exchange(false, true, Ordering::AcqRel, Ordering::Relaxed)
            .is_ok()
    }

    /// Mark generation as finished.
    pub fn end_generation(&self) {
        self.inner.is_generating.store(false, Ordering::Release);
    }

    /// Whether a generation is currently in progress.
    #[must_use]
    pub fn is_generating(&self) -> bool {
        self.inner.is_generating.load(Ordering::Acquire)
    }

    /// The configured memory model id (falls back to primary if None).
    #[must_use]
    pub fn memory_model(&self) -> Option<&ModelId> {
        self.inner.config.memory_model.as_ref()
    }

    /// The configuration.
    #[must_use]
    pub fn config(&self) -> &MemoryConfig {
        &self.inner.config
    }

    /// The session id.
    #[must_use]
    pub fn session_id(&self) -> &SessionId {
        &self.inner.session_id
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agentic::memory_store::KvMemoryStore;
    use crate::types::{ObservationEntry, ObservationKind, ReflectionEntry, UsageReport};
    use foundation_db::{MemoryDocumentStore, MemoryStorage};

    fn setup() -> (
        MemoryHierarchy<KvMemoryStore<MemoryStorage>, MemoryDocumentStore>,
        TokenLedger,
    ) {
        let kv_store = KvMemoryStore::new(MemoryStorage::new());
        let doc_store = MemoryDocumentStore::new();
        let session_id = SessionId::new();
        let coordinator = MemoryCoordinator::new(kv_store, doc_store);
        let ledger = TokenLedger::new();

        let hierarchy = MemoryHierarchy::new(
            session_id,
            coordinator,
            ledger.clone(),
            MemoryConfig::default(),
        );
        (hierarchy, ledger)
    }

    fn usage(input: f64, output: f64) -> UsageReport {
        UsageReport {
            input,
            output,
            cache_read: 0.0,
            cache_write: 0.0,
            total_tokens: input + output,
            cost: crate::types::UsageCosting {
                currency: String::new(),
                input: 0.0,
                output: 0.0,
                cache_read: 0.0,
                cache_write: 0.0,
                total_tokens: 0.0,
                status: crate::types::CostStatus::Estimated,
            },
        }
    }

    #[test]
    fn no_trigger_when_below_thresholds() {
        let (h, _ledger) = setup();
        assert_eq!(h.check_triggers(), MemoryAction::None);
    }

    #[test]
    fn observation_trigger_at_30k() {
        let (h, ledger) = setup();
        ledger.record(&usage(20_000.0, 10_000.0));
        assert!(ledger.rolling() >= 30_000);
        assert_eq!(h.check_triggers(), MemoryAction::GenerateObservation);
    }

    #[test]
    fn reflection_trigger_at_40k_observation_tokens() {
        let (h, _ledger) = setup();
        h.inner.observation_tokens.store(40_000, Ordering::Relaxed);
        assert_eq!(h.check_triggers(), MemoryAction::GenerateReflection);
    }

    #[test]
    fn observation_trigger_takes_priority_over_reflection() {
        let (h, ledger) = setup();
        ledger.record(&usage(20_000.0, 10_000.0));
        h.inner.observation_tokens.store(40_000, Ordering::Relaxed);
        assert_eq!(h.check_triggers(), MemoryAction::GenerateObservation);
    }

    #[test]
    fn no_trigger_while_generating() {
        let (h, ledger) = setup();
        ledger.record(&usage(20_000.0, 10_000.0));
        assert!(h.begin_generation());
        assert_eq!(h.check_triggers(), MemoryAction::None);
        h.end_generation();
        assert_eq!(h.check_triggers(), MemoryAction::GenerateObservation);
    }

    #[test]
    fn persist_observation_resets_rolling() {
        use futures_lite::future::block_on;
        let (h, ledger) = setup();
        ledger.record(&usage(20_000.0, 10_000.0));
        assert!(ledger.rolling() >= 30_000);

        block_on(async {
            let obs = vec![ObservationEntry {
                kind: ObservationKind::Assertion,
                content: "user likes rust".into(),
                timestamp: SystemTime::UNIX_EPOCH,
                source_message_id: foundation_compact::ids::new_scru128(),
                scope: None,
            }];
            h.persist_observation(obs, 500).await.unwrap();
        });

        assert_eq!(ledger.rolling(), 0);
        assert_eq!(h.observation_memory_tokens(), 500);
    }

    #[test]
    fn persist_reflection_resets_observation_tokens() {
        use futures_lite::future::block_on;
        let (h, _ledger) = setup();
        h.inner.observation_tokens.store(45_000, Ordering::Relaxed);

        block_on(async {
            let refls = vec![ReflectionEntry {
                summary: "condensed summary".into(),
                time_range: None,
                observation_refs: vec![],
                importance: 0.8,
            }];
            h.persist_reflection(refls, 45_000).await.unwrap();
        });

        assert_eq!(h.observation_memory_tokens(), 0);
    }

    #[test]
    fn update_working_memory_versions() {
        use futures_lite::future::block_on;
        let (h, _ledger) = setup();

        block_on(async {
            let facts = vec![MemoryFact {
                fact: "user is a Rust developer".into(),
                asserted_at: SystemTime::now(),
                source_message_id: foundation_compact::ids::new_scru128(),
                confidence: 0.95,
            }];
            h.update_working_memory(facts, 0).await.unwrap();
        });
    }

    #[test]
    fn begin_generation_is_exclusive() {
        let (h, _ledger) = setup();
        assert!(h.begin_generation());
        assert!(!h.begin_generation());
        assert!(h.is_generating());
        h.end_generation();
        assert!(!h.is_generating());
        assert!(h.begin_generation());
    }

    #[test]
    fn clone_shares_state() {
        let (h, ledger) = setup();
        let h2 = h.clone();
        ledger.record(&usage(20_000.0, 10_000.0));
        assert_eq!(h.check_triggers(), h2.check_triggers());
    }
}
