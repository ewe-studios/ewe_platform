pub mod barrier_debug;
pub mod integration_tests;
// wasm_tests is conditional
#[cfg(target_arch = "wasm32")]
pub mod wasm_tests;
