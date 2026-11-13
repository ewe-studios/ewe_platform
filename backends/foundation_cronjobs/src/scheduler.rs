//! Job scheduler built on valtron's `Delayed` parking mechanism (spec-57, decision 05).
//!
//! WHY: Periodic jobs (purge expired sends, trashed ciphers) need a reliable
//! scheduler that integrates with valtron's cooperative task model rather than
//! spawning separate timer threads. Persistence ensures jobs survive restarts.
//!
//! WHAT: `CronScheduler` manages registered `CronJob`s. Each job computes its
//! next fire time from a cron expression, parks via `valtron::Delayed`, and
//! re-computes after execution. The scheduler catches up missed jobs on restart.

use std::collections::HashMap;
use std::fmt;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use chrono::{DateTime, Utc};
use foundation_context::Ctx;
use foundation_core::valtron::{AsyncStateTask, AsyncTransition, TaskIterator, TaskStatus};
use foundation_db::{DataValue, QueryStore, SqlRow, StorageError};
use futures_lite::StreamExt;
use saffron::Cron;

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
    /// Run the missed job once, then resume on the normal schedule.
    CatchUp,
    /// Do not run missed jobs; schedule the next fire from now.
    Skip,
    /// Run one catch-up execution for every missed fire, then resume.
    RunAll,
}

/// Errors from parsing a cron expression or building a run task.
#[derive(Debug)]
pub enum CronError {
    /// The cron expression is not a valid five-field expression.
    Parse(String),
    /// A registered job has no runner (a [`Job`] with the same id).
    MissingJob(String),
}

impl fmt::Display for CronError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Parse(e) => write!(f, "Invalid cron expression: {e}"),
            Self::MissingJob(id) => write!(f, "No runner registered for job '{id}'"),
        }
    }
}

impl std::error::Error for CronError {}

/// A registered cron job with its execution state.
#[derive(Debug, Clone)]
pub struct CronJob {
    pub config: JobConfig,
    pub state: JobState,
    cron: Cron,
}

impl CronJob {
    /// Parse `config.cron_expr` and build a job with a default state.
    ///
    /// # Errors
    ///
    /// Returns [`CronError::Parse`] when the expression is invalid.
    pub fn new(config: JobConfig) -> Result<Self, CronError> {
        let cron = config
            .cron_expr
            .parse::<Cron>()
            .map_err(|e| CronError::Parse(e.to_string()))?;
        Ok(Self {
            config,
            state: JobState::default(),
            cron,
        })
    }

    /// Compute the next fire time strictly after `now`.
    ///
    /// `now` is a wall-clock UTC time. A cron expression needs calendar time
    /// (weekday, month, leap day), which valtron's monotonic instant cannot
    /// provide, so the caller passes a wall-clock time derived from the
    /// injected [`foundation_context::Clock::wall_now`]. This method is pure, so a test can move
    /// the time without a live clock.
    #[must_use]
    pub fn next_fire(&self, now: DateTime<Utc>) -> Option<DateTime<Utc>> {
        self.cron.next_after(now)
    }

    /// Turn this job into a runnable valtron task that reads its wall clock
    /// from `ctx`.
    #[must_use]
    pub fn into_run_task(self, job: Arc<dyn Job>, policy: MissedJobPolicy, ctx: Ctx) -> JobRunTask {
        JobRunTask::new(self.config, self.cron, self.state, job, policy, ctx)
    }
}

// ── Run task ────────────────────────────────────────────────────────────

/// A runnable job: the scheduler calls [`run`](Self::run) at each fire time.
///
/// Implement it for each scheduled job and pass the implementation to
/// [`CronScheduler::into_run_tasks`]. `id` must match the [`JobConfig::id`] of
/// the job it runs.
#[async_trait::async_trait]
pub trait Job: Send + Sync {
    /// The id this runner belongs to; it must match a [`JobConfig::id`].
    #[must_use]
    fn id(&self) -> &str;

    /// Run the job once. `Ok(())` records `"success"`; `Err(message)` records
    /// the message and increments the error count.
    async fn run(&self, ctx: Ctx) -> Result<(), String>;
}

/// One recorded execution of a job.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct JobRunResult {
    /// The wall-clock unix second the job actually ran at.
    pub at: i64,
    /// `"success"` or the job's error message.
    pub status: String,
    /// The cumulative error count after this run.
    pub error_count: i64,
}

/// A point-in-time view of a [`JobRunTask`]'s state, for tests and callers.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct JobRunSnapshot {
    pub id: String,
    pub last_run: i64,
    pub last_status: String,
    pub error_count: i64,
    pub next_run: i64,
    pub results: Vec<JobRunResult>,
}

/// The state a run task carries between polls. Not persisted as a whole; the
/// persisted subset is [`JobState`].
#[derive(Clone)]
struct JobRunState {
    config: JobConfig,
    cron: Cron,
    job: Arc<dyn Job>,
    state: JobState,
    policy: MissedJobPolicy,
    ctx: Ctx,
    initialized: bool,
    results: Vec<JobRunResult>,
}

/// One job's run loop, as a valtron task.
///
/// On each poll the task reads the wall clock from its own state, compares it
/// with `next_run`, and either parks on [`TaskStatus::Delayed`] until the next
/// fire or runs the job and records the result. The missed-job policy is applied
/// on the first poll.
pub struct JobRunTask {
    inner: AsyncStateTask<(), (), ()>,
    shared: Arc<Mutex<JobRunState>>,
}

impl JobRunTask {
    /// Build a run task for one job. Its wall clock comes from `ctx`.
    #[must_use]
    pub fn new(
        config: JobConfig,
        cron: Cron,
        state: JobState,
        job: Arc<dyn Job>,
        policy: MissedJobPolicy,
        ctx: Ctx,
    ) -> Self {
        let shared = Arc::new(Mutex::new(JobRunState {
            config,
            cron,
            job,
            state,
            policy,
            ctx,
            initialized: false,
            results: Vec::new(),
        }));
        let transition_shared = shared.clone();
        let transition: AsyncTransition<(), (), ()> = Box::new(move |ctx, ()| {
            let shared = transition_shared.clone();
            Box::pin(async move { job_transition(ctx, shared).await })
        });
        Self {
            inner: AsyncStateTask::new((), transition),
            shared,
        }
    }

    /// A snapshot of the current persisted state and the recorded runs.
    ///
    /// # Panics
    ///
    /// Panics when the internal state mutex is poisoned.
    #[must_use]
    pub fn snapshot(&self) -> JobRunSnapshot {
        let s = self.shared.lock().expect("job run state poisoned");
        JobRunSnapshot {
            id: s.config.id.clone(),
            last_run: s.state.last_run,
            last_status: s.state.last_status.clone(),
            error_count: s.state.error_count,
            next_run: s.state.next_run,
            results: s.results.clone(),
        }
    }
}

impl TaskIterator for JobRunTask {
    type Ready = ();
    type Pending = ();

    fn next_status(&mut self, ctx: Ctx) -> Option<TaskStatus<Self::Ready, Self::Pending>> {
        self.inner.next_status(ctx)
    }
}

/// A decision for the next step of the run loop.
enum Plan {
    /// Park for the given duration until the next fire.
    Park(Duration),
    /// Run the job once now, then recompute the next fire.
    RunOnce,
    /// Run the job once for every missed fire up to now, then recompute.
    RunAll,
    /// No future fire exists: the task is finished.
    Done,
}

/// The async transition driven by [`AsyncStateTask`]. Stateless across polls:
/// everything that must survive a poll lives in `shared`.
async fn job_transition(
    ctx: Ctx,
    shared: Arc<Mutex<JobRunState>>,
) -> Option<((), TaskStatus<(), ()>)> {
    loop {
        let now = {
            let s = shared.lock().expect("job run state poisoned");
            wall_now_utc(&s.ctx)
        };
        let plan = {
            let mut s = shared.lock().expect("job run state poisoned");
            next_plan(&mut s, now)
        };

        match plan {
            Plan::Park(d) => return Some(((), TaskStatus::Delayed(d))),
            Plan::RunOnce => {
                let job = shared.lock().expect("job run state poisoned").job.clone();
                let result = job.run(ctx.clone()).await;
                let mut s = shared.lock().expect("job run state poisoned");
                let now = wall_now_utc(&s.ctx);
                record_run(&mut s, now, &result);
                s.state.next_run = next_fire_unix(&s.cron, now);
            }
            Plan::RunAll => {
                let job = shared.lock().expect("job run state poisoned").job.clone();
                let mut cursor = shared
                    .lock()
                    .expect("job run state poisoned")
                    .state
                    .next_run;
                loop {
                    let result = job.run(ctx.clone()).await;
                    let mut s = shared.lock().expect("job run state poisoned");
                    let now = wall_now_utc(&s.ctx);
                    record_run(&mut s, now, &result);
                    let next = next_fire_unix_from(&s.cron, dt_from_unix(cursor));
                    if next == 0 || next > now.timestamp() {
                        s.state.next_run = next;
                        break;
                    }
                    cursor = next;
                }
            }
            Plan::Done => return None,
        }
    }
}

/// Decide the next step for `s` at wall-clock time `now`.
fn next_plan(s: &mut JobRunState, now: DateTime<Utc>) -> Plan {
    let now_unix = now.timestamp();
    if !s.initialized {
        s.initialized = true;
        if s.state.next_run == 0 {
            // Never scheduled: start from now, so a missed window from a
            // default state does not iterate from the unix epoch.
            s.state.next_run = next_fire_unix(&s.cron, now);
            return park(s, now_unix);
        }
        if s.state.next_run > now_unix {
            // The next fire is still in the future: not missed.
            return park(s, now_unix);
        }
        return match s.policy {
            MissedJobPolicy::Skip => {
                s.state.next_run = next_fire_unix(&s.cron, now);
                park(s, now_unix)
            }
            MissedJobPolicy::CatchUp => Plan::RunOnce,
            MissedJobPolicy::RunAll => Plan::RunAll,
        };
    }

    if s.state.next_run <= now_unix {
        Plan::RunOnce
    } else {
        park(s, now_unix)
    }
}

/// The next fire strictly after `now`, as a unix second; `0` when none exists.
fn next_fire_unix(cron: &Cron, now: DateTime<Utc>) -> i64 {
    next_fire_unix_from(cron, now)
}

/// The next fire strictly after `anchor`, as a unix second; `0` when none.
fn next_fire_unix_from(cron: &Cron, anchor: DateTime<Utc>) -> i64 {
    cron.next_after(anchor).map_or(0, |t| t.timestamp())
}

/// Park until `next_run`, or finish when there is no future fire.
fn park(s: &JobRunState, now_unix: i64) -> Plan {
    if s.state.next_run == 0 {
        return Plan::Done;
    }
    let delay = dt_from_unix(s.state.next_run)
        .signed_duration_since(dt_from_unix(now_unix))
        .to_std()
        .unwrap_or(Duration::ZERO);
    Plan::Park(delay)
}

/// Record one execution in `s.state` and `s.results`.
fn record_run(s: &mut JobRunState, now: DateTime<Utc>, result: &Result<(), String>) {
    let at = now.timestamp();
    let status = match result {
        Ok(()) => "success".to_string(),
        Err(e) => e.clone(),
    };
    if result.is_err() {
        s.state.error_count += 1;
    }
    s.state.last_run = at;
    s.state.last_status.clone_from(&status);
    s.results.push(JobRunResult {
        at,
        status,
        error_count: s.state.error_count,
    });
}

/// A `DateTime<Utc>` for a unix second; the epoch when the value is out of
/// range (a corrupt persisted `next_run`).
fn dt_from_unix(secs: i64) -> DateTime<Utc> {
    DateTime::from_timestamp(secs, 0).unwrap_or(DateTime::UNIX_EPOCH)
}

/// The wall-clock UTC time on the `ctx` clock; the epoch when the value is out
/// of range.
fn wall_now_utc(ctx: &Ctx) -> DateTime<Utc> {
    DateTime::from_timestamp_millis(foundation_compact::time::unix_millis(
        ctx.clock().wall_now(),
    ))
    .unwrap_or(DateTime::UNIX_EPOCH)
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
        Self {
            jobs: Vec::new(),
            query_store,
        }
    }

    /// Register a job. Its cron expression is parsed here, so an invalid
    /// expression is rejected at registration.
    ///
    /// # Errors
    ///
    /// Returns [`CronError::Parse`] when the expression is invalid.
    pub fn register(&mut self, config: JobConfig) -> Result<(), CronError> {
        self.jobs.push(CronJob::new(config)?);
        Ok(())
    }

    #[must_use]
    pub fn job_count(&self) -> usize {
        self.jobs.len()
    }

    /// The registered jobs, in registration order.
    #[must_use]
    pub fn jobs(&self) -> &[CronJob] {
        &self.jobs
    }

    /// Consume the scheduler and turn every registered job into a run task.
    ///
    /// `jobs` maps a job id to its runner; every registered id must have one.
    /// The clock of `ctx` is the wall clock every run task uses; there is no fallback.
    ///
    /// # Errors
    ///
    /// Returns [`CronError::MissingJob`] when a registered job has no runner.
    // The scheduler takes the context by value so the signature states that one
    // task per job gets a shared handle. It clones the `Ctx` for every task.
    #[allow(clippy::needless_pass_by_value)]
    pub fn into_run_tasks(
        self,
        jobs: &[Arc<dyn Job>],
        policy: MissedJobPolicy,
        ctx: Ctx,
    ) -> Result<Vec<JobRunTask>, CronError> {
        let runners: HashMap<&str, Arc<dyn Job>> =
            jobs.iter().map(|j| (j.id(), Arc::clone(j))).collect();
        self.jobs
            .into_iter()
            .map(|job| {
                let id = job.config.id.clone();
                let runner = runners
                    .get(id.as_str())
                    .cloned()
                    .ok_or(CronError::MissingJob(id))?;
                Ok(job.into_run_task(runner, policy, ctx.clone()))
            })
            .collect()
    }

    /// Create the `cron_jobs` table if it doesn't exist.
    ///
    /// # Errors
    ///
    /// Returns `StorageError` if the statement fails.
    pub async fn ensure_table(&self) -> Result<(), StorageError> {
        self.query_store.execute_batch(CREATE_TABLE_SQL).await
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
                .execute(
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
                .query(
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
