//! Durable Object (DO) SQLite storage bindings.
//!
//! The Workers Rust library (`workers-rs`) version 0.8 wraps `SqlStorage::exec`
//! but not `DurableObjectStorage::transactionSync`. This module binds the
//! missing synchronous transaction call. The method names and the variadic
//! `exec` shape match the `worker-sys` declarations, which wrap the same
//! JavaScript objects.
//!
//! A statement runs with `?` placeholders and a separate binding array. The
//! code never puts a value into the statement text.

use wasm_bindgen::closure::Closure;
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(extends = js_sys::Object)]
    #[derive(Clone, Debug)]
    pub type DurableObjectStorage;

    /// The SQLite storage handle of this Durable Object.
    #[wasm_bindgen(method, getter, catch)]
    pub fn sql(this: &DurableObjectStorage) -> Result<SqlStorage, JsValue>;

    /// Run `closure` as one atomic SQLite transaction.
    ///
    /// The transaction commits when `closure` returns. It rolls back when
    /// `closure` throws. The call returns the closure result.
    #[wasm_bindgen(method, catch, js_name = transactionSync)]
    pub fn transaction_sync(
        this: &DurableObjectStorage,
        closure: &Closure<dyn FnMut() -> JsValue>,
    ) -> Result<JsValue, JsValue>;
}

/// # Safety: `Send` and `Sync` on wasm32
///
/// The type wraps a JavaScript object, which is not `Send` by default. On
/// wasm32 there is one thread and no shared memory, so a `Send + Sync` claim is
/// true. This lets an executor hold the handle behind `SqlExecutor`, which
/// requires `Send + Sync`.
unsafe impl Send for DurableObjectStorage {}
unsafe impl Sync for DurableObjectStorage {}

#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(extends = js_sys::Object)]
    #[derive(Clone, Debug)]
    pub type SqlStorage;

    /// Run one statement with its bindings and return a cursor over the result.
    ///
    /// The JavaScript method is variadic: the query text, then one argument per
    /// `?` placeholder. The binding array is spread into those arguments.
    #[wasm_bindgen(structural, method, catch, variadic, js_class = SqlStorage, js_name = exec)]
    pub fn exec(
        this: &SqlStorage,
        query: &str,
        bindings: js_sys::Array,
    ) -> Result<SqlStorageCursor, JsValue>;
}

#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(extends = js_sys::Object)]
    #[derive(Clone, Debug)]
    pub type SqlStorageCursor;

    /// Iterate the remaining rows. Each row is an array of columns in query
    /// order.
    #[wasm_bindgen(method, js_name = raw)]
    pub fn raw(this: &SqlStorageCursor) -> js_sys::Iterator;
}
