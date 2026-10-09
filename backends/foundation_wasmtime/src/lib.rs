//! `foundation_wasmtime` — WASM-in-process shell (Surface 3, F33).
//!
//! WHY: Surface 1 runs WASM in the WebView's JS engine. Surface 3 runs WASM
//! in wasmtime inside the native process — zero-copy Arrow IPC, session access
//! via imports, no WebView overhead.
//!
//! WHAT: `WasmtimeBuilder` wraps compiled WASM bytes with session imports.
//! `WasmtimeInstance` holds a running Engine + Store + Instance ready for
//! export calls. Codegen produces `fn builder() -> WasmtimeBuilder` for each
//! `#[wasm_app]` crate.

use std::path::Path;

use derive_more::Display;
use foundation_errstacks::ErrorTrace;
use wasmtime::{Engine, Instance, Linker, Module, Store};

// ── Error types ──────────────────────────────────────────────────────────

/// Context for wasmtime shell errors.
#[derive(Debug, Display)]
pub enum WasmtimeError {
    /// WASM module compilation failed.
    #[display("failed to compile WASM module: {_0}")]
    Compile(String),
    /// Linking (import registration) failed.
    #[display("failed to link WASM module: {_0}")]
    Link(String),
    /// A WASM export was not found.
    #[display("WASM export not found: {_0}")]
    ExportNotFound(String),
    /// Calling a WASM function failed.
    #[display("WASM call failed: {_0}")]
    Call(String),
    /// File I/O error (loading from disk).
    #[display("file I/O: {_0}")]
    Io(String),
}

impl std::error::Error for WasmtimeError {}

/// Convenience type alias for results from this crate.
pub type Result<T> = std::result::Result<T, ErrorTrace<WasmtimeError>>;

// ── WasmtimeBuilder ──────────────────────────────────────────────────────

/// Builder for a wasmtime-hosted WASM app (Surface 3).
///
/// Constructed by generated code:
/// ```ignore
/// pub fn builder() -> WasmtimeBuilder {
///     WasmtimeBuilder::new(include_bytes!("../shell_wasm/app.wasm"))
/// }
/// ```
pub struct WasmtimeBuilder {
    wasm_bytes: Vec<u8>,
    name: String,
}

impl WasmtimeBuilder {
    #[must_use]
    pub fn new(wasm_bytes: impl Into<Vec<u8>>) -> Self {
        Self { wasm_bytes: wasm_bytes.into(), name: String::new() }
    }

    #[must_use]
    pub fn with_name(mut self, name: &str) -> Self {
        self.name = name.to_string();
        self
    }

    /// Build the Engine, Module, and Instance. Ready to call exports.
    pub fn build(&self) -> Result<WasmtimeInstance> {
        let engine = Engine::default();
        let module = Module::from_binary(&engine, &self.wasm_bytes)
            .map_err(|e| ErrorTrace::new(WasmtimeError::Compile(e.to_string())))?;

        let mut linker: Linker<()> = Linker::new(&engine);
        linker
            .func_wrap("ewe", "log", |_: i32, _: i32| {})
            .map_err(|e| ErrorTrace::new(WasmtimeError::Link(e.to_string())))?;

        let mut store = Store::new(&engine, ());
        let instance = linker
            .instantiate(&mut store, &module)
            .map_err(|e| ErrorTrace::new(WasmtimeError::Link(e.to_string())))?;

        tracing::debug!(name = %self.name, "WASM module built");
        Ok(WasmtimeInstance { engine, store, instance, name: self.name.clone() })
    }

    /// Build from a file path instead of embedded bytes.
    pub fn from_file(path: impl AsRef<Path>) -> Result<Self> {
        let bytes = std::fs::read(path.as_ref())
            .map_err(|e| ErrorTrace::new(WasmtimeError::Io(e.to_string())))?;
        let name = path.as_ref()
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("wasm_app")
            .to_string();
        Ok(Self::new(bytes).with_name(&name))
    }
}

// ── WasmtimeInstance ─────────────────────────────────────────────────────

/// A running wasmtime instance. Holds the engine, store, and linked instance.
pub struct WasmtimeInstance {
    engine: Engine,
    store: Store<()>,
    instance: Instance,
    name: String,
}

impl WasmtimeInstance {
    #[must_use]
    pub fn name(&self) -> &str { &self.name }

    /// Call a WASM export function with no arguments and no return value.
    pub fn call_void(&mut self, func_name: &str) -> Result<()> {
        let func = self
            .instance
            .get_typed_func::<(), ()>(&mut self.store, func_name)
            .map_err(|_| ErrorTrace::new(WasmtimeError::ExportNotFound(func_name.to_string())))?;
        func.call(&mut self.store, ())
            .map_err(|e| ErrorTrace::new(WasmtimeError::Call(e.to_string())))?;
        Ok(())
    }

    /// Call a WASM export taking a single i32 and returning an i32.
    pub fn call_i32_i32(&mut self, func_name: &str, arg: i32) -> Result<i32> {
        let func = self
            .instance
            .get_typed_func::<i32, i32>(&mut self.store, func_name)
            .map_err(|_| ErrorTrace::new(WasmtimeError::ExportNotFound(func_name.to_string())))?;
        let result = func
            .call(&mut self.store, arg)
            .map_err(|e| ErrorTrace::new(WasmtimeError::Call(e.to_string())))?;
        Ok(result)
    }

    #[must_use]
    pub fn engine(&self) -> &Engine { &self.engine }
}

// ── Tests ────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builder_creates_from_bytes() {
        let builder = WasmtimeBuilder::new(b"fake wasm bytes").with_name("test");
        assert_eq!(builder.name, "test");
    }

    #[test]
    fn builder_from_file_missing_returns_error() {
        let result = WasmtimeBuilder::from_file("/tmp/definitely_not_a_wasm_file.wasm");
        assert!(result.is_err());
    }

    #[test]
    fn invalid_wasm_returns_error_on_build() {
        let builder = WasmtimeBuilder::new(b"not valid wasm");
        let result = builder.build();
        assert!(result.is_err());
    }
}
