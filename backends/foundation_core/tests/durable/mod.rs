//! Tests for the Durable Streams protocol core.

#[cfg(not(target_arch = "wasm32"))]
mod producer;
#[cfg(not(target_arch = "wasm32"))]
mod property;
#[cfg(not(target_arch = "wasm32"))]
mod reader;
#[cfg(not(target_arch = "wasm32"))]
mod scenarios;
mod store_contract;
mod support;
mod types;
#[cfg(not(target_arch = "wasm32"))]
mod valtron_reader;
