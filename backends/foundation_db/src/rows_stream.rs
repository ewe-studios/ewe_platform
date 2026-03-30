//! Rows iterator adapter for streaming SQL query results.
//!
//! This module provides a streaming iterator that yields rows one at a time
//! through the Valtron executor system without loading all rows into memory.
//!
//! WHY: Database row iterators (`turso::Rows`, `libsql::Rows`) are `!Send`, so they
//! cannot cross async boundaries. The `ThreadedFuture` executor spawns a dedicated
//! worker thread that owns the `!Send` type forever, streaming results back through
//! an mpp channel. This allows streaming large result sets with O(1) memory per row.

use crate::errors::{StorageError, StorageResult};
use crate::storage_provider::{DataValue, SqlRow};
use futures_lite::future::block_on;

// ============================================================================
// RowsIterator - Stream rows without collecting into Vec
// ============================================================================

/// Iterator that yields [`SqlRow`] values one at a time from a database query.
///
/// Owns `turso::Rows` directly (no `Arc<Mutex<>>`). Stays on worker thread.
/// Uses `futures-lite` polling since `turso::Rows::next()` returns a future that
/// borrows `&mut self` - we can't store both in a struct.
#[cfg(feature = "turso")]
pub struct RowsIterator {
    rows: turso::Rows,
}

#[cfg(feature = "turso")]
impl RowsIterator {
    #[must_use]
    pub fn new(rows: turso::Rows) -> Self {
        Self { rows }
    }

    fn turso_value_to_data_value(value: turso::Value) -> DataValue {
        match value {
            turso::Value::Null => DataValue::Null,
            turso::Value::Integer(i) => DataValue::Integer(i),
            turso::Value::Real(r) => DataValue::Real(r),
            turso::Value::Text(s) => DataValue::Text(s),
            turso::Value::Blob(b) => DataValue::Blob(b),
        }
    }

    fn convert_row(row: &turso::Row) -> StorageResult<SqlRow> {
        let column_count = row.column_count();
        let mut columns = Vec::with_capacity(column_count);
        for i in 0..column_count {
            let name = format!("col{i}");
            let value = Self::turso_value_to_data_value(
                row.get_value(i).map_err(|e| {
                    StorageError::SqlConversion(format!("Column {i} error: {e}"))
                })?
            );
            columns.push((name, value));
        }
        Ok(SqlRow::new(columns))
    }
}

#[cfg(feature = "turso")]
impl Iterator for RowsIterator {
    type Item = Result<SqlRow, StorageError>;

    fn next(&mut self) -> Option<Self::Item> {
        // Use futures-lite to poll the future synchronously
        // This is safe because we're running on a dedicated worker thread
        // spawned by ThreadedFuture
        match block_on(self.rows.next()) {
            Ok(Some(row)) => Some(Self::convert_row(&row)),
            Ok(None) => None,
            Err(e) => Some(Err(StorageError::Backend(e.to_string()))),
        }
    }
}

// ============================================================================
// LibsqlRowsIterator
// ============================================================================

#[cfg(feature = "libsql")]
pub struct LibsqlRowsIterator {
    rows: libsql::Rows,
}

#[cfg(feature = "libsql")]
impl LibsqlRowsIterator {
    pub fn new(rows: libsql::Rows) -> Self {
        Self { rows }
    }

    fn libsql_value_to_data_value(value: libsql::Value) -> DataValue {
        match value {
            libsql::Value::Null => DataValue::Null,
            libsql::Value::Integer(i) => DataValue::Integer(i),
            libsql::Value::Real(r) => DataValue::Real(r),
            libsql::Value::Text(s) => DataValue::Text(s),
            libsql::Value::Blob(b) => DataValue::Blob(b),
        }
    }

    fn convert_row(row: &libsql::Row, column_count: i32) -> StorageResult<SqlRow> {
        let mut columns = Vec::with_capacity(column_count.unsigned_abs() as usize);
        for i in 0..column_count {
            let name = format!("col{i}");
            let value = Self::libsql_value_to_data_value(
                row.get_value(i).map_err(|e| {
                    StorageError::SqlConversion(format!("Column {i} error: {e}"))
                })?
            );
            columns.push((name, value));
        }
        Ok(SqlRow::new(columns))
    }
}

#[cfg(feature = "libsql")]
impl Iterator for LibsqlRowsIterator {
    type Item = Result<SqlRow, StorageError>;

    fn next(&mut self) -> Option<Self::Item> {
        match block_on(self.rows.next()) {
            Ok(Some(row)) => Some(Self::convert_row(&row, row.column_count())),
            Ok(None) => None,
            Err(e) => Some(Err(StorageError::Backend(e.to_string()))),
        }
    }
}
