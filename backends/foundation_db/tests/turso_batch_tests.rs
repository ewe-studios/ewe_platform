//! `TursoStorage::execute_batch_statements` all-or-nothing transaction tests
//! against a local SQLite file.

#![recursion_limit = "256"]

use foundation_core::valtron::valtron_test;
use foundation_db::{QueryStore, TursoStorage};
use tempfile::TempDir;

#[valtron_test]
async fn batch_statements_apply_atomically() {
    let temp_dir = TempDir::new().unwrap();
    let db_path = temp_dir.path().join("test.db");
    let storage = TursoStorage::with_encryption(db_path.to_str().unwrap(), None)
        .await
        .unwrap();
    storage
        .execute_batch("CREATE TABLE batch_test (key TEXT PRIMARY KEY, value TEXT NOT NULL)")
        .await
        .unwrap();

    let stmts = vec![
        ("INSERT INTO batch_test (key, value) VALUES ('a', '1')".to_string(), vec![]),
        ("INSERT INTO batch_test (key, value) VALUES ('b', '2')".to_string(), vec![]),
        ("INSERT INTO missing_table (key, value) VALUES ('c', '3')".to_string(), vec![]),
    ];
    assert!(storage.execute_batch_statements(&stmts).await.is_err());

    let rows = storage
        .query("SELECT COUNT(*) AS cnt FROM batch_test", &[])
        .await
        .unwrap()
        .collect_all()
        .await
        .unwrap();
    assert_eq!(rows[0].get::<i64>(0).unwrap(), 0);
}
