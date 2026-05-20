//! D1 storage via wasm-bindgen — calls Cloudflare D1 JS API directly.
//!
//! Wraps `D1Database` and implements all storage traits using D1's SQL engine.
//! Async `*_async` methods are the source of truth (JS Promises);
//! sync trait methods delegate via `schedule_future`.

use std::sync::Arc;

use base64::{engine::general_purpose::STANDARD, Engine};
use crate::core::backends::schedule_future;
use crate::core::errors::{StorageError, StorageResult};
use crate::core::storage_provider::{
    AsyncBlobStore, AsyncKeyValueStore, AsyncQueryStore, AsyncRateLimiterStore,
    BlobStore, DataValue, KeyValueStore, QueryStore, RateLimiterStore, SqlRow, StorageItemStream,
};
use crate::wasm::bindgen::D1Database;
use foundation_core::valtron::Stream;
use js_sys::{Array, Uint8Array};
use wasm_bindgen::JsCast;
use wasm_bindgen_futures::JsFuture;

// ===========================================================================
// D1WasmStorage
// ===========================================================================

/// D1 storage backend using wasm-bindgen JS bindings.
///
/// # Safety: `Send` and `Sync` on wasm32
/// `D1WasmStorage` wraps a `D1Database` (a JS type) which is `!Send` by default.
/// On the `wasm32-unknown-unknown` target, all code runs on a single thread with
/// no shared memory, so it is safe to mark this type `Send + Sync`. This allows
/// it to be used with APIs that require `Send + Sync` bounds (e.g. `SessionManager`).
#[derive(Clone)]
pub struct D1WasmStorage {
    db: Arc<D1Database>,
    table_prefix: String,
}

// Safety: wasm32-unknown-unknown is single-threaded with no shared memory.
// All JS objects live in the same isolate and cannot race across threads.
unsafe impl Send for D1WasmStorage {}
unsafe impl Sync for D1WasmStorage {}

impl D1WasmStorage {
    /// Create a new D1 storage instance.
    #[must_use]
    pub fn new(db: Arc<D1Database>, table_prefix: &str) -> Self {
        Self {
            db,
            table_prefix: table_prefix.to_string(),
        }
    }

    fn table_name(&self) -> String {
        format!("{}_kv", self.table_prefix)
    }

    fn stream_once<T: Send + 'static>(val: T) -> StorageItemStream<'static, T> {
        Box::new(std::iter::once(Stream::Next(Ok(val))))
    }

    fn stream_many<T: Send + 'static>(vals: Vec<T>) -> StorageItemStream<'static, T> {
        Box::new(vals.into_iter().map(|v| Stream::Next(Ok(v))))
    }

    /// Initialize the KV table schema.
    pub async fn init_schema_async(&self) -> Result<(), StorageError> {
        let sql = format!(
            "CREATE TABLE IF NOT EXISTS {} (key TEXT PRIMARY KEY, value TEXT NOT NULL, created_at INTEGER DEFAULT (strftime('%s', 'now') * 1000), updated_at INTEGER DEFAULT (strftime('%s', 'now') * 1000))",
            self.table_name()
        );
        self.do_execute_sql_async(&sql, &[]).await?;
        Ok(())
    }

    /// Execute a raw SQL statement (no results).
    async fn do_execute_sql_async(&self, sql: &str, params: &[DataValue]) -> Result<(), StorageError> {
        let stmt = self.db.prepare(sql);
        if params.is_empty() {
            let promise = stmt.run();
            JsFuture::from(promise)
                .await
                .map_err(|e| StorageError::Backend(format!("D1 run failed: {e:?}")))?;
        } else {
            let array = data_values_to_js_array(params);
            let bound = stmt.bind(array);
            let promise = bound.run();
            JsFuture::from(promise)
                .await
                .map_err(|e| StorageError::Backend(format!("D1 run failed: {e:?}")))?;
        }
        Ok(())
    }

    /// Execute SQL and return the first row as `JsValue` (object or null).
    async fn do_query_first_async(
        &self,
        sql: &str,
        params: &[DataValue],
    ) -> Result<Option<js_sys::Object>, StorageError> {
        let stmt = self.db.prepare(sql);
        let first_result = if params.is_empty() {
            stmt.first(None)
        } else {
            let array = data_values_to_js_array(params);
            let bound = stmt.bind(array);
            bound.first(None)
        };
        let result = JsFuture::from(first_result)
            .await
            .map_err(|e| StorageError::Backend(format!("D1 first failed: {e:?}")))?;

        if result.is_null() || result.is_undefined() {
            Ok(None)
        } else {
            let obj = result
                .dyn_into::<js_sys::Object>()
                .map_err(|_| StorageError::Backend("D1 first returned non-object".to_string()))?;
            Ok(Some(obj))
        }
    }

    /// Execute SQL and return all rows as `Vec<JsValue>` (objects).
    async fn do_query_all_async(
        &self,
        sql: &str,
        params: &[DataValue],
    ) -> Result<Vec<js_sys::Object>, StorageError> {
        let stmt = self.db.prepare(sql);
        let all_result = if params.is_empty() {
            stmt.all()
        } else {
            let array = data_values_to_js_array(params);
            let bound = stmt.bind(array);
            bound.all()
        };
        let result = JsFuture::from(all_result)
            .await
            .map_err(|e| StorageError::Backend(format!("D1 all failed: {e:?}")))?;

        // D1 returns { results: [...] } or just [...] depending on version
        // Try to extract results array
        let arr = if result.is_array() {
            result.unchecked_into::<Array>()
        } else {
            let obj = result.dyn_into::<js_sys::Object>().map_err(|_| {
                StorageError::Backend("D1 all returned non-object".to_string())
            })?;
            js_sys::Reflect::get(&obj, &"results".into())
                .ok()
                .and_then(|v| v.dyn_into::<Array>().ok())
                .unwrap_or_default()
        };

        let mut rows = Vec::new();
        for i in 0..arr.length() {
            let val = arr.get(i);
            if let Ok(obj) = val.dyn_into::<js_sys::Object>() {
                rows.push(obj);
            }
        }
        Ok(rows)
    }

    /// Execute SQL and return changes count from a D1 run result.
    async fn do_execute_with_changes_async(
        &self,
        sql: &str,
        params: &[DataValue],
    ) -> Result<u64, StorageError> {
        let stmt = self.db.prepare(sql);
        let run_result = if params.is_empty() {
            stmt.run()
        } else {
            let array = data_values_to_js_array(params);
            let bound = stmt.bind(array);
            bound.run()
        };
        let result = JsFuture::from(run_result)
            .await
            .map_err(|e| StorageError::Backend(format!("D1 run failed: {e:?}")))?;

        // Extract meta.changes from result
        let obj = result.dyn_into::<js_sys::Object>().map_err(|_| {
            StorageError::Backend("D1 run returned non-object".to_string())
        })?;

        let changes = js_sys::Reflect::get(&obj, &"meta".into())
            .ok()
            .and_then(|meta| {
                let meta_obj = meta.dyn_into::<js_sys::Object>().ok()?;
                js_sys::Reflect::get(&meta_obj, &"changes".into())
                    .ok()
                    .and_then(|v| v.as_f64().map(|f| f as u64))
            })
            .unwrap_or(0);

        Ok(changes)
    }

    /// Extract a string field from a JS object.
    fn get_str_field(obj: &js_sys::Object, field: &str) -> Option<String> {
        js_sys::Reflect::get(obj, &field.into())
            .ok()
            .and_then(|v| v.as_string())
    }

    /// Extract a number field from a JS object.
    fn get_num_field(obj: &js_sys::Object, field: &str) -> Option<f64> {
        js_sys::Reflect::get(obj, &field.into())
            .ok()
            .and_then(|v| v.as_f64())
    }
}

// ===========================================================================
// KeyValueStore
// ===========================================================================

impl KeyValueStore for D1WasmStorage {
    fn get<'a, V: serde::de::DeserializeOwned + Send + 'static>(
        &'a self,
        key: &str,
    ) -> StorageResult<StorageItemStream<'a, Option<V>>> {
        let this = self.clone();
        let key = key.to_string();
        schedule_future(async move {
            Self::do_get_async(&this.db, &this.table_prefix, &key).await
        })
    }

    fn set<V: serde::Serialize + Send + 'static>(&self, key: &str, value: V) -> StorageResult<StorageItemStream<'_, ()>> {
        let this = self.clone();
        let key = key.to_string();
        schedule_future(async move {
            Self::do_set_async(&this.db, &this.table_prefix, &key, value).await
        })
    }

    fn delete(&self, key: &str) -> StorageResult<StorageItemStream<'_, ()>> {
        let this = self.clone();
        let key = key.to_string();
        schedule_future(async move {
            Self::do_delete_async(&this.db, &this.table_prefix, &key).await
        })
    }

    fn exists(&self, key: &str) -> StorageResult<StorageItemStream<'_, bool>> {
        let this = self.clone();
        let key = key.to_string();
        schedule_future(async move {
            Self::do_exists_async(&this.db, &this.table_prefix, &key).await
        })
    }

    fn list_keys(&self, prefix: Option<&str>) -> StorageResult<StorageItemStream<'_, String>> {
        let this = self.clone();
        let prefix = prefix.map(String::from);
        let keys = crate::core::backends::exec_future(async move {
            Self::do_list_keys_async(&this.db, &this.table_prefix, prefix.as_deref()).await
        })?;
        Ok(Self::stream_many(keys))
    }
}

impl D1WasmStorage {
    /// Get a value by key from D1.
    pub async fn get_async<V: serde::de::DeserializeOwned + Send + 'static>(
        &self,
        key: &str,
    ) -> Result<Option<V>, StorageError> {
        Self::do_get_async(&self.db, &self.table_prefix, key).await
    }

    async fn do_get_async<V: serde::de::DeserializeOwned + Send + 'static>(
        db: &Arc<D1Database>,
        table_prefix: &str,
        key: &str,
    ) -> Result<Option<V>, StorageError> {
        let table = format!("{table_prefix}_kv");
        let sql = format!("SELECT value FROM {table} WHERE key = ?");
        let row = do_query_first(db, &sql, &[DataValue::Text(key.to_string())]).await?;

        match row {
            Some(obj) => {
                let value = get_str_field(&obj, "value").ok_or_else(|| {
                    StorageError::SqlConversion("missing or invalid value field".to_string())
                })?;
                let deserialized: V = serde_json::from_str(&value)
                    .map_err(|e| StorageError::Serialization(e.to_string()))?;
                Ok(Some(deserialized))
            }
            None => Ok(None),
        }
    }

    /// Set a key-value pair in D1.
    pub async fn set_async<V: serde::Serialize>(
        &self,
        key: &str,
        value: V,
    ) -> Result<(), StorageError> {
        Self::do_set_async(&self.db, &self.table_prefix, key, value).await
    }

    async fn do_set_async<V: serde::Serialize>(
        db: &Arc<D1Database>,
        table_prefix: &str,
        key: &str,
        value: V,
    ) -> Result<(), StorageError> {
        let table = format!("{table_prefix}_kv");
        let json_value = serde_json::to_string(&value)
            .map_err(|e| StorageError::Serialization(e.to_string()))?;

        let sql = format!(
            "INSERT INTO {table} (key, value, updated_at) VALUES (?, ?, strftime('%s', 'now') * 1000) ON CONFLICT(key) DO UPDATE SET value = ?, updated_at = strftime('%s', 'now') * 1000"
        );

        do_execute_sql(
            db,
            &sql,
            &[
                DataValue::Text(key.to_string()),
                DataValue::Text(json_value.clone()),
                DataValue::Text(json_value),
            ],
        )
        .await
    }

    /// Delete a key from D1.
    pub async fn delete_async(&self, key: &str) -> Result<(), StorageError> {
        Self::do_delete_async(&self.db, &self.table_prefix, key).await
    }

    async fn do_delete_async(
        db: &Arc<D1Database>,
        table_prefix: &str,
        key: &str,
    ) -> Result<(), StorageError> {
        let table = format!("{table_prefix}_kv");
        let sql = format!("DELETE FROM {table} WHERE key = ?");
        do_execute_sql(db, &sql, &[DataValue::Text(key.to_string())]).await
    }

    /// Check if a key exists in D1.
    pub async fn exists_async(&self, key: &str) -> Result<bool, StorageError> {
        Self::do_exists_async(&self.db, &self.table_prefix, key).await
    }

    async fn do_exists_async(
        db: &Arc<D1Database>,
        table_prefix: &str,
        key: &str,
    ) -> Result<bool, StorageError> {
        let table = format!("{table_prefix}_kv");
        let sql = format!("SELECT 1 FROM {table} WHERE key = ? LIMIT 1");
        let row = do_query_first(db, &sql, &[DataValue::Text(key.to_string())]).await?;
        Ok(row.is_some())
    }

    /// List all keys with optional prefix filter.
    pub async fn list_keys_async(&self, prefix: Option<&str>) -> Result<Vec<String>, StorageError> {
        Self::do_list_keys_async(&self.db, &self.table_prefix, prefix).await
    }

    async fn do_list_keys_async(
        db: &Arc<D1Database>,
        table_prefix: &str,
        prefix: Option<&str>,
    ) -> Result<Vec<String>, StorageError> {
        let table = format!("{table_prefix}_kv");
        let (sql, params) = match prefix {
            Some(p) => (
                format!("SELECT key FROM {table} WHERE key LIKE ? ORDER BY key"),
                vec![DataValue::Text(format!("{p}%"))],
            ),
            None => (
                format!("SELECT key FROM {table} ORDER BY key"),
                vec![],
            ),
        };

        let rows = do_query_all(db, &sql, &params).await?;
        let keys: Result<Vec<String>, StorageError> = rows
            .iter()
            .map(|obj| {
                get_str_field(obj, "key").ok_or_else(|| {
                    StorageError::SqlConversion("missing or invalid key field".to_string())
                })
            })
            .collect();
        keys
    }
}

#[async_trait::async_trait(?Send)]
impl AsyncKeyValueStore for D1WasmStorage {
    async fn get_async<V: serde::de::DeserializeOwned + Send + 'static>(&self, key: &str) -> StorageResult<Option<V>> {
        self.get_async(key).await
    }

    async fn set_async<V: serde::Serialize + Send + 'static>(&self, key: &str, value: V) -> StorageResult<()> {
        self.set_async(key, value).await
    }

    async fn delete_async(&self, key: &str) -> StorageResult<()> {
        self.delete_async(key).await
    }

    async fn exists_async(&self, key: &str) -> StorageResult<bool> {
        self.exists_async(key).await
    }

    async fn list_keys_async(&self, prefix: Option<&str>) -> StorageResult<Vec<String>> {
        self.list_keys_async(prefix).await
    }
}

// ===========================================================================
// QueryStore
// ===========================================================================

impl QueryStore for D1WasmStorage {
    fn query(
        &self,
        sql: &str,
        params: &[DataValue],
    ) -> StorageResult<StorageItemStream<'_, SqlRow>> {
        let this = self.clone();
        let sql = sql.to_string();
        let params = params.to_vec();
        let rows = crate::core::backends::exec_future(async move {
            Self::do_query_rows_async(&this.db, &sql, &params).await
        })?;
        Ok(Self::stream_many(rows))
    }

    fn execute(
        &self,
        sql: &str,
        params: &[DataValue],
    ) -> StorageResult<StorageItemStream<'_, u64>> {
        let this = self.clone();
        let sql = sql.to_string();
        let params = params.to_vec();
        schedule_future(async move {
            Self::do_execute_async(&this.db, &sql, &params).await
        })
    }

    fn execute_batch(&self, sql: &str) -> StorageResult<StorageItemStream<'_, ()>> {
        let this = self.clone();
        let sql = sql.to_string();
        schedule_future(async move {
            Self::do_execute_batch_async(&this.db, &sql).await
        })
    }
}

impl D1WasmStorage {
    /// Execute a SQL query and return rows as `Vec<SqlRow>`.
    pub async fn query_async(&self, sql: &str, params: &[DataValue]) -> Result<Vec<SqlRow>, StorageError> {
        Self::do_query_rows_async(&self.db, sql, params).await
    }

    async fn do_query_rows_async(
        db: &Arc<D1Database>,
        sql: &str,
        params: &[DataValue],
    ) -> Result<Vec<SqlRow>, StorageError> {
        let rows = do_query_all(db, sql, params).await?;

        let results: Vec<SqlRow> = rows
            .iter()
            .map(|obj| {
                let columns: Vec<(String, DataValue)> = js_object_to_columns(obj);
                SqlRow::new(columns)
            })
            .collect();

        Ok(results)
    }

    /// Execute a SQL statement and return the number of rows affected.
    pub async fn execute_async(&self, sql: &str, params: &[DataValue]) -> Result<u64, StorageError> {
        Self::do_execute_async(&self.db, sql, params).await
    }

    async fn do_execute_async(
        db: &Arc<D1Database>,
        sql: &str,
        params: &[DataValue],
    ) -> Result<u64, StorageError> {
        do_execute_with_changes(db, sql, params).await
    }

    /// Execute a batch of SQL statements.
    pub async fn execute_batch_async(&self, sql: &str) -> Result<(), StorageError> {
        Self::do_execute_batch_async(&self.db, sql).await
    }

    async fn do_execute_batch_async(
        db: &Arc<D1Database>,
        sql: &str,
    ) -> Result<(), StorageError> {
        do_execute_sql(db, sql, &[]).await
    }
}

#[async_trait::async_trait(?Send)]
impl AsyncQueryStore for D1WasmStorage {
    async fn query_async(
        &self,
        sql: &str,
        params: &[DataValue],
    ) -> StorageResult<Vec<SqlRow>> {
        self.query_async(sql, params).await
    }

    async fn execute_async(&self, sql: &str, params: &[DataValue]) -> StorageResult<u64> {
        self.execute_async(sql, params).await
    }

    async fn execute_batch_async(&self, sql: &str) -> StorageResult<()> {
        self.execute_batch_async(sql).await
    }
}

// ===========================================================================
// RateLimiterStore
// ===========================================================================

impl RateLimiterStore for D1WasmStorage {
    fn check_rate_limit(
        &self,
        key: &str,
        max_count: u32,
        window_seconds: u64,
    ) -> StorageResult<StorageItemStream<'_, bool>> {
        let this = self.clone();
        let key = key.to_string();
        schedule_future(async move {
            Self::do_check_rate_limit_async(&this.db, &key, max_count, window_seconds).await
        })
    }

    fn record_rate_limit(&self, key: &str) -> StorageResult<StorageItemStream<'_, u32>> {
        let this = self.clone();
        let key = key.to_string();
        schedule_future(async move {
            Self::do_record_rate_limit_async(&this.db, &key).await
        })
    }

    fn reset_rate_limit(&self, key: &str) -> StorageResult<StorageItemStream<'_, ()>> {
        let this = self.clone();
        let key = key.to_string();
        schedule_future(async move {
            Self::do_reset_rate_limit_async(&this.db, &key).await
        })
    }
}

impl D1WasmStorage {
    /// Check if a rate limit key is allowed.
    pub async fn check_rate_limit_async(
        &self,
        key: &str,
        max_count: u32,
        window_seconds: u64,
    ) -> Result<bool, StorageError> {
        Self::do_check_rate_limit_async(&self.db, key, max_count, window_seconds).await
    }

    async fn do_check_rate_limit_async(
        db: &Arc<D1Database>,
        key: &str,
        max_count: u32,
        window_seconds: u64,
    ) -> Result<bool, StorageError> {
        // Ensure table exists
        let create_table = r"
            CREATE TABLE IF NOT EXISTS rate_limits (
                key TEXT PRIMARY KEY,
                count INTEGER NOT NULL,
                window_start INTEGER NOT NULL
            )
        ";
        do_execute_sql(db, create_table, &[]).await?;

        // Use D1's strftime for consistent time
        let sql = "SELECT count, window_start FROM rate_limits WHERE key = ?";
        let row = do_query_first(db, sql, &[DataValue::Text(key.to_string())]).await?;

        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();
        let window_start = now.saturating_sub(window_seconds);

        let allowed = match row {
            Some(obj) => {
                let count = get_num_field(&obj, "count").unwrap_or(0.0) as u32;
                let stored_window = get_num_field(&obj, "window_start").unwrap_or(0.0) as u64;
                if stored_window < window_start {
                    true
                } else {
                    count < max_count
                }
            }
            None => true,
        };

        Ok(allowed)
    }

    /// Record a rate-limited action.
    pub async fn record_rate_limit_async(&self, key: &str) -> Result<u32, StorageError> {
        Self::do_record_rate_limit_async(&self.db, key).await
    }

    async fn do_record_rate_limit_async(
        db: &Arc<D1Database>,
        key: &str,
    ) -> Result<u32, StorageError> {
        let sql = r"
            INSERT INTO rate_limits (key, count, window_start)
            VALUES (?, 1, strftime('%s', 'now'))
            ON CONFLICT(key) DO UPDATE SET count = count + 1, window_start = excluded.window_start
        ";

        do_execute_sql(db, sql, &[DataValue::Text(key.to_string())]).await?;

        // Read back the count
        let row = do_query_first(
            db,
            "SELECT count FROM rate_limits WHERE key = ?",
            &[DataValue::Text(key.to_string())],
        )
        .await?;

        let count = row
            .as_ref()
            .and_then(|obj| get_num_field(obj, "count"))
            .map(|f| f as u32)
            .unwrap_or(1);

        Ok(count)
    }

    /// Reset a rate limit key.
    pub async fn reset_rate_limit_async(&self, key: &str) -> Result<(), StorageError> {
        Self::do_reset_rate_limit_async(&self.db, key).await
    }

    async fn do_reset_rate_limit_async(
        db: &Arc<D1Database>,
        key: &str,
    ) -> Result<(), StorageError> {
        do_execute_sql(
            db,
            "DELETE FROM rate_limits WHERE key = ?",
            &[DataValue::Text(key.to_string())],
        )
        .await
    }
}

#[async_trait::async_trait(?Send)]
impl AsyncRateLimiterStore for D1WasmStorage {
    async fn check_rate_limit_async(
        &self,
        key: &str,
        max_count: u32,
        window_seconds: u64,
    ) -> StorageResult<bool> {
        self.check_rate_limit_async(key, max_count, window_seconds).await
    }

    async fn record_rate_limit_async(&self, key: &str) -> StorageResult<u32> {
        self.record_rate_limit_async(key).await
    }

    async fn reset_rate_limit_async(&self, key: &str) -> StorageResult<()> {
        self.reset_rate_limit_async(key).await
    }
}

// ===========================================================================
// BlobStore
// ===========================================================================

impl BlobStore for D1WasmStorage {
    fn put_blob(&self, key: &str, data: &[u8]) -> StorageResult<StorageItemStream<'_, ()>> {
        let this = self.clone();
        let key = key.to_string();
        let data = data.to_vec();
        schedule_future(async move {
            Self::do_put_blob_async(&this.db, &this.table_prefix, &key, &data).await
        })
    }

    fn get_blob(&self, key: &str) -> StorageResult<StorageItemStream<'_, Option<Vec<u8>>>> {
        let this = self.clone();
        let key = key.to_string();
        schedule_future(async move {
            Self::do_get_blob_async(&this.db, &this.table_prefix, &key).await
        })
    }

    fn delete_blob(&self, key: &str) -> StorageResult<StorageItemStream<'_, ()>> {
        let this = self.clone();
        let key = key.to_string();
        schedule_future(async move {
            Self::do_delete_blob_async(&this.db, &this.table_prefix, &key).await
        })
    }

    fn blob_exists(&self, key: &str) -> StorageResult<StorageItemStream<'_, bool>> {
        let this = self.clone();
        let key = key.to_string();
        schedule_future(async move {
            Self::do_blob_exists_async(&this.db, &this.table_prefix, &key).await
        })
    }
}

impl D1WasmStorage {
    /// Put a blob into D1 (base64-encoded in the KV table).
    pub async fn put_blob_async(&self, key: &str, data: &[u8]) -> Result<(), StorageError> {
        Self::do_put_blob_async(&self.db, &self.table_prefix, key, data).await
    }

    async fn do_put_blob_async(
        db: &Arc<D1Database>,
        table_prefix: &str,
        key: &str,
        data: &[u8],
    ) -> Result<(), StorageError> {
        let table = format!("{table_prefix}_kv");
        let encoded = STANDARD.encode(data);
        let json = serde_json::json!({
            "type": "blob",
            "encoding": "base64",
            "data": encoded
        })
        .to_string();

        let sql = format!(
            "INSERT INTO {table} (key, value, updated_at) VALUES (?, ?, strftime('%s', 'now') * 1000) ON CONFLICT(key) DO UPDATE SET value = ?, updated_at = strftime('%s', 'now') * 1000"
        );

        do_execute_sql(
            db,
            &sql,
            &[
                DataValue::Text(key.to_string()),
                DataValue::Text(json.clone()),
                DataValue::Text(json),
            ],
        )
        .await
    }

    /// Get a blob from D1 (base64-decoded).
    pub async fn get_blob_async(&self, key: &str) -> Result<Option<Vec<u8>>, StorageError> {
        Self::do_get_blob_async(&self.db, &self.table_prefix, key).await
    }

    async fn do_get_blob_async(
        db: &Arc<D1Database>,
        table_prefix: &str,
        key: &str,
    ) -> Result<Option<Vec<u8>>, StorageError> {
        let table = format!("{table_prefix}_kv");
        let sql = format!("SELECT value FROM {table} WHERE key = ?");
        let row = do_query_first(db, &sql, &[DataValue::Text(key.to_string())]).await?;

        match row {
            Some(obj) => {
                let value = get_str_field(&obj, "value").ok_or_else(|| {
                    StorageError::SqlConversion("missing or invalid value field".to_string())
                })?;

                let wrapper: serde_json::Value = serde_json::from_str(&value)
                    .map_err(|e| StorageError::Serialization(e.to_string()))?;

                let is_blob = wrapper.get("type").and_then(|v| v.as_str()) == Some("blob");
                if !is_blob {
                    return Ok(None);
                }

                let encoded = wrapper
                    .get("data")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| StorageError::Serialization("missing data field in blob wrapper".to_string()))?;

                let decoded = STANDARD
                    .decode(encoded)
                    .map_err(|e| StorageError::Backend(format!("Base64 decode failed: {e}")))?;

                Ok(Some(decoded))
            }
            None => Ok(None),
        }
    }

    /// Delete a blob from D1.
    pub async fn delete_blob_async(&self, key: &str) -> Result<(), StorageError> {
        Self::do_delete_blob_async(&self.db, &self.table_prefix, key).await
    }

    async fn do_delete_blob_async(
        db: &Arc<D1Database>,
        table_prefix: &str,
        key: &str,
    ) -> Result<(), StorageError> {
        let table = format!("{table_prefix}_kv");
        let sql = format!("DELETE FROM {table} WHERE key = ?");
        do_execute_sql(db, &sql, &[DataValue::Text(key.to_string())]).await
    }

    /// Check if a blob exists in D1.
    pub async fn blob_exists_async(&self, key: &str) -> Result<bool, StorageError> {
        Self::do_blob_exists_async(&self.db, &self.table_prefix, key).await
    }

    async fn do_blob_exists_async(
        db: &Arc<D1Database>,
        table_prefix: &str,
        key: &str,
    ) -> Result<bool, StorageError> {
        let table = format!("{table_prefix}_kv");
        let sql = format!("SELECT 1 FROM {table} WHERE key = ? LIMIT 1");
        let row = do_query_first(db, &sql, &[DataValue::Text(key.to_string())]).await?;
        Ok(row.is_some())
    }
}

#[async_trait::async_trait(?Send)]
impl AsyncBlobStore for D1WasmStorage {
    async fn put_blob_async(&self, key: &str, data: &[u8]) -> StorageResult<()> {
        self.put_blob_async(key, data).await
    }

    async fn get_blob_async(&self, key: &str) -> StorageResult<Option<Vec<u8>>> {
        self.get_blob_async(key).await
    }

    async fn delete_blob_async(&self, key: &str) -> StorageResult<()> {
        self.delete_blob_async(key).await
    }

    async fn blob_exists_async(&self, key: &str) -> StorageResult<bool> {
        self.blob_exists_async(key).await
    }
}

// ===========================================================================
// Helpers
// ===========================================================================

/// Convert a slice of `DataValue` into a `js_sys::Array` for D1 bind().
fn data_values_to_js_array(values: &[DataValue]) -> Array {
    let array = Array::new();
    for v in values {
        let js_val = match v {
            DataValue::Null => wasm_bindgen::JsValue::null(),
            DataValue::Integer(i) => wasm_bindgen::JsValue::from_f64(*i as f64),
            DataValue::Real(f) => wasm_bindgen::JsValue::from_f64(*f),
            DataValue::Text(s) => wasm_bindgen::JsValue::from_str(s),
            DataValue::Blob(b) => {
                // Use Uint8Array for binary data
                let buf = Uint8Array::new_with_length(b.len() as u32);
                buf.copy_from(b);
                buf.into()
            }
        };
        array.push(&js_val);
    }
    array
}

/// Extract columns from a JS object (row from D1).
fn js_object_to_columns(obj: &js_sys::Object) -> Vec<(String, DataValue)> {
    let keys = js_sys::Object::keys(obj);
    let mut columns = Vec::new();

    for key in keys.iter() {
        let key_str = key.as_string().unwrap_or_default();
        let val = js_sys::Reflect::get(obj, &key).ok().unwrap_or(wasm_bindgen::JsValue::undefined());

        let dv = if val.is_null() || val.is_undefined() {
            DataValue::Null
        } else if let Some(f) = val.as_f64() {
            // Check if it's an integer
            if f.fract() == 0.0 && f >= (i64::MIN as f64) && f <= (i64::MAX as f64) {
                DataValue::Integer(f as i64)
            } else {
                DataValue::Real(f)
            }
        } else if let Some(s) = val.as_string() {
            DataValue::Text(s)
        } else if let Some(b) = val.dyn_ref::<js_sys::ArrayBuffer>() {
            let u8 = Uint8Array::new(b);
            DataValue::Blob(u8.to_vec())
        } else if let Some(b) = val.dyn_ref::<Uint8Array>() {
            DataValue::Blob(b.to_vec())
        } else {
            // Fallback: serialize to JSON string
            DataValue::Text(
                js_sys::JSON::stringify(&val)
                    .map(|s| s.as_string().unwrap_or_default())
                    .unwrap_or_else(|_| String::new()),
            )
        };

        columns.push((key_str, dv));
    }

    columns
}

// ============================================================================
// Inline helper functions for the `do_*` static methods — avoids needing
// `&self` when we just need `db` + params.
// ============================================================================

async fn do_execute_sql(
    db: &Arc<D1Database>,
    sql: &str,
    params: &[DataValue],
) -> Result<(), StorageError> {
    let stmt = db.prepare(sql);
    if params.is_empty() {
        let promise = stmt.run();
        JsFuture::from(promise)
            .await
            .map_err(|e| StorageError::Backend(format!("D1 run failed: {e:?}")))?;
    } else {
        let array = data_values_to_js_array(params);
        let bound = stmt.bind(array);
        let promise = bound.run();
        JsFuture::from(promise)
            .await
            .map_err(|e| StorageError::Backend(format!("D1 run failed: {e:?}")))?;
    }
    Ok(())
}

async fn do_query_first(
    db: &Arc<D1Database>,
    sql: &str,
    params: &[DataValue],
) -> Result<Option<js_sys::Object>, StorageError> {
    let stmt = db.prepare(sql);
    let first_result = if params.is_empty() {
        stmt.first(None)
    } else {
        let array = data_values_to_js_array(params);
        let bound = stmt.bind(array);
        bound.first(None)
    };
    let result = JsFuture::from(first_result)
        .await
        .map_err(|e| StorageError::Backend(format!("D1 first failed: {e:?}")))?;

    if result.is_null() || result.is_undefined() {
        Ok(None)
    } else {
        let obj = result
            .dyn_into::<js_sys::Object>()
            .map_err(|_| StorageError::Backend("D1 first returned non-object".to_string()))?;
        Ok(Some(obj))
    }
}

async fn do_query_all(
    db: &Arc<D1Database>,
    sql: &str,
    params: &[DataValue],
) -> Result<Vec<js_sys::Object>, StorageError> {
    let stmt = db.prepare(sql);
    let all_result = if params.is_empty() {
        stmt.all()
    } else {
        let array = data_values_to_js_array(params);
        let bound = stmt.bind(array);
        bound.all()
    };
    let result = JsFuture::from(all_result)
        .await
        .map_err(|e| StorageError::Backend(format!("D1 all failed: {e:?}")))?;

    let arr = if result.is_array() {
        result.unchecked_into::<Array>()
    } else {
        let obj = result.dyn_into::<js_sys::Object>().map_err(|_| {
            StorageError::Backend("D1 all returned non-object".to_string())
        })?;
        js_sys::Reflect::get(&obj, &"results".into())
            .ok()
            .and_then(|v| v.dyn_into::<Array>().ok())
            .unwrap_or_default()
    };

    let mut rows = Vec::new();
    for i in 0..arr.length() {
        let val = arr.get(i);
        if let Ok(obj) = val.dyn_into::<js_sys::Object>() {
            rows.push(obj);
        }
    }
    Ok(rows)
}

async fn do_execute_with_changes(
    db: &Arc<D1Database>,
    sql: &str,
    params: &[DataValue],
) -> Result<u64, StorageError> {
    let stmt = db.prepare(sql);
    let run_result = if params.is_empty() {
        stmt.run()
    } else {
        let array = data_values_to_js_array(params);
        let bound = stmt.bind(array);
        bound.run()
    };
    let result = JsFuture::from(run_result)
        .await
        .map_err(|e| StorageError::Backend(format!("D1 run failed: {e:?}")))?;

    let obj = result.dyn_into::<js_sys::Object>().map_err(|_| {
        StorageError::Backend("D1 run returned non-object".to_string())
    })?;

    let changes = js_sys::Reflect::get(&obj, &"meta".into())
        .ok()
        .and_then(|meta| {
            let meta_obj = meta.dyn_into::<js_sys::Object>().ok()?;
            js_sys::Reflect::get(&meta_obj, &"changes".into())
                .ok()
                .and_then(|v| v.as_f64().map(|f| f as u64))
        })
        .unwrap_or(0);

    Ok(changes)
}

fn get_str_field(obj: &js_sys::Object, field: &str) -> Option<String> {
    js_sys::Reflect::get(obj, &field.into())
        .ok()
        .and_then(|v| v.as_string())
}

fn get_num_field(obj: &js_sys::Object, field: &str) -> Option<f64> {
    js_sys::Reflect::get(obj, &field.into())
        .ok()
        .and_then(|v| v.as_f64())
}
