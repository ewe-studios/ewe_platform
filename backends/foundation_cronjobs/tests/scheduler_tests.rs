//! `CronScheduler` / `CronJob` tests (moved from the inline module in
//! `src/scheduler.rs`).

use std::sync::Arc;

use foundation_cronjobs::{CronJob, CronScheduler, JobConfig};
use foundation_db::{Blocking, MemoryStorage};

#[test]
fn cron_job_computes_next_fire() {
    let job = CronJob::new(JobConfig {
        id: "purge-sends".into(),
        cron_expr: "0 */6 * * *".into(),
        description: "Purge".into(),
    });
    assert!(job.next_fire().is_some());
}

#[test]
fn scheduler_registers_and_counts() {
    // The scheduler takes a sync `QueryStore`; the in-memory store reaches it
    // through `Blocking`.
    let mut s = CronScheduler::new(Arc::new(Blocking::new(MemoryStorage::new())));
    assert_eq!(s.job_count(), 0);
    s.register(JobConfig {
        id: "j1".into(),
        cron_expr: "* * * * *".into(),
        description: "d".into(),
    });
    assert_eq!(s.job_count(), 1);
}
