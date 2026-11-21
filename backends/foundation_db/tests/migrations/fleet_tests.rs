//! `FleetRunner`: one plan on many databases.
//!
//! Written in ASD-STE100 Simplified Technical English (STE).

use std::collections::HashMap;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

use foundation_core::valtron::valtron_test;
use foundation_db::{
    DatabaseResolver, FleetConfig, FleetOutcome, FleetPlan, FleetRunner, MigrationConfig,
    MigrationLayout, MigrationLayoutConfig, MigrationRunner, QueryStore, StorageError,
};

use crate::common::{count, memory_fs, memory_store, mkdir, rows, write, SHARED};

struct TestResolver {
    stores: HashMap<String, Arc<dyn QueryStore>>,
    active: AtomicUsize,
    max_active: AtomicUsize,
}

impl TestResolver {
    fn new(stores: HashMap<String, Arc<dyn QueryStore>>) -> Self {
        Self {
            stores,
            active: AtomicUsize::new(0),
            max_active: AtomicUsize::new(0),
        }
    }

    fn max_active(&self) -> usize {
        self.max_active.load(Ordering::SeqCst)
    }
}

#[async_trait::async_trait]
impl DatabaseResolver for TestResolver {
    async fn open(&self, database: &str) -> Result<Arc<dyn QueryStore>, StorageError> {
        let active = self.active.fetch_add(1, Ordering::SeqCst) + 1;
        self.max_active.fetch_max(active, Ordering::SeqCst);
        for _ in 0..4 {
            futures_lite::future::yield_now().await;
        }
        self.active.fetch_sub(1, Ordering::SeqCst);
        self.stores
            .get(database)
            .cloned()
            .ok_or_else(|| StorageError::NotFound(database.to_string()))
    }
}

async fn stores_for(names: &[&str]) -> HashMap<String, Arc<dyn QueryStore>> {
    let mut stores: HashMap<String, Arc<dyn QueryStore>> = HashMap::new();
    for name in names {
        stores.insert((*name).to_string(), Arc::new(memory_store().await));
    }
    stores
}

async fn records(store: &dyn QueryStore) -> Vec<(String, i64, String)> {
    rows(
        store,
        "SELECT group_name, number, name FROM _migrations ORDER BY group_name, number",
    )
    .await
    .iter()
    .map(|row| {
        (
            row.get::<String>(0).unwrap(),
            row.get::<i64>(1).unwrap(),
            row.get::<String>(2).unwrap(),
        )
    })
    .collect()
}

#[valtron_test]
async fn sequential_and_parallel_runs_give_the_same_records() {
    let names = ["one", "two", "three", "four"];
    let sequential = stores_for(&names).await;
    let sequential_resolver = Arc::new(TestResolver::new(sequential.clone()));
    let plan = FleetPlan::SameRunner {
        runner: MigrationRunner::from_static(&SHARED),
        databases: names.iter().map(|name| (*name).to_string()).collect(),
    };
    let runner = FleetRunner::new(plan, sequential_resolver.clone(), FleetConfig::default());
    let report = runner.run(&memory_fs()).await;
    assert_eq!(report.outcomes.len(), names.len());
    assert_eq!(sequential_resolver.max_active(), 1);
    for (_, outcome) in &report.outcomes {
        assert!(matches!(outcome, FleetOutcome::Migrated(_)));
    }

    let parallel = stores_for(&names).await;
    let parallel_resolver = Arc::new(TestResolver::new(parallel.clone()));
    let plan = FleetPlan::SameRunner {
        runner: MigrationRunner::from_static(&SHARED),
        databases: names.iter().map(|name| (*name).to_string()).collect(),
    };
    let runner = FleetRunner::new(
        plan,
        parallel_resolver.clone(),
        FleetConfig::default().with_max_parallel(4),
    );
    let report = runner.run(&memory_fs()).await;
    assert!(parallel_resolver.max_active() <= 4);
    assert!(
        parallel_resolver.max_active() >= 2,
        "parallel opens must overlap"
    );
    for (database, outcome) in &report.outcomes {
        assert!(matches!(outcome, FleetOutcome::Migrated(_)), "{database}");
    }

    for name in names {
        assert_eq!(
            records(&*sequential[&name.to_string()]).await,
            records(&*parallel[&name.to_string()]).await
        );
        assert_eq!(
            count(
                &*parallel[&name.to_string()],
                "SELECT COUNT(*) FROM _migrations"
            )
            .await,
            2
        );
    }
}

async fn failing_layout() -> (foundation_nativeapis::MemoryFs, MigrationLayout) {
    let fs = memory_fs();
    mkdir(&fs, "/fleet").await;
    for group in ["shared", "bad", "good"] {
        mkdir(&fs, &format!("/fleet/{group}")).await;
    }
    write(
        &fs,
        "/fleet/shared/0001_create_base.sql",
        "CREATE TABLE IF NOT EXISTS base (id INTEGER PRIMARY KEY)",
    )
    .await;
    write(&fs, "/fleet/bad/0001_broken.sql", "CREATE TABLE broken (").await;
    write(
        &fs,
        "/fleet/good/0001_create_good.sql",
        "CREATE TABLE IF NOT EXISTS good (id INTEGER PRIMARY KEY)",
    )
    .await;
    let layout = MigrationLayout::scan(
        &fs,
        "/fleet",
        MigrationLayoutConfig::default(),
        MigrationConfig::default(),
    )
    .await
    .unwrap();
    (fs, layout)
}

#[valtron_test]
async fn stop_on_first_failure_skips_the_rest() {
    let (fs, layout) = failing_layout().await;
    let resolver = Arc::new(TestResolver::new(stores_for(&["bad", "good"]).await));
    let plan = FleetPlan::FromLayout {
        layout,
        databases: vec!["bad".to_string(), "good".to_string()],
    };
    let runner = FleetRunner::new(plan, resolver, FleetConfig::default());
    let report = runner.run(&fs).await;

    assert_eq!(report.outcomes.len(), 2);
    assert_eq!(report.outcomes[0].0, "bad");
    assert!(matches!(report.outcomes[0].1, FleetOutcome::Failed(_)));
    assert_eq!(report.outcomes[1].0, "good");
    assert!(
        matches!(report.outcomes[1].1, FleetOutcome::Skipped),
        "{:?}",
        report.outcomes
    );
}

#[valtron_test]
async fn without_stop_every_database_is_attempted() {
    let (fs, layout) = failing_layout().await;
    let stores = stores_for(&["bad", "good"]).await;
    let resolver = Arc::new(TestResolver::new(stores.clone()));
    let plan = FleetPlan::FromLayout {
        layout,
        databases: vec!["bad".to_string(), "good".to_string()],
    };
    let runner = FleetRunner::new(
        plan,
        resolver,
        FleetConfig::default().with_stop_on_first_failure(false),
    );
    let report = runner.run(&fs).await;

    assert!(matches!(report.outcomes[0].1, FleetOutcome::Failed(_)));
    assert!(
        matches!(report.outcomes[1].1, FleetOutcome::Migrated(_)),
        "{:?}",
        report.outcomes
    );
    assert_eq!(
        count(&*stores["good"], "SELECT COUNT(*) FROM _migrations").await,
        2
    );
}
