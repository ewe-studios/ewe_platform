//! App Shell — Surface 3 WASM module (wasmtime).
//!
//! Compiles to wasm32-wasip1 with no host imports, so `foundation_wasmtime`
//! links it as-is. The module exports the request and response ABI in the
//! `foundation_wasmtime` crate docs:
//!
//! - `memory`: the linear memory.
//! - `ewe_alloc(size)` returns a writable buffer for the request.
//! - `handle_request(ptr, len)` returns `(response_ptr << 32) | response_len`.
//!
//! The response bytes stay valid until the next `handle_request` call.
//!
//! This is a minimal demo. In production, a #[wasm_app] crate would
//! handle ewe:// routes, call session APIs through imports, and return
//! responses through the same ABI.

#![no_std]

// Marked with #[wasm_app] so codegen discovers it.
// (In a real app this would be a proc macro; for now it's a comment
//  that `build_wasmtime_app` scans for via the `WasmApp` AnnotationKind.)

/// Size of the request buffer and the response buffer in bytes.
///
/// The demo accepts a route and returns a response that fits in 16 KiB.
const BUFFER_SIZE: usize = 16 * 1024;

/// Prefix of every response. The response is the prefix and the request path.
const RESPONSE_PREFIX: &[u8] = b"wasmtime shell handled: ";

/// Buffer that `ewe_alloc` hands to the host for one request.
static mut REQUEST_BUFFER: [u8; BUFFER_SIZE] = [0; BUFFER_SIZE];

/// Buffer that holds the last response until the next `handle_request`.
static mut RESPONSE_BUFFER: [u8; BUFFER_SIZE] = [0; BUFFER_SIZE];

/// The host calls this at startup. The demo has nothing to set up.
#[no_mangle]
pub extern "C" fn init() {}

/// Allocate the request buffer.
///
/// Returns a pointer to `size` writable bytes. Returns 0 when `size` is 0 or
/// larger than the buffer.
#[no_mangle]
pub extern "C" fn ewe_alloc(size: u32) -> u32 {
    if size == 0 || size as usize > BUFFER_SIZE {
        return 0;
    }
    core::ptr::addr_of_mut!(REQUEST_BUFFER).cast::<u8>() as u32
}

/// Handle a request. Returns the response as `(ptr << 32) | len`.
///
/// The response buffer is reused on each call. The host must read the response
/// bytes before the next call to `handle_request`.
#[no_mangle]
pub extern "C" fn handle_request(request_ptr: u32, request_len: u32) -> u64 {
    let request_len = request_len as usize;
    let request = if request_len == 0 {
        &b"/"[..]
    } else {
        // SAFETY: the host writes `request_len` bytes at `request_ptr`, which
        // came from `ewe_alloc`, before this call. The length is capped to the
        // buffer size.
        unsafe {
            core::slice::from_raw_parts(request_ptr as *const u8, request_len.min(BUFFER_SIZE))
        }
    };

    // SAFETY: this module is single-threaded. The host calls `handle_request`
    // one time at a time, and it reads the response before the next call.
    let response = unsafe {
        core::slice::from_raw_parts_mut(
            core::ptr::addr_of_mut!(RESPONSE_BUFFER).cast::<u8>(),
            BUFFER_SIZE,
        )
    };

    let mut len = 0;
    for &byte in RESPONSE_PREFIX.iter().chain(request.iter()) {
        if len == BUFFER_SIZE {
            break;
        }
        response[len] = byte;
        len += 1;
    }

    if len == 0 {
        return 0;
    }
    ((response.as_ptr() as u64) << 32) | len as u64
}

/// The panic handler for a `no_std` crate. The demo never panics.
#[panic_handler]
fn panic(_info: &core::panic::PanicInfo) -> ! {
    loop {}
}
