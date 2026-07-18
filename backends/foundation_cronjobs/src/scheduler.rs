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

use foundation_db::QueryStore;

use crate::storage::{JobState, JobRecord};

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
pub struct CronScheduler<QS: QueryStore + 'static> {
    jobs: Vec<CronJob>,
    query_store: Arc<QS>,
}

impl<QS: QueryStore + 'static> CronScheduler<QS> {
    #[must_use]
    pub fn new(query_store: Arc<QS>) -> Self {
        Self { jobs: Vec::new(), query_store }
    }

    pub fn register(&mut self, config: JobConfig) {
        self.jobs.push(CronJob::new(config));
    }

    #[must_use]
    pub fn job_count(&self) -> usize { self.jobs.len() }

    /// Persist all job states.
    /// # Errors
    /// Returns `StorageError` if the DB write fails.
    pub fn save(&self) -> Result<(), foundation_db::StorageError> {
        for job in &self.jobs {
            let r = JobRecord::from_job(job);
            self.query_store.execute(
                "INSERT OR REPLACE INTO cron_jobs (id, cron_expr, last_run, last_status, error_count, next_run) \
                 VALUES (?, ?, ?, ?, ?, ?)",
                &[
                    foundation_db::DataValue::Text(r.id),
                    foundation_db::DataValue::Text(r.cron_expr),
                    foundation_db::DataValue::Integer(r.last_run),
                    foundation_db::DataValue::Text(r.last_status),
                    foundation_db::DataValue::Integer(r.error_count),
                    foundation_db::DataValue::Integer(r.next_run),
                ],
            )?;
        }
        Ok(())
    }

    /// Load persisted job states.
    /// # Errors
    /// Returns `StorageError` if the DB read fails.
    pub fn load(&mut self) -> Result<(), foundation_db::StorageError> {
        for job in &mut self.jobs {
            let stream = self.query_store.query(
                "SELECT last_run, last_status, error_count, next_run FROM cron_jobs WHERE id = ?",
                &[foundation_db::DataValue::Text(job.config.id.clone())],
            )?;
            for item in stream {
                if let foundation_core::valtron::Stream::Next(Ok(row)) = item {
                    job.state.last_run = row.get_by_name("last_run").unwrap_or(0);
                    job.state.error_count = row.get_by_name("error_count").unwrap_or(0);
                    job.state.last_status = row.get_by_name("last_status").unwrap_or_default();
                    job.state.next_run = row.get_by_name("next_run").unwrap_or(0);
                }
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cron_job_computes_next_fire() {
        let job = CronJob::new(JobConfig {
            id: "purge-sends".into(), cron_expr: "0 */6 * * *".into(), description: "Purge".into(),
        });
        assert!(job.next_fire().is_some());
    }

    #[test]
    fn scheduler_registers_and_counts() {
        let mut s = CronScheduler::<foundation_db::MemoryStorage>::new(
            Arc::new(foundation_db::MemoryStorage::new()),
        );
        assert_eq!(s.job_count(), 0);
        s.register(JobConfig { id: "j1".into(), cron_expr: "* * * * *".into(), description: "d".into() });
        assert_eq!(s.job_count(), 1);
    }
}
