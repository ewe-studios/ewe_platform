//! Cloudflare Workers service bindings via wasm-bindgen.

use wasm_bindgen::JsCast;

pub mod d1;
pub mod kv;
pub mod r2;

/// Extract a typed binding from the CF env object.
///
/// Workers bindings are host objects without a JS class, so a `dyn_into`
/// (`instanceof`) check rejects them. The binding name is the contract: a
/// wrong binding fails at its first method call.
pub fn get_binding<T: JsCast>(
    env: &wasm_bindgen::JsValue,
    binding: &str,
) -> Result<T, wasm_bindgen::JsError> {
    let prop = js_sys::Reflect::get(env, &wasm_bindgen::JsValue::from_str(binding))
        .map_err(|_| wasm_bindgen::JsError::new(&format!("missing binding '{binding}'")))?;
    if prop.is_null() || prop.is_undefined() {
        return Err(wasm_bindgen::JsError::new(&format!(
            "binding '{binding}' is missing"
        )));
    }
    Ok(prop.unchecked_into::<T>())
}

pub use d1::*;
pub use kv::*;
pub use r2::*;
