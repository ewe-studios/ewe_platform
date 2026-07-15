//! wasm-bindgen runtime crate re-exports (F52).
//!
//! ```ignore
//! use wasm_bindgen_test::wasm_bindgen_test;
//! use foundation_testbed::bindgen::{js_sys, wasm_bindgen_futures, web_sys};
//!
//! wasm_bindgen_test_configure!(run_in_browser);
//!
//! #[wasm_bindgen_test]
//! fn my_test() { ... }
//! ```

pub use js_sys;
pub use wasm_bindgen_futures;
pub use web_sys;

pub const WASM_BINDGEN_VERSION: &str = "0.2.126";
