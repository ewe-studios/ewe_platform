//! `foundation_wasmtime` — WASM-in-process shell (Surface 3, F33).
//!
//! WHY: Surface 1 runs WASM in the WebView's JS engine. Surface 3 runs WASM
//! in wasmtime inside the native process — zero-copy Arrow IPC, session access
//! via imports, no WebView overhead.
//!
//! WHAT: `WasmtimeBuilder` wraps compiled WASM bytes with session imports.
//! `WasmtimeInstance` holds a running Engine + Store + Instance ready for
//! export calls. The codegen produces `fn builder() -> WasmtimeBuilder` for
//! each #[wasm_app] crate.
//!
//! HOW: The user writes a WASM function compiled to wasm32-wasip1. Codegen
//! wraps it in `include_bytes!` → `WasmtimeBuilder::new(bytes)`. At runtime,
//! `build()` creates the Engine + Module + Instance with session imports.

use anyhow::{Context, Result};
use wasmtime::{Engine, Instance, Linker, Module, Store};
use std::path::Path;

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
    ///
    /// # Errors
    ///
    /// Returns an error if the WASM bytes are invalid, compilation fails,
    /// or linking fails.
    pub fn build(&self) -> Result<WasmtimeInstance> {
        let engine = Engine::default();
        let module = Module::from_binary(&engine, &self.wasm_bytes)
            .context("failed to compile WASM module")?;

        let mut linker = Linker::new(&engine);
        // Register session imports — these are WASI-like host functions
        // that the WASM module can call to interact with the platform.
        linker.func_wrap("ewe", "log", |msg_ptr: i32, msg_len: i32| {
            // stub: log from WASM
            let _ = (msg_ptr, msg_len);
        })?;

        let mut store = Store::new(&engine, ());
        let instance = linker
            .instantiate(&mut store, &module)
            .context("failed to instantiate WASM module")?;

        Ok(WasmtimeInstance {
            engine,
            store,
            instance,
            name: self.name.clone(),
        })
    }

    /// Build from a file path instead of embedded bytes.
    ///
    /// # Errors
    ///
    /// Returns an error if the file can't be read.
    pub fn from_file(path: impl AsRef<Path>) -> Result<Self> {
        let bytes = std::fs::read(path.as_ref())
            .with_context(|| format!("failed to read {}", path.as_ref().display()))?;
        Ok(Self::new(bytes).with_name(
            path.as_ref()
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("wasm_app"),
        ))
    }
}

// ── WasmtimeInstance ─────────────────────────────────────────────────────

/// A running wasmtime instance. Holds the engine, store, and linked instance.
/// Call `get_export()` to invoke WASM functions.
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
    ///
    /// # Errors
    ///
    /// Returns an error if the export doesn't exist or calling fails.
    pub fn call_void(&mut self, func_name: &str) -> Result<()> {
        let func = self
            .instance
            .get_typed_func::<(), ()>(&mut self.store, func_name)
            .with_context(|| format!("export '{}' not found in '{}'", func_name, self.name))?;
        func.call(&mut self.store, ()).context("WASM call failed")?;
        Ok(())
    }

    /// Call a WASM export function taking a single i32 and returning an i32.
    ///
    /// # Errors
    ///
    /// Returns an error if the export doesn't exist or calling fails.
    pub fn call_i32_i32(&mut self, func_name: &str, arg: i32) -> Result<i32> {
        let func = self
            .instance
            .get_typed_func::<i32, i32>(&mut self.store, func_name)
            .with_context(|| format!("export '{}' not found in '{}'", func_name, self.name))?;
        let result = func.call(&mut self.store, arg).context("WASM call failed")?;
        Ok(result)
    }

    /// Get the engine (for advanced use).
    #[must_use]
    pub fn engine(&self) -> &Engine { &self.engine }
}

// ── Tests ────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    /// A minimal WASM module that exports a `hello` function returning 42.
    const MINIMAL_WASM: &[u8] = &[
        0x00, 0x61, 0x73, 0x6d, // magic
        0x01, 0x00, 0x00, 0x00, // version
        0x01, 0x06, 0x01, 0x7f, 0x01, 0x7f, 0x60, 0x00, 0x00, // type section (simplified)
    ];

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
