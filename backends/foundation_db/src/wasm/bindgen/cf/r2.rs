//! R2 object storage bindings — Cloudflare Workers.

use js_sys::Object;
use wasm_bindgen::prelude::*;

use super::get_binding;

#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(extends = Object)]
    #[derive(Clone, Debug)]
    pub type R2Bucket;

    #[wasm_bindgen(method, js_name = get)]
    pub fn get(this: &R2Bucket, key: &str) -> js_sys::Promise;

    #[wasm_bindgen(method, js_name = put)]
    pub fn put(this: &R2Bucket, key: &str, value: &JsValue) -> js_sys::Promise;

    #[wasm_bindgen(method, js_name = delete)]
    pub fn delete(this: &R2Bucket, key: &str) -> js_sys::Promise;

    #[wasm_bindgen(method, js_name = head)]
    pub fn head(this: &R2Bucket, key: &str) -> js_sys::Promise;

    #[wasm_bindgen(method, js_name = list)]
    pub fn list(this: &R2Bucket, opts: &JsValue) -> js_sys::Promise;
}

impl R2Bucket {
    /// Extract an R2Bucket from the CF env object.
    pub fn from_env(env: &JsValue, binding: &str) -> Result<Self, JsError> {
        get_binding(env, binding)
    }
}
