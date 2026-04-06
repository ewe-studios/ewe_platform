//! Rows iterator adapter for streaming SQL query results.
//!
//! This module provides generic streaming iterators that yield transformed values
//! one at a time through the Valtron executor system without loading all rows into memory.
//!
//! WHY: Database row iterators (`turso::Rows`, `libsql::Rows`) are `!Send`, so they
//! cannot cross async boundaries. The `ThreadedFuture` executor spawns a dedicated
//! worker thread that owns the `!Send` type forever, streaming results back through
//! a channel. This allows streaming large result sets with O(1) memory per row.
//!
//! HOW: The iterators are generic over the output type `T` and accept a transformation
//! function that converts each raw row into the desired type. Each use site provides
//! its own transformation closure specific to the type being extracted.

use futures_lite::future::block_on;

// ============================================================================
// RowsIterator - Stream transformed rows from turso
// ============================================================================

/// Iterator that yields transformed values from a turso database query.
///
/// Generic over output type `T` which must be `Send + 'static`.
/// Accepts a transformation function `F: FnMut(&turso::Row) -> Result<T, StorageError>`.
/// Owns `turso::Rows` directly on the worker thread.
#[cfg(feature = "turso")]
pub struct RowsIterator<T, F> {
    rows: turso::Rows,
    transform: F,
    _marker: core::marker::PhantomData<T>,
}

#[cfg(feature = "turso")]
impl<T, F> RowsIterator<T, F>
where
    T: Send + 'static,
    F: FnMut(&turso::Row) -> Result<T, crate::errors::StorageError>,
{
    #[must_use]
    pub fn new(rows: turso::Rows, transform: F) -> Self {
        Self {
            rows,
            transform,
            _marker: core::marker::PhantomData,
        }
    }
}

#[cfg(feature = "turso")]
impl<T, F> Iterator for RowsIterator<T, F>
where
    T: Send + 'static,
    F: FnMut(&turso::Row) -> Result<T, crate::errors::StorageError>,
{
    type Item = Result<T, crate::errors::StorageError>;

    fn next(&mut self) -> Option<Self::Item> {
        match block_on(self.rows.next()) {
            Ok(Some(row)) => Some((self.transform)(&row)),
            Ok(None) => None,
            Err(e) => Some(Err(crate::errors::StorageError::Backend(e.to_string()))),
        }
    }
}

// ============================================================================
// LibsqlRowsIterator - Stream transformed rows from libsql
// ============================================================================

/// Iterator that yields transformed values from a libsql database query.
///
/// Generic over output type `T` which must be `Send + 'static`.
/// Accepts a transformation function `F: FnMut(&libsql::Row) -> Result<T, StorageError>`.
/// Owns `libsql::Rows` directly on the worker thread.
#[cfg(feature = "libsql")]
pub struct LibsqlRowsIterator<T, F> {
    rows: libsql::Rows,
    transform: F,
    _marker: core::marker::PhantomData<T>,
}

#[cfg(feature = "libsql")]
impl<T, F> LibsqlRowsIterator<T, F>
where
    T: Send + 'static,
    F: FnMut(&libsql::Row) -> Result<T, crate::errors::StorageError>,
{
    #[must_use]
    pub fn new(rows: libsql::Rows, transform: F) -> Self {
        Self {
            rows,
            transform,
            _marker: core::marker::PhantomData,
        }
    }
}

#[cfg(feature = "libsql")]
impl<T, F> Iterator for LibsqlRowsIterator<T, F>
where
    T: Send + 'static,
    F: FnMut(&libsql::Row) -> Result<T, crate::errors::StorageError>,
{
    type Item = Result<T, crate::errors::StorageError>;

    fn next(&mut self) -> Option<Self::Item> {
        match block_on(self.rows.next()) {
            Ok(Some(row)) => Some((self.transform)(&row)),
            Ok(None) => None,
            Err(e) => Some(Err(crate::errors::StorageError::Backend(e.to_string()))),
        }
    }
}
