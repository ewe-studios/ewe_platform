//! Fixed-window rate limiting shared by every `AsyncRateLimiterStore`.
//!
//! A key holds `(count, window_start)`. A window lasts `window_seconds`, a
//! value the caller takes from its own configuration and passes to both
//! `check_rate_limit_async` and `record_rate_limit_async`. Once
//! `now - window_start >= window_seconds` the window has elapsed: a check
//! allows the key again, and the next record starts a new window (count 1,
//! `window_start = now`). Inside the window a record only increments the count
//! and keeps `window_start`, so a busy key still resets on schedule.

use futures_lite::StreamExt;
use serde::{Deserialize, Serialize};

use crate::core::backends::unix_now_secs;
use crate::core::errors::{StorageError, StorageResult};
use crate::core::storage_provider::{AsyncQueryStore, DataValue};

/// The `rate_limits` table every SQL backend uses: the same shape as core
/// migration 014, so a store that wasn't migrated creates the table the
/// migrated ones have. `id` is the key; `window_end` is
/// `window_start + window_seconds`.
pub(crate) const CREATE_TABLE_SQL: &str = "CREATE TABLE IF NOT EXISTS rate_limits (\
     id TEXT PRIMARY KEY, \
     key TEXT UNIQUE NOT NULL, \
     count INTEGER NOT NULL DEFAULT 0, \
     window_start INTEGER NOT NULL, \
     window_end INTEGER NOT NULL)";

/// Record one hit. Parameters: id (the key), key, now, `now + window`,
/// cutoff, cutoff, cutoff (`cutoff = window_cutoff(now, window_seconds)`).
/// A row whose window started at or before the cutoff restarts at count 1
/// with a new window; any other row is incremented and keeps its window.
pub(crate) const RECORD_SQL: &str = "INSERT INTO rate_limits (id, key, count, window_start, window_end) VALUES (?, ?, 1, ?, ?) \
     ON CONFLICT(key) DO UPDATE SET \
     count = CASE WHEN rate_limits.window_start <= ? THEN 1 ELSE rate_limits.count + 1 END, \
     window_start = CASE WHEN rate_limits.window_start <= ? THEN excluded.window_start ELSE rate_limits.window_start END, \
     window_end = CASE WHEN rate_limits.window_start <= ? THEN excluded.window_end ELSE rate_limits.window_end END";

/// Read a key's current count and window start.
pub(crate) const SELECT_SQL: &str = "SELECT count, window_start FROM rate_limits WHERE key = ?";

/// The latest `window_start` that counts as elapsed at `now`.
pub(crate) fn window_cutoff(now: u64, window_seconds: u64) -> u64 {
    now.saturating_sub(window_seconds)
}

/// A key's stored state, for the stores that keep it as a serialized value
/// (in-memory, KV).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct RateLimitEntry {
    pub(crate) count: u32,
    pub(crate) window_start: u64,
}

impl RateLimitEntry {
    /// Whether the window that started at `window_start` has elapsed at `now`.
    pub(crate) fn elapsed(&self, now: u64, window_seconds: u64) -> bool {
        self.window_start <= window_cutoff(now, window_seconds)
    }

    /// Whether one more hit is allowed at `now`.
    pub(crate) fn allows(&self, max_count: u32, now: u64, window_seconds: u64) -> bool {
        self.elapsed(now, window_seconds) || self.count < max_count
    }

    /// The entry after recording one hit at `now` on top of `previous`.
    pub(crate) fn record(previous: Option<Self>, now: u64, window_seconds: u64) -> Self {
        match previous {
            Some(entry) if !entry.elapsed(now, window_seconds) => Self {
                count: entry.count.saturating_add(1),
                window_start: entry.window_start,
            },
            _ => Self {
                count: 1,
                window_start: now,
            },
        }
    }
}

/// `check_rate_limit_async` for any SQL store.
///
/// # Errors
///
/// Returns an error if the clock can't be read or a query fails.
pub(crate) async fn sql_check<Q: AsyncQueryStore + ?Sized>(
    store: &Q,
    key: &str,
    max_count: u32,
    window_seconds: u64,
) -> StorageResult<bool> {
    store.execute_batch_async(CREATE_TABLE_SQL).await?;
    let now = unix_now_secs()?;
    match sql_entry(store, key).await? {
        Some(entry) => Ok(entry.allows(max_count, now, window_seconds)),
        None => Ok(true),
    }
}

/// `record_rate_limit_async` for any SQL store: one upsert, then a read of
/// the resulting count.
///
/// # Errors
///
/// Returns an error if the clock can't be read or a query fails.
pub(crate) async fn sql_record<Q: AsyncQueryStore + ?Sized>(
    store: &Q,
    key: &str,
    window_seconds: u64,
) -> StorageResult<u32> {
    store.execute_batch_async(CREATE_TABLE_SQL).await?;
    let now = unix_now_secs()?;
    let cutoff = to_i64(window_cutoff(now, window_seconds))?;
    let window_end = to_i64(now.saturating_add(window_seconds))?;
    let now = to_i64(now)?;
    store
        .execute_async(
            RECORD_SQL,
            &[
                DataValue::Text(key.to_string()),
                DataValue::Text(key.to_string()),
                DataValue::Integer(now),
                DataValue::Integer(window_end),
                DataValue::Integer(cutoff),
                DataValue::Integer(cutoff),
                DataValue::Integer(cutoff),
            ],
        )
        .await?;
    let entry = sql_entry(store, key)
        .await?
        .ok_or_else(|| StorageError::Backend(format!("rate limit row for {key:?} missing after upsert")))?;
    Ok(entry.count)
}

/// `reset_rate_limit_async` for any SQL store.
///
/// # Errors
///
/// Returns an error if the query fails.
pub(crate) async fn sql_reset<Q: AsyncQueryStore + ?Sized>(store: &Q, key: &str) -> StorageResult<()> {
    store.execute_batch_async(CREATE_TABLE_SQL).await?;
    store
        .execute_async("DELETE FROM rate_limits WHERE key = ?", &[DataValue::Text(key.to_string())])
        .await?;
    Ok(())
}

async fn sql_entry<Q: AsyncQueryStore + ?Sized>(store: &Q, key: &str) -> StorageResult<Option<RateLimitEntry>> {
    let mut rows = store.query_async(SELECT_SQL, &[DataValue::Text(key.to_string())]).await?;
    let Some(row) = rows.next().await else {
        return Ok(None);
    };
    let row = row?;
    let count: i64 = row.get(0)?;
    let window_start: i64 = row.get(1)?;
    Ok(Some(RateLimitEntry {
        count: u32::try_from(count)
            .map_err(|_| StorageError::SqlConversion(format!("rate limit count out of range: {count}")))?,
        window_start: u64::try_from(window_start)
            .map_err(|_| StorageError::SqlConversion(format!("rate limit window_start out of range: {window_start}")))?,
    }))
}

fn to_i64(secs: u64) -> StorageResult<i64> {
    i64::try_from(secs).map_err(|_| StorageError::Backend(format!("timestamp out of range: {secs}")))
}
