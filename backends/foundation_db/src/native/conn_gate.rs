//! Connection gate shared by the SQL stores that keep one connection.
//!
//! `LibsqlStore` and `TursoStorage` run every operation on one connection. A
//! transaction (`BEGIN` ... `COMMIT`) on that connection is awaited in steps, so
//! another task could run a statement between the steps and join the open
//! transaction. The gate stops that:
//!
//! - a single statement takes the **read** side while it uses the connection
//!   (single statements still run together);
//! - a driver that forbids two live statements on one connection takes the
//!   **exclusive** side for a read too (see `ConnGate::read_exclusive`). The
//!   Turso 0.8.3 driver does this;
//! - a batch, or any other multi-statement sequence, takes the **write** side
//!   from before `BEGIN` until after `COMMIT` or `ROLLBACK`;
//! - a row stream holds the read side until it ends or is dropped.
//!
//! Clones of a store share one gate (it is in an `Arc`). A batch waits until
//! open row streams from the store end. Do not start a batch on the same task
//! while you hold a row stream from the same store: the batch waits for the
//! stream, and the stream waits for the task.
//!
//! The lock is `async_lock::RwLock`: a waiting task parks and is woken when the
//! lock is free (no spin, no sleep, no tokio). It prefers writers: once a writer
//! waits, new readers wait too. So a task that holds a row stream (a read guard)
//! and then calls another operation on the same store can wait forever for a
//! writer that waits for the stream. To make that mistake fail instead of hang,
//! every wait has a deadline (`lock_timeout`, 30 s by default, the same as the
//! `sqlx` pool acquire timeout) and ends with [`StorageError::LockTimeout`].
//!
//! If a `ROLLBACK` fails, the connection can stay inside the transaction. The
//! gate then records that the connection needs a check; the store runs the check
//! (see the `recover` method of each store) before its next operation.

use crate::core::errors::{LockMode, StorageError, StorageResult};
#[cfg(feature = "libsql")]
use async_lock::RwLockReadGuardArc;
use async_lock::{RwLock, RwLockWriteGuardArc};
use foundation_core::valtron::timer::sleep_for;
use futures_lite::future::or;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

/// How long a call waits for the connection lock before it fails with
/// `LockTimeout`. 30 s is the default acquire timeout of the `sqlx` pool.
pub const DEFAULT_LOCK_TIMEOUT: Duration = Duration::from_secs(30);

/// Owned read guard: held while one statement, or one row stream, uses the
/// connection. Only a driver that allows two live read statements uses it.
#[cfg(feature = "libsql")]
pub(crate) type ReadGuard = RwLockReadGuardArc<()>;

/// Owned write guard: held for the whole of a multi-statement sequence.
pub(crate) type WriteGuard = RwLockWriteGuardArc<()>;

/// The gate. Cloning it shares the same lock and flags.
#[derive(Clone)]
pub(crate) struct ConnGate {
    lock: Arc<RwLock<()>>,
    /// How long a call waits for the lock.
    timeout: Duration,
    /// Set when a `ROLLBACK` failed: the connection may still be in a transaction.
    needs_check: Arc<AtomicBool>,
    /// Test hook: the next rollback is not run and counts as failed.
    #[cfg(feature = "test-hooks")]
    inject_rollback_failure: Arc<AtomicBool>,
}

impl Default for ConnGate {
    fn default() -> Self {
        Self {
            lock: Arc::default(),
            timeout: DEFAULT_LOCK_TIMEOUT,
            needs_check: Arc::default(),
            #[cfg(feature = "test-hooks")]
            inject_rollback_failure: Arc::default(),
        }
    }
}

impl ConnGate {
    /// A new gate with no holders.
    pub(crate) fn new() -> Self {
        Self::default()
    }

    /// The same lock and flags, with a different wait limit.
    pub(crate) fn with_timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
    }

    /// Wait until no multi-statement sequence is open, then share the connection.
    ///
    /// Only for a driver that allows two live read statements on one
    /// connection (libsql). Turso takes `ConnGate::read_exclusive`.
    ///
    /// # Errors
    ///
    /// `LockTimeout` if the lock is not free within the wait limit.
    #[cfg(feature = "libsql")]
    pub(crate) async fn read(&self) -> StorageResult<ReadGuard> {
        let lock = Arc::clone(&self.lock);
        or(
            async move { Ok(lock.read_arc().await) },
            self.expire(LockMode::Read),
        )
        .await
    }

    /// Wait until no other statement or sequence uses the connection, then own
    /// it, for a read.
    ///
    /// Turso 0.8.3 does not allow two live statements on one connection, even
    /// when both only read: every statement clones the connection's
    /// `ConcurrentGuard`, and `execute`, `step` and `query` call `try_use`. A
    /// second statement that runs while the first is live returns
    /// `TursoError::Misuse("concurrent use forbidden")`. Such a driver takes the
    /// exclusive side for a read too, so a read waits for the previous read to
    /// end just as it waits for a write.
    ///
    /// The wait is still reported as [`LockMode::Read`], because the caller is
    /// a read.
    ///
    /// # Errors
    ///
    /// `LockTimeout` if the lock is not free within the wait limit.
    #[cfg(feature = "turso")]
    pub(crate) async fn read_exclusive(&self) -> StorageResult<WriteGuard> {
        let lock = Arc::clone(&self.lock);
        or(
            async move { Ok(lock.write_arc().await) },
            self.expire(LockMode::Read),
        )
        .await
    }

    /// Wait until no other statement or sequence uses the connection, then own it.
    ///
    /// # Errors
    ///
    /// `LockTimeout` if the lock is not free within the wait limit.
    pub(crate) async fn write(&self) -> StorageResult<WriteGuard> {
        let lock = Arc::clone(&self.lock);
        or(
            async move { Ok(lock.write_arc().await) },
            self.expire(LockMode::Write),
        )
        .await
    }

    /// Completes with `LockTimeout` once the wait limit has passed.
    async fn expire<G>(&self, mode: LockMode) -> StorageResult<G> {
        sleep_for(self.timeout).await;
        Err(StorageError::LockTimeout {
            mode,
            waited: self.timeout,
        })
    }

    /// Record that the connection may be inside a transaction.
    pub(crate) fn mark_needs_check(&self) {
        self.needs_check.store(true, Ordering::SeqCst);
    }

    /// Whether a check is due.
    pub(crate) fn needs_check(&self) -> bool {
        self.needs_check.load(Ordering::SeqCst)
    }

    /// The check passed: the connection is in autocommit mode again.
    pub(crate) fn clear_needs_check(&self) {
        self.needs_check.store(false, Ordering::SeqCst);
    }

    /// Arm the test hook.
    #[cfg(feature = "test-hooks")]
    pub(crate) fn inject_rollback_failure(&self) {
        self.inject_rollback_failure.store(true, Ordering::SeqCst);
    }

    /// Consume the test hook: true once after it was armed. Only
    /// `inject_rollback_failure` arms it, and that method needs the
    /// `test-hooks` feature, so this is always false without the feature.
    #[cfg_attr(
        not(feature = "test-hooks"),
        allow(
            clippy::unused_self,
            reason = "the hook field only exists with test-hooks"
        )
    )]
    pub(crate) fn take_injected_rollback_failure(&self) -> bool {
        #[cfg(feature = "test-hooks")]
        {
            self.inject_rollback_failure.swap(false, Ordering::SeqCst)
        }
        #[cfg(not(feature = "test-hooks"))]
        {
            false
        }
    }
}

/// Whether a raw statement can only read, so it may share the connection.
///
/// Neither `libsql` nor `turso` exposes SQLite's `sqlite3_stmt_readonly` on a
/// prepared statement, so this is a conservative text rule: after whitespace
/// and comments, the statement starts with `SELECT` or `EXPLAIN`, and it holds
/// no second statement. Everything else (`WITH`, which can wrap an `INSERT`;
/// `PRAGMA`; `INSERT ... RETURNING`; DDL) takes the write guard.
pub(crate) fn is_read_only_sql(sql: &str) -> bool {
    let mut rest = sql;
    loop {
        rest = rest.trim_start();
        if let Some(after) = rest.strip_prefix("--") {
            rest = after.split_once('\n').map_or("", |(_, tail)| tail);
        } else if let Some(after) = rest.strip_prefix("/*") {
            rest = after.split_once("*/").map_or("", |(_, tail)| tail);
        } else {
            break;
        }
    }
    let word_end = rest
        .find(|c: char| !c.is_ascii_alphabetic())
        .unwrap_or(rest.len());
    let word = &rest[..word_end];
    if !(word.eq_ignore_ascii_case("select") || word.eq_ignore_ascii_case("explain")) {
        return false;
    }
    // One statement only: a `;` may end it, but nothing may follow.
    match rest.find(';') {
        Some(at) => rest[at + 1..].trim().is_empty(),
        None => true,
    }
}
