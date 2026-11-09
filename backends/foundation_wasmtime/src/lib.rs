//! `foundation_wasmtime` — WASM-in-process shell (Surface 3, F33).
//!
//! WHY: Surface 1 runs WASM in the `WebView`'s JS engine. Surface 3 runs WASM
//! in wasmtime inside the native process — zero-copy Arrow IPC, session access
//! via imports, no `WebView` overhead.
//!
//! WHAT: `WasmtimeBuilder` wraps compiled WASM bytes with session imports.
//! `WasmtimeInstance` holds a running Engine + Store + Instance ready for
//! export calls. Codegen produces `fn builder() -> WasmtimeBuilder` for each
//! `#[wasm_app]` crate.
//!
//! # Request and response ABI
//!
//! The host and the guest agree on one binary contract. The guest module
//! exports three items:
//!
//! - `memory`: the linear memory. The host reads and writes only this memory.
//! - `ewe_alloc(size: u32) -> u32`: allocate `size` bytes and return the byte
//!   offset of the start, or `0` when the allocation fails. The host uses this
//!   buffer for the request.
//! - `handle_request(ptr: u32, len: u32) -> u64`: handle the `len` request
//!   bytes at `ptr`.
//!
//! `handle_request` returns the response as one 64-bit value. The high 32 bits
//! hold the response pointer. The low 32 bits hold the response length in
//! bytes. The guest writes the value as a little-endian number:
//! `(ptr as u64) << 32 | len as u64`.
//!
//! Both `u32` values pass to Wasm as `i32` bit patterns. The host never
//! interprets them as signed numbers.
//!
//! ## Ownership
//!
//! The guest owns all linear memory. The host writes the request into the
//! buffer that `ewe_alloc` returns and reads the response from the buffer that
//! `handle_request` returns. The response bytes stay valid until the next call
//! to `handle_request`. The host copies the response immediately and keeps no
//! reference. The host never frees guest memory.
//!
//! ## Limits and errors
//!
//! A response length of `0` means an empty body. The host then ignores the
//! response pointer and does not read memory.
//!
//! A response length above [`WasmtimeBuilder::max_response_bytes`] is an error
//! and the host reads nothing. The default is
//! [`WasmtimeBuilder::DEFAULT_MAX_RESPONSE_BYTES`] (8 MiB). This byte cap
//! bounds host memory use; it does not limit the guest.
//!
//! The host returns a typed [`WasmtimeError`] and never panics when:
//!
//! - an export or the memory is missing;
//! - the request or response range is outside the memory;
//! - `ewe_alloc` returns `0` for a non-empty request;
//! - the call traps.

use std::path::Path;

use derive_more::Display;
use foundation_errstacks::ErrorTrace;
use wasmtime::{Engine, Instance, Linker, Memory, Module, Store};

/// Export name of the linear memory in the request/response ABI.
const MEMORY_EXPORT: &str = "memory";
/// Export name of the guest request buffer allocator.
const ALLOC_EXPORT: &str = "ewe_alloc";
/// Export name of the guest request handler.
const HANDLE_REQUEST_EXPORT: &str = "handle_request";

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
    /// The module does not export its linear memory.
    #[display("WASM memory export not found: {_0}")]
    MemoryNotFound(String),
    /// A linear memory access is outside the memory bounds.
    #[display("WASM memory access out of bounds: ptr={ptr}, len={len}, memory_size={memory_size}")]
    MemoryOutOfBounds {
        /// The requested byte offset.
        ptr: u32,
        /// The requested length in bytes.
        len: usize,
        /// The current size of the memory in bytes.
        memory_size: usize,
    },
    /// The guest allocator returned a null pointer for a non-empty request.
    #[display("WASM allocator returned a null pointer for a request of {_0} bytes")]
    AllocationFailed(usize),
    /// The request does not fit in the 32-bit guest address space.
    #[display("WASM request of {_0} bytes exceeds the 32-bit address space")]
    RequestTooLarge(usize),
    /// The response is larger than the configured maximum.
    #[display("WASM response of {len} bytes exceeds the maximum of {max} bytes")]
    ResponseTooLarge {
        /// The response length that the guest returned.
        len: u32,
        /// The configured maximum in bytes.
        max: usize,
    },
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
    max_response_bytes: usize,
}

impl WasmtimeBuilder {
    /// Default cap for one response body.
    ///
    /// The value is 8 MiB. A native response passes through the
    /// `RouteResponder` trait, which returns a `Vec<u8>`, so the host buffers
    /// the body. The cap bounds host memory use for a faulty or malicious
    /// module.
    pub const DEFAULT_MAX_RESPONSE_BYTES: usize = 8 * 1024 * 1024;

    #[must_use]
    pub fn new(wasm_bytes: impl Into<Vec<u8>>) -> Self {
        Self {
            wasm_bytes: wasm_bytes.into(),
            name: String::new(),
            max_response_bytes: Self::DEFAULT_MAX_RESPONSE_BYTES,
        }
    }

    #[must_use]
    pub fn with_name(mut self, name: &str) -> Self {
        self.name = name.to_string();
        self
    }

    /// Set the maximum response length in bytes.
    ///
    /// The default is [`WasmtimeBuilder::DEFAULT_MAX_RESPONSE_BYTES`].
    #[must_use]
    pub fn with_max_response_bytes(mut self, max_response_bytes: usize) -> Self {
        self.max_response_bytes = max_response_bytes;
        self
    }

    /// The module name set by [`WasmtimeBuilder::with_name`].
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    /// The response cap set by
    /// [`WasmtimeBuilder::with_max_response_bytes`].
    #[must_use]
    pub fn max_response_bytes(&self) -> usize {
        self.max_response_bytes
    }

    /// Build the Engine, Module, and Instance. Ready to call exports.
    ///
    /// # Errors
    ///
    /// Returns an error if the module does not compile or link.
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
        Ok(WasmtimeInstance {
            engine,
            store,
            instance,
            name: self.name.clone(),
            max_response_bytes: self.max_response_bytes,
        })
    }

    /// Build from a file path instead of embedded bytes.
    ///
    /// # Errors
    ///
    /// Returns an error if the file cannot be read.
    pub fn from_file(path: impl AsRef<Path>) -> Result<Self> {
        let bytes = std::fs::read(path.as_ref())
            .map_err(|e| ErrorTrace::new(WasmtimeError::Io(e.to_string())))?;
        let name = path
            .as_ref()
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
    max_response_bytes: usize,
}

impl WasmtimeInstance {
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    /// The response cap from the builder that produced this instance.
    #[must_use]
    pub fn max_response_bytes(&self) -> usize {
        self.max_response_bytes
    }

    /// Return the exported linear memory.
    fn memory(&mut self) -> Result<Memory> {
        self.instance
            .get_memory(&mut self.store, MEMORY_EXPORT)
            .ok_or_else(|| {
                ErrorTrace::new(WasmtimeError::MemoryNotFound(MEMORY_EXPORT.to_string()))
            })
    }

    /// Return the size of the linear memory in bytes.
    ///
    /// # Errors
    ///
    /// Returns an error if the module does not export `memory`.
    pub fn memory_size(&mut self) -> Result<usize> {
        let memory = self.memory()?;
        Ok(memory.data_size(&self.store))
    }

    /// Write `bytes` at byte offset `ptr` in linear memory.
    ///
    /// # Errors
    ///
    /// Returns an error if the module does not export `memory` or the range
    /// `ptr..ptr + bytes.len()` is outside the memory.
    pub fn write_memory(&mut self, ptr: u32, bytes: &[u8]) -> Result<()> {
        let memory = self.memory()?;
        let memory_size = memory.data_size(&self.store);
        // A `u32` fits a 32-bit or larger `usize`. On a smaller `usize`, the
        // value becomes `usize::MAX`, and the bounds check below rejects it.
        let offset = usize::try_from(ptr).unwrap_or(usize::MAX);
        let end = offset
            .checked_add(bytes.len())
            .ok_or_else(|| out_of_bounds(ptr, bytes.len(), memory_size))?;
        if end > memory_size {
            return Err(out_of_bounds(ptr, bytes.len(), memory_size));
        }
        memory
            .write(&mut self.store, offset, bytes)
            .map_err(|e| ErrorTrace::new(WasmtimeError::Call(e.to_string())))?;
        Ok(())
    }

    /// Read `len` bytes at byte offset `ptr` from linear memory.
    ///
    /// # Errors
    ///
    /// Returns an error if the module does not export `memory` or the range
    /// `ptr..ptr + len` is outside the memory.
    pub fn read_memory(&mut self, ptr: u32, len: u32) -> Result<Vec<u8>> {
        let memory = self.memory()?;
        let memory_size = memory.data_size(&self.store);
        // A `u32` fits a 32-bit or larger `usize`. On a smaller `usize`, the
        // value becomes `usize::MAX`, and the bounds check below rejects it.
        let offset = usize::try_from(ptr).unwrap_or(usize::MAX);
        let length = usize::try_from(len).unwrap_or(usize::MAX);
        let end = offset
            .checked_add(length)
            .ok_or_else(|| out_of_bounds(ptr, length, memory_size))?;
        if end > memory_size {
            return Err(out_of_bounds(ptr, length, memory_size));
        }
        let mut buffer = vec![0u8; length];
        memory
            .read(&self.store, offset, &mut buffer)
            .map_err(|e| ErrorTrace::new(WasmtimeError::Call(e.to_string())))?;
        Ok(buffer)
    }

    /// Call a WASM export function with no arguments and no return value.
    ///
    /// # Errors
    ///
    /// Returns an error if the export is missing or the call fails.
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
    ///
    /// # Errors
    ///
    /// Returns an error if the export is missing or the call fails.
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

    /// Call a WASM export taking two i32 arguments and returning an i64.
    ///
    /// # Errors
    ///
    /// Returns an error if the export is missing or the call fails.
    pub fn call_i32_i32_i64(&mut self, func_name: &str, first: i32, second: i32) -> Result<i64> {
        let func = self
            .instance
            .get_typed_func::<(i32, i32), i64>(&mut self.store, func_name)
            .map_err(|_| ErrorTrace::new(WasmtimeError::ExportNotFound(func_name.to_string())))?;
        let result = func
            .call(&mut self.store, (first, second))
            .map_err(|e| ErrorTrace::new(WasmtimeError::Call(e.to_string())))?;
        Ok(result)
    }

    /// Send one request to the module with the crate-level ABI.
    ///
    /// The method allocates `request.len()` bytes with `ewe_alloc`, writes the
    /// request there, calls `handle_request`, and reads the response. See the
    /// crate docs for the byte order, the ownership rules, and the meaning of a
    /// zero response length. The response cap comes from
    /// [`WasmtimeBuilder::with_max_response_bytes`].
    ///
    /// # Errors
    ///
    /// Returns a [`WasmtimeError`] when an export or the memory is missing,
    /// a range is outside the memory, the allocator fails, the response is
    /// larger than the configured cap, or a call traps.
    pub fn call_handle_request(&mut self, request: &[u8]) -> Result<Vec<u8>> {
        let request_len = u32::try_from(request.len())
            .map_err(|_| ErrorTrace::new(WasmtimeError::RequestTooLarge(request.len())))?;

        let request_ptr = if request.is_empty() {
            0
        } else {
            let allocated =
                self.call_i32_i32(ALLOC_EXPORT, i32::from_ne_bytes(request_len.to_ne_bytes()))?;
            let allocated = u32::from_ne_bytes(allocated.to_ne_bytes());
            if allocated == 0 {
                return Err(ErrorTrace::new(WasmtimeError::AllocationFailed(
                    request.len(),
                )));
            }
            self.write_memory(allocated, request)?;
            allocated
        };

        let packed = self.call_i32_i32_i64(
            HANDLE_REQUEST_EXPORT,
            i32::from_ne_bytes(request_ptr.to_ne_bytes()),
            i32::from_ne_bytes(request_len.to_ne_bytes()),
        )?;

        // The ABI packs the response as low 32 bits = length and high 32 bits
        // = pointer, in little-endian order.
        let packed = u64::from_ne_bytes(packed.to_ne_bytes());
        let bytes = packed.to_le_bytes();
        let response_len = u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]);
        let response_ptr = u32::from_le_bytes([bytes[4], bytes[5], bytes[6], bytes[7]]);

        if response_len == 0 {
            return Ok(Vec::new());
        }
        // A `u32` fits a 32-bit or larger `usize`. On a smaller `usize`, the
        // response is treated as above any cap.
        let response_length = usize::try_from(response_len).unwrap_or(usize::MAX);
        if response_length > self.max_response_bytes {
            return Err(ErrorTrace::new(WasmtimeError::ResponseTooLarge {
                len: response_len,
                max: self.max_response_bytes,
            }));
        }
        self.read_memory(response_ptr, response_len)
    }

    #[must_use]
    pub fn engine(&self) -> &Engine {
        &self.engine
    }
}

/// Build the out-of-bounds error for one memory range.
fn out_of_bounds(ptr: u32, len: usize, memory_size: usize) -> ErrorTrace<WasmtimeError> {
    ErrorTrace::new(WasmtimeError::MemoryOutOfBounds {
        ptr,
        len,
        memory_size,
    })
}

// ── Tests ────────────────────────────────────────────────────────────────
