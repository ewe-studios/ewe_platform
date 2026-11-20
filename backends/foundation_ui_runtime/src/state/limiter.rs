use std::collections::BTreeMap;
use std::sync::{Mutex, MutexGuard, PoisonError};

use foundation_compact::Instant;
use foundation_context::Ctx;

use super::{FailureLimits, StateError};

/// Identifies one failure record by session and network address.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct FailureKey {
    pub session: String,
    pub address: String,
}

/// Tracks failed opens for each caller.
pub trait FailureLimiter: Send + Sync {
    /// # Errors
    ///
    /// Returns an error while the caller is blocked or the clock is missing.
    fn check(&self, ctx: &Ctx, key: &FailureKey) -> Result<(), StateError>;

    /// Records a failed open.
    fn record_failure(&self, ctx: &Ctx, key: &FailureKey);
}

struct Record {
    count: u32,
    window_start: Instant,
    blocked_at: Option<Instant>,
}

/// Keeps failure records in memory.
///
/// The host must bound the number of distinct caller keys. A host with many
/// clients supplies a limiter backed by its own store.
pub struct InMemoryFailureLimiter {
    limits: FailureLimits,
    records: Mutex<BTreeMap<FailureKey, Record>>,
}

impl InMemoryFailureLimiter {
    #[must_use]
    pub fn new(limits: FailureLimits) -> Self {
        Self {
            limits,
            records: Mutex::new(BTreeMap::new()),
        }
    }

    fn records(&self) -> MutexGuard<'_, BTreeMap<FailureKey, Record>> {
        self.records.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

impl FailureLimiter for InMemoryFailureLimiter {
    fn check(&self, ctx: &Ctx, key: &FailureKey) -> Result<(), StateError> {
        let clock = ctx
            .try_clock()
            .ok_or(StateError::MissingEnv { what: "clock" })?;
        let now = clock.now();
        let mut records = self.records();
        if let Some(blocked_at) = records.get(key).and_then(|record| record.blocked_at) {
            if now.saturating_duration_since(blocked_at) < self.limits.block_duration() {
                return Err(StateError::RateLimited);
            }
            records.remove(key);
        }
        Ok(())
    }

    fn record_failure(&self, ctx: &Ctx, key: &FailureKey) {
        let Some(clock) = ctx.try_clock() else {
            return;
        };
        let now = clock.now();
        let mut records = self.records();
        let record = records.entry(key.clone()).or_insert(Record {
            count: 0,
            window_start: now,
            blocked_at: None,
        });

        if let Some(blocked_at) = record.blocked_at {
            if now.saturating_duration_since(blocked_at) < self.limits.block_duration() {
                return;
            }
            record.count = 0;
            record.window_start = now;
        } else if now.saturating_duration_since(record.window_start) >= self.limits.window_value() {
            record.count = 0;
            record.window_start = now;
        }
        record.count = record.count.saturating_add(1);
        record.blocked_at = (record.count >= self.limits.failure_count()).then_some(now);
    }
}
