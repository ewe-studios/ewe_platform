//! Cloudflare Workers service bindings via wasm-bindgen.

use wasm_bindgen::JsCast;

pub mod d1;
pub mod kv;
pub mod r2;

/// Extract a typed binding from the CF env object.
pub fn get_binding<T: JsCast>(env: &wasm_bindgen::JsValue, binding: &str) -> Result<T, wasm_bindgen::JsError> {
    let prop = js_sys::Reflect::get(env, &wasm_bindgen::JsValue::from_str(binding))
        .map_err(|_| wasm_bindgen::JsError::new(&format!("missing binding '{binding}'")))?;
    prop.dyn_into::<T>()
        .map_err(|_| wasm_bindgen::JsError::new(&format!("binding '{binding}' is not the expected type")))
}

pub use d1::*;
pub use kv::*;
pub use r2::*;
