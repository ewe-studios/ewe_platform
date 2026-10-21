//! Persisted job state records (spec-57, decision 05).

use serde::{Deserialize, Serialize};
use crate::scheduler::CronJob;

/// Runtime and persisted state for a single cron job.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct JobState {
    pub last_run: i64,
    pub last_status: String,
    pub error_count: i64,
    pub next_run: i64,
}

/// A row in the `cron_jobs` table.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JobRecord {
    pub id: String,
    pub cron_expr: String,
    pub last_run: i64,
    pub last_status: String,
    pub error_count: i64,
    pub next_run: i64,
}

impl JobRecord {
    #[must_use]
    pub fn from_job(job: &CronJob) -> Self {
        Self {
            id: job.config.id.clone(),
            cron_expr: job.config.cron_expr.clone(),
            last_run: job.state.last_run,
            last_status: job.state.last_status.clone(),
            error_count: job.state.error_count,
            next_run: job.state.next_run,
        }
    }
}
