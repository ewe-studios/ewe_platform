//! Test support for the Cloudflare wasm stores under the wasm testbed.
//!
//! WHY: the D1, R2 and KV stores call the real Workers bindings, so they need
//! the `env` object that only the worker gets.
//! WHAT: read the bindings from `globalThis.__CF_TEST_ENV__`, which the testbed
//! worker sets before it runs the tests.
//! HOW: `foundation_testbed`'s `bindgen-wrangler` worker assigns `env` to that
//! global; each helper extracts one binding from it.

use wasm_bindgen::JsValue;

use foundation_db::{D1Database, KVNamespace, R2Bucket};

/// The worker `env` object, set by the testbed worker.
///
/// # Panics
///
/// Panics when the test does not run inside the testbed worker (the global is
/// unset).
#[must_use]
pub fn test_env() -> JsValue {
    js_sys::Reflect::get(&js_sys::global(), &JsValue::from_str("__CF_TEST_ENV__"))
        .expect("the testbed worker sets globalThis.__CF_TEST_ENV__")
}

/// The D1 binding named `binding` (the template declares `DB`).
///
/// # Panics
///
/// Panics when the binding is missing or is not a D1 database.
#[must_use]
pub fn d1(binding: &str) -> D1Database {
    D1Database::from_env(&test_env(), binding).expect("D1 binding")
}

/// The R2 binding named `binding` (the template declares `BUCKET`).
///
/// # Panics
///
/// Panics when the binding is missing or is not an R2 bucket.
#[must_use]
pub fn r2(binding: &str) -> R2Bucket {
    R2Bucket::from_env(&test_env(), binding).expect("R2 binding")
}

/// The KV binding named `binding` (the template declares `KV`).
///
/// # Panics
///
/// Panics when the binding is missing or is not a KV namespace.
#[must_use]
pub fn kv(binding: &str) -> KVNamespace {
    KVNamespace::from_env(&test_env(), binding).expect("KV binding")
}
