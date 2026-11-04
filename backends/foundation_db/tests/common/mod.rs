//! Shared test utilities for Cloudflare D1/R2 integration tests.
//!
//! WHY: Both the D1 and R2 integration suites need to agree on where the
//! local Cloudflare API is, which database and bucket to use, and how to decide
//! whether a test should run or skip.
//!
//! WHAT: The API is **wrangler's local explorer API** (`wrangler dev --local`,
//! miniflare), served at `/cdn-cgi/local/explorer/api` and backed by the same
//! local D1 and R2 storage as the bindings. It is not the hosted front end
//! (`api.cloudflare.com`): the paths have no `/accounts/{id}` segment, and R2
//! has no range read and no conditional put there. The hosted API follows the
//! same published spec (string params), but no test here runs against it. Those
//! R2 features are tested on the real binding, in the wasm testbed.
//!
//! The explorer API is beta and may change, so the wrangler version is pinned in
//! the test tooling (`WRANGLER_VERSION` in `mise.toml`).
//!
//! HOW: Reads `LOCAL_CF_API_BASE` (default
//! `http://localhost:8789/cdn-cgi/local/explorer/api`, the whole prefix) and
//! `CF_INTEGRATION_TEST=1` to opt in. Both D1 and R2 tests share the prefix.

#![allow(dead_code)]

use foundation_db::{D1Store, R2Store};

/// Env var used to opt into Cloudflare integration tests.
pub const ENV_ENABLE: &str = "CF_INTEGRATION_TEST";

/// Env var that holds the API prefix of the local explorer API (single source of truth).
pub const ENV_BASE_URL: &str = "LOCAL_CF_API_BASE";

/// Env var overriding the local D1 database id (the binding's `preview_database_id` in `wrangler.toml`).
pub const ENV_D1_DB_ID: &str = "LOCAL_D1_DATABASE_ID";

/// Env var overriding the local R2 bucket name (the binding's `preview_bucket_name` in `wrangler.toml`).
pub const ENV_R2_BUCKET: &str = "LOCAL_R2_BUCKET";

/// Default API prefix when [`ENV_BASE_URL`] is unset.
pub const DEFAULT_BASE_URL: &str = "http://localhost:8789/cdn-cgi/local/explorer/api";

fn env_or(name: &str, default: &str) -> String {
    std::env::var(name).unwrap_or_else(|_| default.to_string())
}

/// Return the API prefix of wrangler's local explorer API.
pub fn local_cf_api_base() -> String {
    env_or(ENV_BASE_URL, DEFAULT_BASE_URL)
}

/// Check whether integration tests are enabled and the local worker is reachable.
pub fn is_local_cf_available() -> bool {
    if std::env::var(ENV_ENABLE).ok().as_deref() != Some("1") {
        return false;
    }

    let base = local_cf_api_base();
    let response = std::process::Command::new("curl")
        .args([
            "-s",
            "-o",
            "/dev/null",
            "-w",
            "%{http_code}",
            &format!("{base}/d1/database"),
        ])
        .output();

    match response {
        Ok(output) => {
            let status = String::from_utf8_lossy(&output.stdout);
            let status = status.trim();
            status == "200"
        }
        Err(_) => false,
    }
}

/// Initialize the Valtron executor for tests.
/// Uses a shared Mutex<Option<PoolGuard>> so that parallel tests share the same
/// pool instead of each creating and tearing down their own.
static POOL_GUARD: std::sync::Mutex<Option<foundation_core::valtron::PoolGuard>> =
    std::sync::Mutex::new(None);

pub fn init_valtron() {
    let mut guard = POOL_GUARD.lock().unwrap();
    if guard.is_none() {
        *guard = Some(foundation_core::valtron::initialize_pool(42, None));
    }
}

/// Build a `D1KeyValueStore` pointed at the local worker, or return `None`
/// when the integration environment is not available.
pub fn make_d1_store() -> Option<D1Store> {
    if !is_local_cf_available() {
        return None;
    }
    let db_id = env_or(ENV_D1_DB_ID, "test-db-preview");
    Some(D1Store::new_kv_with_base_url(
        "test-token",
        "test-account",
        &db_id,
        "test",
        &local_cf_api_base(),
    ))
}

/// Build an `R2BlobStore` pointed at the local worker, or return `None`
/// when the integration environment is not available.
pub fn make_r2_store() -> Option<R2Store> {
    if !is_local_cf_available() {
        return None;
    }
    let bucket = env_or(ENV_R2_BUCKET, "test-bucket-preview");
    Some(R2Store::new_blob_with_base_url(
        "test-token",
        "test-account",
        &bucket,
        "test",
        &local_cf_api_base(),
    ))
}
