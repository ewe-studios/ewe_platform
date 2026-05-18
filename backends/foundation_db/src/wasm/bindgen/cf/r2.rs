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

/// R2 object returned by `R2Bucket::get()`.
#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(extends = Object)]
    #[derive(Clone, Debug)]
    pub type R2Object;

    #[wasm_bindgen(method, js_name = arrayBuffer)]
    pub fn array_buffer(this: &R2Object) -> js_sys::Promise;

    #[wasm_bindgen(method, getter)]
    pub fn size(this: &R2Object) -> f64;
}

/// R2 list output returned by `R2Bucket::list()`.
#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(extends = Object)]
    #[derive(Clone, Debug)]
    pub type R2Objects;

    #[wasm_bindgen(method, getter, js_name = objects)]
    pub fn objects(this: &R2Objects) -> js_sys::Array;

    #[wasm_bindgen(method, getter, js_name = truncated)]
    pub fn truncated(this: &R2Objects) -> bool;

    #[wasm_bindgen(method, getter, js_name = cursor)]
    pub fn cursor(this: &R2Objects) -> Option<String>;
}
