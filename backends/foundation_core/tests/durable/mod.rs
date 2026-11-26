//! Tests for the Durable Streams protocol core.

#[cfg(not(target_arch = "wasm32"))]
mod property;
#[cfg(not(target_arch = "wasm32"))]
mod scenarios;
mod store_contract;
mod support;
mod types;
