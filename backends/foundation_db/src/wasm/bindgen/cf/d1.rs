//! D1 database bindings — Cloudflare Workers.

use js_sys::{Array, Object, Promise};
use wasm_bindgen::prelude::*;

use super::get_binding;

#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(extends = Object)]
    #[derive(Clone, Debug)]
    pub type D1Database;

    #[wasm_bindgen(method, js_name = prepare)]
    pub fn prepare(this: &D1Database, query: &str) -> D1PreparedStatement;
}

impl D1Database {
    /// Extract a D1Database from the CF env object.
    pub fn from_env(env: &JsValue, binding: &str) -> Result<Self, JsError> {
        get_binding(env, binding)
    }
}

#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(extends = Object)]
    #[derive(Clone, Debug)]
    pub type D1PreparedStatement;

    #[wasm_bindgen(method, js_name = bind)]
    pub fn bind(this: &D1PreparedStatement, values: &Array) -> D1PreparedStatement;

    #[wasm_bindgen(method, js_name = first)]
    pub fn first(this: &D1PreparedStatement, col_name: Option<&str>) -> Promise;

    #[wasm_bindgen(method, js_name = run)]
    pub fn run(this: &D1PreparedStatement) -> Promise;

    #[wasm_bindgen(method, js_name = all)]
    pub fn all(this: &D1PreparedStatement) -> Promise;
}
