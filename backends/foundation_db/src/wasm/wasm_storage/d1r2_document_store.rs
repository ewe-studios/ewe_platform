//! CF D1+R2 document store alias (F23).
//!
//! The store logic is target-agnostic and lives in
//! [`crate::core::backends::d1r2_document_store`]; on Cloudflare Workers it is
//! instantiated over the wasm CF bindings (`D1WasmStorage` + `R2WasmStorage`).
//! Native callers (and integration tests against a wrangler/miniflare worker)
//! use the same generic with `D1Store` + `R2Store`.

use crate::wasm::wasm_storage::d1_wasm::D1WasmStorage;
use crate::wasm::wasm_storage::r2_wasm::R2WasmStorage;

/// `D1R2DocumentStore` specialized for the Cloudflare Workers wasm bindings.
pub type CfD1R2DocumentStore =
    crate::core::backends::D1R2DocumentStore<D1WasmStorage, R2WasmStorage>;
