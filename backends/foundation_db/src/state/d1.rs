//! Cloudflare D1 edge `SQLite` state store.
//!
//! WHY: Edge-native SQL state for Cloudflare-centric teams. D1 is serverless
//! `SQLite` accessible via HTTP — no connection management, no drivers.
//!
//! WHAT: `D1StateStore` executes SQL over HTTP via the Cloudflare D1 API.
//! Table names use `{project}_{stage}_resources` pattern for namespacing.
//!
//! HOW: Uses `SimpleHttpClient` (synchronous) for all HTTP calls. SQL queries
//! are POST'd as JSON. D1 returns JSON rows — no `!Send` issues, no Valtron.

use foundation_core::valtron::ThreadedValue;
use foundation_core::wire::simple_http::client::SimpleHttpClient;
use foundation_core::wire::simple_http::{SendSafeBody, SimpleHeader, Status};

use super::traits::{StateStore, StateStoreStream};
use super::types::{ResourceState, StateStatus};
use crate::crypto::ZeroizingString;
use crate::errors::StorageError;

const CF_API_BASE: &str = "https://api.cloudflare.com/client/v4";

/// SQL schema — table name is prefixed with `{project}_{stage}_`.
fn create_table_sql(table_name: &str) -> String {
    format!(
        "CREATE TABLE IF NOT EXISTS {table_name} (id TEXT PRIMARY KEY, kind TEXT NOT NULL, provider TEXT NOT NULL, status TEXT NOT NULL, environment TEXT, config_hash TEXT NOT NULL, output TEXT NOT NULL, config_snapshot TEXT NOT NULL, created_at TEXT NOT NULL, updated_at TEXT NOT NULL)",
    )
}

fn upsert_sql(table_name: &str) -> String {
    format!(
        "INSERT INTO {table_name} (id, kind, provider, status, environment, config_hash, output, config_snapshot, created_at, updated_at) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?) ON CONFLICT(id) DO UPDATE SET kind = excluded.kind, provider = excluded.provider, status = excluded.status, environment = excluded.environment, config_hash = excluded.config_hash, output = excluded.output, config_snapshot = excluded.config_snapshot, updated_at = excluded.updated_at",
    )
}

/// Cloudflare D1 edge `SQLite` state store.
///
/// Executes SQL over HTTP via the Cloudflare D1 API.
/// Uses `SimpleHttpClient` for all requests — synchronous, no Valtron.
///
/// Table names are prefixed with `{project}_{stage}_resources` for namespacing.
pub struct D1StateStore {
    api_token: ZeroizingString,
    account_id: String,
    database_id: String,
    table_name: String,  // "{project}_{stage}_resources"
    client: SimpleHttpClient,
}

impl D1StateStore {
    /// Create a new D1 state store.
    ///
    /// Table name will be `{project}_{stage}_resources` for namespacing.
    #[must_use]
    pub fn new(api_token: &str, account_id: &str, database_id: &str, project: &str, stage: &str) -> Self {
        let table_name = format!("{}_{}_resources",
            project.replace(['-', ' '], "_"),
            stage.replace(['-', ' '], "_")
        );
        Self {
            api_token: ZeroizingString::from_string(api_token.to_string()),
            account_id: account_id.to_string(),
            database_id: database_id.to_string(),
            table_name,
            client: SimpleHttpClient::from_system(),
        }
    }

    /// Create from environment variables.
    ///
    /// - `DEPLOYMENT_D1_DATABASE_ID` (required)
    /// - `CLOUDFLARE_API_TOKEN` (required)
    /// - `CLOUDFLARE_ACCOUNT_ID` (required)
    ///
    /// # Errors
    ///
    /// Returns an error if required env vars are missing.
    pub fn from_env(project: &str, stage: &str) -> Result<Self, StorageError> {
        let db_id = std::env::var("DEPLOYMENT_D1_DATABASE_ID").map_err(|_| {
            StorageError::Connection("DEPLOYMENT_D1_DATABASE_ID must be set".to_string())
        })?;
        let token = std::env::var("CLOUDFLARE_API_TOKEN").map_err(|_| {
            StorageError::Connection("CLOUDFLARE_API_TOKEN must be set".to_string())
        })?;
        let account = std::env::var("CLOUDFLARE_ACCOUNT_ID").map_err(|_| {
            StorageError::Connection("CLOUDFLARE_ACCOUNT_ID must be set".to_string())
        })?;
        Ok(Self::new(&token, &account, &db_id, project, stage))
    }

    fn query_url(&self) -> String {
        format!(
            "{CF_API_BASE}/accounts/{}/d1/database/{}/query",
            self.account_id, self.database_id
        )
    }

    fn auth_header(&self) -> String {
        format!("Bearer {}", self.api_token.as_str())
    }

    /// Execute a SQL query via the D1 HTTP API and return the parsed response.
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
    ///
    /// D1 response format: `{ "success": true, "result": [{ "results": [...] }] }`
    fn extract_rows(response: &serde_json::Value) -> Vec<serde_json::Value> {
        response
            .pointer("/result/0/results")
            .and_then(serde_json::Value::as_array)
            .cloned()
            .unwrap_or_default()
    }

    /// Parse a D1 JSON row into a `ResourceState`.
    fn parse_row(row: &serde_json::Value) -> Result<ResourceState, StorageError> {
        let get_str = |key: &str| -> Result<String, StorageError> {
            row.get(key)
                .and_then(serde_json::Value::as_str)
                .map(String::from)
                .ok_or_else(|| {
                    StorageError::SqlConversion(format!("missing or invalid field: {key}"))
                })
        };

        let id = get_str("id")?;
        let kind = get_str("kind")?;
        let provider = get_str("provider")?;
        let status_str = get_str("status")?;
        let environment = row
            .get("environment")
            .and_then(serde_json::Value::as_str)
            .map(String::from);
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

        Ok(ResourceState {
            id,
            kind,
            provider,
            status,
            environment,
            config_hash,
            output,
            config_snapshot,
            created_at,
            updated_at,
        })
    }

    /// Serialize state into D1 SQL parameter values.
    fn state_to_params(state: &ResourceState) -> Result<Vec<serde_json::Value>, StorageError> {
        let status_json = serde_json::to_string(&state.status)
            .map_err(|e| StorageError::Serialization(e.to_string()))?;
        let output_json = serde_json::to_string(&state.output)
            .map_err(|e| StorageError::Serialization(e.to_string()))?;
        let snapshot_json = serde_json::to_string(&state.config_snapshot)
            .map_err(|e| StorageError::Serialization(e.to_string()))?;

        Ok(vec![
            serde_json::Value::String(state.id.clone()),
            serde_json::Value::String(state.kind.clone()),
            serde_json::Value::String(state.provider.clone()),
            serde_json::Value::String(status_json),
            state
                .environment
                .as_ref()
                .map_or(serde_json::Value::Null, |e| {
                    serde_json::Value::String(e.clone())
                }),
            serde_json::Value::String(state.config_hash.clone()),
            serde_json::Value::String(output_json),
            serde_json::Value::String(snapshot_json),
            serde_json::Value::String(state.created_at.to_rfc3339()),
            serde_json::Value::String(state.updated_at.to_rfc3339()),
        ])
    }

    /// Wrap a single value into a `StateStoreStream`.
    fn wrap_value<T: Send + 'static>(val: T) -> StateStoreStream<T> {
        Box::new(std::iter::once(ThreadedValue::Value(Ok(val))))
    }

    /// Wrap a `Vec` into a `StateStoreStream`.
    fn wrap_vec<T: Send + 'static>(vals: Vec<T>) -> StateStoreStream<T> {
        Box::new(vals.into_iter().map(|v| ThreadedValue::Value(Ok(v))))
    }
}

impl StateStore for D1StateStore {
    fn init(&self) -> Result<(), StorageError> {
        let sql = create_table_sql(&self.table_name);
        self.execute_sql(&sql, &[])?;
        Ok(())
    }

    fn list(&self) -> Result<StateStoreStream<String>, StorageError> {
        let sql = format!("SELECT id FROM {} ORDER BY id", self.table_name);
        let response = self.execute_sql(&sql, &[])?;
        let rows = Self::extract_rows(&response);
        let ids: Result<Vec<String>, StorageError> = rows
            .iter()
            .map(|row| {
                row.get("id")
                    .and_then(serde_json::Value::as_str)
                    .map(String::from)
                    .ok_or_else(|| {
                        StorageError::SqlConversion("missing id field".to_string())
                    })
            })
            .collect();
        Ok(Self::wrap_vec(ids?))
    }

    fn count(&self) -> Result<StateStoreStream<usize>, StorageError> {
        let sql = format!("SELECT COUNT(*) as cnt FROM {}", self.table_name);
        let response = self.execute_sql(&sql, &[])?;
        let rows = Self::extract_rows(&response);
        let count = rows
            .first()
            .and_then(|r| r.get("cnt"))
            .and_then(serde_json::Value::as_u64)
            .and_then(|cnt| usize::try_from(cnt).ok())
            .unwrap_or(0);
        Ok(Self::wrap_value(count))
    }

    fn get(
        &self,
        resource_id: &str,
    ) -> Result<StateStoreStream<Option<ResourceState>>, StorageError> {
        let sql = format!(
            "SELECT id, kind, provider, status, environment, config_hash, output, config_snapshot, created_at, updated_at FROM {} WHERE id = ?",
            self.table_name
        );
        let response = self.execute_sql(
            &sql,
            &[serde_json::Value::String(resource_id.to_string())],
        )?;
        let rows = Self::extract_rows(&response);
        match rows.first() {
            Some(row) => {
                let state = Self::parse_row(row)?;
                Ok(Self::wrap_value(Some(state)))
            }
            None => Ok(Self::wrap_value(None)),
        }
    }

    fn get_batch(&self, ids: &[&str]) -> Result<StateStoreStream<ResourceState>, StorageError> {
        if ids.is_empty() {
            return Ok(Self::wrap_vec(Vec::new()));
        }

        // Build IN clause placeholders
        let placeholders = ids
            .iter()
            .map(|_| "?")
            .collect::<Vec<_>>()
            .join(",");
        let sql = format!(
            "SELECT id, kind, provider, status, environment, config_hash, output, config_snapshot, created_at, updated_at FROM {} WHERE id IN ({placeholders})",
            self.table_name
        );
        let params: Vec<serde_json::Value> = ids
            .iter()
            .map(|id| serde_json::Value::String((*id).to_string()))
            .collect();

        let response = self.execute_sql(&sql, &params)?;
        let rows = Self::extract_rows(&response);
        let results: Result<Vec<_>, _> = rows
            .iter()
            .map(Self::parse_row)
            .collect();
        Ok(Self::wrap_vec(results?))
    }

    fn all(&self) -> Result<StateStoreStream<ResourceState>, StorageError> {
        let sql = format!(
            "SELECT id, kind, provider, status, environment, config_hash, output, config_snapshot, created_at, updated_at FROM {} ORDER BY id",
            self.table_name
        );
        let response = self.execute_sql(&sql, &[])?;
        let rows = Self::extract_rows(&response);
        let results: Result<Vec<_>, _> = rows
            .iter()
            .map(Self::parse_row)
            .collect();
        Ok(Self::wrap_vec(results?))
    }

    fn set(
        &self,
        _resource_id: &str,
        state: &ResourceState,
    ) -> Result<StateStoreStream<()>, StorageError> {
        let params = Self::state_to_params(state)?;
        let sql = upsert_sql(&self.table_name);
        self.execute_sql(&sql, &params)?;
        Ok(Self::wrap_value(()))
    }

    fn delete(&self, resource_id: &str) -> Result<StateStoreStream<()>, StorageError> {
        let sql = format!("DELETE FROM {} WHERE id = ?", self.table_name);
        self.execute_sql(
            &sql,
            &[serde_json::Value::String(resource_id.to_string())],
        )?;
        Ok(Self::wrap_value(()))
    }
}
