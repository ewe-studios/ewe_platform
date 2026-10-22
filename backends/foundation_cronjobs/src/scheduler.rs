//! Job scheduler built on valtron's `Delayed` parking mechanism (spec-57, decision 05).
//!
//! WHY: Periodic jobs (purge expired sends, trashed ciphers) need a reliable
//! scheduler that integrates with valtron's cooperative task model rather than
//! spawning separate timer threads. Persistence ensures jobs survive restarts.
//!
//! WHAT: `CronScheduler` manages registered `CronJob`s. Each job computes its
//! next fire time from a cron expression, parks via `valtron::Delayed`, and
//! re-computes after execution. The scheduler catches up missed jobs on restart.

use std::sync::Arc;
use std::time::{Duration, Instant};

use foundation_db::{QueryStore, DataValue, SqlRow, StorageError};
use futures_lite::StreamExt;

use crate::storage::{JobRecord, JobState};

/// The `cron_jobs` table: one row of persisted state per job.
pub const CREATE_TABLE_SQL: &str = "\
CREATE TABLE IF NOT EXISTS cron_jobs (
    id TEXT PRIMARY KEY,
    cron_expr TEXT NOT NULL,
    last_run INTEGER NOT NULL DEFAULT 0,
    last_status TEXT NOT NULL DEFAULT '',
    error_count INTEGER NOT NULL DEFAULT 0,
    next_run INTEGER NOT NULL DEFAULT 0
)";

// ── Core types ──────────────────────────────────────────────────────────

/// Configuration for a single cron job.
#[derive(Debug, Clone)]
pub struct JobConfig {
    pub id: String,
    pub cron_expr: String,
    pub description: String,
}

/// Missed-job policy: what to do when the process was down during a scheduled fire.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MissedJobPolicy {
    CatchUp,
    Skip,
    RunAll,
}

/// A registered cron job with its execution state.
#[derive(Debug, Clone)]
pub struct CronJob {
    pub config: JobConfig,
    pub state: JobState,
}

impl CronJob {
    #[must_use]
    pub fn new(config: JobConfig) -> Self {
        Self { config, state: JobState::default() }
    }

    /// Compute the next fire time. Stub: returns 5 minutes from now.
    /// TODO: integrate a cron-parsing crate (`cron` or `saffron`).
    #[must_use]
    pub fn next_fire(&self) -> Option<Instant> {
        Some(Instant::now() + Duration::from_secs(300))
    }
}

// ── Scheduler ───────────────────────────────────────────────────────────

/// The cron scheduler — owns registered jobs and drives their lifecycle.
///
/// Job state persists in the `cron_jobs` table of an async SQL store; call
/// [`ensure_table`](Self::ensure_table) once before the first `save`/`load`.
pub struct CronScheduler<QS: QueryStore + ?Sized + 'static> {
    jobs: Vec<CronJob>,
    query_store: Arc<QS>,
}

impl<QS: QueryStore + ?Sized + 'static> CronScheduler<QS> {
    #[must_use]
    pub fn new(query_store: Arc<QS>) -> Self {
        Self { jobs: Vec::new(), query_store }
    }

    pub fn register(&mut self, config: JobConfig) {
        self.jobs.push(CronJob::new(config));
    }

    #[must_use]
    pub fn job_count(&self) -> usize { self.jobs.len() }

    /// The registered jobs, in registration order.
    #[must_use]
    pub fn jobs(&self) -> &[CronJob] {
        &self.jobs
    }

    /// Create the `cron_jobs` table if it doesn't exist.
    ///
    /// # Errors
    ///
    /// Returns `StorageError` if the statement fails.
    pub async fn ensure_table(&self) -> Result<(), StorageError> {
        self.query_store.execute_batch_async(CREATE_TABLE_SQL).await
    }

    /// Persist all job states.
    ///
    /// # Errors
    ///
    /// Returns `StorageError` if a DB write fails.
    pub async fn save(&self) -> Result<(), StorageError> {
        for job in &self.jobs {
            let r = JobRecord::from_job(job);
            self.query_store
                .execute_async(
                    "INSERT OR REPLACE INTO cron_jobs (id, cron_expr, last_run, last_status, error_count, next_run) \
                     VALUES (?, ?, ?, ?, ?, ?)",
                    &[
                        DataValue::Text(r.id),
                        DataValue::Text(r.cron_expr),
                        DataValue::Integer(r.last_run),
                        DataValue::Text(r.last_status),
                        DataValue::Integer(r.error_count),
                        DataValue::Integer(r.next_run),
                    ],
                )
                .await?;
        }
        Ok(())
    }

    /// Load persisted job states. A job with no saved row keeps its current
    /// state.
    ///
    /// # Errors
    ///
    /// Returns `StorageError` if a DB read fails or a saved row can't be
    /// decoded (a bad row is reported, not silently reset).
    pub async fn load(&mut self) -> Result<(), StorageError> {
        for job in &mut self.jobs {
            let mut rows = self
                .query_store
                .query_async(
                    "SELECT last_run, last_status, error_count, next_run FROM cron_jobs WHERE id = ?",
                    &[DataValue::Text(job.config.id.clone())],
                )
                .await?;
            if let Some(row) = rows.next().await {
                job.state = job_state_from_row(&row?)?;
            }
        }
        Ok(())
    }
}

fn job_state_from_row(row: &SqlRow) -> Result<JobState, StorageError> {
    Ok(JobState {
        last_run: row.get_by_name("last_run")?,
        last_status: row.get_by_name("last_status")?,
        error_count: row.get_by_name("error_count")?,
        next_run: row.get_by_name("next_run")?,
    })
}
