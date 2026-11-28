//! A real SQLite executor for the block-tier tests.
//!
//! The executor runs the actual Structured Query Language (SQL) statements that
//! the SQL virtual file system (VFS) emits. It uses libsql, the engine that the
//! workspace already builds for `foundation_db` and `foundation_nativeapis`.

use foundation_core::durable::sql::{
    SqlError, SqlExecutor, SqlFuture, SqlRow, SqlStatement, SqlValue,
};
use libsql_backend as libsql;
use std::sync::Arc;

/// A libsql-backed executor over an in-memory database.
pub struct LibsqlExecutor {
    conn: Arc<libsql::Connection>,
}

fn io_error(error: libsql::Error) -> SqlError {
    SqlError::Io(Box::new(error))
}

fn to_params(values: Vec<SqlValue>) -> Vec<libsql::Value> {
    values
        .into_iter()
        .map(|value| match value {
            SqlValue::Null => libsql::Value::Null,
            SqlValue::Integer(value) => libsql::Value::Integer(value),
            SqlValue::Real(value) => libsql::Value::Real(value),
            SqlValue::Text(value) => libsql::Value::Text(value),
            SqlValue::Blob(value) => libsql::Value::Blob(value),
        })
        .collect()
}

fn from_value(value: libsql::Value) -> SqlValue {
    match value {
        libsql::Value::Null => SqlValue::Null,
        libsql::Value::Integer(value) => SqlValue::Integer(value),
        libsql::Value::Real(value) => SqlValue::Real(value),
        libsql::Value::Text(value) => SqlValue::Text(value),
        libsql::Value::Blob(value) => SqlValue::Blob(value),
    }
}

impl LibsqlExecutor {
    /// Open a fresh in-memory database.
    ///
    /// # Panics
    ///
    /// Panics when the in-memory database cannot be created.
    pub async fn open() -> Self {
        let database = libsql::Builder::new_local(":memory:")
            .build()
            .await
            .expect("open the in-memory database");
        let conn = database
            .connect()
            .expect("connect to the in-memory database");
        Self {
            conn: Arc::new(conn),
        }
    }
}

impl SqlExecutor for LibsqlExecutor {
    fn execute(&self, statement: SqlStatement) -> SqlFuture<'_, u64> {
        Box::pin(async move {
            self.conn
                .execute(&statement.sql, to_params(statement.params))
                .await
                .map_err(io_error)
        })
    }

    fn query(&self, statement: SqlStatement) -> SqlFuture<'_, Vec<SqlRow>> {
        Box::pin(async move {
            let mut rows = self
                .conn
                .query(&statement.sql, to_params(statement.params))
                .await
                .map_err(io_error)?;
            let mut out = Vec::new();
            while let Some(row) = rows.next().await.map_err(io_error)? {
                let columns = row.column_count();
                let mut values = Vec::with_capacity(usize::try_from(columns).unwrap_or(0));
                for index in 0..columns {
                    values.push(from_value(row.get_value(index).map_err(io_error)?));
                }
                out.push(SqlRow::new(values));
            }
            Ok(out)
        })
    }

    fn transaction(&self, statements: Vec<SqlStatement>) -> SqlFuture<'_, Vec<u64>> {
        Box::pin(async move {
            let transaction = self.conn.transaction().await.map_err(io_error)?;
            let mut counts = Vec::with_capacity(statements.len());
            for statement in statements {
                counts.push(
                    transaction
                        .execute(&statement.sql, to_params(statement.params))
                        .await
                        .map_err(io_error)?,
                );
            }
            transaction.commit().await.map_err(io_error)?;
            Ok(counts)
        })
    }
}
