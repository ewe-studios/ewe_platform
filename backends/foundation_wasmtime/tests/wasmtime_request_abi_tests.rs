//! Tests for the request and response ABI and the linear-memory primitives.
//!
//! WHY: the host must call the guest `handle_request` export and must refuse
//! bad pointers, missing exports and traps with a typed error. It must never
//! panic or read outside the guest memory.
//!
//! WHAT: each test builds a small WAT module, then drives the ABI or one
//! primitive.
//!
//! HOW: `wat::parse_str` turns the text module into the bytes that
//! `WasmtimeBuilder` compiles.

use foundation_wasmtime::{WasmtimeBuilder, WasmtimeError, WasmtimeInstance};

/// A module whose `handle_request` echoes the request bytes.
const ECHO_MODULE: &str = r#"
(module
  (memory (export "memory") 1)
  (func (export "ewe_alloc") (param $size i32) (result i32)
    i32.const 1024)
  (func (export "handle_request") (param $ptr i32) (param $len i32) (result i64)
    (i64.or
      (i64.shl (i64.extend_i32_u (local.get $ptr)) (i64.const 32))
      (i64.extend_i32_u (local.get $len)))))
"#;

fn instance(wat_source: &str) -> WasmtimeInstance {
    instance_with_cap(wat_source, WasmtimeBuilder::DEFAULT_MAX_RESPONSE_BYTES)
}

fn instance_with_cap(wat_source: &str, max_response_bytes: usize) -> WasmtimeInstance {
    let bytes = wat::parse_str(wat_source).expect("the WAT source parses");
    WasmtimeBuilder::new(bytes)
        .with_max_response_bytes(max_response_bytes)
        .build()
        .expect("the module builds")
}

#[test]
fn handle_request_returns_the_guest_response() {
    let mut instance = instance(ECHO_MODULE);
    let body = instance
        .call_handle_request(b"/shell/items/42")
        .expect("the guest responds");
    assert_eq!(body, b"/shell/items/42");
}

#[test]
fn zero_response_length_is_an_empty_body() {
    let module = r#"
    (module
      (memory (export "memory") 1)
      (func (export "ewe_alloc") (param $size i32) (result i32)
        i32.const 1024)
      (func (export "handle_request") (param $ptr i32) (param $len i32) (result i64)
        i64.const 0))
    "#;
    // The cap is 0. A zero length response is still valid.
    let mut instance = instance_with_cap(module, 0);
    let body = instance
        .call_handle_request(b"/empty")
        .expect("a zero length body is valid");
    assert!(body.is_empty(), "a zero length response has no body");
}

#[test]
fn missing_handle_request_export_is_a_typed_error() {
    let module = r#"
    (module
      (memory (export "memory") 1)
      (func (export "ewe_alloc") (param $size i32) (result i32)
        i32.const 1024))
    "#;
    let mut instance = instance(module);
    let error = instance
        .call_handle_request(b"/missing")
        .expect_err("the export is missing");
    assert!(matches!(
        error.current_context(),
        WasmtimeError::ExportNotFound(name) if name == "handle_request"
    ));
}

#[test]
fn missing_memory_is_a_typed_error() {
    let module = r#"
    (module
      (func (export "ewe_alloc") (param $size i32) (result i32)
        i32.const 1024)
      (func (export "handle_request") (param $ptr i32) (param $len i32) (result i64)
        i64.const 1))
    "#;
    let mut instance = instance(module);
    let error = instance
        .call_handle_request(b"/no-memory")
        .expect_err("the memory is missing");
    assert!(matches!(
        error.current_context(),
        WasmtimeError::MemoryNotFound(name) if name == "memory"
    ));
}

#[test]
fn null_allocation_is_a_typed_error() {
    let module = r#"
    (module
      (memory (export "memory") 1)
      (func (export "ewe_alloc") (param $size i32) (result i32)
        i32.const 0)
      (func (export "handle_request") (param $ptr i32) (param $len i32) (result i64)
        i64.const 0))
    "#;
    let mut instance = instance(module);
    let error = instance
        .call_handle_request(b"/null")
        .expect_err("the allocator returned null");
    assert!(matches!(
        error.current_context(),
        WasmtimeError::AllocationFailed(5)
    ));
}

#[test]
fn out_of_bounds_response_is_a_typed_error() {
    // Memory is one 64 KiB page. The response starts at the last byte and
    // claims two bytes, so the range crosses the end.
    let module = r#"
    (module
      (memory (export "memory") 1)
      (func (export "ewe_alloc") (param $size i32) (result i32)
        i32.const 1024)
      (func (export "handle_request") (param $ptr i32) (param $len i32) (result i64)
        (i64.or (i64.shl (i64.const 65535) (i64.const 32)) (i64.const 2))))
    "#;
    let mut instance = instance(module);
    let error = instance
        .call_handle_request(b"/oob")
        .expect_err("the response range is outside the memory");
    assert!(matches!(
        error.current_context(),
        WasmtimeError::MemoryOutOfBounds {
            ptr: 65535,
            len: 2,
            memory_size: 65536
        }
    ));
}

#[test]
fn out_of_bounds_memory_access_is_a_typed_error() {
    let mut instance = instance(ECHO_MODULE);
    assert_eq!(instance.memory_size().expect("the memory exists"), 65536);

    let write_error = instance
        .write_memory(65535, &[1, 2])
        .expect_err("the write crosses the end of the memory");
    assert!(matches!(
        write_error.current_context(),
        WasmtimeError::MemoryOutOfBounds {
            ptr: 65535,
            len: 2,
            memory_size: 65536
        }
    ));

    let read_error = instance
        .read_memory(65536, 1)
        .expect_err("the read starts after the memory");
    assert!(matches!(
        read_error.current_context(),
        WasmtimeError::MemoryOutOfBounds {
            ptr: 65536,
            len: 1,
            memory_size: 65536
        }
    ));
}

#[test]
fn memory_read_and_write_round_trip() {
    let mut instance = instance(ECHO_MODULE);
    instance
        .write_memory(16, b"hello")
        .expect("the write is in bounds");
    let read = instance.read_memory(16, 5).expect("the read is in bounds");
    assert_eq!(read, b"hello");
}

#[test]
fn trapping_call_is_a_typed_error() {
    let module = r#"
    (module
      (memory (export "memory") 1)
      (func (export "ewe_alloc") (param $size i32) (result i32)
        i32.const 1024)
      (func (export "handle_request") (param $ptr i32) (param $len i32) (result i64)
        unreachable))
    "#;
    let mut instance = instance(module);
    let error = instance
        .call_handle_request(b"/trap")
        .expect_err("the call traps");
    assert!(matches!(error.current_context(), WasmtimeError::Call(_)));
}

#[test]
fn response_above_the_cap_is_a_typed_error() {
    let mut instance = instance_with_cap(ECHO_MODULE, 4);
    let error = instance
        .call_handle_request(b"/too-large")
        .expect_err("the response is above the cap");
    assert!(matches!(
        error.current_context(),
        WasmtimeError::ResponseTooLarge { len: 10, max: 4 }
    ));
}

#[test]
fn builder_exposes_the_response_cap() {
    let builder = WasmtimeBuilder::new(Vec::new()).with_max_response_bytes(1234);
    assert_eq!(builder.max_response_bytes(), 1234);

    let default_builder = WasmtimeBuilder::new(Vec::new());
    assert_eq!(
        default_builder.max_response_bytes(),
        WasmtimeBuilder::DEFAULT_MAX_RESPONSE_BYTES
    );
}
