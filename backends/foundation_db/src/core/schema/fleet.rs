//! Run migration groups on many databases.
//!
//! [`FleetRunner`] plans every runner first (it reads no database), then
//! migrates the databases with at most [`FleetConfig::max_parallel`] in
//! flight. Each database uses its own transactions and its own record rows, so
//! two databases never share a row.
//!
//! Written in ASD-STE100 Simplified Technical English (STE).

use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use foundation_vfs::async_traits::AsyncVfsFileSystem;
use futures_util::stream::{FuturesUnordered, StreamExt};

use crate::core::errors::StorageError;
use crate::core::storage_provider::QueryStore;

use super::layout::MigrationLayout;
use super::migrations::{MigrationError, MigrationRunner, PlannedMigration, RunSummary};

/// Opens a database by name.
///
/// The host implements this trait. For example, it opens one local database
/// file for each tenant, or uses one Cloudflare D1 binding for each tenant.
#[async_trait::async_trait]
pub trait DatabaseResolver: Send + Sync {
    /// Open the database named `database`.
    ///
    /// # Errors
    ///
    /// Returns [`StorageError`] when the store cannot be opened.
    async fn open(&self, database: &str) -> Result<Arc<dyn QueryStore>, StorageError>;
}

/// Which runner each database gets.
#[derive(Debug, Clone)]
pub enum FleetPlan {
    /// The same runner on every listed database. Example: a new template
    /// migration on every tenant.
    SameRunner {
        /// The runner for every database.
        runner: MigrationRunner,
        /// The database names.
        databases: Vec<String>,
    },
    /// `layout.runner_for(name)` on every listed database.
    FromLayout {
        /// The layout that gives each database its runner.
        layout: MigrationLayout,
        /// The database names.
        databases: Vec<String>,
    },
}

impl FleetPlan {
    fn databases(&self) -> &[String] {
        match self {
            Self::SameRunner { databases, .. } | Self::FromLayout { databases, .. } => databases,
        }
    }

    fn runner_for(&self, database: &str) -> MigrationRunner {
        match self {
            Self::SameRunner { runner, .. } => runner.clone(),
            Self::FromLayout { layout, .. } => layout.runner_for(database),
        }
    }
}

/// Limits of a fleet run.
#[derive(Debug, Clone)]
pub struct FleetConfig {
    /// Most databases that migrate at the same time. Default 1: one database
    /// at a time, as Flyway and Liquibase migrate one target per run. Set it
    /// higher for many small tenant databases.
    pub max_parallel: usize,
    /// Stop starting new databases after the first failure. Default `true`: a
    /// broken migration usually fails on every database, so stop early.
    pub stop_on_first_failure: bool,
}

impl Default for FleetConfig {
    fn default() -> Self {
        Self {
            max_parallel: 1,
            stop_on_first_failure: true,
        }
    }
}

impl FleetConfig {
    /// Set the most databases that migrate at the same time.
    #[must_use]
    pub fn with_max_parallel(mut self, max_parallel: usize) -> Self {
        self.max_parallel = max_parallel;
        self
    }

    /// Stop starting new databases after the first failure.
    #[must_use]
    pub fn with_stop_on_first_failure(mut self, stop: bool) -> Self {
        self.stop_on_first_failure = stop;
        self
    }
}

/// Runs migrations for a set of databases.
#[derive(Clone)]
pub struct FleetRunner {
    plan: FleetPlan,
    resolver: Arc<dyn DatabaseResolver>,
    config: FleetConfig,
}

impl std::fmt::Debug for FleetRunner {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("FleetRunner")
            .field("plan", &self.plan)
            .field("config", &self.config)
            .finish_non_exhaustive()
    }
}

impl FleetRunner {
    /// Make a fleet runner.
    #[must_use]
    pub fn new(plan: FleetPlan, resolver: Arc<dyn DatabaseResolver>, config: FleetConfig) -> Self {
        Self {
            plan,
            resolver,
            config,
        }
    }

    /// Plan every runner first (no database access), then migrate each
    /// database with at most [`FleetConfig::max_parallel`] in flight.
    ///
    /// Returns one outcome for each database, in the order of the database
    /// list.
    pub async fn run<F: AsyncVfsFileSystem>(&self, files: &F) -> FleetReport {
        let databases = self.plan.databases().to_vec();
        let targets: Vec<(String, MigrationRunner)> = databases
            .iter()
            .map(|database| (database.clone(), self.plan.runner_for(database)))
            .collect();

        let mut outcomes: Vec<Option<FleetOutcome>> = (0..targets.len()).map(|_| None).collect();
        let mut plans: Vec<Option<Vec<PlannedMigration>>> =
            (0..targets.len()).map(|_| None).collect();

        let mut any_plan_failure = false;
        for (index, (_, runner)) in targets.iter().enumerate() {
            match runner.plan(files).await {
                Ok(plan) => plans[index] = Some(plan),
                Err(error) => {
                    outcomes[index] = Some(FleetOutcome::Failed(error));
                    any_plan_failure = true;
                }
            }
        }
        if any_plan_failure && self.config.stop_on_first_failure {
            for outcome in &mut outcomes {
                if outcome.is_none() {
                    *outcome = Some(FleetOutcome::Skipped);
                }
            }
        }

        let queue: Vec<usize> = (0..targets.len())
            .filter(|index| outcomes[*index].is_none() && plans[*index].is_some())
            .collect();
        self.run_queue(&targets, &mut plans, &queue, &mut outcomes)
            .await;

        let outcomes = outcomes
            .into_iter()
            .zip(databases)
            .map(|(outcome, database)| (database, outcome.unwrap_or(FleetOutcome::Skipped)))
            .collect();
        FleetReport { outcomes }
    }

    async fn run_queue(
        &self,
        targets: &[(String, MigrationRunner)],
        plans: &mut [Option<Vec<PlannedMigration>>],
        queue: &[usize],
        outcomes: &mut [Option<FleetOutcome>],
    ) {
        type Task = Pin<Box<dyn Future<Output = (usize, FleetOutcome)> + Send>>;

        let max_parallel = self.config.max_parallel.max(1);
        let mut in_flight: FuturesUnordered<Task> = FuturesUnordered::new();
        let mut next = 0usize;
        let mut stopped = false;

        loop {
            while !stopped && in_flight.len() < max_parallel && next < queue.len() {
                let index = queue[next];
                next += 1;
                let Some(plan) = plans[index].take() else {
                    continue;
                };
                let runner = targets[index].1.clone();
                let database = targets[index].0.clone();
                let resolver = Arc::clone(&self.resolver);
                in_flight.push(Box::pin(run_one(index, resolver, runner, plan, database)));
            }
            if in_flight.is_empty() {
                break;
            }
            if let Some((index, outcome)) = in_flight.next().await {
                if matches!(outcome, FleetOutcome::Failed(_)) && self.config.stop_on_first_failure {
                    stopped = true;
                }
                outcomes[index] = Some(outcome);
            }
        }
        for index in &queue[next..] {
            if outcomes[*index].is_none() {
                outcomes[*index] = Some(FleetOutcome::Skipped);
            }
        }
    }
}

async fn run_one(
    index: usize,
    resolver: Arc<dyn DatabaseResolver>,
    runner: MigrationRunner,
    plan: Vec<PlannedMigration>,
    database: String,
) -> (usize, FleetOutcome) {
    let store = match resolver.open(&database).await {
        Ok(store) => store,
        Err(error) => return (index, FleetOutcome::Failed(error.into())),
    };
    match runner.run_planned(&plan, &*store).await {
        Ok(summary) => (index, FleetOutcome::Migrated(summary)),
        Err(error) => (index, FleetOutcome::Failed(error)),
    }
}

/// The result of one [`FleetRunner::run`].
#[derive(Debug)]
pub struct FleetReport {
    /// One outcome for each database, in the order of the database list.
    pub outcomes: Vec<(String, FleetOutcome)>,
}

/// What one database did in a fleet run.
#[derive(Debug)]
pub enum FleetOutcome {
    /// The database applied these migrations.
    Migrated(RunSummary),
    /// The database failed.
    Failed(MigrationError),
    /// The database did not start, because an earlier database failed and
    /// [`FleetConfig::stop_on_first_failure`] is set.
    Skipped,
}
