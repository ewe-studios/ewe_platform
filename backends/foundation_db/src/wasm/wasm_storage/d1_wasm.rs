//! D1 storage via wasm-bindgen — calls Cloudflare D1 JS API directly.
//!
//! Wraps `D1Database` and implements all storage traits using D1's SQL engine.
//! Async `*_async` methods resolve JS Promises; trait methods call them via
//! `futures_lite::block_on`.

use base64::{engine::general_purpose::STANDARD, Engine};
use js_sys::{Array, Uint8Array};
use wasm_bindgen::JsCast;
use wasm_bindgen_futures::JsFuture;

use crate::core::errors::{StorageError, StorageResult};
use crate::core::storage_provider::{
    BlobStore, DataValue, KeyValueStore, QueryStore, RateLimiterStore, SqlRow, StorageItemStream,
};
use crate::wasm::bindgen::D1Database;
use foundation_core::valtron::Stream;

// ===========================================================================
// D1WasmStorage
// ===========================================================================

/// D1 storage backend using wasm-bindgen JS bindings.
pub struct D1WasmStorage {
    db: D1Database,
    table_prefix: String,
}

impl D1WasmStorage {
    /// Create a new D1 storage instance.
    #[must_use]
    pub fn new(db: D1Database, table_prefix: &str) -> Self {
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
        self.execute_sql_async(&sql, &[]).await?;
        Ok(())
    }

    /// Execute a raw SQL statement (no results).
    async fn execute_sql_async(&self, sql: &str, params: &[DataValue]) -> Result<(), StorageError> {
        let stmt = self.db.prepare(sql);
        let array = data_values_to_js_array(params);
        let bound = stmt.bind(&array);
        let promise = bound.run();
        JsFuture::from(promise)
            .await
            .map_err(|e| StorageError::Backend(format!("D1 run failed: {e:?}")))?;
        Ok(())
    }

    /// Execute SQL and return the first row as `JsValue` (object or null).
    async fn query_first_async(
        &self,
        sql: &str,
        params: &[DataValue],
    ) -> Result<Option<js_sys::Object>, StorageError> {
        let stmt = self.db.prepare(sql);
        let array = data_values_to_js_array(params);
        let bound = stmt.bind(&array);
        let promise = bound.first(None);
        let result = JsFuture::from(promise)
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
    async fn query_all_async(
        &self,
        sql: &str,
        params: &[DataValue],
    ) -> Result<Vec<js_sys::Object>, StorageError> {
        let stmt = self.db.prepare(sql);
        let array = data_values_to_js_array(params);
        let bound = stmt.bind(&array);
        let promise = bound.all();
        let result = JsFuture::from(promise)
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

    /// Get changes count from a D1 run result.
    async fn execute_sql_with_changes_async(
        &self,
        sql: &str,
        params: &[DataValue],
    ) -> Result<u64, StorageError> {
        let stmt = self.db.prepare(sql);
        let array = data_values_to_js_array(params);
        let bound = stmt.bind(&array);
        let promise = bound.run();
        let result = JsFuture::from(promise)
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
        let result = futures_lite::future::block_on(self.get_async(key))?;
        Ok(Self::stream_once(result))
    }

    fn set<V: serde::Serialize>(&self, key: &str, value: V) -> StorageResult<StorageItemStream<'_, ()>> {
        futures_lite::future::block_on(self.set_async(key, value))?;
        Ok(Self::stream_once(()))
    }

    fn delete(&self, key: &str) -> StorageResult<StorageItemStream<'_, ()>> {
        futures_lite::future::block_on(self.delete_async(key))?;
        Ok(Self::stream_once(()))
    }

    fn exists(&self, key: &str) -> StorageResult<StorageItemStream<'_, bool>> {
        let result = futures_lite::future::block_on(self.exists_async(key))?;
        Ok(Self::stream_once(result))
    }

    fn list_keys(&self, prefix: Option<&str>) -> StorageResult<StorageItemStream<'_, String>> {
        let result = futures_lite::future::block_on(self.list_keys_async(prefix))?;
        Ok(Self::stream_many(result))
    }
}

impl D1WasmStorage {
    async fn get_async<V: serde::de::DeserializeOwned + Send + 'static>(
        &self,
        key: &str,
    ) -> Result<Option<V>, StorageError> {
        let sql = format!("SELECT value FROM {} WHERE key = ?", self.table_name());
        let row = self.query_first_async(&sql, &[DataValue::Text(key.to_string())]).await?;

        match row {
            Some(obj) => {
                let value = Self::get_str_field(&obj, "value").ok_or_else(|| {
                    StorageError::SqlConversion("missing or invalid value field".to_string())
                })?;
                let deserialized: V = serde_json::from_str(&value)
                    .map_err(|e| StorageError::Serialization(e.to_string()))?;
                Ok(Some(deserialized))
            }
            None => Ok(None),
        }
    }

    async fn set_async<V: serde::Serialize>(
        &self,
        key: &str,
        value: V,
    ) -> Result<(), StorageError> {
        let json_value = serde_json::to_string(&value)
            .map_err(|e| StorageError::Serialization(e.to_string()))?;

        let sql = format!(
            "INSERT INTO {} (key, value, updated_at) VALUES (?, ?, strftime('%s', 'now') * 1000) ON CONFLICT(key) DO UPDATE SET value = ?, updated_at = strftime('%s', 'now') * 1000",
            self.table_name()
        );

        self.execute_sql_async(
            &sql,
            &[
                DataValue::Text(key.to_string()),
                DataValue::Text(json_value.clone()),
                DataValue::Text(json_value),
            ],
        )
        .await
    }

    async fn delete_async(&self, key: &str) -> Result<(), StorageError> {
        let sql = format!("DELETE FROM {} WHERE key = ?", self.table_name());
        self.execute_sql_async(&sql, &[DataValue::Text(key.to_string())]).await
    }

    async fn exists_async(&self, key: &str) -> Result<bool, StorageError> {
        let sql = format!("SELECT 1 FROM {} WHERE key = ? LIMIT 1", self.table_name());
        let row = self.query_first_async(&sql, &[DataValue::Text(key.to_string())]).await?;
        Ok(row.is_some())
    }

    async fn list_keys_async(&self, prefix: Option<&str>) -> Result<Vec<String>, StorageError> {
        let (sql, params) = match prefix {
            Some(p) => (
                format!("SELECT key FROM {} WHERE key LIKE ? ORDER BY key", self.table_name()),
                vec![DataValue::Text(format!("{p}%"))],
            ),
            None => (
                format!("SELECT key FROM {} ORDER BY key", self.table_name()),
                vec![],
            ),
        };

        let rows = self.query_all_async(&sql, &params).await?;
        let keys: Result<Vec<String>, StorageError> = rows
            .iter()
            .map(|obj| {
                Self::get_str_field(obj, "key").ok_or_else(|| {
                    StorageError::SqlConversion("missing or invalid key field".to_string())
                })
            })
            .collect();
        keys
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
        let result = futures_lite::future::block_on(self.query_async(sql, params))?;
        Ok(Self::stream_many(result))
    }

    fn execute(
        &self,
        sql: &str,
        params: &[DataValue],
    ) -> StorageResult<StorageItemStream<'_, u64>> {
        let result = futures_lite::future::block_on(self.execute_async(sql, params))?;
        Ok(Self::stream_once(result))
    }

    fn execute_batch(&self, sql: &str) -> StorageResult<StorageItemStream<'_, ()>> {
        futures_lite::future::block_on(self.execute_batch_async(sql))?;
        Ok(Self::stream_once(()))
    }
}

impl D1WasmStorage {
    async fn query_async(&self, sql: &str, params: &[DataValue]) -> Result<Vec<SqlRow>, StorageError> {
        let rows = self.query_all_async(sql, params).await?;

        let results: Vec<SqlRow> = rows
            .iter()
            .map(|obj| {
                let columns: Vec<(String, DataValue)> = js_object_to_columns(obj);
                SqlRow::new(columns)
            })
            .collect();

        Ok(results)
    }

    async fn execute_async(&self, sql: &str, params: &[DataValue]) -> Result<u64, StorageError> {
        self.execute_sql_with_changes_async(sql, params).await
    }

    async fn execute_batch_async(&self, sql: &str) -> Result<(), StorageError> {
        self.execute_sql_async(sql, &[]).await
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
        let result = futures_lite::future::block_on(self.check_rate_limit_async(key, max_count, window_seconds))?;
        Ok(Self::stream_once(result))
    }

    fn record_rate_limit(&self, key: &str) -> StorageResult<StorageItemStream<'_, u32>> {
        let result = futures_lite::future::block_on(self.record_rate_limit_async(key))?;
        Ok(Self::stream_once(result))
    }

    fn reset_rate_limit(&self, key: &str) -> StorageResult<StorageItemStream<'_, ()>> {
        futures_lite::future::block_on(self.reset_rate_limit_async(key))?;
        Ok(Self::stream_once(()))
    }
}

impl D1WasmStorage {
    async fn check_rate_limit_async(
        &self,
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
        self.execute_sql_async(create_table, &[]).await?;

        // Use D1's strftime for consistent time
        let sql = "SELECT count, window_start FROM rate_limits WHERE key = ?";
        let row = self.query_first_async(sql, &[DataValue::Text(key.to_string())]).await?;

        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();
        let window_start = now.saturating_sub(window_seconds);

        let allowed = match row {
            Some(obj) => {
                let count = Self::get_num_field(&obj, "count").unwrap_or(0.0) as u32;
                let stored_window = Self::get_num_field(&obj, "window_start").unwrap_or(0.0) as u64;
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

    async fn record_rate_limit_async(&self, key: &str) -> Result<u32, StorageError> {
        let sql = r"
            INSERT INTO rate_limits (key, count, window_start)
            VALUES (?, 1, strftime('%s', 'now'))
            ON CONFLICT(key) DO UPDATE SET count = count + 1, window_start = excluded.window_start
        ";

        self.execute_sql_async(sql, &[DataValue::Text(key.to_string())]).await?;

        // Read back the count
        let row = self
            .query_first_async("SELECT count FROM rate_limits WHERE key = ?", &[DataValue::Text(key.to_string())])
            .await?;

        let count = row
            .as_ref()
            .and_then(|obj| Self::get_num_field(obj, "count"))
            .map(|f| f as u32)
            .unwrap_or(1);

        Ok(count)
    }

    async fn reset_rate_limit_async(&self, key: &str) -> Result<(), StorageError> {
        self.execute_sql_async("DELETE FROM rate_limits WHERE key = ?", &[DataValue::Text(key.to_string())])
            .await
    }
}

// ===========================================================================
// BlobStore
// ===========================================================================

impl BlobStore for D1WasmStorage {
    fn put_blob(&self, key: &str, data: &[u8]) -> StorageResult<StorageItemStream<'_, ()>> {
        futures_lite::future::block_on(self.put_blob_async(key, data))?;
        Ok(Self::stream_once(()))
    }

    fn get_blob(&self, key: &str) -> StorageResult<StorageItemStream<'_, Option<Vec<u8>>>> {
        let result = futures_lite::future::block_on(self.get_blob_async(key))?;
        Ok(Self::stream_once(result))
    }

    fn delete_blob(&self, key: &str) -> StorageResult<StorageItemStream<'_, ()>> {
        futures_lite::future::block_on(self.delete_blob_async(key))?;
        Ok(Self::stream_once(()))
    }

    fn blob_exists(&self, key: &str) -> StorageResult<StorageItemStream<'_, bool>> {
        let result = futures_lite::future::block_on(self.blob_exists_async(key))?;
        Ok(Self::stream_once(result))
    }
}

impl D1WasmStorage {
    async fn put_blob_async(&self, key: &str, data: &[u8]) -> Result<(), StorageError> {
        let encoded = STANDARD.encode(data);
        let json = serde_json::json!({
            "type": "blob",
            "encoding": "base64",
            "data": encoded
        })
        .to_string();

        let sql = format!(
            "INSERT INTO {} (key, value, updated_at) VALUES (?, ?, strftime('%s', 'now') * 1000) ON CONFLICT(key) DO UPDATE SET value = ?, updated_at = strftime('%s', 'now') * 1000",
            self.table_name()
        );

        self.execute_sql_async(
            &sql,
            &[
                DataValue::Text(key.to_string()),
                DataValue::Text(json.clone()),
                DataValue::Text(json),
            ],
        )
        .await
    }

    async fn get_blob_async(&self, key: &str) -> Result<Option<Vec<u8>>, StorageError> {
        let sql = format!("SELECT value FROM {} WHERE key = ?", self.table_name());
        let row = self.query_first_async(&sql, &[DataValue::Text(key.to_string())]).await?;

        match row {
            Some(obj) => {
                let value = Self::get_str_field(&obj, "value").ok_or_else(|| {
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

    async fn delete_blob_async(&self, key: &str) -> Result<(), StorageError> {
        let sql = format!("DELETE FROM {} WHERE key = ?", self.table_name());
        self.execute_sql_async(&sql, &[DataValue::Text(key.to_string())]).await
    }

    async fn blob_exists_async(&self, key: &str) -> Result<bool, StorageError> {
        let sql = format!("SELECT 1 FROM {} WHERE key = ? LIMIT 1", self.table_name());
        let row = self.query_first_async(&sql, &[DataValue::Text(key.to_string())]).await?;
        Ok(row.is_some())
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
