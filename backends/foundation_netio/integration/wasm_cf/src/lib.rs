//! Test support for the Durable Object stub client under the wasm testbed.
//!
//! WHY: `StubHttpClient` calls a Durable Object through `worker::Stub`, which
//! needs the `env` object that only a Worker receives.
//! WHAT: read the bindings from `globalThis.__CF_TEST_ENV__`, which the testbed
//! worker sets before it runs the tests.
//! HOW: `foundation_testbed`'s `bindgen-wrangler` worker assigns `env` to that
//! global; [`test_env`] casts it to `worker::Env`.

use wasm_bindgen::{JsCast, JsValue};

/// The worker `env` object, set by the testbed worker.
///
/// # Panics
///
/// Panics when the test does not run inside the testbed worker (the global is
/// unset).
#[must_use]
pub fn test_env() -> worker::Env {
    js_sys::Reflect::get(&js_sys::global(), &JsValue::from_str("__CF_TEST_ENV__"))
        .expect("the testbed worker sets globalThis.__CF_TEST_ENV__")
        .unchecked_into()
}
