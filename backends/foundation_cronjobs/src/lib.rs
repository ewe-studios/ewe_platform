//! Cron job scheduling with valtron executor and foundation_db persistence.
//!
//! WHY: scheduled jobs need reliable execution with persistence — if the process
//! restarts, missed jobs should be caught up and execution history preserved.
//!
//! WHAT: `CronScheduler` parses cron expressions, computes next fire times, and
//! parks valtron tasks via `Delayed` until the next execution. Job state
//! (last_run, next_run, success/failure, consecutive_errors) persists in
//! foundation_db.
//!
//! HOW:
//! 1. At startup, load all registered jobs from the DB, compute next fire time,
//!    spawn a valtron task for each.
//! 2. Each task parks via `Delayed(until_next_fire)`.
//! 3. On wake, execute the job, record result in DB, recompute next fire time,
//!    re-park.
//! 4. On process restart, the scheduler loads persisted state and catches up
//!    any missed executions (configurable: catch up once, skip, or run all).

pub mod scheduler;
pub mod storage;

pub use scheduler::{CronScheduler, CronJob, JobConfig, MissedJobPolicy};
pub use storage::{JobState, JobRecord};
