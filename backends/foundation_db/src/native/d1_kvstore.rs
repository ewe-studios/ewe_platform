//! Cloudflare D1 storage backend (unified).
//!
//! WHY: D1 is Cloudflare's edge SQLite - useful for KV, query execution,
//! blob storage, rate limiting, and deployment state.
//!
//! WHAT: `D1Store` implements multiple storage traits via `SimpleHttpClient`
//! for HTTP calls to the D1 API over HTTP.
//!
//! HOW: Different constructors for different usage modes:
//!   - `D1Store::new()` / `D1Store::new_kv()` — KV/query/rate-limit/blob store (table: `{prefix}_kv`)
//!   - `D1Store::new_state()` — StateStore (table: `{project}_{stage}_resources`)

use base64::{engine::general_purpose::STANDARD, Engine};
use foundation_core::valtron::{Stream, ThreadedValue};
use foundation_netio::simple_http::client::shared::body_reader::{AsyncSendSafeBody, collect_string_async};
use foundation_netio::simple_http::client::SimpleHttpClient;
use foundation_netio::simple_http::shared::{SendSafeBody, SimpleHeader, Status};
use serde::{de::DeserializeOwned, Serialize};

use crate::core::errors::{StorageError, StorageResult};
use crate::core::state::traits::{StateStore, StateStoreStream};
use crate::core::state::types::{ResourceState, StateStatus};
use crate::core::crypto::ZeroizingString;
use crate::core::storage_provider::{
    AsyncBlobStore, AsyncKeyValueStore, AsyncQueryStore, AsyncRateLimiterStore,
    BlobStore, DataValue, KeyValueStore, QueryStore, RateLimiterStore, SqlRow, StorageItemStream,
};

/// Default Cloudflare API base. Tests override via `D1Store::with_base_url`.
pub const CF_API_BASE: &str = "https://api.cloudflare.com/client/v4";

// ---- Schema helpers for State mode ----

fn state_create_table_sql(table_name: &str) -> String {
    format!(
        "CREATE TABLE IF NOT EXISTS {table_name} (id TEXT PRIMARY KEY, kind TEXT NOT NULL, provider TEXT NOT NULL, status TEXT NOT NULL, environment TEXT, config_hash TEXT NOT NULL, output TEXT NOT NULL, config_snapshot TEXT NOT NULL, created_at TEXT NOT NULL, updated_at TEXT NOT NULL)",
    )
}

fn state_upsert_sql(table_name: &str) -> String {
    format!(
        "INSERT INTO {table_name} (id, kind, provider, status, environment, config_hash, output, config_snapshot, created_at, updated_at) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?) ON CONFLICT(id) DO UPDATE SET kind = excluded.kind, provider = excluded.provider, status = excluded.status, environment = excluded.environment, config_hash = excluded.config_hash, output = excluded.output, config_snapshot = excluded.config_snapshot, updated_at = excluded.updated_at",
    )
}

fn state_create_indexes_sql(table_name: &str) -> String {
    format!(
        "CREATE INDEX IF NOT EXISTS idx_{table_name}_kind ON {table_name}(kind);\
         CREATE INDEX IF NOT EXISTS idx_{table_name}_provider ON {table_name}(provider);\
         CREATE INDEX IF NOT EXISTS idx_{table_name}_status ON {table_name}(status);\
         CREATE INDEX IF NOT EXISTS idx_{table_name}_environment ON {table_name}(environment);"
    )
}

fn escape_like(s: &str) -> String {
    s.replace('%', "\\%").replace('_', "\\_")
}

// ---- State mode row parsing ----

fn parse_d1_row(row: &serde_json::Value) -> Result<ResourceState, StorageError> {
    let get_str = |key: &str| -> Result<String, StorageError> {
        row.get(key)
            .and_then(serde_json::Value::as_str)
            .map(String::from)
            .ok_or_else(|| StorageError::SqlConversion(format!("missing or invalid field: {key}")))
    };
    let id = get_str("id")?;
    let kind = get_str("kind")?;
    let provider = get_str("provider")?;
    let status_str = get_str("status")?;
    let environment = row.get("environment").and_then(serde_json::Value::as_str).map(String::from);
    let config_hash = get_str("config_hash")?;
    let output_str = get_str("output")?;
    let snapshot_str = get_str("config_snapshot")?;
    let created_str = get_str("created_at")?;
    let updated_str = get_str("updated_at")?;
    let status: StateStatus = serde_json::from_str(&status_str)
        .map_err(|e| StorageError::Serialization(format!("status parse: {e}")))?;
    let output: serde_json::Value = serde_json::from_str(&output_str)
        .map_err(|e| StorageError::Serialization(format!("output parse: {e}")))?;
    let config_snapshot: serde_json::Value = serde_json::from_str(&snapshot_str)
        .map_err(|e| StorageError::Serialization(format!("config_snapshot parse: {e}")))?;
    let created_at = chrono::DateTime::parse_from_rfc3339(&created_str)
        .map(|dt| dt.with_timezone(&chrono::Utc))
        .map_err(|e| StorageError::Serialization(format!("created_at parse: {e}")))?;
    let updated_at = chrono::DateTime::parse_from_rfc3339(&updated_str)
        .map(|dt| dt.with_timezone(&chrono::Utc))
        .map_err(|e| StorageError::Serialization(format!("updated_at parse: {e}")))?;
    Ok(ResourceState { id, kind, provider, status, environment, config_hash, output, config_snapshot, created_at, updated_at })
}

fn state_to_params(state: &ResourceState) -> Result<Vec<serde_json::Value>, StorageError> {
    let status_json = serde_json::to_string(&state.status).map_err(|e| StorageError::Serialization(e.to_string()))?;
    let output_json = serde_json::to_string(&state.output).map_err(|e| StorageError::Serialization(e.to_string()))?;
    let snapshot_json = serde_json::to_string(&state.config_snapshot).map_err(|e| StorageError::Serialization(e.to_string()))?;
    Ok(vec![
        serde_json::Value::String(state.id.clone()),
        serde_json::Value::String(state.kind.clone()),
        serde_json::Value::String(state.provider.clone()),
        serde_json::Value::String(status_json),
        state.environment.as_ref().map_or(serde_json::Value::Null, |e| serde_json::Value::String(e.clone())),
        serde_json::Value::String(state.config_hash.clone()),
        serde_json::Value::String(output_json),
        serde_json::Value::String(snapshot_json),
        serde_json::Value::String(state.created_at.to_rfc3339()),
        serde_json::Value::String(state.updated_at.to_rfc3339()),
    ])
}

// ---- Mode enum ----

#[derive(Clone)]
enum D1Mode {
    /// KV/query/rate-limit/blob store. Table name = `{prefix}_kv`.
    KeyValue { kv_table: String },
    /// StateStore. Table name = `{project}_{stage}_resources`.
    State { state_table: String },
}

// ---- D1Store ----

#[derive(Clone)]
pub struct D1Store {
    api_token: ZeroizingString,
    account_id: String,
    database_id: String,
    base_url: String,
    client: SimpleHttpClient,
    mode: D1Mode,
}

impl D1Store {
    // ========== KV-mode constructors ==========

    /// KV/query/rate-limit/blob store at production Cloudflare API.
    #[must_use]
    pub fn new_kv(api_token: &str, account_id: &str, database_id: &str, table_prefix: &str) -> Self {
        Self::new_kv_with_base_url(api_token, account_id, database_id, table_prefix, CF_API_BASE)
    }

    /// KV store with custom base URL (for tests).
    #[must_use]
    pub fn new_kv_with_base_url(
        api_token: &str, account_id: &str, database_id: &str, table_prefix: &str, base_url: &str,
    ) -> Self {
        Self {
            api_token: ZeroizingString::from_string(api_token.to_string()),
            account_id: account_id.to_string(),
            database_id: database_id.to_string(),
            base_url: base_url.trim_end_matches('/').to_string(),
            client: SimpleHttpClient::from_system(),
            mode: D1Mode::KeyValue { kv_table: format!("{table_prefix}_kv") },
        }
    }

    /// KV store from environment.
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
        Ok(Self::new_kv(&token, &account, &db_id, &prefix))
    }

    // ========== State-mode constructors ==========

    /// StateStore at production Cloudflare API.
    #[must_use]
    pub fn new_state(api_token: &str, account_id: &str, database_id: &str, project: &str, stage: &str) -> Self {
        Self::new_state_with_base_url(api_token, account_id, database_id, project, stage, CF_API_BASE)
    }

    /// StateStore with custom base URL (for tests).
    #[must_use]
    pub fn new_state_with_base_url(
        api_token: &str, account_id: &str, database_id: &str, project: &str, stage: &str, base_url: &str,
    ) -> Self {
        let state_table = format!(
            "{}_{}_resources",
            project.replace(['-', ' '], "_"),
            stage.replace(['-', ' '], "_"),
        );
        Self {
            api_token: ZeroizingString::from_string(api_token.to_string()),
            account_id: account_id.to_string(),
            database_id: database_id.to_string(),
            base_url: base_url.trim_end_matches('/').to_string(),
            client: SimpleHttpClient::from_system(),
            mode: D1Mode::State { state_table },
        }
    }

    /// StateStore from environment.
    pub fn from_env_state(project: &str, stage: &str) -> Result<Self, StorageError> {
        let db_id = std::env::var("DEPLOYMENT_D1_DATABASE_ID").map_err(|_| {
            StorageError::Connection("DEPLOYMENT_D1_DATABASE_ID must be set".to_string())
        })?;
        let token = std::env::var("CLOUDFLARE_API_TOKEN").map_err(|_| {
            StorageError::Connection("CLOUDFLARE_API_TOKEN must be set".to_string())
        })?;
        let account = std::env::var("CLOUDFLARE_ACCOUNT_ID").map_err(|_| {
            StorageError::Connection("CLOUDFLARE_ACCOUNT_ID must be set".to_string())
        })?;
        Ok(Self::new_state(&token, &account, &db_id, project, stage))
    }

    // ========== Shared helpers ==========

    fn query_url(&self) -> String {
        format!("{}/accounts/{}/d1/database/{}/query", self.base_url, self.account_id, self.database_id)
    }

    fn auth_header(&self) -> String {
        format!("Bearer {}", self.api_token.as_str())
    }

    fn execute_sql(&self, sql: &str, params: &[serde_json::Value]) -> Result<serde_json::Value, StorageError> {
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
            return Err(StorageError::Backend(format!("D1 query failed with status {}", response.get_status())));
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

    /// Async version of `execute_sql`. Uses `SimpleHttpClient::send_async()`
    /// to perform the HTTP request without blocking.
    async fn execute_sql_async(&self, sql: &str, params: &[serde_json::Value]) -> Result<serde_json::Value, StorageError> {
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
            .send_async()
            .await
            .map_err(|e| StorageError::Backend(format!("D1 request failed: {e}")))?;

        if response.get_status() != Status::OK {
            return Err(StorageError::Backend(format!("D1 query failed with status {}", response.get_status())));
        }

        let (_, _, send_safe_body, ..) = response.into_parts();
        let text = collect_string_async(AsyncSendSafeBody::from(send_safe_body))
            .await
            .map_err(|e| StorageError::Backend(format!("D1 body read failed: {e}")))?;

        serde_json::from_str(&text)
            .map_err(|e| StorageError::Serialization(format!("D1 response parse failed: {e}")))
    }

    fn extract_rows(response: &serde_json::Value) -> Vec<serde_json::Value> {
        response
            .pointer("/result/0/results")
            .and_then(serde_json::Value::as_array)
            .cloned()
            .unwrap_or_default()
    }

    fn kv_table(&self) -> &str {
        match &self.mode {
            D1Mode::KeyValue { kv_table } => kv_table,
            D1Mode::State { .. } => unreachable!("kv_table called on State mode"),
        }
    }

    fn state_table(&self) -> &str {
        match &self.mode {
            D1Mode::State { state_table } => state_table,
            D1Mode::KeyValue { .. } => unreachable!("state_table called on KV mode"),
        }
    }

    fn wrap_value<T: Send + 'static>(val: T) -> StorageItemStream<'static, T> {
        Box::new(std::iter::once(Stream::Next(Ok(val))))
    }

    fn wrap_vec<T: Send + 'static>(vals: Vec<T>) -> StorageItemStream<'static, T> {
        Box::new(vals.into_iter().map(|v| Stream::Next(Ok(v))))
    }

    fn wrap_value_state<T: Send + 'static>(val: T) -> StateStoreStream<T> {
        Box::new(std::iter::once(ThreadedValue::Value(Ok(val))))
    }

    fn wrap_vec_state<T: Send + 'static>(vals: Vec<T>) -> StateStoreStream<T> {
        Box::new(vals.into_iter().map(|v| ThreadedValue::Value(Ok(v))))
    }

    // ========== KV init ==========

    pub fn init_kv(&self) -> StorageResult<()> {
        let sql = format!(
            "CREATE TABLE IF NOT EXISTS {} (key TEXT PRIMARY KEY, value TEXT NOT NULL, created_at INTEGER DEFAULT (strftime('%s', 'now') * 1000), updated_at INTEGER DEFAULT (strftime('%s', 'now') * 1000))",
            self.kv_table()
        );
        self.execute_sql(&sql, &[])?;
        Ok(())
    }
}

// ===========================================================================
// KeyValueStore
// ===========================================================================

impl KeyValueStore for D1Store {
    fn get<'a, V: DeserializeOwned + Send + 'static>(&'a self, key: &str) -> StorageResult<StorageItemStream<'a, Option<V>>> {
        let sql = format!("SELECT value FROM {} WHERE key = ?", self.kv_table());
        let response = self.execute_sql(&sql, &[serde_json::Value::String(key.to_string())])?;
        let rows = Self::extract_rows(&response);
        match rows.first() {
            Some(row) => {
                let value: String = row.get("value").and_then(serde_json::Value::as_str).map(String::from)
                    .ok_or_else(|| StorageError::SqlConversion("missing or invalid value field".to_string()))?;
                let deserialized: V = serde_json::from_str(&value)
                    .map_err(|e| StorageError::Serialization(e.to_string()))?;
                Ok(Self::wrap_value(Some(deserialized)))
            }
            None => Ok(Self::wrap_value(None)),
        }
    }

    fn set<V: Serialize>(&self, key: &str, value: V) -> StorageResult<StorageItemStream<'_, ()>> {
        let json_value = serde_json::to_string(&value).map_err(|e| StorageError::Serialization(e.to_string()))?;
        let sql = format!(
            "INSERT INTO {} (key, value, updated_at) VALUES (?, ?, strftime('%s', 'now') * 1000) ON CONFLICT(key) DO UPDATE SET value = ?, updated_at = strftime('%s', 'now') * 1000",
            self.kv_table()
        );
        let kv = serde_json::Value::String(key.to_string());
        let jv = serde_json::Value::String(json_value);
        self.execute_sql(&sql, &[kv.clone(), jv.clone(), jv])?;
        Ok(Self::wrap_value(()))
    }

    fn delete(&self, key: &str) -> StorageResult<StorageItemStream<'_, ()>> {
        let sql = format!("DELETE FROM {} WHERE key = ?", self.kv_table());
        self.execute_sql(&sql, &[serde_json::Value::String(key.to_string())])?;
        Ok(Self::wrap_value(()))
    }

    fn exists(&self, key: &str) -> StorageResult<StorageItemStream<'_, bool>> {
        let sql = format!("SELECT 1 FROM {} WHERE key = ? LIMIT 1", self.kv_table());
        let response = self.execute_sql(&sql, &[serde_json::Value::String(key.to_string())])?;
        Ok(Self::wrap_value(!Self::extract_rows(&response).is_empty()))
    }

    fn list_keys(&self, prefix: Option<&str>) -> StorageResult<StorageItemStream<'_, String>> {
        let (sql, params) = match prefix {
            Some(p) => (
                format!("SELECT key FROM {} WHERE key LIKE ? ORDER BY key", self.kv_table()),
                vec![serde_json::Value::String(format!("{p}%"))],
            ),
            None => (format!("SELECT key FROM {} ORDER BY key", self.kv_table()), vec![]),
        };
        let response = self.execute_sql(&sql, &params)?;
        let keys: Result<Vec<String>, StorageError> = Self::extract_rows(&response)
            .iter()
            .map(|row| row.get("key").and_then(serde_json::Value::as_str).map(String::from)
                .ok_or_else(|| StorageError::SqlConversion("missing or invalid key field".to_string())))
            .collect();
        Ok(Self::wrap_vec(keys?))
    }
}

// ===========================================================================
// QueryStore
// ===========================================================================

impl QueryStore for D1Store {
    fn query(&self, sql: &str, params: &[DataValue]) -> StorageResult<StorageItemStream<'_, SqlRow>> {
        let json_params: Vec<serde_json::Value> = params.iter().map(|v| match v {
            DataValue::Null => serde_json::Value::Null,
            DataValue::Integer(i) => serde_json::Value::Number(serde_json::Number::from(*i)),
            DataValue::Real(f) => serde_json::Number::from_f64(*f).map_or(serde_json::Value::Null, serde_json::Value::Number),
            DataValue::Text(s) => serde_json::Value::String(s.clone()),
            DataValue::Blob(b) => serde_json::Value::String(STANDARD.encode(b)),
        }).collect();
        let response = self.execute_sql(sql, &json_params)?;
        let results: Result<Vec<SqlRow>, StorageError> = Self::extract_rows(&response)
            .iter()
            .map(|row| {
                let obj = row.as_object().ok_or_else(|| StorageError::SqlConversion("row is not an object".to_string()))?;
                let columns: Vec<(String, DataValue)> = obj.iter().map(|(k, v)| {
                    let dv = match v {
                        serde_json::Value::Number(n) => {
                            if let Some(i) = n.as_i64() { DataValue::Integer(i) }
                            else if let Some(f) = n.as_f64() { DataValue::Real(f) }
                            else { DataValue::Null }
                        }
                        serde_json::Value::String(s) => DataValue::Text(s.clone()),
                        _ => DataValue::Null,
                    };
                    (k.clone(), dv)
                }).collect();
                Ok(SqlRow::new(columns))
            }).collect();
        Ok(Self::wrap_vec(results?))
    }

    fn execute(&self, sql: &str, params: &[DataValue]) -> StorageResult<StorageItemStream<'_, u64>> {
        let json_params: Vec<serde_json::Value> = params.iter().map(|v| match v {
            DataValue::Null => serde_json::Value::Null,
            DataValue::Integer(i) => serde_json::Value::Number(serde_json::Number::from(*i)),
            DataValue::Real(f) => serde_json::Number::from_f64(*f).map_or(serde_json::Value::Null, serde_json::Value::Number),
            DataValue::Text(s) => serde_json::Value::String(s.clone()),
            DataValue::Blob(b) => serde_json::Value::String(STANDARD.encode(b)),
        }).collect();
        let response = self.execute_sql(sql, &json_params)?;
        let affected = response.pointer("/result/0/meta/changes")
            .and_then(serde_json::Value::as_i64)
            .map_or(0, i64::unsigned_abs);
        Ok(Self::wrap_value(affected))
    }

    fn execute_batch(&self, sql: &str) -> StorageResult<StorageItemStream<'_, ()>> {
        self.execute_sql(sql, &[])?;
        Ok(Self::wrap_value(()))
    }
}

// ===========================================================================
// RateLimiterStore
// ===========================================================================

impl RateLimiterStore for D1Store {
    fn check_rate_limit(&self, key: &str, max_count: u32, window_seconds: u64) -> StorageResult<StorageItemStream<'_, bool>> {
        let create_table = r"CREATE TABLE IF NOT EXISTS rate_limits (key TEXT PRIMARY KEY, count INTEGER NOT NULL, window_start INTEGER NOT NULL)";
        self.execute_sql(create_table, &[])?;
        let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_secs();
        let window_start = now - window_seconds;
        let sql = "SELECT count, window_start FROM rate_limits WHERE key = ?";
        let response = self.execute_sql(sql, &[serde_json::Value::String(key.to_string())])?;
        let allowed = match Self::extract_rows(&response).first() {
            Some(row) => {
                #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
                let count = row.get("count").and_then(serde_json::Value::as_i64).unwrap_or(0) as u32;
                #[allow(clippy::cast_sign_loss)]
                let stored = row.get("window_start").and_then(serde_json::Value::as_i64).unwrap_or(0).cast_unsigned();
                if stored < window_start { true } else { count < max_count }
            }
            None => true,
        };
        Ok(Self::wrap_value(allowed))
    }

    fn record_rate_limit(&self, key: &str) -> StorageResult<StorageItemStream<'_, u32>> {
        let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_secs();
        let sql = "INSERT INTO rate_limits (key, count, window_start) VALUES (?, 1, ?) ON CONFLICT(key) DO UPDATE SET count = count + 1, window_start = excluded.window_start";
        self.execute_sql(sql, &[serde_json::Value::String(key.to_string()), serde_json::Value::Number(serde_json::Number::from(now))])?;
        let response = self.execute_sql("SELECT count FROM rate_limits WHERE key = ?", &[serde_json::Value::String(key.to_string())])?;
        #[allow(clippy::cast_sign_loss, clippy::cast_possible_truncation)]
        let count = Self::extract_rows(&response).first()
            .and_then(|r| r.get("count")).and_then(serde_json::Value::as_i64)
            .map_or(1, |c| c as u32);
        Ok(Self::wrap_value(count))
    }

    fn reset_rate_limit(&self, key: &str) -> StorageResult<StorageItemStream<'_, ()>> {
        let sql = "DELETE FROM rate_limits WHERE key = ?";
        self.execute_sql(sql, &[serde_json::Value::String(key.to_string())])?;
        Ok(Self::wrap_value(()))
    }
}

// ===========================================================================
// BlobStore (D1 blob via base64 in KV table)
// ===========================================================================

impl BlobStore for D1Store {
    fn put_blob(&self, key: &str, data: &[u8]) -> StorageResult<StorageItemStream<'_, ()>> {
        let encoded = STANDARD.encode(data);
        let json_value = serde_json::json!({ "type": "blob", "encoding": "base64", "data": encoded }).to_string();
        let sql = format!(
            "INSERT INTO {} (key, value, updated_at) VALUES (?, ?, strftime('%s', 'now') * 1000) ON CONFLICT(key) DO UPDATE SET value = ?, updated_at = strftime('%s', 'now') * 1000",
            self.kv_table()
        );
        let kv = serde_json::Value::String(key.to_string());
        let jv = serde_json::Value::String(json_value);
        self.execute_sql(&sql, &[kv.clone(), jv.clone(), jv])?;
        Ok(Self::wrap_value(()))
    }

    fn get_blob(&self, key: &str) -> StorageResult<StorageItemStream<'_, Option<Vec<u8>>>> {
        let sql = format!("SELECT value FROM {} WHERE key = ?", self.kv_table());
        let response = self.execute_sql(&sql, &[serde_json::Value::String(key.to_string())])?;
        let rows = Self::extract_rows(&response);
        match rows.first() {
            Some(row) => {
                let value: String = row.get("value").and_then(serde_json::Value::as_str).map(String::from)
                    .ok_or_else(|| StorageError::SqlConversion("missing or invalid value field".to_string()))?;
                let wrapper: serde_json::Value = serde_json::from_str(&value)
                    .map_err(|e| StorageError::Serialization(e.to_string()))?;
                if wrapper.get("type").and_then(serde_json::Value::as_str) != Some("blob") {
                    return Ok(Self::wrap_value(None));
                }
                let encoded = wrapper.get("data").and_then(serde_json::Value::as_str)
                    .ok_or_else(|| StorageError::Serialization("missing data field in blob wrapper".to_string()))?;
                let decoded = STANDARD.decode(encoded)
                    .map_err(|e| StorageError::Backend(format!("Base64 decode failed: {e}")))?;
                Ok(Self::wrap_value(Some(decoded)))
            }
            None => Ok(Self::wrap_value(None)),
        }
    }

    fn delete_blob(&self, key: &str) -> StorageResult<StorageItemStream<'_, ()>> {
        let sql = format!("DELETE FROM {} WHERE key = ?", self.kv_table());
        self.execute_sql(&sql, &[serde_json::Value::String(key.to_string())])?;
        Ok(Self::wrap_value(()))
    }

    fn blob_exists(&self, key: &str) -> StorageResult<StorageItemStream<'_, bool>> {
        let sql = format!("SELECT 1 FROM {} WHERE key = ? LIMIT 1", self.kv_table());
        let response = self.execute_sql(&sql, &[serde_json::Value::String(key.to_string())])?;
        Ok(Self::wrap_value(!Self::extract_rows(&response).is_empty()))
    }
}

// ===========================================================================
// StateStore
// ===========================================================================

impl StateStore for D1Store {
    fn init(&self) -> Result<(), StorageError> {
        let table = self.state_table();
        let table_sql = state_create_table_sql(table);
        let index_sql = state_create_indexes_sql(table);
        self.execute_sql(&format!("{table_sql}\n{index_sql}"), &[])?;
        Ok(())
    }

    fn list(&self) -> Result<StateStoreStream<String>, StorageError> {
        let sql = format!("SELECT id FROM {} ORDER BY id", self.state_table());
        let response = self.execute_sql(&sql, &[])?;
        let ids: Result<Vec<String>, StorageError> = Self::extract_rows(&response)
            .iter()
            .map(|row| row.get("id").and_then(serde_json::Value::as_str).map(String::from)
                .ok_or_else(|| StorageError::SqlConversion("missing id field".to_string())))
            .collect();
        Ok(Self::wrap_vec_state(ids?))
    }

    fn count(&self) -> Result<StateStoreStream<usize>, StorageError> {
        let sql = format!("SELECT COUNT(*) as cnt FROM {}", self.state_table());
        let response = self.execute_sql(&sql, &[])?;
        let count = Self::extract_rows(&response).first()
            .and_then(|r| r.get("cnt")).and_then(serde_json::Value::as_u64)
            .and_then(|cnt| usize::try_from(cnt).ok())
            .unwrap_or(0);
        Ok(Self::wrap_value_state(count))
    }

    fn get(&self, resource_id: &str) -> Result<StateStoreStream<Option<ResourceState>>, StorageError> {
        let sql = format!(
            "SELECT id, kind, provider, status, environment, config_hash, output, config_snapshot, created_at, updated_at FROM {} WHERE id = ?",
            self.state_table()
        );
        let response = self.execute_sql(&sql, &[serde_json::Value::String(resource_id.to_string())])?;
        match Self::extract_rows(&response).first() {
            Some(row) => Ok(Self::wrap_value_state(Some(parse_d1_row(row)?))),
            None => Ok(Self::wrap_value_state(None)),
        }
    }

    fn get_batch(&self, ids: &[&str]) -> Result<StateStoreStream<ResourceState>, StorageError> {
        if ids.is_empty() { return Ok(Self::wrap_vec_state(Vec::new())); }
        let placeholders = ids.iter().map(|_| "?").collect::<Vec<_>>().join(",");
        let sql = format!(
            "SELECT id, kind, provider, status, environment, config_hash, output, config_snapshot, created_at, updated_at FROM {} WHERE id IN ({placeholders})",
            self.state_table()
        );
        let params: Vec<serde_json::Value> = ids.iter().map(|id| serde_json::Value::String((*id).to_string())).collect();
        let response = self.execute_sql(&sql, &params)?;
        let results: Result<Vec<_>, _> = Self::extract_rows(&response).iter().map(parse_d1_row).collect();
        Ok(Self::wrap_vec_state(results?))
    }

    fn all(&self) -> Result<StateStoreStream<ResourceState>, StorageError> {
        let sql = format!("SELECT id, kind, provider, status, environment, config_hash, output, config_snapshot, created_at, updated_at FROM {} ORDER BY id", self.state_table());
        let response = self.execute_sql(&sql, &[])?;
        let results: Result<Vec<_>, _> = Self::extract_rows(&response).iter().map(parse_d1_row).collect();
        Ok(Self::wrap_vec_state(results?))
    }

    fn set(&self, _resource_id: &str, state: &ResourceState) -> Result<StateStoreStream<()>, StorageError> {
        let params = state_to_params(state)?;
        let sql = state_upsert_sql(self.state_table());
        self.execute_sql(&sql, &params)?;
        Ok(Self::wrap_value_state(()))
    }

    fn delete(&self, resource_id: &str) -> Result<StateStoreStream<()>, StorageError> {
        let sql = format!("DELETE FROM {} WHERE id = ?", self.state_table());
        self.execute_sql(&sql, &[serde_json::Value::String(resource_id.to_string())])?;
        Ok(Self::wrap_value_state(()))
    }

    fn list_by_prefix(&self, prefix: &str) -> Result<StateStoreStream<String>, StorageError> {
        let prefix = escape_like(prefix);
        let sql = format!("SELECT id FROM {} WHERE id LIKE ?||'%' ESCAPE '\\' ORDER BY id", self.state_table());
        let response = self.execute_sql(&sql, &[serde_json::Value::String(prefix)])?;
        let ids: Result<Vec<String>, StorageError> = Self::extract_rows(&response)
            .iter()
            .map(|row| row.get("id").and_then(serde_json::Value::as_str).map(String::from)
                .ok_or_else(|| StorageError::SqlConversion("missing id field".to_string())))
            .collect();
        Ok(Self::wrap_vec_state(ids?))
    }

    fn count_by_prefix(&self, prefix: &str) -> Result<StateStoreStream<usize>, StorageError> {
        let prefix = escape_like(prefix);
        let sql = format!("SELECT COUNT(*) as cnt FROM {} WHERE id LIKE ?||'%' ESCAPE '\\'", self.state_table());
        let response = self.execute_sql(&sql, &[serde_json::Value::String(prefix)])?;
        let count = Self::extract_rows(&response).first()
            .and_then(|r| r.get("cnt")).and_then(serde_json::Value::as_u64)
            .and_then(|cnt| usize::try_from(cnt).ok())
            .unwrap_or(0);
        Ok(Self::wrap_value_state(count))
    }

    fn all_by_prefix(&self, prefix: &str) -> Result<StateStoreStream<ResourceState>, StorageError> {
        let prefix = escape_like(prefix);
        let sql = format!(
            "SELECT id, kind, provider, status, environment, config_hash, output, config_snapshot, created_at, updated_at FROM {} WHERE id LIKE ?||'%' ESCAPE '\\' ORDER BY id",
            self.state_table()
        );
        let response = self.execute_sql(&sql, &[serde_json::Value::String(prefix)])?;
        let results: Result<Vec<_>, _> = Self::extract_rows(&response).iter().map(parse_d1_row).collect();
        Ok(Self::wrap_vec_state(results?))
    }

    fn find_by_kind(&self, kind: &str) -> Result<StateStoreStream<ResourceState>, StorageError> {
        let sql = format!(
            "SELECT id, kind, provider, status, environment, config_hash, output, config_snapshot, created_at, updated_at FROM {} WHERE kind = ? ORDER BY id",
            self.state_table()
        );
        let response = self.execute_sql(&sql, &[serde_json::Value::String(kind.to_string())])?;
        let results: Result<Vec<_>, _> = Self::extract_rows(&response).iter().map(parse_d1_row).collect();
        Ok(Self::wrap_vec_state(results?))
    }

    fn find_by_status(&self, status: &str) -> Result<StateStoreStream<ResourceState>, StorageError> {
        let sql = format!(
            "SELECT id, kind, provider, status, environment, config_hash, output, config_snapshot, created_at, updated_at FROM {} WHERE status = ? ORDER BY id",
            self.state_table()
        );
        let response = self.execute_sql(&sql, &[serde_json::Value::String(status.to_string())])?;
        let results: Result<Vec<_>, _> = Self::extract_rows(&response).iter().map(parse_d1_row).collect();
        Ok(Self::wrap_vec_state(results?))
    }

    fn find_by_provider(&self, provider: &str) -> Result<StateStoreStream<ResourceState>, StorageError> {
        let sql = format!(
            "SELECT id, kind, provider, status, environment, config_hash, output, config_snapshot, created_at, updated_at FROM {} WHERE provider = ? ORDER BY id",
            self.state_table()
        );
        let response = self.execute_sql(&sql, &[serde_json::Value::String(provider.to_string())])?;
        let results: Result<Vec<_>, _> = Self::extract_rows(&response).iter().map(parse_d1_row).collect();
        Ok(Self::wrap_vec_state(results?))
    }

    fn delete_by_prefix(&self, prefix: &str) -> Result<StateStoreStream<usize>, StorageError> {
        let prefix = escape_like(prefix);
        let sql = format!("DELETE FROM {} WHERE id LIKE ?||'%' ESCAPE '\\'", self.state_table());
        let response = self.execute_sql(&sql, &[serde_json::Value::String(prefix)])?;
        let count = response.pointer("/result/0/meta/rows_written")
            .and_then(serde_json::Value::as_u64)
            .and_then(|n| usize::try_from(n).ok())
            .unwrap_or(0);
        Ok(Self::wrap_value_state(count))
    }
}

// ===========================================================================
// Async trait implementations
// ===========================================================================

#[async_trait::async_trait(?Send)]
impl AsyncKeyValueStore for D1Store {
    async fn get_async<V: DeserializeOwned + Send + 'static>(&self, key: &str) -> StorageResult<Option<V>> {
        let sql = format!("SELECT value FROM {} WHERE key = ?", self.kv_table());
        let response = self.execute_sql_async(&sql, &[serde_json::Value::String(key.to_string())]).await?;
        let rows = Self::extract_rows(&response);
        match rows.first() {
            Some(row) => {
                let value: String = row.get("value").and_then(serde_json::Value::as_str).map(String::from)
                    .ok_or_else(|| StorageError::SqlConversion("missing or invalid value field".to_string()))?;
                Ok(Some(serde_json::from_str(&value).map_err(|e| StorageError::Serialization(e.to_string()))?))
            }
            None => Ok(None),
        }
    }

    async fn set_async<V: Serialize + Send + 'static>(&self, key: &str, value: V) -> StorageResult<()> {
        let json_value = serde_json::to_string(&value).map_err(|e| StorageError::Serialization(e.to_string()))?;
        let sql = format!(
            "INSERT INTO {} (key, value, updated_at) VALUES (?, ?, strftime('%s', 'now') * 1000) ON CONFLICT(key) DO UPDATE SET value = ?, updated_at = strftime('%s', 'now') * 1000",
            self.kv_table()
        );
        let kv = serde_json::Value::String(key.to_string());
        let jv = serde_json::Value::String(json_value);
        self.execute_sql_async(&sql, &[kv.clone(), jv.clone(), jv]).await?;
        Ok(())
    }

    async fn delete_async(&self, key: &str) -> StorageResult<()> {
        let sql = format!("DELETE FROM {} WHERE key = ?", self.kv_table());
        self.execute_sql_async(&sql, &[serde_json::Value::String(key.to_string())]).await?;
        Ok(())
    }

    async fn exists_async(&self, key: &str) -> StorageResult<bool> {
        let sql = format!("SELECT 1 FROM {} WHERE key = ? LIMIT 1", self.kv_table());
        let response = self.execute_sql_async(&sql, &[serde_json::Value::String(key.to_string())]).await?;
        Ok(!Self::extract_rows(&response).is_empty())
    }

    async fn list_keys_async(&self, prefix: Option<&str>) -> StorageResult<Vec<String>> {
        let (sql, params) = match prefix {
            Some(p) => (
                format!("SELECT key FROM {} WHERE key LIKE ? ORDER BY key", self.kv_table()),
                vec![serde_json::Value::String(format!("{p}%"))],
            ),
            None => (
                format!("SELECT key FROM {} ORDER BY key", self.kv_table()),
                vec![],
            ),
        };
        let response = self.execute_sql_async(&sql, &params).await?;
        Self::extract_rows(&response)
            .iter()
            .map(|row| row.get("key").and_then(serde_json::Value::as_str).map(String::from)
                .ok_or_else(|| StorageError::SqlConversion("missing or invalid key field".to_string())))
            .collect()
    }
}

#[async_trait::async_trait(?Send)]
impl AsyncQueryStore for D1Store {
    async fn query_async(&self, sql: &str, params: &[DataValue]) -> StorageResult<Vec<SqlRow>> {
        let json_params: Vec<serde_json::Value> = params.iter().map(|v| match v {
            DataValue::Null => serde_json::Value::Null,
            DataValue::Integer(i) => serde_json::Value::Number(serde_json::Number::from(*i)),
            DataValue::Real(f) => serde_json::Number::from_f64(*f).map_or(serde_json::Value::Null, serde_json::Value::Number),
            DataValue::Text(s) => serde_json::Value::String(s.clone()),
            DataValue::Blob(b) => serde_json::Value::String(STANDARD.encode(b)),
        }).collect();
        let response = self.execute_sql_async(sql, &json_params).await?;
        Self::extract_rows(&response)
            .iter()
            .map(|row| {
                let obj = row.as_object().ok_or_else(|| StorageError::SqlConversion("row is not an object".to_string()))?;
                let columns: Vec<(String, DataValue)> = obj.iter().map(|(k, v)| {
                    let dv = match v {
                        serde_json::Value::Number(n) => {
                            if let Some(i) = n.as_i64() { DataValue::Integer(i) }
                            else if let Some(f) = n.as_f64() { DataValue::Real(f) }
                            else { DataValue::Null }
                        }
                        serde_json::Value::String(s) => DataValue::Text(s.clone()),
                        _ => DataValue::Null,
                    };
                    (k.clone(), dv)
                }).collect();
                Ok(SqlRow::new(columns))
            }).collect()
    }

    async fn execute_async(&self, sql: &str, params: &[DataValue]) -> StorageResult<u64> {
        let json_params: Vec<serde_json::Value> = params.iter().map(|v| match v {
            DataValue::Null => serde_json::Value::Null,
            DataValue::Integer(i) => serde_json::Value::Number(serde_json::Number::from(*i)),
            DataValue::Real(f) => serde_json::Number::from_f64(*f).map_or(serde_json::Value::Null, serde_json::Value::Number),
            DataValue::Text(s) => serde_json::Value::String(s.clone()),
            DataValue::Blob(b) => serde_json::Value::String(STANDARD.encode(b)),
        }).collect();
        let response = self.execute_sql_async(sql, &json_params).await?;
        let affected = response.pointer("/result/0/meta/changes")
            .and_then(serde_json::Value::as_i64)
            .map_or(0, i64::unsigned_abs);
        Ok(affected)
    }

    async fn execute_batch_async(&self, sql: &str) -> StorageResult<()> {
        self.execute_sql_async(sql, &[]).await?;
        Ok(())
    }
}

#[async_trait::async_trait(?Send)]
impl AsyncRateLimiterStore for D1Store {
    async fn check_rate_limit_async(&self, key: &str, max_count: u32, window_seconds: u64) -> StorageResult<bool> {
        let create_table = r"CREATE TABLE IF NOT EXISTS rate_limits (key TEXT PRIMARY KEY, count INTEGER NOT NULL, window_start INTEGER NOT NULL)";
        self.execute_sql_async(create_table, &[]).await?;
        let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_secs();
        let window_start = now - window_seconds;
        let sql = "SELECT count, window_start FROM rate_limits WHERE key = ?";
        let response = self.execute_sql_async(sql, &[serde_json::Value::String(key.to_string())]).await?;
        let allowed = match Self::extract_rows(&response).first() {
            Some(row) => {
                #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
                let count = row.get("count").and_then(serde_json::Value::as_i64).unwrap_or(0) as u32;
                #[allow(clippy::cast_sign_loss)]
                let stored = row.get("window_start").and_then(serde_json::Value::as_i64).unwrap_or(0).cast_unsigned();
                if stored < window_start { true } else { count < max_count }
            }
            None => true,
        };
        Ok(allowed)
    }

    async fn record_rate_limit_async(&self, key: &str) -> StorageResult<u32> {
        let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_secs();
        let sql = "INSERT INTO rate_limits (key, count, window_start) VALUES (?, 1, ?) ON CONFLICT(key) DO UPDATE SET count = count + 1, window_start = excluded.window_start";
        self.execute_sql_async(sql, &[serde_json::Value::String(key.to_string()), serde_json::Value::Number(serde_json::Number::from(now))]).await?;
        let response = self.execute_sql_async("SELECT count FROM rate_limits WHERE key = ?", &[serde_json::Value::String(key.to_string())]).await?;
        #[allow(clippy::cast_sign_loss, clippy::cast_possible_truncation)]
        let count = Self::extract_rows(&response).first()
            .and_then(|r| r.get("count")).and_then(serde_json::Value::as_i64)
            .map_or(1, |c| c as u32);
        Ok(count)
    }

    async fn reset_rate_limit_async(&self, key: &str) -> StorageResult<()> {
        let sql = "DELETE FROM rate_limits WHERE key = ?";
        self.execute_sql_async(sql, &[serde_json::Value::String(key.to_string())]).await?;
        Ok(())
    }
}

#[async_trait::async_trait(?Send)]
impl AsyncBlobStore for D1Store {
    async fn put_blob_async(&self, key: &str, data: &[u8]) -> StorageResult<()> {
        let encoded = STANDARD.encode(data);
        let json_value = serde_json::json!({ "type": "blob", "encoding": "base64", "data": encoded }).to_string();
        let sql = format!(
            "INSERT INTO {} (key, value, updated_at) VALUES (?, ?, strftime('%s', 'now') * 1000) ON CONFLICT(key) DO UPDATE SET value = ?, updated_at = strftime('%s', 'now') * 1000",
            self.kv_table()
        );
        let kv = serde_json::Value::String(key.to_string());
        let jv = serde_json::Value::String(json_value);
        self.execute_sql_async(&sql, &[kv.clone(), jv.clone(), jv]).await?;
        Ok(())
    }

    async fn get_blob_async(&self, key: &str) -> StorageResult<Option<Vec<u8>>> {
        let sql = format!("SELECT value FROM {} WHERE key = ?", self.kv_table());
        let response = self.execute_sql_async(&sql, &[serde_json::Value::String(key.to_string())]).await?;
        let rows = Self::extract_rows(&response);
        match rows.first() {
            Some(row) => {
                let value: String = row.get("value").and_then(serde_json::Value::as_str).map(String::from)
                    .ok_or_else(|| StorageError::SqlConversion("missing or invalid value field".to_string()))?;
                let wrapper: serde_json::Value = serde_json::from_str(&value)
                    .map_err(|e| StorageError::Serialization(e.to_string()))?;
                if wrapper.get("type").and_then(serde_json::Value::as_str) != Some("blob") {
                    return Ok(None);
                }
                let encoded = wrapper.get("data").and_then(serde_json::Value::as_str)
                    .ok_or_else(|| StorageError::Serialization("missing data field in blob wrapper".to_string()))?;
                Ok(Some(STANDARD.decode(encoded).map_err(|e| StorageError::Backend(format!("Base64 decode failed: {e}")))?))
            }
            None => Ok(None),
        }
    }

    async fn delete_blob_async(&self, key: &str) -> StorageResult<()> {
        let sql = format!("DELETE FROM {} WHERE key = ?", self.kv_table());
        self.execute_sql_async(&sql, &[serde_json::Value::String(key.to_string())]).await?;
        Ok(())
    }

    async fn blob_exists_async(&self, key: &str) -> StorageResult<bool> {
        let sql = format!("SELECT 1 FROM {} WHERE key = ? LIMIT 1", self.kv_table());
        let response = self.execute_sql_async(&sql, &[serde_json::Value::String(key.to_string())]).await?;
        Ok(!Self::extract_rows(&response).is_empty())
    }
}
