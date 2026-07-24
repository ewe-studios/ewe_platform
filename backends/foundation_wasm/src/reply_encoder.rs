//! ReplyEncoder — produces the same `[Begin][type][value_bytes]…[End]` wire
//! format as JS `ReplyEncoder.encode()`.
//!
//! Used by Rust code that needs to generate callback replies (host→WASM
//! event dispatch, IPC responses) without going through the JS side.
//!
//! ## Wire format
//!
//! ```text
//! [Begin: u8(100)][ReturnType: u8][value_bytes]...[End: u8(101)]
//! ```
//!
//! Nested types (Text8, array buffers, MemorySlice) write their payload
//! into a fresh arena slot first, then encode the slot's `u64` id inline.

use alloc::vec::Vec;

use crate::host_runtime::exposed_runtime;
use crate::ReturnValues;

const BEGIN: u8 = 100;
const END: u8 = 101;

// ── Public API ──────────────────────────────────────────────────────────

/// Encode values into wire bytes. Nested types (arrays, Text8) are written
/// into the global arena, with their slot ids encoded inline — safe for
/// `ReturnTypeHints::from_binary`.
#[must_use]
pub fn encode_reply(values: &[ReturnValues]) -> Vec<u8> {
    let mut buf = Vec::new();
    buf.push(BEGIN);
    for v in values {
        buf.push(v.to_return_value_type_u8());
        encode_value(&mut buf, v);
    }
    buf.push(END);
    buf
}

/// Encode + write into a global-arena slot. Returns the slot id.
#[must_use]
pub fn encode_reply_into_slot(values: &[ReturnValues]) -> u64 {
    let bytes = encode_reply(values);
    write_to_global_arena(&bytes)
}

/// Write raw bytes into the global arena. Returns the slot id as u64,
/// ready for `run_internal_callbacks(callback_id, slot_id)`.
#[must_use]
pub fn write_to_global_arena(data: &[u8]) -> u64 {
    let id = exposed_runtime::create_allocation(data.len() as u64);
    let ptr = exposed_runtime::allocation_start_pointer(id);
    unsafe { core::ptr::copy_nonoverlapping(data.as_ptr(), ptr as *mut u8, data.len()) };
    id
}

/// Success callback — encode into slot.
#[must_use]
pub fn callback_success(values: &[ReturnValues]) -> u64 {
    encode_reply_into_slot(values)
}

/// Failure callback — encode ErrorCode into slot.
#[must_use]
pub fn callback_failure(code: u16) -> u64 {
    encode_reply_into_slot(&[ReturnValues::ErrorCode(code)])
}

// ── Per-type encoding ──────────────────────────────────────────────────

fn encode_value(buf: &mut Vec<u8>, v: &ReturnValues) {
    match v {
        ReturnValues::None => {}
        ReturnValues::Bool(b) => buf.push(if *b { 1 } else { 0 }),
        ReturnValues::Int8(n) => buf.push(*n as u8),
        ReturnValues::Uint8(n) => buf.push(*n),
        ReturnValues::Int16(n) => buf.extend_from_slice(&n.to_le_bytes()),
        ReturnValues::Uint16(n) => buf.extend_from_slice(&n.to_le_bytes()),
        ReturnValues::ErrorCode(c) => buf.extend_from_slice(&c.to_le_bytes()),
        ReturnValues::Int32(n) => buf.extend_from_slice(&n.to_le_bytes()),
        ReturnValues::Uint32(n) => buf.extend_from_slice(&n.to_le_bytes()),
        ReturnValues::Float32(n) => buf.extend_from_slice(&n.to_le_bytes()),
        ReturnValues::Int64(n) => buf.extend_from_slice(&n.to_le_bytes()),
        ReturnValues::Uint64(n) => buf.extend_from_slice(&n.to_le_bytes()),
        ReturnValues::Float64(n) => buf.extend_from_slice(&n.to_le_bytes()),
        ReturnValues::Int128(n) => {
            let b = n.to_le_bytes();
            buf.extend_from_slice(&b);
        }
        ReturnValues::Uint128(n) => {
            let b = n.to_le_bytes();
            buf.extend_from_slice(&b);
        }
        ReturnValues::ExternalReference(ep) => {
            buf.extend_from_slice(&ep.into_inner().to_le_bytes())
        }
        ReturnValues::InternalReference(ip) => {
            buf.extend_from_slice(&ip.into_inner().to_le_bytes())
        }
        ReturnValues::Object(ep) => buf.extend_from_slice(&ep.into_inner().to_le_bytes()),
        ReturnValues::DOMObject(ip) => buf.extend_from_slice(&ip.into_inner().to_le_bytes()),
        ReturnValues::MemorySlice(mid) => buf.extend_from_slice(&mid.as_u64().to_le_bytes()),

        // Nested: payload in global-arena slot, slot id goes inline.
        ReturnValues::Text8(s) => buf.extend_from_slice(&nest(s.as_bytes()).to_le_bytes()),
        ReturnValues::Uint8Array(a) => buf.extend_from_slice(&nest(a).to_le_bytes()),
        ReturnValues::Int8Array(a) => {
            let b: Vec<u8> = a.iter().map(|n| *n as u8).collect();
            buf.extend_from_slice(&nest(&b).to_le_bytes());
        }
        ReturnValues::Int16Array(a) => {
            let b: Vec<u8> = a.iter().flat_map(|n| n.to_le_bytes()).collect();
            buf.extend_from_slice(&nest(&b).to_le_bytes());
        }
        ReturnValues::Int32Array(a) => {
            let b: Vec<u8> = a.iter().flat_map(|n| n.to_le_bytes()).collect();
            buf.extend_from_slice(&nest(&b).to_le_bytes());
        }
        ReturnValues::Int64Array(a) => {
            let b: Vec<u8> = a.iter().flat_map(|n| n.to_le_bytes()).collect();
            buf.extend_from_slice(&nest(&b).to_le_bytes());
        }
        ReturnValues::Uint16Array(a) => {
            let b: Vec<u8> = a.iter().flat_map(|n| n.to_le_bytes()).collect();
            buf.extend_from_slice(&nest(&b).to_le_bytes());
        }
        ReturnValues::Uint32Array(a) => {
            let b: Vec<u8> = a.iter().flat_map(|n| n.to_le_bytes()).collect();
            buf.extend_from_slice(&nest(&b).to_le_bytes());
        }
        ReturnValues::Uint64Array(a) => {
            let b: Vec<u8> = a.iter().flat_map(|n| n.to_le_bytes()).collect();
            buf.extend_from_slice(&nest(&b).to_le_bytes());
        }
        ReturnValues::Float32Array(a) => {
            let b: Vec<u8> = a.iter().flat_map(|n| n.to_bits().to_le_bytes()).collect();
            buf.extend_from_slice(&nest(&b).to_le_bytes());
        }
        ReturnValues::Float64Array(a) => {
            let b: Vec<u8> = a.iter().flat_map(|n| n.to_bits().to_le_bytes()).collect();
            buf.extend_from_slice(&nest(&b).to_le_bytes());
        }
        ReturnValues::TypedArraySlice(ts, ml) => {
            buf.push(*ts as u8);
            buf.extend_from_slice(&(ml.0 as u64).to_le_bytes());
            buf.extend_from_slice(&ml.1.to_le_bytes());
        }
    }
}

fn nest(data: &[u8]) -> u64 {
    write_to_global_arena(data)
}
