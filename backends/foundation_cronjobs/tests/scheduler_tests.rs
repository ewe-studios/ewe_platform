//! `CronScheduler` / `CronJob` / `JobRunTask` tests (moved from the inline
//! module in `src/scheduler.rs`).

// The `TursoStorage::new` async_trait Send-check nests past the default
// recursion limit (the same overflow `foundation_db/src/lib.rs` works around
// with this attribute).
#![recursion_limit = "256"]

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use chrono::{DateTime, TimeZone, Utc};
use foundation_core::valtron::{valtron_test, Ctx, TaskIterator, TaskStatus};
use foundation_cronjobs::{
    CronClock, CronError, CronJob, CronScheduler, Job, JobConfig, MissedJobPolicy, SharedCronClock,
};
use foundation_db::{DataValue, MemoryStorage, QueryStore, TursoStorage};

fn job(id: &str) -> JobConfig {
    JobConfig {
        id: id.into(),
        cron_expr: "* * * * *".into(),
        description: "d".into(),
    }
}

fn job_with(id: &str, expr: &str) -> JobConfig {
    JobConfig {
        id: id.into(),
        cron_expr: expr.into(),
        description: "d".into(),
    }
}

async fn turso() -> (Arc<TursoStorage>, tempfile::TempDir) {
    let dir = tempfile::tempdir().expect("tempdir");
    let url = dir.path().join("cron.db").to_string_lossy().into_owned();
    (
        Arc::new(TursoStorage::new(&url).await.expect("open turso")),
        dir,
    )
}

/// A wall clock a test can move.
#[derive(Clone)]
struct MockClock {
    now: Arc<Mutex<DateTime<Utc>>>,
}

impl MockClock {
    fn new(now: DateTime<Utc>) -> Self {
        Self {
            now: Arc::new(Mutex::new(now)),
        }
    }

    fn set(&self, now: DateTime<Utc>) {
        *self.now.lock().expect("mock clock poisoned") = now;
    }
}

impl CronClock for MockClock {
    fn now_utc(&self) -> DateTime<Utc> {
        *self.now.lock().expect("mock clock poisoned")
    }
}

fn utc(y: i32, mo: u32, d: u32, h: u32, mi: u32, s: u32) -> DateTime<Utc> {
    Utc.with_ymd_and_hms(y, mo, d, h, mi, s)
        .single()
        .expect("valid wall-clock time")
}

/// A job that counts its runs and always succeeds.
struct CountingJob {
    id: String,
    runs: Arc<AtomicUsize>,
}

#[async_trait::async_trait]
impl Job for CountingJob {
    fn id(&self) -> &str {
        &self.id
    }

    async fn run(&self, _ctx: Ctx) -> Result<(), String> {
        self.runs.fetch_add(1, Ordering::SeqCst);
        Ok(())
    }
}

/// Build a run task through the scheduler (no DB: `MemoryStorage`).
fn scheduler_task(
    config: JobConfig,
    job: Arc<dyn Job>,
    policy: MissedJobPolicy,
    clock: &MockClock,
) -> foundation_cronjobs::JobRunTask {
    let mut s = CronScheduler::new(Arc::new(MemoryStorage::new()));
    s.register(config).expect("valid expression");
    let shared_clock = SharedCronClock::new(Arc::new(clock.clone()));
    let mut tasks = s
        .into_run_tasks(&[job], policy, &shared_clock)
        .expect("runner registered");
    tasks.pop().expect("one task")
}

/// Build a run task directly, seeding `next_run` so missed-policy tests can
/// simulate a restart with a persisted fire time in the past.
fn seeded_task(
    config: JobConfig,
    job: Arc<dyn Job>,
    policy: MissedJobPolicy,
    next_run: DateTime<Utc>,
    clock: &MockClock,
) -> foundation_cronjobs::JobRunTask {
    let mut cron_job = CronJob::new(config).expect("valid expression");
    cron_job.state.next_run = next_run.timestamp();
    cron_job.into_run_task(job, policy, SharedCronClock::new(Arc::new(clock.clone())))
}

// ── E1: next fire follows the expression ────────────────────────────────

#[test]
fn next_fire_follows_expression() {
    // saffron numbers days of the week 1=Sunday .. 7=Saturday (Quartz), so
    // `1-5` is Sunday..Thursday; Monday..Friday would be `2-6`.
    let cases = [
        (
            "*/15 * * * *",
            utc(2026, 1, 1, 0, 7, 0),
            utc(2026, 1, 1, 0, 15, 0),
        ),
        (
            "0 9 * * 1-5",
            utc(2026, 1, 2, 10, 0, 0), // Friday
            utc(2026, 1, 4, 9, 0, 0),  // next Sunday
        ),
        (
            // Month ends: 2026-02 has no 31st, so the next one is 2026-03-31.
            "0 0 31 * *",
            utc(2026, 2, 1, 12, 0, 0),
            utc(2026, 3, 31, 0, 0, 0),
        ),
        (
            // Leap day: 2026 and 2027 are not leap years; 2028 is.
            "0 0 29 2 *",
            utc(2026, 1, 1, 0, 0, 0),
            utc(2028, 2, 29, 0, 0, 0),
        ),
    ];

    for (expr, now, expected) in cases {
        let cron_job = CronJob::new(job_with("j1", expr)).expect("valid expression");
        assert_eq!(
            cron_job.next_fire(now),
            Some(expected),
            "expression {expr:?} from {now:?}"
        );
    }
}

#[test]
fn register_rejects_invalid_expression() {
    let mut s = CronScheduler::new(Arc::new(MemoryStorage::new()));
    let err = s
        .register(job_with("bad", "61 * * * *"))
        .expect_err("minute 61 is invalid");
    assert!(matches!(err, CronError::Parse(_)));
    assert_eq!(s.job_count(), 0);
}

// ── E2: the scheduler runs jobs ─────────────────────────────────────────

#[valtron_test]
async fn scheduler_runs_job_at_fire_time() {
    let runs = Arc::new(AtomicUsize::new(0));
    let job = Arc::new(CountingJob {
        id: "j1".into(),
        runs: Arc::clone(&runs),
    });
    let clock = MockClock::new(utc(2026, 1, 1, 0, 0, 0));
    let mut task = scheduler_task(
        job_with("j1", "*/15 * * * *"),
        job,
        MissedJobPolicy::Skip,
        &clock,
    );
    let ctx = Ctx::new();

    // Before the first fire: parks for 15 minutes and does not run.
    let status = task.next_status(ctx.clone());
    assert_eq!(
        status,
        Some(TaskStatus::Delayed(Duration::from_secs(15 * 60)))
    );
    assert_eq!(runs.load(Ordering::SeqCst), 0);

    // At the fire time: runs once and parks for the next fire.
    clock.set(utc(2026, 1, 1, 0, 15, 0));
    let status = task.next_status(ctx.clone());
    assert!(matches!(status, Some(TaskStatus::Delayed(_))));
    assert_eq!(runs.load(Ordering::SeqCst), 1);
}

#[valtron_test]
async fn scheduler_records_result() {
    let runs = Arc::new(AtomicUsize::new(0));
    let job = Arc::new(CountingJob {
        id: "j1".into(),
        runs: Arc::clone(&runs),
    });
    let clock = MockClock::new(utc(2026, 1, 1, 0, 0, 0));
    let mut task = scheduler_task(
        job_with("j1", "*/15 * * * *"),
        job,
        MissedJobPolicy::Skip,
        &clock,
    );
    let ctx = Ctx::new();
    task.next_status(ctx.clone());

    let fire = utc(2026, 1, 1, 0, 15, 0);
    clock.set(fire);
    task.next_status(ctx.clone());

    let snap = task.snapshot();
    assert_eq!(snap.last_run, fire.timestamp());
    assert_eq!(snap.last_status, "success");
    assert_eq!(snap.error_count, 0);
    assert_eq!(snap.results.len(), 1);
    assert_eq!(snap.results[0].at, fire.timestamp());
    assert_eq!(snap.results[0].status, "success");
    // The next fire is 00:30 the same day.
    assert_eq!(snap.next_run, utc(2026, 1, 1, 0, 30, 0).timestamp());
    assert_eq!(runs.load(Ordering::SeqCst), 1);
}

#[valtron_test]
async fn missed_policy_skip_runs_nothing() {
    let runs = Arc::new(AtomicUsize::new(0));
    let job = Arc::new(CountingJob {
        id: "j1".into(),
        runs: Arc::clone(&runs),
    });
    // Fires every 15 minutes; the persisted next fire (00:00) is missed by 01:00.
    let clock = MockClock::new(utc(2026, 1, 1, 1, 0, 0));
    let mut task = seeded_task(
        job_with("j1", "*/15 * * * *"),
        job,
        MissedJobPolicy::Skip,
        utc(2026, 1, 1, 0, 0, 0),
        &clock,
    );
    let ctx = Ctx::new();
    let status = task.next_status(ctx.clone());

    assert!(matches!(status, Some(TaskStatus::Delayed(_))));
    assert_eq!(runs.load(Ordering::SeqCst), 0, "Skip must not run the job");
    // Next fire is the first occurrence after now: 01:15.
    assert_eq!(
        task.snapshot().next_run,
        utc(2026, 1, 1, 1, 15, 0).timestamp()
    );
}

#[valtron_test]
async fn missed_policy_catch_up_runs_once() {
    let runs = Arc::new(AtomicUsize::new(0));
    let job = Arc::new(CountingJob {
        id: "j1".into(),
        runs: Arc::clone(&runs),
    });
    // Several fires were missed; catch-up runs the job exactly once.
    let clock = MockClock::new(utc(2026, 1, 1, 1, 0, 0));
    let mut task = seeded_task(
        job_with("j1", "*/15 * * * *"),
        job,
        MissedJobPolicy::CatchUp,
        utc(2026, 1, 1, 0, 0, 0),
        &clock,
    );
    let ctx = Ctx::new();
    let status = task.next_status(ctx.clone());

    assert!(matches!(status, Some(TaskStatus::Delayed(_))));
    assert_eq!(runs.load(Ordering::SeqCst), 1, "CatchUp runs the job once");
    assert_eq!(
        task.snapshot().next_run,
        utc(2026, 1, 1, 1, 15, 0).timestamp()
    );
}

#[valtron_test]
async fn missed_policy_run_all_runs_each_missed_fire() {
    let runs = Arc::new(AtomicUsize::new(0));
    let job = Arc::new(CountingJob {
        id: "j1".into(),
        runs: Arc::clone(&runs),
    });
    // From 00:00 to 00:59 there are four fires: 00:00, 00:15, 00:30, 00:45.
    let clock = MockClock::new(utc(2026, 1, 1, 0, 59, 0));
    let mut task = seeded_task(
        job_with("j1", "*/15 * * * *"),
        job,
        MissedJobPolicy::RunAll,
        utc(2026, 1, 1, 0, 0, 0),
        &clock,
    );
    let ctx = Ctx::new();
    let status = task.next_status(ctx.clone());

    assert!(matches!(status, Some(TaskStatus::Delayed(_))));
    assert_eq!(
        runs.load(Ordering::SeqCst),
        4,
        "RunAll runs once per missed fire"
    );
    assert_eq!(
        task.snapshot().next_run,
        utc(2026, 1, 1, 1, 0, 0).timestamp()
    );
}

#[valtron_test]
async fn scheduler_parks_between_fires() {
    let runs = Arc::new(AtomicUsize::new(0));
    let job = Arc::new(CountingJob {
        id: "j1".into(),
        runs: Arc::clone(&runs),
    });
    // Fires daily at midnight.
    let clock = MockClock::new(utc(2026, 1, 1, 0, 0, 0));
    let mut task = scheduler_task(
        job_with("j1", "0 0 * * *"),
        job,
        MissedJobPolicy::Skip,
        &clock,
    );
    let ctx = Ctx::new();

    let status = task.next_status(ctx.clone());
    assert_eq!(
        status,
        Some(TaskStatus::Delayed(Duration::from_secs(24 * 60 * 60)))
    );
    assert_eq!(runs.load(Ordering::SeqCst), 0);

    // Polling again without moving the clock parks again and never runs.
    let status = task.next_status(ctx.clone());
    assert!(matches!(status, Some(TaskStatus::Delayed(_))));
    assert_eq!(runs.load(Ordering::SeqCst), 0);
}

// ── Scheduler registration and persistence ──────────────────────────────

#[test]
fn scheduler_registers_and_counts() {
    let mut s = CronScheduler::new(Arc::new(MemoryStorage::new()));
    assert_eq!(s.job_count(), 0);
    s.register(job("j1")).expect("valid expression");
    assert_eq!(s.job_count(), 1);
}

#[valtron_test]
async fn save_then_load_round_trips_job_state() {
    let (db, _dir) = turso().await;

    let mut first = CronScheduler::new(Arc::clone(&db));
    first.ensure_table().await.expect("create table");
    first.register(job("j1")).expect("valid expression");
    first.load().await.expect("load with no saved rows");
    assert_eq!(first.jobs()[0].state.last_run, 0);
    first.save().await.expect("save defaults");

    // Record a run and save it.
    db.execute(
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
    second.register(job("j1")).expect("valid expression");
    second
        .register(job("never-saved"))
        .expect("valid expression");
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
    db.execute_batch(
        "CREATE TABLE cron_jobs (id TEXT PRIMARY KEY, cron_expr TEXT, last_run TEXT, \
         last_status TEXT, error_count INTEGER, next_run INTEGER)",
    )
    .await
    .expect("create table");
    db.execute(
        "INSERT INTO cron_jobs VALUES ('j1', '* * * * *', 'not-a-number', '', 0, 0)",
        &[],
    )
    .await
    .expect("insert");

    let mut s = CronScheduler::new(db);
    s.register(job("j1")).expect("valid expression");
    assert!(s.load().await.is_err());
}

#[valtron_test]
async fn save_without_the_table_is_an_error() {
    let (db, _dir) = turso().await;
    let mut s = CronScheduler::new(db);
    s.register(job("j1")).expect("valid expression");
    assert!(s.save().await.is_err());
}
