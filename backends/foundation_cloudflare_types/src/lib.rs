//! Shared Cloudflare D1/R2 wire types.
//!
//! WHY: `foundation_db` parses Cloudflare D1 and R2 REST responses. It cannot
//! depend on `foundation_deployment_cloudflare` for those types: that crate
//! depends on `foundation_deployment`, which depends on `foundation_db`. This
//! leaf crate depends on neither, so both storage crates can use it.
//!
//! WHAT: The D1 `/query` response types and the R2 object-listing response
//! types.
//!
//! HOW: Plain `serde` structs. The D1 types match the generated
//! `foundation_deployment_cloudflare` output exactly. The R2 object-listing
//! endpoint is absent from the vendored Cloudflare `OpenAPI` spec, so those
//! types are defined here by hand to the shape the R2 REST API returns.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// D1 `/query`
// ---------------------------------------------------------------------------

/// One statement's result inside a D1 `/query` response.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct D1QueryResultResponse {
    /// `meta` property.
    pub meta: Option<D1QueryMeta>,
    /// `results` property: the returned rows.
    pub results: Option<Vec<serde_json::Value>>,
    /// `success` property.
    pub success: Option<bool>,
}

/// The `meta` object of a D1 statement result.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct D1QueryMeta {
    /// `changed_db` property.
    pub changed_db: Option<bool>,
    /// `changes` property: the number of changed rows (an integer in D1).
    pub changes: Option<i64>,
    /// `duration` property.
    pub duration: Option<f64>,
    /// `last_row_id` property.
    pub last_row_id: Option<f64>,
    /// `rows_read` property.
    pub rows_read: Option<f64>,
    /// `rows_written` property: the number of written rows (an integer in D1).
    pub rows_written: Option<i64>,
    /// `served_by_colo` property.
    pub served_by_colo: Option<String>,
    /// `served_by_primary` property.
    pub served_by_primary: Option<bool>,
    /// `served_by_region` property.
    pub served_by_region: Option<String>,
    /// `size_after` property.
    pub size_after: Option<f64>,
    /// `timings` property.
    pub timings: Option<HashMap<String, serde_json::Value>>,
}

/// A D1 `/query` response: `result` holds one entry per statement.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct D1QueryDatabaseResponse {
    /// `errors` property.
    pub errors: Vec<HashMap<String, serde_json::Value>>,
    /// `messages` property.
    pub messages: Vec<HashMap<String, serde_json::Value>>,
    /// The per-statement results.
    pub result: Vec<D1QueryResultResponse>,
    /// `success` property.
    pub success: bool,
}

// ---------------------------------------------------------------------------
// R2 object listing
// ---------------------------------------------------------------------------

/// One object in an R2 object-listing page.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct R2ObjectInfo {
    /// `key` property: the object key.
    pub key: String,
    /// `size` property, in bytes.
    pub size: Option<u64>,
    /// `etag` property.
    pub etag: Option<String>,
    /// `uploaded` property: RFC 3339 timestamp.
    pub uploaded: Option<String>,
    /// `version` property.
    pub version: Option<String>,
    /// `http_metadata` property.
    pub http_metadata: Option<HashMap<String, String>>,
    /// `custom_metadata` property.
    pub custom_metadata: Option<HashMap<String, String>>,
}

/// The `result_info` of an R2 object listing.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct R2ListObjectsResultInfo {
    /// `is_truncated` property: true when more pages follow.
    pub is_truncated: Option<bool>,
    /// `cursor` property: the cursor of the next page.
    pub cursor: Option<String>,
}

/// An R2 object-listing response.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct R2ListObjectsResponse {
    /// `errors` property.
    pub errors: Vec<HashMap<String, serde_json::Value>>,
    /// `messages` property.
    pub messages: Vec<HashMap<String, serde_json::Value>>,
    /// `result` property: the objects on this page.
    pub result: Vec<R2ObjectInfo>,
    /// `result_info` property.
    pub result_info: Option<R2ListObjectsResultInfo>,
    /// `success` property.
    pub success: bool,
}
