//! A Durable Object (DO) SQLite executor for the Durable Streams SQL virtual
//! file system (VFS).
//!
//! The VFS engine lives in `foundation_core::durable::sql`. This module maps its
//! three calls to the Durable Object SQLite storage application programming
//! interface (API):
//!
//! - `execute` runs one statement and reads the changed-row count.
//! - `query` runs one statement and reads the rows in column order.
//! - `transaction` runs the statements inside one `transactionSync` closure.
//!   The closure commits when it returns and rolls back when it throws.
//!
//! Every value is a `?` placeholder with a binding. The code never puts a value
//! into the statement text.
//!
//! The Workers Rust library (`workers-rs`) version 0.8 does not bind
//! `transactionSync`. The binding in `crate::wasm::bindgen::do_sql` adds it.

use foundation_compact::SendWrapper;
use foundation_core::durable::sql::{SqlError, SqlExecutor, SqlFuture, SqlRow, SqlStatement};
use js_sys::Array;
use std::cell::RefCell;
use std::rc::Rc;
use wasm_bindgen::closure::Closure;
use wasm_bindgen::{JsCast, JsValue};

use super::sql_value::{js_to_value, value_to_js};
use crate::wasm::bindgen::{DurableObjectStorage, SqlStorage, SqlStorageCursor};

/// A Durable Object SQLite-backed executor.
///
/// The executor holds the storage handle. It has no global state, and it does
/// no runtime lookup.
#[derive(Clone)]
pub struct DoSqlExecutor {
    storage: DurableObjectStorage,
}

impl DoSqlExecutor {
    /// Build an executor from the Durable Object storage handle.
    #[must_use]
    pub fn new(storage: DurableObjectStorage) -> Self {
        Self { storage }
    }

    /// Build an executor from the Durable Object state that the runtime passes
    /// to a Durable Object constructor.
    ///
    /// This is the normal path. `workers-rs` hides the raw storage handle, so
    /// this method takes the state and reads its `storage` property.
    ///
    /// # Errors
    ///
    /// Returns a Workers error when the state has no storage handle.
    pub fn from_state(state: worker::durable::State) -> Result<Self, worker::Error> {
        let storage = state._inner().storage().map_err(worker::Error::from)?;
        let storage: JsValue = storage.into();
        Ok(Self::new(storage.unchecked_into()))
    }
}

impl SqlExecutor for DoSqlExecutor {
    fn execute(&self, statement: SqlStatement) -> SqlFuture<'_, u64> {
        let storage = self.storage.clone();
        Box::pin(SendWrapper::new(async move {
            let sql = storage.sql().map_err(|error| js_error(&error))?;
            execute_statement(&sql, &statement)
        }))
    }

    fn query(&self, statement: SqlStatement) -> SqlFuture<'_, Vec<SqlRow>> {
        let storage = self.storage.clone();
        Box::pin(SendWrapper::new(async move {
            let sql = storage.sql().map_err(|error| js_error(&error))?;
            query_statement(&sql, &statement)
        }))
    }

    fn transaction(&self, statements: Vec<SqlStatement>) -> SqlFuture<'_, Vec<u64>> {
        let storage = self.storage.clone();
        Box::pin(SendWrapper::new(async move {
            run_transaction(&storage, statements)
        }))
    }
}

/// The result of the transaction closure.
#[derive(Default)]
struct TransactionOutcome {
    counts: Vec<u64>,
    error: Option<SqlError>,
}

/// Run one transaction on the storage handle.
fn run_transaction(
    storage: &DurableObjectStorage,
    statements: Vec<SqlStatement>,
) -> Result<Vec<u64>, SqlError> {
    let sql = storage.sql().map_err(|error| js_error(&error))?;
    let outcome: Rc<RefCell<TransactionOutcome>> = Rc::new(RefCell::new(TransactionOutcome {
        counts: Vec::with_capacity(statements.len()),
        error: None,
    }));
    let shared = Rc::clone(&outcome);
    let closure: Closure<dyn FnMut() -> JsValue> =
        Closure::once_assert_unwind_safe(move || -> JsValue {
            let mut counts = Vec::with_capacity(statements.len());
            for statement in &statements {
                match execute_statement(&sql, statement) {
                    Ok(count) => counts.push(count),
                    Err(error) => {
                        // A throw rolls the whole transaction back.
                        shared.borrow_mut().error = Some(error);
                        wasm_bindgen::throw_str("the Durable Object SQL transaction failed");
                    }
                }
            }
            shared.borrow_mut().counts = counts;
            JsValue::NULL
        });
    let result = storage.transaction_sync(&closure);
    drop(closure);
    match result {
        Ok(_) => {
            let mut outcome = outcome.borrow_mut();
            Ok(std::mem::take(&mut outcome.counts))
        }
        Err(error) => {
            let mut outcome = outcome.borrow_mut();
            match outcome.error.take() {
                Some(error) => Err(error),
                None => Err(js_error(&error)),
            }
        }
    }
}

/// Run one statement and return its changed-row count.
///
/// The runtime's `rowsWritten` counter includes index rows, so an insert into
/// an indexed table reports two. The engine needs the changed-row count, which
/// is what Cloudflare D1 returns as `meta.changes`. The change in the SQLite
/// `total_changes()` counter is that count: it rises by one for each inserted,
/// updated or deleted row, and a query leaves it unchanged.
fn execute_statement(sql: &SqlStorage, statement: &SqlStatement) -> Result<u64, SqlError> {
    let before = total_changes(sql)?;
    let cursor = exec(sql, statement)?;
    // The runtime runs the statement before it returns the cursor. A write has
    // no rows, so this loop is short.
    for_next(&cursor)?;
    let after = total_changes(sql)?;
    Ok(u64::try_from(after.saturating_sub(before)).unwrap_or(0))
}

/// Return the SQLite `total_changes()` counter of this connection.
fn total_changes(sql: &SqlStorage) -> Result<i64, SqlError> {
    let rows = query_statement(sql, &SqlStatement::plain("SELECT total_changes()"))?;
    Ok(rows
        .first()
        .and_then(|row| row.integer(0).ok())
        .unwrap_or(0))
}

/// Iterate a cursor to its end without keeping the rows.
fn for_next(cursor: &SqlStorageCursor) -> Result<(), SqlError> {
    let iterator = cursor.raw();
    loop {
        let next = iterator.next().map_err(|error| js_error(&error))?;
        if next.done() {
            return Ok(());
        }
    }
}

/// Run one query and return its rows in column order.
fn query_statement(sql: &SqlStorage, statement: &SqlStatement) -> Result<Vec<SqlRow>, SqlError> {
    let cursor = exec(sql, statement)?;
    let iterator = cursor.raw();
    let mut rows = Vec::new();
    loop {
        let next = iterator.next().map_err(|error| js_error(&error))?;
        if next.done() {
            break;
        }
        let row = Array::from(&next.value());
        let mut values = Vec::with_capacity(usize::try_from(row.length()).unwrap_or(0));
        for index in 0..row.length() {
            values.push(js_to_value(&row.get(index)));
        }
        rows.push(SqlRow::new(values));
    }
    Ok(rows)
}

/// Run one statement with its bound values.
fn exec(sql: &SqlStorage, statement: &SqlStatement) -> Result<SqlStorageCursor, SqlError> {
    let bindings = Array::new();
    for value in &statement.params {
        bindings.push(&value_to_js(value));
    }
    sql.exec(&statement.sql, bindings)
        .map_err(|error| js_error(&error))
}

fn js_error(error: &JsValue) -> SqlError {
    SqlError::Io(Box::new(std::io::Error::other(format!(
        "Durable Object SQL call failed: {error:?}"
    ))))
}
