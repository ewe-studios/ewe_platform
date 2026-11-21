//! Shared helpers for the migration-group tests.
//!
//! Written in ASD-STE100 Simplified Technical English (STE).

#![allow(dead_code)]

use std::borrow::Cow;

use foundation_context::Ctx;
use foundation_db::{
    DataValue, Migration, MigrationSet, MigrationSource, QueryStore, SqlRow, TursoStorage,
};
use foundation_nativeapis::MemoryFs;
use foundation_vfs::AsyncVfsFileSystem;

/// A host context for the tests.
pub fn ctx() -> Ctx {
    Ctx::host()
}

/// An in-memory Structured Query Language (SQL) store.
pub async fn memory_store() -> TursoStorage {
    TursoStorage::new(":memory:", ctx()).await.unwrap()
}

/// An empty in-memory file system.
pub fn memory_fs() -> MemoryFs {
    MemoryFs::new(ctx())
}

/// Make a directory in a memory file system.
pub async fn mkdir(fs: &MemoryFs, path: &str) {
    fs.mkdir_async(path.to_string()).await.unwrap();
}

/// Write a text file in a memory file system.
pub async fn write(fs: &MemoryFs, path: &str, text: &str) {
    fs.write_file_async(path.to_string(), text.as_bytes().to_vec())
        .await
        .unwrap();
}

/// All rows of a query.
pub async fn rows(store: &dyn QueryStore, sql: &str) -> Vec<SqlRow> {
    store
        .query(sql, &[])
        .await
        .unwrap()
        .collect_all()
        .await
        .unwrap()
}

/// One number from a query.
pub async fn count(store: &dyn QueryStore, sql: &str) -> i64 {
    rows(store, sql).await[0].get::<i64>(0).unwrap()
}

/// Whether a table exists in a SQLite database.
pub async fn table_exists(store: &dyn QueryStore, table: &str) -> bool {
    !rows(
        store,
        &format!("SELECT name FROM sqlite_master WHERE type = 'table' AND name = '{table}'"),
    )
    .await
    .is_empty()
}

/// Whether a table has a column.
pub async fn has_column(store: &dyn QueryStore, table: &str, column: &str) -> bool {
    rows(store, &format!("PRAGMA table_info('{table}')"))
        .await
        .iter()
        .any(|row| row.get::<String>(1).unwrap() == column)
}

/// Insert a record row directly, for a test that simulates another writer.
pub async fn insert_record(
    store: &dyn QueryStore,
    group: &str,
    number: i64,
    name: &str,
    checksum: &str,
) {
    store
        .execute(
            "INSERT INTO _migrations (group_name, number, name, checksum, applied_at) \
             VALUES (?, ?, ?, ?, 0)",
            &[
                DataValue::Text(group.to_string()),
                DataValue::Integer(number),
                DataValue::Text(name.to_string()),
                DataValue::Text(checksum.to_string()),
            ],
        )
        .await
        .unwrap();
}

/// The group `shared`: users and sessions.
pub static SHARED: MigrationSet = MigrationSet {
    group: Cow::Borrowed("shared"),
    source: MigrationSource::Static(&[
        Migration {
            id: "001_create_users",
            name: "create users",
            sql: "CREATE TABLE IF NOT EXISTS users (id INTEGER PRIMARY KEY)",
        },
        Migration {
            id: "002_create_sessions",
            name: "create sessions",
            sql: "CREATE TABLE IF NOT EXISTS sessions (id INTEGER PRIMARY KEY)",
        },
    ]),
};

/// The group `tenant_a`: invoices and the value-added tax (VAT) rate.
pub static TENANT_A: MigrationSet = MigrationSet {
    group: Cow::Borrowed("tenant_a"),
    source: MigrationSource::Static(&[
        Migration {
            id: "001_create_invoices",
            name: "create invoices",
            sql: "CREATE TABLE IF NOT EXISTS invoices (id INTEGER PRIMARY KEY)",
        },
        Migration {
            id: "002_add_vat_rate",
            name: "add vat rate",
            sql: "CREATE TABLE IF NOT EXISTS vat_rates (id INTEGER PRIMARY KEY)",
        },
    ]),
};

/// The group `tenant_b`: reports.
pub static TENANT_B: MigrationSet = MigrationSet {
    group: Cow::Borrowed("tenant_b"),
    source: MigrationSource::Static(&[Migration {
        id: "001_create_reports",
        name: "create reports",
        sql: "CREATE TABLE IF NOT EXISTS reports (id INTEGER PRIMARY KEY)",
    }]),
};
