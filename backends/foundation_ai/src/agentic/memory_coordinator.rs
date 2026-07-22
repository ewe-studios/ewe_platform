//! `MemoryCoordinator` — bridges the latest-memory cache to the audit log (F07).
//!
//! WHY: [`MemoryStore`] is a **dumb cache** (latest memory record per tier) and the
//! append-only `DocumentStore` is the **source of truth** (every memory record is
//! also appended for replay, Decision 03). Something has to keep them consistent:
//! the `MemoryCoordinator` (held by `AgentSession`, F20) owns both and is the only
//! thing that bridges them — `MemoryStore` never wraps `DocumentStore` (OD-07-3).
//!
//! WHAT:
//! - **Dual-write (audit first):** append the memory record to the `DocumentStore`
//!   **first**; only on success `set` the `MemoryStore` latest pointer. If the cache
//!   write then fails the audit trail is intact and the cache rebuilds on the next
//!   hydrate fallback. If the audit write fails the cache is NOT touched.
//! - **Hydrate + fallback:** `MemoryStore.hydrate` is the O(1) fast path; any tier
//!   missing (e.g. a crash before the cache write) falls back to a `DocumentStore`
//!   scan for the latest record of that tier (matched via `record_type`, the F06
//!   promoted column), then **re-populates** the cache. The cache is a derived view,
//!   never the sole record.

use foundation_core::valtron::Stream;
use foundation_db::traits::{DocumentStore, PromotableDocument};
use foundation_db::StorageResult;

use super::memory_store::{MemoryStore, MemoryTier, SessionMemory};
use crate::types::{SessionId, SessionRecord};

/// Populate the `DocumentStore` promoted columns from a `SessionRecord` — the
/// `record_type` (serde tag) drives the coordinator's tier fallback; `summary` is a
/// human-readable first line. The full record stays the JSON `content` blob.
impl PromotableDocument for SessionRecord {
    fn record_type(&self) -> Option<String> {
        Some(
            match self {
                SessionRecord::Conversation { .. } => "conversation",
                SessionRecord::WorkingMemory { .. } => "working_memory",
                SessionRecord::Observation { .. } => "observation",
                SessionRecord::Reflection { .. } => "reflection",
                SessionRecord::FailedAction { .. } => "failed_action",
                SessionRecord::Summary { .. } => "summary",
                SessionRecord::Retracted { .. } => "retracted",
            }
            .to_string(),
        )
    }

    fn summary(&self) -> Option<String> {
        match self {
            SessionRecord::WorkingMemory { facts, .. } => facts.first().map(|f| f.fact.clone()),
            SessionRecord::Observation { observations, .. } => {
                observations.first().map(|o| o.content.clone())
            }
            SessionRecord::Reflection { reflections, .. } => {
                reflections.first().map(|r| r.summary.clone())
            }
            _ => None,
        }
    }
}

/// Read a tier off `SessionMemory` (its per-tier accessor is private to the store).
fn tier_ref(memory: &SessionMemory, tier: MemoryTier) -> Option<&SessionRecord> {
    match tier {
        MemoryTier::Working => memory.working.as_ref(),
        MemoryTier::Observation => memory.observation.as_ref(),
        MemoryTier::Reflection => memory.reflection.as_ref(),
    }
}

/// Owns the latest-memory cache + the audit log and keeps them consistent.
///
/// `M` is any [`MemoryStore`]; `D` is any [`DocumentStore`] (the audit log). The
/// audit log uses one collection per session, `session:{session_id}`.
pub struct MemoryCoordinator<M, D> {
    memory: M,
    audit: D,
}

impl<M: MemoryStore, D: DocumentStore> MemoryCoordinator<M, D> {
    /// Create a coordinator over a cache + an audit `DocumentStore`.
    pub fn new(memory: M, audit: D) -> Self {
        Self { memory, audit }
    }

    /// The audit collection key for a session.
    fn audit_key(session: &SessionId) -> String {
        format!("session:{session}")
    }

    /// Record a memory `SessionRecord`: audit first, then cache (OD-07-3 ordering).
    ///
    /// A non-memory variant is rejected by `MemoryStore::set_async`. If the audit
    /// append fails the error propagates and the cache is left untouched; if the
    /// cache write fails afterwards it is logged and `Ok` is returned (the audit is
    /// the source of truth and the cache rebuilds on the next hydrate fallback).
    /// # Errors
    /// Returns [`StorageError`] if the record cannot be stored.
    pub async fn record_async(
        &self,
        session: &SessionId,
        record: &SessionRecord,
    ) -> StorageResult<()> {
        // Audit first — promoted columns (record_type/summary) come from
        // `PromotableDocument for SessionRecord` above.
        self.audit
            .append_promotable(&Self::audit_key(session), record.clone())?;

        if let Err(e) = self.memory.set_async(session, record).await {
            tracing::warn!(
                session = %session,
                error = %e,
                "memory cache write failed after audit succeeded; cache will rebuild on hydrate fallback"
            );
        }
        Ok(())
    }

    /// Hydrate all tiers for a session: the cache fast-path, with a `DocumentStore`
    /// fallback + re-populate for any missing tier.
    /// # Errors
    /// Returns [`StorageError`] if the record cannot be retrieved.
    pub async fn hydrate_async(&self, session: &SessionId) -> StorageResult<SessionMemory> {
        let mut memory = self.memory.hydrate_async(session).await?;

        let missing: Vec<MemoryTier> = [
            MemoryTier::Working,
            MemoryTier::Observation,
            MemoryTier::Reflection,
        ]
        .into_iter()
        .filter(|t| tier_ref(&memory, *t).is_none())
        .collect();

        if missing.is_empty() {
            return Ok(memory);
        }

        // Fallback: scan the audit log oldest-first and keep the last (newest)
        // record of each missing tier. (Rare — only after a crash between the audit
        // and cache writes.) Then re-populate the cache so the next resume is O(1).
        for item in self
            .audit
            .scan_all::<SessionRecord>(&Self::audit_key(session))?
        {
            let record = match item {
                Stream::Next(Ok(r)) => r,
                Stream::Next(Err(e)) => return Err(e),
                _ => continue,
            };
            if let Some(tier) = MemoryTier::of(&record) {
                if missing.contains(&tier) {
                    *slot_mut(&mut memory, tier) = Some(record); // overwrite → last wins = newest
                }
            }
        }

        for tier in missing {
            if let Some(record) = tier_ref(&memory, tier).cloned() {
                self.memory.set_async(session, &record).await?;
            }
        }

        Ok(memory)
    }

    /// Borrow the underlying cache (e.g. for direct `get_async` of a single tier).
    pub fn memory(&self) -> &M {
        &self.memory
    }
}

/// Mutable access to a tier slot (kept here so `SessionMemory`'s accessor stays
/// private to the store module; the fields themselves are public).
fn slot_mut(memory: &mut SessionMemory, tier: MemoryTier) -> &mut Option<SessionRecord> {
    match tier {
        MemoryTier::Working => &mut memory.working,
        MemoryTier::Observation => &mut memory.observation,
        MemoryTier::Reflection => &mut memory.reflection,
    }
}
