//! Cloudflare D1 key-value storage backend.
//!
//! WHY: D1 is Cloudflare's edge `SQLite` - useful for KV storage at the edge.
//!
//! WHAT: `D1KeyValueStore` implements `KeyValueStore`, `QueryStore`, and
//! `RateLimiterStore` traits using D1's SQL API over HTTP.
//!
//! HOW: Uses `SimpleHttpClient` for synchronous HTTP calls to the D1 API.
//! Values are serialized as JSON and stored in a key-value table.

use base64::{engine::general_purpose::STANDARD, Engine};
use foundation_core::valtron::Stream;
use foundation_core::wire::simple_http::client::SimpleHttpClient;
use foundation_core::wire::simple_http::{SendSafeBody, SimpleHeader, Status};
use serde::{de::DeserializeOwned, Serialize};

use crate::errors::{StorageError, StorageResult};
use crate::storage_provider::{
    BlobStore, DataValue, KeyValueStore, QueryStore, RateLimiterStore, SqlRow, StorageItemStream,
};

/// Default Cloudflare API base. Tests override this via [`D1KeyValueStore::with_base_url`].
pub const CF_API_BASE: &str = "https://api.cloudflare.com/client/v4";

/// Cloudflare D1 key-value storage backend.
///
/// Stores key-value pairs in a D1 database table.
/// Uses the Cloudflare D1 API for SQL operations over HTTP.
///
/// Table name is `{prefix}_kv` for namespacing.
pub struct D1KeyValueStore {
    api_token: String,
    account_id: String,
    database_id: String,
    table_name: String,
    base_url: String,
    client: SimpleHttpClient,
}

impl D1KeyValueStore {
    /// Create a new D1 key-value store pointed at the production Cloudflare API.
    ///
    /// Table name will be `{table_prefix}_kv` for namespacing.
    #[must_use]
    pub fn new(
        api_token: &str,
        account_id: &str,
        database_id: &str,
        table_prefix: &str,
    ) -> Self {
        Self::with_base_url(api_token, account_id, database_id, table_prefix, CF_API_BASE)
    }

    /// Create a new D1 key-value store with a custom API base URL.
    ///
    /// Used by integration tests to point the client at a local wrangler
    /// worker that emulates the Cloudflare D1 REST API.
    #[must_use]
    pub fn with_base_url(
        api_token: &str,
        account_id: &str,
        database_id: &str,
        table_prefix: &str,
        base_url: &str,
    ) -> Self {
        let table_name = format!("{table_prefix}_kv");
        Self {
            api_token: api_token.to_string(),
            account_id: account_id.to_string(),
            database_id: database_id.to_string(),
            table_name,
            base_url: base_url.trim_end_matches('/').to_string(),
            client: SimpleHttpClient::from_system(),
        }
    }

    /// Create from environment variables.
    ///
    /// - `DEPLOYMENT_D1_DATABASE_ID` (required)
    /// - `CLOUDFLARE_API_TOKEN` (required)
    /// - `CLOUDFLARE_ACCOUNT_ID` (required)
    /// - `D1_KV_TABLE_PREFIX` (optional, defaults to "app")
    ///
    /// # Errors
    ///
    /// Returns an error if required env vars are missing.
    pub fn from_env() -> Result<Self, StorageError> {
        let db_id = std::env::var("DEPLOYMENT_D1_DATABASE_ID").map_err(|_| {
            StorageError::Connection("DEPLOYMENT_D1_DATABASE_ID must be set".to_string())
        })?;
        let token = std::env::var("CLOUDFLARE_API_TOKEN").map_err(|_| {
            StorageError::Connection("CLOUDFLARE_API_TOKEN must be set".to_string())
        })?;
        let account = std::env::var("CLOUDFLARE_ACCOUNT_ID").map_err(|_| {
            StorageError::Connection("CLOUDFLARE_ACCOUNT_ID must be set".to_string())
        })?;
        let prefix = std::env::var("D1_KV_TABLE_PREFIX").unwrap_or_else(|_| "app".to_string());
        Ok(Self::new(&token, &account, &db_id, &prefix))
    }

    fn query_url(&self) -> String {
        format!(
            "{}/accounts/{}/d1/database/{}/query",
            self.base_url, self.account_id, self.database_id
        )
    }

    fn auth_header(&self) -> String {
        format!("Bearer {}", self.api_token)
    }

    /// Initialize the key-value table schema.
    ///
    /// # Errors
    ///
    /// Returns a `StorageError` if the table creation fails.
    pub fn init(&self) -> StorageResult<()> {
        let sql = format!(
            "CREATE TABLE IF NOT EXISTS {} (key TEXT PRIMARY KEY, value TEXT NOT NULL, created_at INTEGER DEFAULT (strftime('%s', 'now') * 1000), updated_at INTEGER DEFAULT (strftime('%s', 'now') * 1000))",
            self.table_name
        );
        self.execute_sql(&sql, &[])?;
        Ok(())
    }

    /// Execute a SQL query via the D1 HTTP API.
    fn execute_sql(
        &self,
        sql: &str,
        params: &[serde_json::Value],
    ) -> Result<serde_json::Value, StorageError> {
        let body = serde_json::json!({ "sql": sql, "params": params });
        let response = self
            .client
            .post(&self.query_url())
            .map_err(|e| StorageError::Backend(format!("D1 request build failed: {e}")))?
            .header(SimpleHeader::AUTHORIZATION, self.auth_header())
            .header(SimpleHeader::CONTENT_TYPE, "application/json")
            .body_text(body.to_string())
            .build_client()
            .map_err(|e| StorageError::Backend(format!("D1 request build failed: {e}")))?
            .send()
            .map_err(|e| StorageError::Backend(format!("D1 request failed: {e}")))?;

        if response.get_status() != Status::OK {
            return Err(StorageError::Backend(format!(
                "D1 query failed with status {}",
                response.get_status()
            )));
        }

        let text = match response.get_body_ref() {
            SendSafeBody::Text(s) => s.clone(),
            SendSafeBody::Bytes(b) => String::from_utf8(b.clone())
                .map_err(|e| StorageError::Backend(format!("D1 response not UTF-8: {e}")))?,
            _ => return Err(StorageError::Backend("D1: empty response body".to_string())),
        };

        serde_json::from_str(&text)
            .map_err(|e| StorageError::Serialization(format!("D1 response parse failed: {e}")))
    }

    /// Extract rows from a D1 API response.
    fn extract_rows(response: &serde_json::Value) -> Vec<serde_json::Value> {
        response
            .pointer("/result/0/results")
            .and_then(serde_json::Value::as_array)
            .cloned()
            .unwrap_or_default()
    }

    /// Wrap a single value into a `StorageItemStream`.
    fn wrap_value<T: Send + 'static>(val: T) -> StorageItemStream<'static, T> {
        Box::new(std::iter::once(Stream::Next(Ok(val))))
    }

    /// Wrap a `Vec` into a `StorageItemStream`.
    fn wrap_vec<T: Send + 'static>(vals: Vec<T>) -> StorageItemStream<'static, T> {
        Box::new(vals.into_iter().map(|v| Stream::Next(Ok(v))))
    }
}

impl KeyValueStore for D1KeyValueStore {
    fn get<'a, V: DeserializeOwned + Send + 'static>(
        &'a self,
        key: &str,
    ) -> StorageResult<StorageItemStream<'a, Option<V>>> {
        let sql = format!("SELECT value FROM {} WHERE key = ?", self.table_name);
        let response = self.execute_sql(
            &sql,
            &[serde_json::Value::String(key.to_string())],
        )?;
        let rows = Self::extract_rows(&response);

        match rows.first() {
            Some(row) => {
                let value: String = row
                    .get("value")
                    .and_then(serde_json::Value::as_str)
                    .map(String::from)
                    .ok_or_else(|| {
                        StorageError::SqlConversion("missing or invalid value field".to_string())
                    })?;

                // Deserialize from JSON
                let deserialized: V = serde_json::from_str(&value)
                    .map_err(|e| StorageError::Serialization(e.to_string()))?;

                Ok(Self::wrap_value(Some(deserialized)))
            }
            None => Ok(Self::wrap_value(None)),
        }
    }

    fn set<V: Serialize>(&self, key: &str, value: V) -> StorageResult<StorageItemStream<'_, ()>> {
        // Serialize to JSON string
        let json_value = serde_json::to_string(&value)
            .map_err(|e| StorageError::Serialization(e.to_string()))?;

        let sql = format!(
            "INSERT INTO {} (key, value, updated_at) VALUES (?, ?, strftime('%s', 'now') * 1000) ON CONFLICT(key) DO UPDATE SET value = ?, updated_at = strftime('%s', 'now') * 1000",
            self.table_name
        );

        let key_value = serde_json::Value::String(key.to_string());
        let json_value_param = serde_json::Value::String(json_value);

        self.execute_sql(
            &sql,
            &[
                key_value.clone(),
                json_value_param.clone(),
                json_value_param,
            ],
        )?;

        Ok(Self::wrap_value(()))
    }

    fn delete(&self, key: &str) -> StorageResult<StorageItemStream<'_, ()>> {
        let sql = format!("DELETE FROM {} WHERE key = ?", self.table_name);
        self.execute_sql(&sql, &[serde_json::Value::String(key.to_string())])?;
        Ok(Self::wrap_value(()))
    }

    fn exists(&self, key: &str) -> StorageResult<StorageItemStream<'_, bool>> {
        let sql = format!("SELECT 1 FROM {} WHERE key = ? LIMIT 1", self.table_name);
        let response = self.execute_sql(
            &sql,
            &[serde_json::Value::String(key.to_string())],
        )?;
        let rows = Self::extract_rows(&response);
        Ok(Self::wrap_value(!rows.is_empty()))
    }

    fn list_keys(&self, prefix: Option<&str>) -> StorageResult<StorageItemStream<'_, String>> {
        let (sql, param) = match prefix {
            Some(p) => (
                format!("SELECT key FROM {} WHERE key LIKE ? ORDER BY key", self.table_name),
                format!("{p}%"),
            ),
            None => (
                format!("SELECT key FROM {} ORDER BY key", self.table_name),
                String::new(),
            ),
        };

        let params = if param.is_empty() {
            vec![]
        } else {
            vec![serde_json::Value::String(param)]
        };

        let response = self.execute_sql(&sql, &params)?;
        let rows = Self::extract_rows(&response);

        let keys: Result<Vec<String>, StorageError> = rows
            .iter()
            .map(|row| {
                row.get("key")
                    .and_then(serde_json::Value::as_str)
                    .map(String::from)
                    .ok_or_else(|| {
                        StorageError::SqlConversion("missing or invalid key field".to_string())
                    })
            })
            .collect();

        Ok(Self::wrap_vec(keys?))
    }
}

impl QueryStore for D1KeyValueStore {
    fn query(
        &self,
        sql: &str,
        params: &[DataValue],
    ) -> StorageResult<StorageItemStream<'_, SqlRow>> {
        // Convert DataValue to serde_json::Value
        let json_params: Vec<serde_json::Value> = params
            .iter()
            .map(|v| match v {
                DataValue::Null => serde_json::Value::Null,
                DataValue::Integer(i) => serde_json::Value::Number(serde_json::Number::from(*i)),
                DataValue::Real(f) => serde_json::Number::from_f64(*f)
                    .map_or(serde_json::Value::Null, serde_json::Value::Number),
                DataValue::Text(s) => serde_json::Value::String(s.clone()),
                DataValue::Blob(b) => {
                    serde_json::Value::String(STANDARD.encode(b))
                }
            })
            .collect();

        let response = self.execute_sql(sql, &json_params)?;
        let rows = Self::extract_rows(&response);

        let results: Result<Vec<SqlRow>, StorageError> = rows
            .iter()
            .map(|row| {
                // Convert D1 row to SqlRow
                let obj = row
                    .as_object()
                    .ok_or_else(|| StorageError::SqlConversion("row is not an object".to_string()))?;

                let columns: Vec<(String, DataValue)> = obj
                    .iter()
                    .map(|(k, v)| {
                        let dv = match v {
                            serde_json::Value::Number(n) => {
                                if let Some(i) = n.as_i64() {
                                    DataValue::Integer(i)
                                } else if let Some(f) = n.as_f64() {
                                    DataValue::Real(f)
                                } else {
                                    DataValue::Null
                                }
                            }
                            serde_json::Value::String(s) => DataValue::Text(s.clone()),
                            _ => DataValue::Null,
                        };
                        (k.clone(), dv)
                    })
                    .collect();

                Ok(SqlRow::new(columns))
            })
            .collect();

        Ok(Self::wrap_vec(results?))
    }

    fn execute(
        &self,
        sql: &str,
        params: &[DataValue],
    ) -> StorageResult<StorageItemStream<'_, u64>> {
        let json_params: Vec<serde_json::Value> = params
            .iter()
            .map(|v| match v {
                DataValue::Null => serde_json::Value::Null,
                DataValue::Integer(i) => serde_json::Value::Number(serde_json::Number::from(*i)),
                DataValue::Real(f) => serde_json::Number::from_f64(*f)
                    .map_or(serde_json::Value::Null, serde_json::Value::Number),
                DataValue::Text(s) => serde_json::Value::String(s.clone()),
                DataValue::Blob(b) => {
                    serde_json::Value::String(STANDARD.encode(b))
                }
            })
            .collect();

        let response = self.execute_sql(sql, &json_params)?;

        // D1 returns rows affected in a different format
        let affected = response
            .pointer("/result/0/meta/changes")
            .and_then(serde_json::Value::as_i64)
            .map_or(0, i64::unsigned_abs);

        Ok(Self::wrap_value(affected))
    }

    fn execute_batch(&self, sql: &str) -> StorageResult<StorageItemStream<'_, ()>> {
        // Execute batch as single SQL (D1 doesn't have explicit batch API)
        self.execute_sql(sql, &[])?;
        Ok(Self::wrap_value(()))
    }
}

impl RateLimiterStore for D1KeyValueStore {
    fn check_rate_limit(
        &self,
        key: &str,
        max_count: u32,
        window_seconds: u64,
    ) -> StorageResult<StorageItemStream<'_, bool>> {
        // Ensure rate_limits table exists
        let create_table = r"
            CREATE TABLE IF NOT EXISTS rate_limits (
                key TEXT PRIMARY KEY,
                count INTEGER NOT NULL,
                window_start INTEGER NOT NULL
            )
        ";
        self.execute_sql(create_table, &[])?;

        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();
        let window_start = now - window_seconds;

        let sql = "SELECT count, window_start FROM rate_limits WHERE key = ?";
        let response = self.execute_sql(
            sql,
            &[serde_json::Value::String(key.to_string())],
        )?;
        let rows = Self::extract_rows(&response);

        let allowed = match rows.first() {
            Some(row) => {
                #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
                let count = row
                    .get("count")
                    .and_then(serde_json::Value::as_i64)
                    .unwrap_or(0) as u32;
                #[allow(clippy::cast_sign_loss)]
                let stored_window = row
                    .get("window_start")
                    .and_then(serde_json::Value::as_i64)
                    .unwrap_or(0)
                    .cast_unsigned();

                if stored_window < window_start {
                    true
                } else {
                    count < max_count
                }
            }
            None => true,
        };

        Ok(Self::wrap_value(allowed))
    }

    fn record_rate_limit(&self, key: &str) -> StorageResult<StorageItemStream<'_, u32>> {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();

        let sql =
            "INSERT INTO rate_limits (key, count, window_start) VALUES (?, 1, ?) ON CONFLICT(key) DO UPDATE SET count = count + 1, window_start = excluded.window_start";

        self.execute_sql(
            sql,
            &[
                serde_json::Value::String(key.to_string()),
                serde_json::Value::Number(serde_json::Number::from(now)),
            ],
        )?;

        // Read back the count
        let response = self.execute_sql(
            "SELECT count FROM rate_limits WHERE key = ?",
            &[serde_json::Value::String(key.to_string())],
        )?;
        let rows = Self::extract_rows(&response);

        #[allow(clippy::cast_sign_loss, clippy::cast_possible_truncation)]
        let count = rows
            .first()
            .and_then(|r| r.get("count"))
            .and_then(serde_json::Value::as_i64)
            .map_or(1, |c| c as u32);

        Ok(Self::wrap_value(count))
    }

    fn reset_rate_limit(&self, key: &str) -> StorageResult<StorageItemStream<'_, ()>> {
        let sql = "DELETE FROM rate_limits WHERE key = ?";
        self.execute_sql(sql, &[serde_json::Value::String(key.to_string())])?;
        Ok(Self::wrap_value(()))
    }
}

impl BlobStore for D1KeyValueStore {
    fn put_blob(&self, key: &str, data: &[u8]) -> StorageResult<StorageItemStream<'_, ()>> {
        // Store blobs as base64-encoded text in D1
        let encoded = STANDARD.encode(data);
        let json_value = serde_json::json!({
            "type": "blob",
            "encoding": "base64",
            "data": encoded
        }).to_string();

        let sql = format!(
            "INSERT INTO {} (key, value, updated_at) VALUES (?, ?, strftime('%s', 'now') * 1000) ON CONFLICT(key) DO UPDATE SET value = ?, updated_at = strftime('%s', 'now') * 1000",
            self.table_name
        );

        let key_value = serde_json::Value::String(key.to_string());
        let json_value_param = serde_json::Value::String(json_value);

        self.execute_sql(
            &sql,
            &[
                key_value.clone(),
                json_value_param.clone(),
                json_value_param,
            ],
        )?;

        Ok(Self::wrap_value(()))
    }

    fn get_blob(&self, key: &str) -> StorageResult<StorageItemStream<'_, Option<Vec<u8>>>> {
        let sql = "SELECT value FROM {} WHERE key = ?".to_string();
        let response = self.execute_sql(
            &sql,
            &[serde_json::Value::String(key.to_string())],
        )?;
        let rows = Self::extract_rows(&response);

        match rows.first() {
            Some(row) => {
                let value: String = row
                    .get("value")
                    .and_then(serde_json::Value::as_str)
                    .map(String::from)
                    .ok_or_else(|| {
                        StorageError::SqlConversion("missing or invalid value field".to_string())
                    })?;

                // Parse JSON wrapper
                let wrapper: serde_json::Value = serde_json::from_str(&value)
                    .map_err(|e| StorageError::Serialization(e.to_string()))?;

                // Check if it's a blob wrapper
                let is_blob = wrapper
                    .get("type")
                    .and_then(serde_json::Value::as_str)
                    == Some("blob");

                if !is_blob {
                    return Ok(Self::wrap_value(None));
                }

                let encoded = wrapper
                    .get("data")
                    .and_then(serde_json::Value::as_str)
                    .ok_or_else(|| {
                        StorageError::Serialization("missing data field in blob wrapper".to_string())
                    })?;

                let decoded = STANDARD.decode(encoded)
                    .map_err(|e| StorageError::Backend(format!("Base64 decode failed: {e}")))?;

                Ok(Self::wrap_value(Some(decoded)))
            }
            None => Ok(Self::wrap_value(None)),
        }
    }

    fn delete_blob(&self, key: &str) -> StorageResult<StorageItemStream<'_, ()>> {
        let sql = "DELETE FROM {} WHERE key = ?".to_string();
        self.execute_sql(&sql, &[serde_json::Value::String(key.to_string())])?;
        Ok(Self::wrap_value(()))
    }

    fn blob_exists(&self, key: &str) -> StorageResult<StorageItemStream<'_, bool>> {
        let sql = "SELECT 1 FROM {} WHERE key = ? LIMIT 1".to_string();
        let response = self.execute_sql(
            &sql,
            &[serde_json::Value::String(key.to_string())],
        )?;
        let rows = Self::extract_rows(&response);
        Ok(Self::wrap_value(!rows.is_empty()))
    }
}
