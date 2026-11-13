//! Cloudflare D1 storage backend (unified).
//!
//! WHY: D1 is Cloudflare's edge `SQLite` - useful for KV, query execution,
//! blob storage, rate limiting, and deployment state.
//!
//! WHAT: `D1Store` implements the `Async*` storage traits (KV, query, rate
//! limiting, blobs) and `StateStore` via `DynNetClient` (`HttpClientBuilder`) calls to the D1
//! HTTP API.
//!
//! HOW: Different constructors for different usage modes:
//!   - `D1Store::new()` / `D1Store::new_kv()` — KV/query/rate-limit/blob store (table: `{prefix}_kv`)
//!   - `D1Store::new_state()` — `StateStore` (table: `{project}_{stage}_resources`)

use base64::{engine::general_purpose::STANDARD, Engine};
use foundation_cloudflare_types::{ApiEnvelope, D1BatchQuery, D1RawResultResponse, D1SingleQuery};
use foundation_context::Ctx;
use foundation_netio::shared::http::{SimpleHeader, Status};
use foundation_netio::{DynNetClient, HttpClientBuilder, PreparedRequestBuilder};
use futures_lite::{stream, StreamExt};
use serde::{de::DeserializeOwned, Serialize};

use crate::core::crypto::ZeroizingString;
use crate::core::errors::{StorageError, StorageResult};
use crate::core::rate_limit;
use crate::core::state::traits::{StateStore, StateStoreStream};
use crate::core::state::types::{status_column_value, ResourceState, StateStatus};
use crate::core::storage_provider::{
    AsyncListStream, AsyncQueryStream, BlobListStream, BlobStore, DataValue, KeyValueStore,
    QueryStore, RateLimiterStore, SqlRow,
};

/// Default for [`D1Store::with_max_response_body`]: 16 MiB.
///
/// D1 documents a maximum row size of 2 MB (developers.cloudflare.com/d1/platform/limits)
/// but no maximum result size, so the default allows eight maximum-size rows
/// (Go's `http.MaxBytesReader` and nginx's `proxy_buffers` also want a finite
/// number from the caller; there is no standard default). A larger result
/// needs `LIMIT`/paging or a larger cap.
pub const DEFAULT_MAX_RESPONSE_BODY: usize = 16 * 1024 * 1024;

/// Default for [`D1Store::with_max_error_body`]: 8 KiB, the size nginx uses for
/// its error-page buffers (`proxy_buffer_size`: one memory page).
pub const DEFAULT_MAX_ERROR_BODY: usize = 8 * 1024;

/// Default Cloudflare API base (hosted). `D1Store::new_*` put
/// `/accounts/{account_id}` after it; the `*_with_base_url` constructors take
/// the whole prefix instead (see [`hosted_api_prefix`]).
pub const CF_API_BASE: &str = "https://api.cloudflare.com/client/v4";

/// The response of `POST .../d1/database/{id}/raw`: one result per statement,
/// each with the column names once and the rows as ordered arrays.
type D1RawResponse = ApiEnvelope<Vec<D1RawResultResponse>>;

/// The API prefix of the hosted Cloudflare API for `account_id`
/// (`https://api.cloudflare.com/client/v4/accounts/{account_id}`). Pass it, or
/// the prefix of wrangler's local explorer API
/// (`http://127.0.0.1:<port>/cdn-cgi/local/explorer/api`, no account segment),
/// as the `base_url` of the `*_with_base_url` constructors.
#[must_use]
pub fn hosted_api_prefix(account_id: &str) -> String {
    format!("{CF_API_BASE}/accounts/{account_id}")
}

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

/// `INSERT` that does nothing if the id exists (for `set_if_version` with no
/// expected version).
fn state_insert_if_absent_sql(table_name: &str) -> String {
    format!(
        "INSERT INTO {table_name} (id, kind, provider, status, environment, config_hash, output, config_snapshot, created_at, updated_at) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?) ON CONFLICT(id) DO NOTHING",
    )
}

/// `UPDATE` that changes the row only if its `updated_at` is the expected one
/// (for `set_if_version`). Parameters: `kind`, `provider`, `status`,
/// `environment`, `config_hash`, `output`, `config_snapshot`, the new
/// `updated_at`, `id`, and the expected `updated_at`.
fn state_update_if_version_sql(table_name: &str) -> String {
    format!(
        "UPDATE {table_name} SET kind = ?, provider = ?, status = ?, environment = ?, config_hash = ?, output = ?, config_snapshot = ?, updated_at = ? WHERE id = ? AND updated_at = ?",
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

/// For each parameter: its text and the SQL that types it (`None` for NULL,
/// which is written inline so its parameter is dropped); the new number of each
/// kept parameter (its place among the kept ones, so `?N` keeps its number
/// unless a NULL before it was dropped); and the texts of the kept parameters.
#[allow(clippy::type_complexity)]
fn typed_parameters(
    params: &[DataValue],
) -> Result<(Vec<Option<(String, &'static str)>>, Vec<usize>, Vec<String>), StorageError> {
    let mut typed: Vec<Option<(String, &'static str)>> = Vec::with_capacity(params.len());
    for value in params {
        typed.push(match value {
            DataValue::Null => None,
            DataValue::Integer(i) => Some((i.to_string(), "CAST(? AS INTEGER)")),
            DataValue::Real(f) if f.is_finite() => Some((f.to_string(), "CAST(? AS REAL)")),
            DataValue::Real(f) => {
                return Err(StorageError::SqlConversion(format!(
                    "non-finite REAL parameter ({f}) can not be sent to D1"
                )));
            }
            DataValue::Text(t) => Some((t.clone(), "?")),
            DataValue::Blob(b) => Some((hex(b), "unhex(?)")),
        });
    }
    let mut slots = Vec::with_capacity(params.len());
    let mut kept = 0usize;
    for t in &typed {
        if t.is_some() {
            kept += 1;
        }
        slots.push(kept);
    }
    let strings = typed
        .iter()
        .flatten()
        .map(|(text, _)| text.clone())
        .collect();
    Ok((typed, slots, strings))
}

/// Lowercase hex of `bytes`.
fn hex(bytes: &[u8]) -> String {
    use std::fmt::Write as _;
    bytes.iter().fold(String::new(), |mut out, byte| {
        // Writing to a `String` cannot fail.
        let _ = write!(out, "{byte:02x}");
        out
    })
}

/// Turn SQL with `?` placeholders and typed [`DataValue`]s into SQL and string
/// parameters for the D1 REST API.
///
/// The REST API takes **strings only** as parameters (the published spec says
/// `array of string`; wrangler's local explorer API rejects anything else with
/// code 10001). So each value is sent as a string and given its type in the
/// SQL:
///
/// - `Integer(i)`: `CAST(? AS INTEGER)` with the decimal text, so the full
///   `i64` range is exact (a JSON number would pass through a double);
/// - `Real(f)`: `CAST(? AS REAL)` with the shortest text that reads back as the
///   same `f64`;
/// - `Null`: the `NULL` literal replaces the placeholder (no parameter);
/// - `Text(s)`: `?` with `s`;
/// - `Blob(b)`: `unhex(?)` with the hex text.
///
/// Only `?` and `?N` placeholders outside string literals, quoted names and
/// comments are rewritten. When a `NULL` is written inline its parameter is
/// dropped, so a `?N` is renumbered to its place among the parameters that stay.
///
/// # Errors
///
/// Returns [`StorageError::SqlConversion`] for a `Real` that is NaN or infinite
/// (JSON has no such number and SQL has no such literal), and when the
/// placeholders name a parameter that is not there or leave one unused.
fn typed_statement(sql: &str, params: &[DataValue]) -> Result<(String, Vec<String>), StorageError> {
    let mismatch = || {
        StorageError::SqlConversion(format!(
            "the SQL has a different number of `?` placeholders than the {} parameters",
            params.len()
        ))
    };
    let mut out = String::with_capacity(sql.len() + params.len() * 16);
    let (typed, slots, strings) = typed_parameters(params)?;
    let mut next_anonymous = 0usize;
    let mut used = 0usize;
    let mut chars = sql.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            // Quoted text and names: copy to the closing quote (doubled quote = escaped).
            '\'' | '"' | '`' => {
                out.push(c);
                while let Some(q) = chars.next() {
                    out.push(q);
                    if q == c {
                        if chars.peek() == Some(&c) {
                            out.extend(chars.next());
                        } else {
                            break;
                        }
                    }
                }
            }
            '[' => {
                out.push(c);
                for q in chars.by_ref() {
                    out.push(q);
                    if q == ']' {
                        break;
                    }
                }
            }
            '-' if chars.peek() == Some(&'-') => {
                out.push(c);
                for q in chars.by_ref() {
                    out.push(q);
                    if q == '\n' {
                        break;
                    }
                }
            }
            '/' if chars.peek() == Some(&'*') => {
                out.push(c);
                out.extend(chars.next());
                let mut prev = '\0';
                for q in chars.by_ref() {
                    out.push(q);
                    if prev == '*' && q == '/' {
                        break;
                    }
                    prev = q;
                }
            }
            '?' => {
                // `?` takes the next parameter; `?N` names parameter N (1-based).
                let mut digits = String::new();
                while let Some(d) = chars.peek().copied().filter(char::is_ascii_digit) {
                    digits.push(d);
                    chars.next();
                }
                let index = if digits.is_empty() {
                    let i = next_anonymous;
                    next_anonymous += 1;
                    i
                } else {
                    digits
                        .parse::<usize>()
                        .ok()
                        .and_then(|n| n.checked_sub(1))
                        .ok_or_else(mismatch)?
                };
                let Some(typed_param) = typed.get(index) else {
                    return Err(mismatch());
                };
                used = used.max(index + 1);
                match typed_param {
                    None => out.push_str("NULL"),
                    Some((_, wrap)) => {
                        if digits.is_empty() {
                            out.push_str(wrap);
                        } else {
                            out.push_str(&wrap.replace('?', &format!("?{}", slots[index])));
                        }
                    }
                }
            }
            other => out.push(other),
        }
    }
    if used != params.len() {
        return Err(mismatch());
    }
    Ok((out, strings))
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

// ---- Mode enum ----

#[derive(Clone)]
enum D1Mode {
    /// KV/query/rate-limit/blob store. Table name = `{prefix}_kv`.
    KeyValue { kv_table: String },
    /// `StateStore`. Table name = `{project}_{stage}_resources`.
    State { state_table: String },
}

// ---- D1Store ----

#[derive(Clone)]
pub struct D1Store {
    api_token: ZeroizingString,
    database_id: String,
    /// Everything before `/d1/database/...` in the request URL.
    api_prefix: String,
    client: DynNetClient,
    mode: D1Mode,
    max_response_body: usize,
    max_error_body: usize,
}

impl D1Store {
    // ========== KV-mode constructors ==========

    /// KV/query/rate-limit/blob store at production Cloudflare API.
    #[must_use]
    pub fn new_kv(
        api_token: &str,
        account_id: &str,
        database_id: &str,
        table_prefix: &str,
    ) -> Self {
        Self::new_kv_with_base_url(
            api_token,
            account_id,
            database_id,
            table_prefix,
            &hosted_api_prefix(account_id),
        )
    }

    /// KV store with a custom API prefix: everything before `/d1/database/...`.
    /// For the hosted API that is [`hosted_api_prefix`]; for wrangler's local
    /// explorer API it is `http://127.0.0.1:<port>/cdn-cgi/local/explorer/api`
    /// (no account segment). `account_id` is not used in the URL then.
    #[must_use]
    pub fn new_kv_with_base_url(
        api_token: &str,
        _account_id: &str,
        database_id: &str,
        table_prefix: &str,
        base_url: &str,
    ) -> Self {
        Self {
            api_token: ZeroizingString::from_string(api_token.to_string()),
            database_id: database_id.to_string(),
            api_prefix: base_url.trim_end_matches('/').to_string(),
            client: HttpClientBuilder::new().build(),
            mode: D1Mode::KeyValue {
                kv_table: format!("{table_prefix}_kv"),
            },
            max_response_body: DEFAULT_MAX_RESPONSE_BODY,
            max_error_body: DEFAULT_MAX_ERROR_BODY,
        }
    }

    /// KV store from environment.
    ///
    /// # Errors
    ///
    /// Returns [`StorageError::Connection`] if a required environment
    /// variable is unset.
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

    /// `StateStore` at production Cloudflare API.
    #[must_use]
    pub fn new_state(
        api_token: &str,
        account_id: &str,
        database_id: &str,
        project: &str,
        stage: &str,
    ) -> Self {
        Self::new_state_with_base_url(
            api_token,
            account_id,
            database_id,
            project,
            stage,
            &hosted_api_prefix(account_id),
        )
    }

    /// `StateStore` with a custom API prefix (see
    /// [`new_kv_with_base_url`](Self::new_kv_with_base_url)).
    #[must_use]
    pub fn new_state_with_base_url(
        api_token: &str,
        _account_id: &str,
        database_id: &str,
        project: &str,
        stage: &str,
        base_url: &str,
    ) -> Self {
        let state_table = format!(
            "{}_{}_resources",
            project.replace(['-', ' '], "_"),
            stage.replace(['-', ' '], "_"),
        );
        Self {
            api_token: ZeroizingString::from_string(api_token.to_string()),
            database_id: database_id.to_string(),
            api_prefix: base_url.trim_end_matches('/').to_string(),
            client: HttpClientBuilder::new().build(),
            mode: D1Mode::State { state_table },
            max_response_body: DEFAULT_MAX_RESPONSE_BODY,
            max_error_body: DEFAULT_MAX_ERROR_BODY,
        }
    }

    /// `StateStore` from environment.
    ///
    /// # Errors
    ///
    /// Returns [`StorageError::Connection`] if a required environment
    /// variable is unset.
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

    // ========== Limits ==========

    /// Largest successful `/query` response body, in bytes, that the store
    /// reads. A larger body is a [`StorageError::Backend`] error, never a
    /// partial parse. Default: [`DEFAULT_MAX_RESPONSE_BODY`].
    #[must_use]
    pub fn with_max_response_body(mut self, bytes: usize) -> Self {
        self.max_response_body = bytes;
        self
    }

    /// Most bytes of an error response body that go into the error message.
    /// The rest is not read. Default: [`DEFAULT_MAX_ERROR_BODY`].
    #[must_use]
    pub fn with_max_error_body(mut self, bytes: usize) -> Self {
        self.max_error_body = bytes;
        self
    }

    // ========== Shared helpers ==========

    /// `POST` target for SQL. `/raw` returns the column names once and the rows
    /// as ordered arrays, so column order is kept; the local explorer API serves
    /// only `/raw`.
    fn raw_url(&self) -> String {
        format!("{}/d1/database/{}/raw", self.api_prefix, self.database_id)
    }

    fn auth_header(&self) -> String {
        format!("Bearer {}", self.api_token.as_str())
    }

    /// Run one SQL statement through the D1 HTTP API and return the parsed
    /// response.
    async fn execute_sql_async(
        &self,
        sql: &str,
        params: &[serde_json::Value],
    ) -> Result<D1RawResponse, StorageError> {
        let typed = params
            .iter()
            .map(|v| match v {
                serde_json::Value::Null => Ok(DataValue::Null),
                serde_json::Value::String(t) => Ok(DataValue::Text(t.clone())),
                other => Err(StorageError::SqlConversion(format!(
                    "unsupported D1 parameter {other}"
                ))),
            })
            .collect::<Result<Vec<_>, _>>()?;
        self.execute_typed(sql, &typed).await
    }

    /// Run one statement with typed values: each value is sent as a string and
    /// typed in the SQL (see [`typed_statement`]).
    async fn execute_typed(
        &self,
        sql: &str,
        params: &[DataValue],
    ) -> Result<D1RawResponse, StorageError> {
        let (sql, strings) = typed_statement(sql, params)?;
        self.post_query(&D1BatchQuery::D1SingleQuery(D1SingleQuery {
            sql,
            params: Some(strings),
        }))
        .await
    }

    /// POST a generated `/query` request body (`send_async`, no blocking) and
    /// return the parsed response. A single query and a batch share this path:
    /// the body type says which one it is.
    async fn post_query(&self, query: &D1BatchQuery) -> Result<D1RawResponse, StorageError> {
        let body = serde_json::to_string(query)
            .map_err(|e| StorageError::Serialization(format!("D1 request encode failed: {e}")))?;
        let response = self
            .client
            .send_async(
                PreparedRequestBuilder::post(&self.raw_url())
                    .map_err(|e| StorageError::Backend(format!("D1 request build failed: {e}")))?
                    .header(SimpleHeader::AUTHORIZATION, self.auth_header())
                    .header(SimpleHeader::CONTENT_TYPE, "application/json")
                    .body_text(body)
                    .build(),
            )
            .await
            .map_err(|e| StorageError::Backend(format!("D1 request failed: {e}")))?;

        let status = response.get_status();
        let (_, _, mut body) = response.into_async_parts();

        if status != Status::OK {
            // Status first; the body only explains the error, so read a bounded part.
            let mut shown = Vec::new();
            while shown.len() < self.max_error_body {
                match body.next().await {
                    Some(Ok(chunk)) => {
                        let room = self.max_error_body - shown.len();
                        shown.extend_from_slice(&chunk[..chunk.len().min(room)]);
                    }
                    Some(Err(_)) | None => break,
                }
            }
            return Err(StorageError::Backend(format!(
                "D1 query failed with status {status}: {}",
                String::from_utf8_lossy(&shown)
            )));
        }

        // Read the body as a stream, stopping as soon as it passes the cap.
        let mut text = Vec::new();
        while let Some(chunk) = body.next().await {
            let chunk =
                chunk.map_err(|e| StorageError::Backend(format!("D1 body read failed: {e}")))?;
            if text.len().saturating_add(chunk.len()) > self.max_response_body {
                return Err(StorageError::Backend(format!(
                    "D1 response is larger than max_response_body ({} bytes)",
                    self.max_response_body
                )));
            }
            text.extend_from_slice(&chunk);
        }

        serde_json::from_slice::<D1RawResponse>(&text)
            .map_err(|e| StorageError::Serialization(format!("D1 response parse failed: {e}")))
    }

    /// The columns and rows of the first statement's result, in the order the
    /// server returned them.
    fn raw_rows(response: &D1RawResponse) -> (Vec<String>, Vec<Vec<serde_json::Value>>) {
        let Some(results) = response
            .result
            .first()
            .and_then(|statement| statement.results.as_ref())
        else {
            return (Vec::new(), Vec::new());
        };
        let columns = results
            .get("columns")
            .and_then(serde_json::Value::as_array)
            .map_or_default(|c| {
                c.iter()
                    .map(|n| n.as_str().unwrap_or_default().to_string())
                    .collect()
            });
        let rows = results
            .get("rows")
            .and_then(serde_json::Value::as_array)
            .map_or_default(|r| {
                r.iter()
                    .map(|row| row.as_array().cloned().unwrap_or_default())
                    .collect()
            });
        (columns, rows)
    }

    /// The rows as JSON objects keyed by column name, for the stores that read
    /// a few named columns (the order of the keys does not matter there).
    fn extract_rows(response: &D1RawResponse) -> Vec<serde_json::Value> {
        let (columns, rows) = Self::raw_rows(response);
        rows.into_iter()
            .map(|row| {
                serde_json::Value::Object(
                    columns
                        .iter()
                        .cloned()
                        .zip(
                            row.into_iter()
                                .chain(std::iter::repeat(serde_json::Value::Null)),
                        )
                        .collect(),
                )
            })
            .collect()
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

    /// The D1 API returns a query's rows in one JSON body; stream them,
    /// decoding each row as it is pulled.
    fn state_rows(response: &D1RawResponse) -> StateStoreStream<ResourceState> {
        Box::pin(stream::iter(Self::extract_rows(response)).map(|row| parse_d1_row(&row)))
    }

    fn id_rows(response: &D1RawResponse) -> StateStoreStream<String> {
        Box::pin(stream::iter(Self::extract_rows(response)).map(|row| {
            row.get("id")
                .and_then(serde_json::Value::as_str)
                .map(String::from)
                .ok_or_else(|| StorageError::SqlConversion("missing id field".to_string()))
        }))
    }

    fn count_row(response: &D1RawResponse) -> StorageResult<usize> {
        let rows = Self::extract_rows(response);
        let Some(row) = rows.first() else {
            return Ok(0);
        };
        let n = row
            .get("cnt")
            .and_then(serde_json::Value::as_u64)
            .ok_or_else(|| StorageError::SqlConversion("missing cnt field".to_string()))?;
        usize::try_from(n).map_err(|e| StorageError::SqlConversion(format!("count: {e}")))
    }

    async fn state_query(
        &self,
        sql: &str,
        params: &[serde_json::Value],
    ) -> StorageResult<StateStoreStream<ResourceState>> {
        let response = self.execute_sql_async(sql, params).await?;
        Ok(Self::state_rows(&response))
    }

    // ========== KV init ==========

    /// Create the KV table (`{prefix}_kv`) if it does not exist.
    ///
    /// # Errors
    ///
    /// Returns a `StorageError` if the D1 request fails.
    pub async fn init_kv(&self) -> StorageResult<()> {
        let sql = format!(
            "CREATE TABLE IF NOT EXISTS {} (key TEXT PRIMARY KEY, value TEXT NOT NULL, created_at INTEGER DEFAULT (strftime('%s', 'now') * 1000), updated_at INTEGER DEFAULT (strftime('%s', 'now') * 1000))",
            self.kv_table()
        );
        self.execute_sql_async(&sql, &[]).await?;
        Ok(())
    }
}

// ===========================================================================
// StateStore
// ===========================================================================

#[async_trait::async_trait]
impl StateStore for D1Store {
    async fn init(&self) -> StorageResult<()> {
        let table = self.state_table();
        let sql = format!(
            "{};\n{}",
            state_create_table_sql(table),
            state_create_indexes_sql(table)
        );
        self.execute_sql_async(&sql, &[]).await?;
        Ok(())
    }

    async fn list(&self) -> StorageResult<StateStoreStream<String>> {
        let sql = format!("SELECT id FROM {} ORDER BY id", self.state_table());
        Ok(Self::id_rows(&self.execute_sql_async(&sql, &[]).await?))
    }

    async fn count(&self) -> StorageResult<usize> {
        let sql = format!("SELECT COUNT(*) as cnt FROM {}", self.state_table());
        Self::count_row(&self.execute_sql_async(&sql, &[]).await?)
    }

    async fn get(&self, resource_id: &str) -> StorageResult<Option<ResourceState>> {
        let sql = format!(
            "SELECT {D1_STATE_COLUMNS} FROM {} WHERE id = ?",
            self.state_table()
        );
        let response = self
            .execute_sql_async(&sql, &[serde_json::Value::String(resource_id.to_string())])
            .await?;
        Self::extract_rows(&response)
            .first()
            .map(parse_d1_row)
            .transpose()
    }

    async fn get_batch(&self, ids: &[&str]) -> StorageResult<StateStoreStream<ResourceState>> {
        if ids.is_empty() {
            return Ok(Box::pin(stream::empty()));
        }
        let placeholders = vec!["?"; ids.len()].join(",");
        let sql = format!(
            "SELECT {D1_STATE_COLUMNS} FROM {} WHERE id IN ({placeholders})",
            self.state_table()
        );
        let params: Vec<serde_json::Value> = ids
            .iter()
            .map(|id| serde_json::Value::String((*id).to_string()))
            .collect();
        self.state_query(&sql, &params).await
    }

    async fn all(&self) -> StorageResult<StateStoreStream<ResourceState>> {
        let sql = format!(
            "SELECT {D1_STATE_COLUMNS} FROM {} ORDER BY id",
            self.state_table()
        );
        self.state_query(&sql, &[]).await
    }

    async fn set(&self, resource_id: &str, state: &ResourceState) -> StorageResult<()> {
        let state = state.with_id(resource_id);
        let params = state_to_params(&state)?;
        self.execute_sql_async(&state_upsert_sql(self.state_table()), &params)
            .await?;
        Ok(())
    }

    async fn set_if_version(
        &self,
        resource_id: &str,
        state: &ResourceState,
        expected: Option<chrono::DateTime<chrono::Utc>>,
    ) -> StorageResult<bool> {
        // One statement: D1 runs it atomically, and `meta.changes` says whether
        // the row was written.
        let state = state.with_id(resource_id);
        let mut params = state_to_params(&state)?;
        let (sql, params) = match expected {
            None => (state_insert_if_absent_sql(self.state_table()), params),
            Some(version) => {
                let updated_at = params.pop().unwrap_or(serde_json::Value::Null);
                params.pop(); // created_at is not changed
                let id = params.remove(0);
                params.push(updated_at);
                params.push(id);
                params.push(serde_json::Value::String(version.to_rfc3339()));
                (state_update_if_version_sql(self.state_table()), params)
            }
        };
        let response = self.execute_sql_async(&sql, &params).await?;
        let changes = response
            .result
            .first()
            .and_then(|statement| statement.meta.as_ref())
            .and_then(|meta| meta.changes)
            .map_or(0, i64::unsigned_abs);
        Ok(changes == 1)
    }

    async fn delete(&self, resource_id: &str) -> StorageResult<()> {
        let sql = format!("DELETE FROM {} WHERE id = ?", self.state_table());
        self.execute_sql_async(&sql, &[serde_json::Value::String(resource_id.to_string())])
            .await?;
        Ok(())
    }

    async fn list_by_prefix(&self, prefix: &str) -> StorageResult<StateStoreStream<String>> {
        let sql = format!(
            "SELECT id FROM {} WHERE id LIKE ?||'%' ESCAPE '\\' ORDER BY id",
            self.state_table()
        );
        let response = self
            .execute_sql_async(&sql, &[serde_json::Value::String(escape_like(prefix))])
            .await?;
        Ok(Self::id_rows(&response))
    }

    async fn count_by_prefix(&self, prefix: &str) -> StorageResult<usize> {
        let sql = format!(
            "SELECT COUNT(*) as cnt FROM {} WHERE id LIKE ?||'%' ESCAPE '\\'",
            self.state_table()
        );
        let response = self
            .execute_sql_async(&sql, &[serde_json::Value::String(escape_like(prefix))])
            .await?;
        Self::count_row(&response)
    }

    async fn all_by_prefix(&self, prefix: &str) -> StorageResult<StateStoreStream<ResourceState>> {
        let sql = format!(
            "SELECT {D1_STATE_COLUMNS} FROM {} WHERE id LIKE ?||'%' ESCAPE '\\' ORDER BY id",
            self.state_table()
        );
        self.state_query(&sql, &[serde_json::Value::String(escape_like(prefix))])
            .await
    }

    async fn find_by_kind(&self, kind: &str) -> StorageResult<StateStoreStream<ResourceState>> {
        let sql = format!(
            "SELECT {D1_STATE_COLUMNS} FROM {} WHERE kind = ? ORDER BY id",
            self.state_table()
        );
        self.state_query(&sql, &[serde_json::Value::String(kind.to_string())])
            .await
    }

    async fn find_by_status(&self, status: &str) -> StorageResult<StateStoreStream<ResourceState>> {
        let sql = format!(
            "SELECT {D1_STATE_COLUMNS} FROM {} WHERE status = ? ORDER BY id",
            self.state_table()
        );
        let status =
            status_column_value(status).map_err(|e| StorageError::Serialization(e.to_string()))?;
        self.state_query(&sql, &[serde_json::Value::String(status)])
            .await
    }

    async fn find_by_provider(
        &self,
        provider: &str,
    ) -> StorageResult<StateStoreStream<ResourceState>> {
        let sql = format!(
            "SELECT {D1_STATE_COLUMNS} FROM {} WHERE provider = ? ORDER BY id",
            self.state_table()
        );
        self.state_query(&sql, &[serde_json::Value::String(provider.to_string())])
            .await
    }

    async fn delete_by_prefix(&self, prefix: &str) -> StorageResult<usize> {
        let sql = format!(
            "DELETE FROM {} WHERE id LIKE ?||'%' ESCAPE '\\'",
            self.state_table()
        );
        let response = self
            .execute_sql_async(&sql, &[serde_json::Value::String(escape_like(prefix))])
            .await?;
        let n = response
            .result
            .first()
            .and_then(|statement| statement.meta.as_ref())
            .and_then(|meta| meta.rows_written)
            .ok_or_else(|| StorageError::SqlConversion("missing rows_written".to_string()))?;
        usize::try_from(n.unsigned_abs())
            .map_err(|e| StorageError::SqlConversion(format!("rows_written: {e}")))
    }
}

/// The `ResourceState` columns, in `parse_d1_row`'s order.
const D1_STATE_COLUMNS: &str =
    "id, kind, provider, status, environment, config_hash, output, config_snapshot, created_at, updated_at";

// ===========================================================================
// Async trait implementations
// ===========================================================================

#[async_trait::async_trait]
impl KeyValueStore for D1Store {
    async fn get<V: DeserializeOwned + Send + 'static>(
        &self,
        key: &str,
    ) -> StorageResult<Option<V>> {
        let sql = format!("SELECT value FROM {} WHERE key = ?", self.kv_table());
        let response = self
            .execute_sql_async(&sql, &[serde_json::Value::String(key.to_string())])
            .await?;
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
                Ok(Some(
                    serde_json::from_str(&value)
                        .map_err(|e| StorageError::Serialization(e.to_string()))?,
                ))
            }
            None => Ok(None),
        }
    }

    async fn set<V: Serialize + Send + 'static>(&self, key: &str, value: V) -> StorageResult<()> {
        let json_value = serde_json::to_string(&value)
            .map_err(|e| StorageError::Serialization(e.to_string()))?;
        let sql = format!(
            "INSERT INTO {} (key, value, updated_at) VALUES (?, ?, strftime('%s', 'now') * 1000) ON CONFLICT(key) DO UPDATE SET value = ?, updated_at = strftime('%s', 'now') * 1000",
            self.kv_table()
        );
        let kv = serde_json::Value::String(key.to_string());
        let jv = serde_json::Value::String(json_value);
        self.execute_sql_async(&sql, &[kv.clone(), jv.clone(), jv])
            .await?;
        Ok(())
    }

    async fn delete(&self, key: &str) -> StorageResult<()> {
        let sql = format!("DELETE FROM {} WHERE key = ?", self.kv_table());
        self.execute_sql_async(&sql, &[serde_json::Value::String(key.to_string())])
            .await?;
        Ok(())
    }

    fn take_is_atomic(&self) -> bool {
        true
    }

    async fn take<V: DeserializeOwned + Send + 'static>(
        &self,
        key: &str,
    ) -> StorageResult<Option<V>> {
        // One statement: D1 runs it atomically, so only one caller gets the row.
        let sql = format!(
            "DELETE FROM {} WHERE key = ? RETURNING value",
            self.kv_table()
        );
        let response = self
            .execute_sql_async(&sql, &[serde_json::Value::String(key.to_string())])
            .await?;
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
                Ok(Some(
                    serde_json::from_str(&value)
                        .map_err(|e| StorageError::Serialization(e.to_string()))?,
                ))
            }
            None => Ok(None),
        }
    }

    async fn exists(&self, key: &str) -> StorageResult<bool> {
        let sql = format!("SELECT 1 FROM {} WHERE key = ? LIMIT 1", self.kv_table());
        let response = self
            .execute_sql_async(&sql, &[serde_json::Value::String(key.to_string())])
            .await?;
        Ok(!Self::extract_rows(&response).is_empty())
    }

    async fn list_keys(&self, prefix: Option<&str>) -> StorageResult<AsyncListStream> {
        let (sql, params) = match prefix {
            Some(p) => (
                format!(
                    "SELECT key FROM {} WHERE key LIKE ? ORDER BY key",
                    self.kv_table()
                ),
                vec![serde_json::Value::String(format!("{p}%"))],
            ),
            None => (
                format!("SELECT key FROM {} ORDER BY key", self.kv_table()),
                vec![],
            ),
        };
        let response = self.execute_sql_async(&sql, &params).await?;
        let keys: Vec<String> = Self::extract_rows(&response)
            .iter()
            .map(|row| {
                row.get("key")
                    .and_then(serde_json::Value::as_str)
                    .map(String::from)
                    .ok_or_else(|| {
                        StorageError::SqlConversion("missing or invalid key field".to_string())
                    })
            })
            .collect::<Result<Vec<_>, _>>()?;
        Ok(AsyncListStream::new(stream::iter(keys.into_iter().map(Ok))))
    }
}

#[async_trait::async_trait]
impl QueryStore for D1Store {
    async fn query(&self, sql: &str, params: &[DataValue]) -> StorageResult<AsyncQueryStream> {
        let response = self.execute_typed(sql, params).await?;
        let (columns, raw_rows) = Self::raw_rows(&response);
        // Columns keep the server's order and duplicates (a SELECT can name a
        // column twice); this is what a JSON object per row used to lose.
        let rows: Vec<SqlRow> = raw_rows
            .into_iter()
            .map(|row| {
                let cells: Vec<(String, DataValue)> = columns
                    .iter()
                    .zip(row)
                    .map(|(name, v)| {
                        let dv = match &v {
                            serde_json::Value::Number(n) => {
                                if let Some(i) = n.as_i64() {
                                    DataValue::Integer(i)
                                } else if let Some(f) = n.as_f64() {
                                    DataValue::Real(f)
                                } else {
                                    DataValue::Null
                                }
                            }
                            serde_json::Value::String(t) => DataValue::Text(t.clone()),
                            _ => DataValue::Null,
                        };
                        (name.clone(), dv)
                    })
                    .collect();
                SqlRow::new(cells)
            })
            .collect();
        Ok(AsyncQueryStream::new(stream::iter(
            rows.into_iter().map(Ok),
        )))
    }

    async fn execute(&self, sql: &str, params: &[DataValue]) -> StorageResult<u64> {
        let response = self.execute_typed(sql, params).await?;
        let affected = response
            .result
            .first()
            .and_then(|statement| statement.meta.as_ref())
            .and_then(|meta| meta.changes)
            .map_or(0, i64::unsigned_abs);
        Ok(affected)
    }

    async fn execute_batch(&self, sql: &str) -> StorageResult<()> {
        self.execute_sql_async(sql, &[]).await?;
        Ok(())
    }

    async fn execute_batch_statements(
        &self,
        statements: &[(String, Vec<DataValue>)],
    ) -> StorageResult<()> {
        if statements.is_empty() {
            return Ok(());
        }
        // One `/query` call with a `batch` of statements, each with its own
        // parameter list (numbered `?1` parameters restart per statement).
        // Cloudflare runs a batch as one transaction: if a statement fails, the
        // whole batch rolls back.
        let batch = statements
            .iter()
            .map(|(sql, params)| {
                let (sql, strings) = typed_statement(sql, params)?;
                Ok(D1SingleQuery {
                    sql,
                    params: Some(strings),
                })
            })
            .collect::<Result<Vec<_>, StorageError>>()?;
        let response = self
            .post_query(&D1BatchQuery::MultipleQueries { batch })
            .await?;
        let any_failed = !response.success
            || !response.errors.is_empty()
            || response.result.len() != statements.len()
            || response
                .result
                .iter()
                .any(|stmt| stmt.success != Some(true));
        if any_failed {
            return Err(StorageError::Backend(
                "D1 batch: one or more statements failed".to_string(),
            ));
        }
        Ok(())
    }
}

#[async_trait::async_trait]
impl RateLimiterStore for D1Store {
    async fn check_rate_limit(
        &self,
        ctx: &Ctx,
        key: &str,
        max_count: u32,
        window_seconds: u64,
    ) -> StorageResult<bool> {
        rate_limit::sql_check(self, ctx, key, max_count, window_seconds).await
    }

    async fn record_rate_limit(
        &self,
        ctx: &Ctx,
        key: &str,
        window_seconds: u64,
    ) -> StorageResult<u32> {
        rate_limit::sql_record(self, ctx, key, window_seconds).await
    }

    async fn reset_rate_limit(&self, key: &str) -> StorageResult<()> {
        rate_limit::sql_reset(self, key).await
    }
}

#[async_trait::async_trait]
impl BlobStore for D1Store {
    async fn put_blob(&self, key: &str, data: &[u8]) -> StorageResult<()> {
        let encoded = STANDARD.encode(data);
        let json_value =
            serde_json::json!({ "type": "blob", "encoding": "base64", "data": encoded })
                .to_string();
        let sql = format!(
            "INSERT INTO {} (key, value, updated_at) VALUES (?, ?, strftime('%s', 'now') * 1000) ON CONFLICT(key) DO UPDATE SET value = ?, updated_at = strftime('%s', 'now') * 1000",
            self.kv_table()
        );
        let kv = serde_json::Value::String(key.to_string());
        let jv = serde_json::Value::String(json_value);
        self.execute_sql_async(&sql, &[kv.clone(), jv.clone(), jv])
            .await?;
        Ok(())
    }

    async fn get_blob(&self, key: &str) -> StorageResult<Option<Vec<u8>>> {
        let sql = format!("SELECT value FROM {} WHERE key = ?", self.kv_table());
        let response = self
            .execute_sql_async(&sql, &[serde_json::Value::String(key.to_string())])
            .await?;
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
                let wrapper: serde_json::Value = serde_json::from_str(&value)
                    .map_err(|e| StorageError::Serialization(e.to_string()))?;
                if wrapper.get("type").and_then(serde_json::Value::as_str) != Some("blob") {
                    return Ok(None);
                }
                let encoded = wrapper
                    .get("data")
                    .and_then(serde_json::Value::as_str)
                    .ok_or_else(|| {
                        StorageError::Serialization(
                            "missing data field in blob wrapper".to_string(),
                        )
                    })?;
                Ok(Some(STANDARD.decode(encoded).map_err(|e| {
                    StorageError::Backend(format!("Base64 decode failed: {e}"))
                })?))
            }
            None => Ok(None),
        }
    }

    async fn delete_blob(&self, key: &str) -> StorageResult<()> {
        let sql = format!("DELETE FROM {} WHERE key = ?", self.kv_table());
        self.execute_sql_async(&sql, &[serde_json::Value::String(key.to_string())])
            .await?;
        Ok(())
    }

    async fn blob_exists(&self, key: &str) -> StorageResult<bool> {
        let sql = format!("SELECT 1 FROM {} WHERE key = ? LIMIT 1", self.kv_table());
        let response = self
            .execute_sql_async(&sql, &[serde_json::Value::String(key.to_string())])
            .await?;
        Ok(!Self::extract_rows(&response).is_empty())
    }

    async fn list_blobs_after(
        &self,
        prefix: &str,
        start_after: Option<&str>,
        limit: Option<usize>,
    ) -> StorageResult<BlobListStream> {
        // `list_keys` matches with SQL `LIKE`; `%` and `_` in blob keys widen
        // that match, so keep the literal prefix here. `list_keys` already
        // sorts (`ORDER BY key`).
        let mut keys = self.list_keys(Some(prefix)).await?.collect_all().await?;
        keys.retain(|k| k.starts_with(prefix) && start_after.is_none_or(|s| k.as_str() > s));
        if let Some(limit) = limit {
            keys.truncate(limit);
        }
        Ok(Box::pin(stream::iter(keys.into_iter().map(Ok))))
    }
}
