//! KV namespace bindings — Cloudflare Workers.

use js_sys::Object;
use wasm_bindgen::prelude::*;

use super::get_binding;

#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(extends = Object)]
    #[derive(Clone, Debug)]
    pub type KVNamespace;

    #[wasm_bindgen(method, js_name = get)]
    pub fn get(this: &KVNamespace, key: &str) -> js_sys::Promise;

    #[wasm_bindgen(method, js_name = put)]
    pub fn put(this: &KVNamespace, key: &str, value: &JsValue) -> js_sys::Promise;

    #[wasm_bindgen(method, js_name = delete)]
    pub fn delete(this: &KVNamespace, key: &str) -> js_sys::Promise;

    #[wasm_bindgen(method, js_name = list)]
    pub fn list(this: &KVNamespace, opts: &JsValue) -> js_sys::Promise;
}

impl KVNamespace {
    /// Extract a KVNamespace from the CF env object.
    pub fn from_env(env: &JsValue, binding: &str) -> Result<Self, JsError> {
        get_binding(env, binding)
    }
}
