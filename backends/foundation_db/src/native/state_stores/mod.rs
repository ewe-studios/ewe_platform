#[cfg(all(feature = "multi", not(target_arch = "wasm32")))]
pub mod native;
