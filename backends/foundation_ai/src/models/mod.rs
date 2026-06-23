#[cfg(not(target_family = "wasm"))]
pub mod generator;
pub mod providers;

pub use providers::model_descriptors;
