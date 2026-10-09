//! Typed binding helpers for the Workers `Env`.
//!
//! WHY: `env.d1("NAME")`, `env.secret("NAME")`, etc. return raw `JsValue` /
//! `Result<T, Error>` types. Every Workers crate writes the same 3-line binding
//! extraction. Centralising it here means one place to get it right.
//!
//! WHAT: thin wrappers around `Env::d1`, `Env::secret`, `Env::kv`, `Env::r2`
//! that convert to the `foundation_db` / native Rust types the rest of the
//! platform expects.

use worker::{Env, Result as WorkerResult};

/// Extract a D1 database binding from the Worker environment.
///
/// The binding name must match `[[d1_databases]]` in `wrangler.toml`.
///
/// # Errors
///
/// Returns `worker::Error` if the binding is missing or has the wrong type.
pub fn get_d1(env: &Env, name: &str) -> WorkerResult<worker::D1Database> {
    env.d1(name)
}

/// Extract a KV namespace binding from the Worker environment.
///
/// # Errors
///
/// Returns `worker::Error` if the binding is missing or has the wrong type.
pub fn get_kv(env: &Env, name: &str) -> WorkerResult<worker::KvStore> {
    env.kv(name)
}

/// Extract an R2 bucket binding from the Worker environment.
///
/// # Errors
///
/// Returns `worker::Error` if the binding is missing or has the wrong type.
pub fn get_r2(env: &Env, name: &str) -> WorkerResult<worker::Bucket> {
    env.bucket(name)
}

/// Extract a secret binding from the Worker environment.
///
/// Secrets are set via `wrangler secret put <NAME>` and exposed as env bindings.
///
/// # Errors
///
/// Returns `worker::Error` if the binding is missing.
pub fn get_secret(env: &Env, name: &str) -> WorkerResult<String> {
    let secret = env.secret(name)?;
    Ok(secret.to_string())
}

/// Extract a Durable Object namespace binding from the Worker environment.
///
/// # Errors
///
/// Returns `worker::Error` if the binding is missing.
pub fn get_do_namespace(
    env: &Env,
    name: &str,
) -> WorkerResult<worker::durable::ObjectNamespace> {
    env.durable_object(name)
}
