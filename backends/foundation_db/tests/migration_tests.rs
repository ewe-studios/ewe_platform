//! Migration tests: rebuild and unit-conversion migrations.

use foundation_core::valtron::valtron_test;
use foundation_db::{DataValue, MigrationRunner, QueryStore, TursoStorage, MIGRATIONS};

#[valtron_test]
async fn old_rate_limit_table_is_rebuilt() {
    // Simulate a deployed database whose `rate_limits` table has the old shape
    // (`key` is the primary key; no `id` and no `window_end`) and whose earlier
    // migrations are already recorded. Only 027 is still pending, so the runner
    // must not re-run 014 (which would fail on the old shape) and must rebuild.
    let store = TursoStorage::new(":memory:").await.unwrap();
    store
        .execute_batch(
            "CREATE TABLE _migrations (\
                 id TEXT PRIMARY KEY, \
                 name TEXT NOT NULL, \
                 applied_at INTEGER DEFAULT 0)",
        )
        .await
        .unwrap();
    for migration in MIGRATIONS
        .iter()
        .filter(|m| m.id != "027_rebuild_rate_limits")
    {
        store
            .execute(
                "INSERT INTO _migrations (id, name) VALUES (?, ?)",
                &[
                    DataValue::Text(migration.id.to_string()),
                    DataValue::Text(migration.name.to_string()),
                ],
            )
            .await
            .unwrap();
    }
    store
        .execute_batch(
            "CREATE TABLE rate_limits (\
                 key TEXT PRIMARY KEY, \
                 count INTEGER NOT NULL DEFAULT 0, \
                 window_start INTEGER NOT NULL)",
        )
        .await
        .unwrap();

    let applied = MigrationRunner::new(MIGRATIONS)
        .run_async(&store)
        .await
        .unwrap();
    assert_eq!(applied, 1, "only 027 should be pending");

    // The documented columns must now exist: an insert with all of them
    // succeeds (it would fail on the old shape), and the row reads back.
    store
        .execute(
            "INSERT INTO rate_limits (id, key, count, window_start, window_end) VALUES (?, ?, ?, ?, ?)",
            &[
                DataValue::Text("id".into()),
                DataValue::Text("k".into()),
                DataValue::Integer(3),
                DataValue::Integer(0),
                DataValue::Integer(60),
            ],
        )
        .await
        .unwrap();

    let rows = store
        .query(
            "SELECT id, key, count, window_start, window_end FROM rate_limits WHERE key = ?",
            &[DataValue::Text("k".into())],
        )
        .await
        .unwrap()
        .collect_all()
        .await
        .unwrap();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].get::<i64>(2).unwrap(), 3);
}
