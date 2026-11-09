//! Regression test for issue #416: a guest built with `std` for
//! `wasm32-wasip1` imports functions from `wasi_snapshot_preview1`. The host
//! must link them so the module instantiates.
//!
//! WHY: before the fix the linker had no `wasi_snapshot_preview1` imports, so
//! `Linker::instantiate` failed with a link error. A `no_std` guest imports
//! nothing, so it worked and hid the defect.
//!
//! WHAT: the module imports `fd_write`, exports the request/response ABI, and
//! answers every request with a fixed body. `handle_request` also calls the
//! import once and discards the result. That proves the host serves a WASI
//! context, not only that the import name resolves.
//!
//! HOW: `wat::parse_str` turns the text module into the bytes that
//! `WasmtimeBuilder` compiles.

use foundation_core::valtron::Ctx;
use foundation_wasmtime::WasmtimeBuilder;

/// The fixed response body.
const RESPONSE: &[u8] = b"wasi-ok";

/// A module that imports `wasi_snapshot_preview1.fd_write` and returns
/// [`RESPONSE`] for every request.
const WASI_IMPORT_MODULE: &str = r#"
(module
  (import "wasi_snapshot_preview1" "fd_write"
    (func $fd_write (param i32 i32 i32 i32) (result i32)))

  (memory (export "memory") 1)
  (data (i32.const 2048) "wasi-ok")

  (func (export "ewe_alloc") (param $size i32) (result i32)
    i32.const 1024)

  (func (export "handle_request") (param $ptr i32) (param $len i32) (result i64)
    ;; The host context is empty, so this call returns an errno and does not
    ;; trap. The result is discarded. The call proves the import is linked and
    ;; served, not only declared.
    (drop (call $fd_write (i32.const 1) (i32.const 0) (i32.const 0) (i32.const 0)))
    (i64.or
      (i64.shl (i64.extend_i32_u (i32.const 2048)) (i64.const 32))
      (i64.extend_i32_u (i32.const 7)))))
"#;

#[test]
fn guest_with_a_wasi_import_instantiates_and_responds() {
    let bytes = wat::parse_str(WASI_IMPORT_MODULE).expect("the WAT source parses");
    // This test reads no clock and no randomness, so the host bundle is enough.
    let ctx = Ctx::host();
    let mut instance = WasmtimeBuilder::new(bytes)
        .with_name("wasi-import")
        .build(&ctx)
        .expect("a guest with a WASI import must link and instantiate");

    let body = instance
        .call_handle_request(b"/x")
        .expect("the guest responds");
    assert_eq!(body, RESPONSE);
}
