//! Browser interface to the shared application.
#[cfg(feature = "web")]
#[wasm_bindgen::prelude::wasm_bindgen]
pub fn increment(value: u32) -> u32 {
    app_identity::increment(value)
}
