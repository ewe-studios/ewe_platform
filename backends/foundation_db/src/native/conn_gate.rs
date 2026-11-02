//! Connection gate shared by the SQL stores that keep one connection.
//!
//! `LibsqlStore` and `TursoStorage` run every operation on one connection. A
//! transaction (`BEGIN` ... `COMMIT`) on that connection is awaited in steps, so
//! another task could run a statement between the steps and join the open
//! transaction. The gate stops that:
//!
//! - a single statement takes the **read** side while it uses the connection
//!   (single statements still run together);
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
//! lock is free (no spin, no sleep, no tokio).
//!
//! If a `ROLLBACK` fails, the connection can stay inside the transaction. The
//! gate then records that the connection needs a check; the store runs the check
//! (see the `recover` method of each store) before its next operation.

use async_lock::{RwLock, RwLockReadGuardArc, RwLockWriteGuardArc};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

/// Owned read guard: held while one statement, or one row stream, uses the connection.
pub(crate) type ReadGuard = RwLockReadGuardArc<()>;

/// Owned write guard: held for the whole of a multi-statement sequence.
pub(crate) type WriteGuard = RwLockWriteGuardArc<()>;

/// The gate. Cloning it shares the same lock and flags.
#[derive(Clone, Default)]
pub(crate) struct ConnGate {
    lock: Arc<RwLock<()>>,
    /// Set when a `ROLLBACK` failed: the connection may still be in a transaction.
    needs_check: Arc<AtomicBool>,
    /// Test hook: the next rollback is not run and counts as failed.
    inject_rollback_failure: Arc<AtomicBool>,
}

impl ConnGate {
    /// A new gate with no holders.
    pub(crate) fn new() -> Self {
        Self::default()
    }

    /// Wait until no multi-statement sequence is open, then share the connection.
    pub(crate) async fn read(&self) -> ReadGuard {
        self.lock.read_arc().await
    }

    /// Wait until no other statement or sequence uses the connection, then own it.
    pub(crate) async fn write(&self) -> WriteGuard {
        self.lock.write_arc().await
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
    pub(crate) fn inject_rollback_failure(&self) {
        self.inject_rollback_failure.store(true, Ordering::SeqCst);
    }

    /// Consume the test hook: true once after it was armed.
    pub(crate) fn take_injected_rollback_failure(&self) -> bool {
        self.inject_rollback_failure.swap(false, Ordering::SeqCst)
    }
}
