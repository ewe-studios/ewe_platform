//! Shared test utilities for Cloudflare D1/R2 integration tests.
//!
//! WHY: Both the D1 and R2 integration suites need to agree on where the
//! local wrangler worker is listening, which bucket/database names to use,
//! and how to decide whether the test should run or skip. Keeping that in
//! one place means the mise tasks only need to set a single env var.
//!
//! WHAT: Exposes [`local_cf_api_base`] for the base URL, a skip helper, and
//! store builders that wire the local base URL into `D1KeyValueStore` /
//! `R2BlobStore` via their `with_base_url` constructors.
//!
//! HOW: Reads `LOCAL_CF_API_BASE` (default `http://localhost:8789`) and
//! `CF_INTEGRATION_TEST=1` to opt in. Both D1 and R2 tests share the same
//! base URL — the worker-stub routes by path.

#![allow(dead_code)]

use foundation_db::{D1KeyValueStore, R2BlobStore};

/// Env var used to opt into Cloudflare integration tests.
pub const ENV_ENABLE: &str = "CF_INTEGRATION_TEST";

/// Env var that points at the local wrangler worker (single source of truth).
pub const ENV_BASE_URL: &str = "LOCAL_CF_API_BASE";

/// Env var overriding the local D1 database id (matches `wrangler.toml`).
pub const ENV_D1_DB_ID: &str = "LOCAL_D1_DATABASE_ID";

/// Env var overriding the local R2 bucket name (matches `wrangler.toml`).
pub const ENV_R2_BUCKET: &str = "LOCAL_R2_BUCKET";

/// Default base URL when [`ENV_BASE_URL`] is unset.
pub const DEFAULT_BASE_URL: &str = "http://localhost:8789";

fn env_or(name: &str, default: &str) -> String {
    std::env::var(name).unwrap_or_else(|_| default.to_string())
}

/// Return the base URL of the local Cloudflare emulator.
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
        .args(["-s", "-o", "/dev/null", "-w", "%{http_code}", &base])
        .output();

    match response {
        Ok(output) => {
            let status = String::from_utf8_lossy(&output.stdout);
            let status = status.trim();
            status == "200" || status == "404"
        }
        Err(_) => false,
    }
}

/// Initialize the Valtron executor for tests.
pub fn init_valtron() {
    foundation_core::valtron::single::initialize_pool(42);
}

/// Build a `D1KeyValueStore` pointed at the local worker, or return `None`
/// when the integration environment is not available.
pub fn make_d1_store() -> Option<D1KeyValueStore> {
    if !is_local_cf_available() {
        return None;
    }
    let db_id = env_or(ENV_D1_DB_ID, "test-db");
    Some(D1KeyValueStore::with_base_url(
        "test-token",
        "test-account",
        &db_id,
        "test",
        &local_cf_api_base(),
    ))
}

/// Build an `R2BlobStore` pointed at the local worker, or return `None`
/// when the integration environment is not available.
pub fn make_r2_store() -> Option<R2BlobStore> {
    if !is_local_cf_available() {
        return None;
    }
    let bucket = env_or(ENV_R2_BUCKET, "test-bucket");
    Some(R2BlobStore::with_base_url(
        "test-token",
        "test-account",
        &bucket,
        "test",
        &local_cf_api_base(),
    ))
}
