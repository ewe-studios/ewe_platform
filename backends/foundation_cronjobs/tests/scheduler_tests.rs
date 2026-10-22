//! `CronScheduler` / `CronJob` tests (moved from the inline module in
//! `src/scheduler.rs`).

use std::sync::Arc;

use foundation_core::valtron::valtron_test;
use foundation_cronjobs::{CronJob, CronScheduler, JobConfig};
use foundation_db::{AsyncQueryStore, DataValue, MemoryStorage, TursoStorage};

fn job(id: &str) -> JobConfig {
    JobConfig {
        id: id.into(),
        cron_expr: "* * * * *".into(),
        description: "d".into(),
    }
}

async fn turso() -> (Arc<TursoStorage>, tempfile::TempDir) {
    let dir = tempfile::tempdir().expect("tempdir");
    let url = dir.path().join("cron.db").to_string_lossy().into_owned();
    (Arc::new(TursoStorage::new(&url).await.expect("open turso")), dir)
}

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
    let mut s = CronScheduler::new(Arc::new(MemoryStorage::new()));
    assert_eq!(s.job_count(), 0);
    s.register(job("j1"));
    assert_eq!(s.job_count(), 1);
}

#[valtron_test]
async fn save_then_load_round_trips_job_state() {
    let (db, _dir) = turso().await;

    let mut first = CronScheduler::new(Arc::clone(&db));
    first.ensure_table().await.expect("create table");
    first.register(job("j1"));
    first.load().await.expect("load with no saved rows");
    assert_eq!(first.jobs()[0].state.last_run, 0);
    first.save().await.expect("save defaults");

    // Record a run and save it.
    db.execute_async(
        "UPDATE cron_jobs SET last_run = ?, last_status = ?, error_count = ?, next_run = ? WHERE id = ?",
        &[
            DataValue::Integer(100),
            DataValue::Text("success".into()),
            DataValue::Integer(2),
            DataValue::Integer(400),
            DataValue::Text("j1".into()),
        ],
    )
    .await
    .expect("update row");

    let mut second = CronScheduler::new(Arc::clone(&db));
    second.register(job("j1"));
    second.register(job("never-saved"));
    second.load().await.expect("load");

    let state = &second.jobs()[0].state;
    assert_eq!(state.last_run, 100);
    assert_eq!(state.last_status, "success");
    assert_eq!(state.error_count, 2);
    assert_eq!(state.next_run, 400);
    // A job without a saved row keeps its defaults.
    assert_eq!(second.jobs()[1].state.last_run, 0);
}

#[valtron_test]
async fn load_reports_a_bad_row_instead_of_resetting_it() {
    let (db, _dir) = turso().await;
    // A table whose `last_run` holds text: decoding it must fail loudly.
    db.execute_batch_async(
        "CREATE TABLE cron_jobs (id TEXT PRIMARY KEY, cron_expr TEXT, last_run TEXT, \
         last_status TEXT, error_count INTEGER, next_run INTEGER)",
    )
    .await
    .expect("create table");
    db.execute_async(
        "INSERT INTO cron_jobs VALUES ('j1', '* * * * *', 'not-a-number', '', 0, 0)",
        &[],
    )
    .await
    .expect("insert");

    let mut s = CronScheduler::new(db);
    s.register(job("j1"));
    assert!(s.load().await.is_err());
}

#[valtron_test]
async fn save_without_the_table_is_an_error() {
    let (db, _dir) = turso().await;
    let mut s = CronScheduler::new(db);
    s.register(job("j1"));
    assert!(s.save().await.is_err());
}
