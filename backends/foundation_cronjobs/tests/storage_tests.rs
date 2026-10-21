//! `JobRecord` tests (moved from the inline module in `src/storage.rs`).

use foundation_cronjobs::{CronJob, JobConfig, JobRecord};

#[test]
fn job_record_from_cron_job() {
    let mut job = CronJob::new(JobConfig {
        id: "test-job".into(),
        cron_expr: "0 0 * * *".into(),
        description: "Test".into(),
    });
    job.state.last_run = 123_456;
    job.state.last_status = "success".into();
    job.state.next_run = 86_400;
    let r = JobRecord::from_job(&job);
    assert_eq!(r.id, "test-job");
    assert_eq!(r.last_status, "success");
}
