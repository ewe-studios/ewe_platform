//! Sample `#[wasm_test]` cases (owned harness — features 12/13, no wasm-bindgen).
//!
//! Run with: `wasm-testbed node <this-crate>` (or `deno` / `web`).
//! Requirements in Cargo.toml:
//!   [lib] crate-type = ["cdylib"]
//!   foundation_wasm = { workspace = true, features = ["web"] }
//!   foundation_macros = { workspace = true }

use foundation_macros::wasm_test;

#[wasm_test]
fn it_works() {
    assert_eq!(2 + 2, 4);
}

#[wasm_test(ignore)]
fn not_yet() {
    unreachable!("ignored cases never run");
}

#[wasm_test]
async fn async_works() {
    assert!(true);
}
