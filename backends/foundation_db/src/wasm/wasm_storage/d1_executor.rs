//! A Cloudflare D1 executor for the Durable Streams SQL virtual file system
//! (VFS).
//!
//! The VFS engine lives in `foundation_core::durable::sql`. This file only maps
//! its three executor calls to D1: `prepare` and `bind` for a statement,
//! `raw` for a query and `batch` for an atomic transaction.
//!
//! D1 returns a JavaScript Number for every parameter. A 64-bit value past
//! 2^53 does not fit a Number, so this executor binds such a value as decimal
//! text; the typed SQLite column converts it back to an integer.

use foundation_compact::SendWrapper;
use foundation_core::durable::sql::{SqlError, SqlExecutor, SqlFuture, SqlRow, SqlStatement};
use js_sys::{Array, Reflect};
use std::sync::Arc;
use wasm_bindgen::prelude::*;
use wasm_bindgen_futures::JsFuture;

use super::sql_value::{js_to_value, value_to_js};
use crate::wasm::bindgen::{D1Database, D1PreparedStatement};

/// A D1-backed executor.
#[derive(Clone)]
pub struct D1Executor {
    db: Arc<D1Database>,
}

impl D1Executor {
    /// Build an executor from a D1 binding.
    #[must_use]
    pub fn new(db: Arc<D1Database>) -> Self {
        Self { db }
    }

    /// Extract the D1 binding from the Worker environment.
    ///
    /// # Errors
    ///
    /// Returns a JavaScript error when the binding is missing.
    pub fn from_env(env: &JsValue, binding: &str) -> Result<Self, JsError> {
        Ok(Self::new(Arc::new(D1Database::from_env(env, binding)?)))
    }

    fn prepare(&self, statement: &SqlStatement) -> D1PreparedStatement {
        let prepared = self.db.prepare(&statement.sql);
        if statement.params.is_empty() {
            return prepared;
        }
        let bound = Array::new();
        for value in &statement.params {
            bound.push(&value_to_js(value));
        }
        prepared.bind(bound)
    }
}

impl SqlExecutor for D1Executor {
    fn execute(&self, statement: SqlStatement) -> SqlFuture<'_, u64> {
        Box::pin(SendWrapper::new(async move {
            let prepared = self.prepare(&statement);
            let result = JsFuture::from(prepared.run())
                .await
                .map_err(|error| js_error(&error))?;
            changes(&result)
        }))
    }

    fn query(&self, statement: SqlStatement) -> SqlFuture<'_, Vec<SqlRow>> {
        Box::pin(SendWrapper::new(async move {
            let prepared = self.prepare(&statement);
            let result = JsFuture::from(prepared.raw())
                .await
                .map_err(|error| js_error(&error))?;
            rows(&result)
        }))
    }

    fn transaction(&self, statements: Vec<SqlStatement>) -> SqlFuture<'_, Vec<u64>> {
        Box::pin(SendWrapper::new(async move {
            let batch = Array::new();
            for statement in &statements {
                let prepared = self.prepare(statement);
                batch.push(&prepared.into());
            }
            let result = JsFuture::from(self.db.batch(batch))
                .await
                .map_err(|error| js_error(&error))?;
            let results = result
                .dyn_ref::<Array>()
                .ok_or_else(|| fail("the D1 batch returned no result array"))?;
            let mut counts = Vec::with_capacity(usize::try_from(results.length()).unwrap_or(0));
            for index in 0..results.length() {
                counts.push(changes(&results.get(index))?);
            }
            Ok(counts)
        }))
    }
}

fn fail(message: &str) -> SqlError {
    SqlError::Io(Box::new(std::io::Error::other(message.to_string())))
}

fn js_error(error: &JsValue) -> SqlError {
    SqlError::Io(Box::new(std::io::Error::other(format!(
        "D1 call failed: {error:?}"
    ))))
}

/// Read the changed-row count from one D1 result.
fn changes(result: &JsValue) -> Result<u64, SqlError> {
    let object = result
        .dyn_ref::<js_sys::Object>()
        .ok_or_else(|| fail("the D1 result is not an object"))?;
    if let Ok(success) = Reflect::get(object, &JsValue::from_str("success")) {
        if success.as_bool() == Some(false) {
            return Err(fail("a D1 batch statement failed"));
        }
    }
    let meta = Reflect::get(object, &JsValue::from_str("meta"))
        .ok()
        .and_then(|value| value.dyn_into::<js_sys::Object>().ok());
    let count = meta
        .and_then(|meta| Reflect::get(&meta, &JsValue::from_str("changes")).ok())
        .and_then(|value| value.as_f64())
        .unwrap_or(0.0);
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    Ok(count.max(0.0) as u64)
}

/// Read the rows of a `raw()` result, in column order.
fn rows(result: &JsValue) -> Result<Vec<SqlRow>, SqlError> {
    let rows = result
        .dyn_ref::<Array>()
        .ok_or_else(|| fail("the D1 raw query returned no row array"))?;
    let mut out = Vec::with_capacity(usize::try_from(rows.length()).unwrap_or(0));
    for row_index in 0..rows.length() {
        let row = rows
            .get(row_index)
            .dyn_into::<Array>()
            .map_err(|_| fail("a D1 raw row is not an array"))?;
        let mut values = Vec::with_capacity(usize::try_from(row.length()).unwrap_or(0));
        for column in 0..row.length() {
            values.push(js_to_value(&row.get(column)));
        }
        out.push(SqlRow::new(values));
    }
    Ok(out)
}
