//! Tests for the Durable Streams protocol core.

#[cfg(not(target_arch = "wasm32"))]
mod architecture;
#[cfg(not(target_arch = "wasm32"))]
mod cache;
#[cfg(not(target_arch = "wasm32"))]
mod client;
#[cfg(not(target_arch = "wasm32"))]
mod conformance_edges;
#[cfg(not(target_arch = "wasm32"))]
mod coverage_memory;
#[cfg(not(target_arch = "wasm32"))]
mod fork;
#[cfg(not(target_arch = "wasm32"))]
mod json;
#[cfg(not(target_arch = "wasm32"))]
mod producer;
#[cfg(not(target_arch = "wasm32"))]
mod property;
#[cfg(not(target_arch = "wasm32"))]
mod reader;
#[cfg(not(target_arch = "wasm32"))]
mod retention;
#[cfg(not(target_arch = "wasm32"))]
mod scenarios;
#[cfg(not(target_arch = "wasm32"))]
mod service;
mod store_contract;
mod support;
#[cfg(not(target_arch = "wasm32"))]
mod ttl;
mod types;
#[cfg(not(target_arch = "wasm32"))]
mod valtron_producer;
#[cfg(not(target_arch = "wasm32"))]
mod valtron_reader;
#[cfg(all(not(target_arch = "wasm32"), feature = "vfs-testkit"))]
mod vfs_conformance;
