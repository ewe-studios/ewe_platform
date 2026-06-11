//! WHY: Encoded protocol bytes (produced by `foundation_ui_traits` encoders) have
//! to cross the WASM↔JS boundary through arena memory and the uniform `host_apply`
//! FFI. That transport contract is WASM-specific, so it lives here in
//! `foundation_wasm` rather than in the dependency-free encoding crate.
//!
//! WHAT: Layer 2 of the three-layer protocol architecture (decisions 014/028/030).
//! Defines the [`ProtocolHandler`] transport trait, the 14-byte [`WasmEnvelope`]
//! (the 6-byte base `Envelope` extended with the arena `memory_id`), a
//! [`ProtocolHandlerRegistry`], and [`dispatch_message`] which demuxes an incoming
//! message to the correct handler by protocol byte.
//!
//! HOW: Every message is `[protocol:1][version:1][memory_id:8 LE][length:4 LE]
//! [payload...]`. The first byte selects the handler; the `memory_id` tells JS
//! which arena slot to `dispose_allocation` after processing. One slot, one ACK,
//! for all protocols.

use alloc::boxed::Box;
use alloc::string::String;
use alloc::vec::Vec;

use crate::MemoryId;
// Shared arena — the relocated return-value parser reads/frees array-buffer slots.
use crate::host_runtime::ALLOCATIONS;
use crate::{
    BinaryReadError, BinaryReaderResult, FromBinary, GroupReturnHintMarker, ReturnIds,
    ReturnTypeHints, ReturnTypeId, ReturnValueMarker, ReturnValues, Returns, ThreeState,
    ThreeStateId, TypedSlice, MOVE_ONE_BYTE, MOVE_SIXTEEN_BYTES, MOVE_SIXTY_FOUR_BYTES,
    MOVE_THIRTY_TWO_BYTES,
};

// ─── ProtocolHandler trait ─────────────────────────────────────────────────────

/// WHY: The router needs a uniform contract to ship/receive a protocol payload
/// without knowing the format inside the arena slot.
///
/// WHAT: The bidirectional WASM transport contract. Both directions follow the
/// same loan pattern — the producer allocates an arena slot, the consumer ACKs by
/// calling `dispose_allocation`.
///
/// HOW: Implementors (in `foundation_wasm_ui`: `ArrowV1`, `BatchInstructionsV1`,
/// `JsonV1`) wrap the uniform 3-param `host_apply` FFI for `send_to_js`, and read
/// + deallocate the arena slot for `handle_from_js`.
pub trait ProtocolHandler {
    /// Protocol discriminant: `0` = Custom Binary, `1` = Arrow, `2` = JSON.
    fn protocol_byte(&self) -> u8;

    /// Protocol version for backward compatibility.
    fn version(&self) -> u8;

    /// WASM → JS: ship the payload in arena slot `memory_id` at `(ptr, len)`.
    ///
    /// JS reads the envelope header, dispatches by protocol byte, applies the
    /// payload, then calls `dispose_allocation(memory_id)`.
    fn send_to_js(&self, memory_id: MemoryId, ptr: *const u8, len: usize);

    /// JS → WASM: receive the payload from arena slot `memory_id` at `(ptr, len)`.
    ///
    /// The handler copies what it needs into owned storage, then releases the slot.
    fn handle_from_js(&self, memory_id: MemoryId, ptr: *const u8, len: usize);

    /// Release arena slot `memory_id` back to `memory` — the WASM-side ACK that
    /// mirrors JS's `dispose_allocation`.
    ///
    /// Part of the transport contract (every handler ships through arena slots, so
    /// every handler can release one). The default deallocates and ignores a stale
    /// id — generation-checked ids make double-ACKs safe to drop.
    fn ack(&self, memory_id: MemoryId, memory: &mut crate::MemoryAllocations) {
        let _ = memory.deallocate(memory_id);
    }
}

// ─── WasmEnvelope (14-byte WASM-specific header) ───────────────────────────────

/// WHY: The 6-byte base `Envelope` (in `foundation_ui_traits`) has no arena id —
/// fine for HTTP/SSE/WebSocket, but the JS side needs the `memory_id` to ACK a
/// WASM arena slot. This header carries it.
///
/// WHAT: The 14-byte WASM message header:
/// `[protocol:1][version:1][memory_id:8 LE][length:4 LE]`, followed by `length`
/// payload bytes.
///
/// HOW: `write` frames a payload; `parse` splits it back. All multi-byte fields
/// are little-endian. `memory_id` is `MemoryId::as_u64()` (index in the high 32
/// bits, generation in the low 32).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct WasmEnvelope {
    /// Protocol discriminant (`0` custom binary, `1` arrow, `2` json).
    pub protocol: u8,
    /// Protocol version.
    pub version: u8,
    /// Arena slot id (`MemoryId::as_u64()`) that JS releases after processing.
    pub memory_id: u64,
    /// Byte length of the payload that follows the header.
    pub length: u32,
}

impl WasmEnvelope {
    /// Header size in bytes.
    pub const HEADER_LEN: usize = 14;

    /// Frame a payload as `[protocol][version][memory_id:8 LE][length:4 LE][payload]`.
    ///
    /// # Panics
    /// Panics if `payload` is larger than `u32::MAX` (~4 GiB) — the length field is a
    /// `u32`, so a bigger payload can't be framed. This never happens for a DOM batch.
    #[must_use]
    pub fn write(protocol: u8, version: u8, memory_id: u64, payload: &[u8]) -> Vec<u8> {
        // Checked so the length can't silently truncate on a 64-bit target.
        let length =
            u32::try_from(payload.len()).expect("WasmEnvelope payload exceeds u32::MAX (~4 GiB)");
        let mut buf = Vec::with_capacity(Self::HEADER_LEN + payload.len());
        buf.push(protocol);
        buf.push(version);
        buf.extend_from_slice(&memory_id.to_le_bytes());
        buf.extend_from_slice(&length.to_le_bytes());
        buf.extend_from_slice(payload);
        buf
    }

    /// Parse the 14-byte header and return it alongside the payload slice.
    ///
    /// # Panics
    /// Panics if `bytes.len() < 14` or if the declared `length` overruns the slice.
    /// A sub-14-byte message is always invalid and indicates transport corruption
    /// (decision 028, Error Cases) — there is no recovery path.
    #[must_use]
    pub fn parse(bytes: &[u8]) -> (WasmEnvelope, &[u8]) {
        let protocol = bytes[0];
        let version = bytes[1];
        let memory_id = u64::from_le_bytes(bytes[2..10].try_into().unwrap());
        let length = u32::from_le_bytes(bytes[10..14].try_into().unwrap());
        let end = Self::HEADER_LEN + length as usize;
        (
            WasmEnvelope {
                protocol,
                version,
                memory_id,
                length,
            },
            &bytes[Self::HEADER_LEN..end],
        )
    }

    /// Fallible variant of [`parse`](Self::parse) for callers that prefer not to
    /// panic on a malformed header.
    ///
    /// # Errors
    /// Returns `None` if `bytes` is shorter than the header or the declared
    /// `length` overruns the slice.
    #[must_use]
    pub fn try_parse(bytes: &[u8]) -> Option<(WasmEnvelope, &[u8])> {
        if bytes.len() < Self::HEADER_LEN {
            return None;
        }
        let protocol = bytes[0];
        let version = bytes[1];
        let memory_id = u64::from_le_bytes(bytes[2..10].try_into().ok()?);
        let length = u32::from_le_bytes(bytes[10..14].try_into().ok()?);
        let end = Self::HEADER_LEN.checked_add(length as usize)?;
        if bytes.len() < end {
            return None;
        }
        Some((
            WasmEnvelope {
                protocol,
                version,
                memory_id,
                length,
            },
            &bytes[Self::HEADER_LEN..end],
        ))
    }
}

// ─── Protocol routing ──────────────────────────────────────────────────────────

/// WHY: The three protocol handlers must be addressable by the single protocol
/// byte at the front of every message.
///
/// WHAT: Holds the three boxed handlers (custom binary, arrow, json) that
/// [`dispatch_message`] routes to.
///
/// HOW: Indexed by protocol byte — `0` → `custom_binary`, `1` → `arrow`, `2` →
/// `json`.
pub struct ProtocolHandlerRegistry {
    /// Handler for protocol byte `0`.
    pub custom_binary: Box<dyn ProtocolHandler>,
    /// Handler for protocol byte `1`.
    pub arrow: Box<dyn ProtocolHandler>,
    /// Handler for protocol byte `2`.
    pub json: Box<dyn ProtocolHandler>,
}

impl ProtocolHandlerRegistry {
    /// Construct a registry from the three protocol handlers.
    #[must_use]
    pub fn new(
        custom_binary: Box<dyn ProtocolHandler>,
        arrow: Box<dyn ProtocolHandler>,
        json: Box<dyn ProtocolHandler>,
    ) -> Self {
        Self {
            custom_binary,
            arrow,
            json,
        }
    }
}

/// WHY: Incoming JS→WASM messages arrive as a flat byte buffer; the runtime must
/// route each to the handler that understands its payload.
///
/// WHAT: Reads the 14-byte [`WasmEnvelope`] header and calls `handle_from_js` on
/// the matching handler.
///
/// HOW: Parses protocol/version/memory_id/length, slices the payload, and matches
/// the protocol byte to a handler in `handlers`.
///
/// # Panics
/// Panics on an unknown protocol byte (anything other than `0`, `1`, `2`) — this
/// indicates binary-level corruption or a version mismatch with no recovery path
/// (decision 028, Error Cases). Also panics if `bytes` is shorter than the header.
pub fn dispatch_message(bytes: &[u8], handlers: &ProtocolHandlerRegistry) {
    let (envelope, payload) = WasmEnvelope::parse(bytes);
    let memory_id = MemoryId::from_u64(envelope.memory_id);
    let handler: &dyn ProtocolHandler = match envelope.protocol {
        0 => handlers.custom_binary.as_ref(),
        1 => handlers.arrow.as_ref(),
        2 => handlers.json.as_ref(),
        other => panic!("unknown protocol: {other}"),
    };
    handler.handle_from_js(memory_id, payload.as_ptr(), payload.len());
}

// ─── Return-value parsing (relocated from jsapi.rs — feature 00 Layer 2) ───────
//
// WHY: Binary return-value parsing is part of the WASM transport contract, so it
// lives in the protocol layer. Moved verbatim from jsapi.rs; behaviour unchanged.

struct ReturnValueParserIter<'a> {
    hint: ReturnTypeHints,
    item_index: usize,
    index: usize,
    src: &'a [u8],
}

impl<'a> ReturnValueParserIter<'a> {
    fn new(hint: ReturnTypeHints, src: &'a [u8]) -> Self {
        Self {
            src,
            hint,
            index: 0,
            item_index: 0,
        }
    }
}

// -- Parsing

impl ReturnValueParserIter<'_> {
    fn parse_next(&mut self) -> Option<BinaryReaderResult<ReturnValues>> {
        let bin = &self.src;
        let mut index = self.index;

        if self.index >= self.src.len() {
            return None;
        }

        let return_id: ReturnTypeId = bin[index].into();

        // move by 1 byte
        index += MOVE_ONE_BYTE;

        let result = match return_id {
            ReturnTypeId::None => {
                self.index = index;
                Ok(ReturnValues::None)
            }
            ReturnTypeId::ErrorCode => {
                let end = index + MOVE_SIXTEEN_BYTES;
                let portion = &bin[index..end];
                let mut section: [u8; 2] = Default::default();
                section.copy_from_slice(portion);

                let item = u16::from_le_bytes(section);

                self.index = end;

                Ok(ReturnValues::ErrorCode(item))
            }
            ReturnTypeId::Bool => {
                let value = if bin[index] == 1 {
                    ReturnValues::Bool(true)
                } else {
                    ReturnValues::Bool(false)
                };

                index += MOVE_ONE_BYTE;

                self.index = index;

                Ok(value)
            }
            ReturnTypeId::Uint8 => {
                let item = u8::from_le(bin[index]);
                index += MOVE_ONE_BYTE;

                self.index = index;
                Ok(ReturnValues::Uint8(item))
            }
            ReturnTypeId::Uint16 => {
                let end = index + MOVE_SIXTEEN_BYTES;
                let portion = &bin[index..end];
                let mut section: [u8; 2] = Default::default();
                section.copy_from_slice(portion);

                let item = u16::from_le_bytes(section);

                self.index = end;

                Ok(ReturnValues::Uint16(item))
            }
            ReturnTypeId::Uint32 => {
                let end = index + MOVE_THIRTY_TWO_BYTES;
                let portion = &bin[index..end];
                let mut section: [u8; 4] = Default::default();
                section.copy_from_slice(portion);

                let item = u32::from_le_bytes(section);

                self.index = end;

                Ok(ReturnValues::Uint32(item))
            }
            ReturnTypeId::Uint64 => {
                let end = index + MOVE_SIXTY_FOUR_BYTES;
                let portion = &bin[index..end];
                let mut section: [u8; 8] = Default::default();
                section.copy_from_slice(portion);

                let item = u64::from_le_bytes(section);

                self.index = end;

                Ok(ReturnValues::Uint64(item))
            }
            ReturnTypeId::Uint128 => {
                let msb_end = index + MOVE_SIXTY_FOUR_BYTES;
                let msb_portion = &bin[index..msb_end];
                let mut msb_section: [u8; 8] = Default::default();
                msb_section.copy_from_slice(msb_portion);

                let lsb_end = msb_end + MOVE_SIXTY_FOUR_BYTES;
                let lsb_portion = &bin[msb_end..lsb_end];
                let mut lsb_section: [u8; 8] = Default::default();
                lsb_section.copy_from_slice(lsb_portion);

                let value_msb = u64::from_le_bytes(msb_section);
                let value_lsb = u64::from_le_bytes(lsb_section);

                let mut value: u128 = u128::from(value_msb) << 64;
                value |= u128::from(value_lsb);

                self.index = lsb_end;

                Ok(ReturnValues::Uint128(value))
            }
            ReturnTypeId::Int8 => {
                let item = i8::from_le(bin[index] as i8);
                index += MOVE_ONE_BYTE;

                self.index = index;

                Ok(ReturnValues::Int8(item))
            }
            ReturnTypeId::Int16 => {
                let end = index + MOVE_SIXTEEN_BYTES;
                let portion = &bin[index..end];
                let mut section: [u8; 2] = Default::default();
                section.copy_from_slice(portion);

                let item = i16::from_le_bytes(section);

                self.index = end;

                Ok(ReturnValues::Int16(item))
            }
            ReturnTypeId::Int32 => {
                let end = index + MOVE_THIRTY_TWO_BYTES;
                let portion = &bin[index..end];
                let mut section: [u8; 4] = Default::default();
                section.copy_from_slice(portion);

                let item = i32::from_le_bytes(section);

                self.index = end;

                Ok(ReturnValues::Int32(item))
            }
            ReturnTypeId::Int64 => {
                let end = index + MOVE_SIXTY_FOUR_BYTES;
                let portion = &bin[index..end];
                let mut section: [u8; 8] = Default::default();
                section.copy_from_slice(portion);

                let item = i64::from_le_bytes(section);

                self.index = end;

                Ok(ReturnValues::Int64(item))
            }
            ReturnTypeId::Int128 => {
                let msb_end = index + MOVE_SIXTY_FOUR_BYTES;
                let msb_portion = &bin[index..msb_end];
                let mut msb_section: [u8; 8] = Default::default();
                msb_section.copy_from_slice(msb_portion);

                let lsb_end = msb_end + MOVE_SIXTY_FOUR_BYTES;
                let lsb_portion = &bin[msb_end..lsb_end];
                let mut lsb_section: [u8; 8] = Default::default();
                lsb_section.copy_from_slice(lsb_portion);

                let value_msb = i64::from_le_bytes(msb_section);
                let value_lsb = i64::from_le_bytes(lsb_section);

                let mut value: i128 = i128::from(value_msb) << 64;
                value |= i128::from(value_lsb);

                index = lsb_end;

                self.index = index;

                Ok(ReturnValues::Int128(value))
            }
            ReturnTypeId::Float32 => {
                let end = index + MOVE_THIRTY_TWO_BYTES;
                let portion = &bin[index..end];
                let mut section: [u8; 4] = Default::default();
                section.copy_from_slice(portion);

                let item = f32::from_le_bytes(section);

                self.index = end;

                Ok(ReturnValues::Float32(item))
            }
            ReturnTypeId::Float64 => {
                let end = index + MOVE_THIRTY_TWO_BYTES;
                let portion = &bin[index..end];
                let mut section: [u8; 8] = Default::default();
                section.copy_from_slice(portion);

                let item = f64::from_le_bytes(section);

                self.index = end;

                Ok(ReturnValues::Float64(item))
            }
            ReturnTypeId::TypedArraySlice => {
                let item_type: TypedSlice = u8::from_le(bin[index]).into();
                index += MOVE_ONE_BYTE;

                let ptr_end = index + MOVE_SIXTY_FOUR_BYTES;
                let ptr_portion = &bin[index..ptr_end];
                let mut ptr_section: [u8; 8] = Default::default();
                ptr_section.copy_from_slice(ptr_portion);

                index = ptr_end;

                let address_as_u64 = u64::from_le_bytes(ptr_section);
                let address_as_ptr = address_as_u64 as *const u8;

                let length_end = index + MOVE_SIXTY_FOUR_BYTES;
                let len_portion = &bin[index..length_end];
                let mut len_section: [u8; 8] = Default::default();
                len_section.copy_from_slice(len_portion);

                let length_as_u64 = u64::from_le_bytes(len_section);

                self.index = length_end;

                Ok(ReturnValues::TypedArraySlice(
                    item_type,
                    crate::MemoryLocation(address_as_ptr, length_as_u64),
                ))
            }
            ReturnTypeId::MemorySlice => {
                let end = index + MOVE_SIXTY_FOUR_BYTES;
                let portion = &bin[index..end];
                let mut section: [u8; 8] = Default::default();
                section.copy_from_slice(portion);

                let item = u64::from_le_bytes(section);
                let mem_id = MemoryId::from_u64(item);

                self.index = end;

                Ok(ReturnValues::MemorySlice(mem_id))
            }
            ReturnTypeId::Object => {
                let end = index + MOVE_SIXTY_FOUR_BYTES;
                let portion = &bin[index..end];
                let mut section: [u8; 8] = Default::default();
                section.copy_from_slice(portion);

                let item = u64::from_le_bytes(section);

                self.index = end;

                Ok(ReturnValues::Object(item.into()))
            }
            ReturnTypeId::DOMObject => {
                let end = index + MOVE_SIXTY_FOUR_BYTES;
                let portion = &bin[index..end];
                let mut section: [u8; 8] = Default::default();
                section.copy_from_slice(portion);

                let item = u64::from_le_bytes(section);

                self.index = end;

                Ok(ReturnValues::DOMObject(item.into()))
            }
            ReturnTypeId::ExternalReference => {
                let end = index + MOVE_SIXTY_FOUR_BYTES;
                let portion = &bin[index..end];
                let mut section: [u8; 8] = Default::default();
                section.copy_from_slice(portion);

                let item = u64::from_le_bytes(section);

                self.index = end;

                Ok(ReturnValues::ExternalReference(item.into()))
            }
            ReturnTypeId::InternalReference => {
                let end = index + MOVE_SIXTY_FOUR_BYTES;
                let portion = &bin[index..end];
                let mut section: [u8; 8] = Default::default();
                section.copy_from_slice(portion);

                let item = u64::from_le_bytes(section);

                self.index = end;

                Ok(ReturnValues::InternalReference(item.into()))
            }
            ReturnTypeId::Uint8ArrayBuffer => {
                let end = index + MOVE_SIXTY_FOUR_BYTES;
                let portion = &bin[index..end];

                let mut section: [u8; 8] = Default::default();
                section.copy_from_slice(portion);

                let alloc_id = u64::from_le_bytes(section);
                let mem_id = MemoryId::from_u64(alloc_id);

                let memory_result = ALLOCATIONS
                    .lock()
                    .unwrap_or_else(foundation_nostd::comp::basic::PoisonError::into_inner)
                    .get(mem_id);
                if let Err(err) = memory_result {
                    return Some(Err(err.into()));
                }
                let mut memory = memory_result.unwrap();
                let memory_vec = memory.take();
                if memory_vec.is_none() {
                    return Some(Err(BinaryReadError::MemoryError(String::from(
                        "No Vec<u8> not found, big problem",
                    ))));
                }

                if let Err(err) = ALLOCATIONS
                    .lock()
                    .unwrap_or_else(foundation_nostd::comp::basic::PoisonError::into_inner)
                    .deallocate(mem_id)
                {
                    return Some(Err(err.into()));
                }

                self.index = end;

                Ok(ReturnValues::Uint8Array(memory_vec.unwrap()))
            }
            ReturnTypeId::Uint16ArrayBuffer => {
                const TOTAL_U8_IN_U18: usize = 2;

                let end = index + MOVE_SIXTY_FOUR_BYTES;
                let portion = &bin[index..end];

                let mut section: [u8; 8] = Default::default();
                section.copy_from_slice(portion);

                let alloc_id = u64::from_le_bytes(section);
                let mem_id = MemoryId::from_u64(alloc_id);

                let memory_result = ALLOCATIONS
                    .lock()
                    .unwrap_or_else(foundation_nostd::comp::basic::PoisonError::into_inner)
                    .get(mem_id);
                if let Err(err) = memory_result {
                    return Some(Err(err.into()));
                }
                let mut memory = memory_result.unwrap();
                let memory_vec_container = memory.take();
                if memory_vec_container.is_none() {
                    return Some(Err(BinaryReadError::MemoryError(String::from(
                        "No Vec<u8> not found, big problem",
                    ))));
                }

                if let Err(err) = ALLOCATIONS
                    .lock()
                    .unwrap_or_else(foundation_nostd::comp::basic::PoisonError::into_inner)
                    .deallocate(mem_id)
                {
                    return Some(Err(err.into()));
                }

                let memory_vec = memory_vec_container.unwrap();
                let memory_size = memory_vec.len();

                // if the mode of 2 (size in bytes/u8) is not zero then
                // then its an invalid u16 array converted to u8
                if !memory_size.is_multiple_of(TOTAL_U8_IN_U18) {
                    return Some(Err(BinaryReadError::MemoryError(String::from(
                            "Vec<u8> of u16 send as u8 should have even lengths, because u16 in u8 is two u8",
                        ))));
                }

                let arr_size = memory_size / TOTAL_U8_IN_U18;
                let mut arr_content: Vec<u16> = Vec::with_capacity(arr_size);

                let mut move_index = 0;
                while move_index < arr_size {
                    let portion_end = move_index + TOTAL_U8_IN_U18;
                    let portion = &memory_vec[move_index..portion_end];
                    let mut arr: [u8; TOTAL_U8_IN_U18] = Default::default();
                    arr.copy_from_slice(portion);
                    arr_content.push(u16::from_le_bytes(arr));
                    move_index = portion_end;
                }

                self.index = end;

                Ok(ReturnValues::Uint16Array(arr_content))
            }
            ReturnTypeId::Uint32ArrayBuffer => {
                const TOTAL_U8_IN_U32: usize = 4;

                let end = index + MOVE_SIXTY_FOUR_BYTES;
                let portion = &bin[index..end];

                let mut section: [u8; 8] = Default::default();
                section.copy_from_slice(portion);

                let alloc_id = u64::from_le_bytes(section);
                let mem_id = MemoryId::from_u64(alloc_id);

                let memory_result = ALLOCATIONS
                    .lock()
                    .unwrap_or_else(foundation_nostd::comp::basic::PoisonError::into_inner)
                    .get(mem_id);
                if let Err(err) = memory_result {
                    return Some(Err(err.into()));
                }
                let mut memory = memory_result.unwrap();
                let memory_vec_container = memory.take();
                if memory_vec_container.is_none() {
                    return Some(Err(BinaryReadError::MemoryError(String::from(
                        "No Vec<u8> not found, big problem",
                    ))));
                }

                if let Err(err) = ALLOCATIONS
                    .lock()
                    .unwrap_or_else(foundation_nostd::comp::basic::PoisonError::into_inner)
                    .deallocate(mem_id)
                {
                    return Some(Err(err.into()));
                }

                let memory_vec = memory_vec_container.unwrap();
                let memory_size = memory_vec.len();

                // if the mode of 2 (size in bytes/u8) is not zero then
                // then its an invalid u32 array converted to u8
                if !memory_size.is_multiple_of(TOTAL_U8_IN_U32) {
                    return Some(Err(BinaryReadError::MemoryError(String::from(
                            "Vec<u8> of u32 send as u8 should have even lengths, because u32 in u8 is four u8",
                        ))));
                }

                let arr_size = memory_size / TOTAL_U8_IN_U32;
                let mut arr_content: Vec<u32> = Vec::with_capacity(arr_size);

                let mut move_index = 0;
                while move_index < arr_size {
                    let portion_end = move_index + TOTAL_U8_IN_U32;
                    let portion = &memory_vec[move_index..portion_end];
                    let mut arr: [u8; TOTAL_U8_IN_U32] = Default::default();
                    arr.copy_from_slice(portion);
                    arr_content.push(u32::from_le_bytes(arr));
                    move_index = portion_end;
                }

                self.index = end;

                Ok(ReturnValues::Uint32Array(arr_content))
            }
            ReturnTypeId::Uint64ArrayBuffer => {
                const TOTAL_U8_IN_U64: usize = 8;

                let end = index + MOVE_SIXTY_FOUR_BYTES;
                let portion = &bin[index..end];

                let mut section: [u8; 8] = Default::default();
                section.copy_from_slice(portion);

                let alloc_id = u64::from_le_bytes(section);
                let mem_id = MemoryId::from_u64(alloc_id);

                let memory_result = ALLOCATIONS
                    .lock()
                    .unwrap_or_else(foundation_nostd::comp::basic::PoisonError::into_inner)
                    .get(mem_id);
                if let Err(err) = memory_result {
                    return Some(Err(err.into()));
                }
                let mut memory = memory_result.unwrap();
                let memory_vec_container = memory.take();
                if memory_vec_container.is_none() {
                    return Some(Err(BinaryReadError::MemoryError(String::from(
                        "No Vec<u8> not found, big problem",
                    ))));
                }

                if let Err(err) = ALLOCATIONS
                    .lock()
                    .unwrap_or_else(foundation_nostd::comp::basic::PoisonError::into_inner)
                    .deallocate(mem_id)
                {
                    return Some(Err(err.into()));
                }

                let memory_vec = memory_vec_container.unwrap();
                let memory_size = memory_vec.len();

                // if the mode of 2 (size in bytes/u8) is not zero then
                // then its an invalid u64 array converted to u8
                if !memory_size.is_multiple_of(TOTAL_U8_IN_U64) {
                    return Some(Err(BinaryReadError::MemoryError(String::from(
                            "Vec<u8> of u64 send as u8 should have even lengths, because u64 in u8 is eight's u8",
                        ))));
                }

                let arr_size = memory_size / TOTAL_U8_IN_U64;
                let mut arr_content: Vec<u64> = Vec::with_capacity(arr_size);

                let mut move_index = 0;
                while move_index < arr_size {
                    let portion_end = move_index + TOTAL_U8_IN_U64;
                    let portion = &memory_vec[move_index..portion_end];
                    let mut arr: [u8; TOTAL_U8_IN_U64] = Default::default();
                    arr.copy_from_slice(portion);
                    arr_content.push(u64::from_le_bytes(arr));
                    move_index = portion_end;
                }

                self.index = end;

                Ok(ReturnValues::Uint64Array(arr_content))
            }
            ReturnTypeId::Int8ArrayBuffer => {
                let end = index + MOVE_SIXTY_FOUR_BYTES;
                let portion = &bin[index..end];

                let mut section: [u8; 8] = Default::default();
                section.copy_from_slice(portion);

                let alloc_id = u64::from_le_bytes(section);
                let mem_id = MemoryId::from_u64(alloc_id);

                let memory_result = ALLOCATIONS
                    .lock()
                    .unwrap_or_else(foundation_nostd::comp::basic::PoisonError::into_inner)
                    .get(mem_id);
                if let Err(err) = memory_result {
                    return Some(Err(err.into()));
                }
                let mut memory = memory_result.unwrap();
                let memory_vec_container = memory.take();
                if memory_vec_container.is_none() {
                    return Some(Err(BinaryReadError::MemoryError(String::from(
                        "No Vec<u8> not found, big problem",
                    ))));
                }

                if let Err(err) = ALLOCATIONS
                    .lock()
                    .unwrap_or_else(foundation_nostd::comp::basic::PoisonError::into_inner)
                    .deallocate(mem_id)
                {
                    return Some(Err(err.into()));
                }

                let memory_vec = memory_vec_container.unwrap();
                let mut arr_content: Vec<i8> = Vec::with_capacity(memory_vec.len());

                for value in memory_vec {
                    arr_content.push(i8::from_le(value as i8));
                }

                self.index = end;

                Ok(ReturnValues::Int8Array(arr_content))
            }
            ReturnTypeId::Int16ArrayBuffer => {
                const TOTAL_U8_IN_U16: usize = 2;

                let end = index + MOVE_SIXTY_FOUR_BYTES;
                let portion = &bin[index..end];

                let mut section: [u8; 8] = Default::default();
                section.copy_from_slice(portion);

                let alloc_id = u64::from_le_bytes(section);
                let mem_id = MemoryId::from_u64(alloc_id);

                let memory_result = ALLOCATIONS
                    .lock()
                    .unwrap_or_else(foundation_nostd::comp::basic::PoisonError::into_inner)
                    .get(mem_id);
                if let Err(err) = memory_result {
                    return Some(Err(err.into()));
                }
                let mut memory = memory_result.unwrap();
                let memory_vec_container = memory.take();
                if memory_vec_container.is_none() {
                    return Some(Err(BinaryReadError::MemoryError(String::from(
                        "No Vec<u8> not found, big problem",
                    ))));
                }

                if let Err(err) = ALLOCATIONS
                    .lock()
                    .unwrap_or_else(foundation_nostd::comp::basic::PoisonError::into_inner)
                    .deallocate(mem_id)
                {
                    return Some(Err(err.into()));
                }

                let memory_vec = memory_vec_container.unwrap();
                let memory_size = memory_vec.len();

                // if the mode of 2 (size in bytes/u8) is not zero then
                // then its an invalid u16 array converted to u8
                if !memory_size.is_multiple_of(TOTAL_U8_IN_U16) {
                    return Some(Err(BinaryReadError::MemoryError(String::from(
                            "Vec<u8> of u16 send as u8 should have even lengths, because u16 in u8 is two u8",
                        ))));
                }

                let arr_size = memory_size / TOTAL_U8_IN_U16;
                let mut arr_content: Vec<i16> = Vec::with_capacity(arr_size);

                let mut move_index = 0;
                while move_index < arr_size {
                    let portion_end = move_index + TOTAL_U8_IN_U16;
                    let portion = &memory_vec[move_index..portion_end];
                    let mut arr: [u8; TOTAL_U8_IN_U16] = Default::default();
                    arr.copy_from_slice(portion);
                    arr_content.push(i16::from_le_bytes(arr));
                    move_index = portion_end;
                }

                self.index = end;

                Ok(ReturnValues::Int16Array(arr_content))
            }
            ReturnTypeId::Int32ArrayBuffer => {
                const TOTAL_U8_IN_U32: usize = 4;

                let end = index + MOVE_SIXTY_FOUR_BYTES;
                let portion = &bin[index..end];

                let mut section: [u8; 8] = Default::default();
                section.copy_from_slice(portion);

                let alloc_id = u64::from_le_bytes(section);
                let mem_id = MemoryId::from_u64(alloc_id);

                let memory_result = ALLOCATIONS
                    .lock()
                    .unwrap_or_else(foundation_nostd::comp::basic::PoisonError::into_inner)
                    .get(mem_id);
                if let Err(err) = memory_result {
                    return Some(Err(err.into()));
                }
                let mut memory = memory_result.unwrap();
                let memory_vec_container = memory.take();
                if memory_vec_container.is_none() {
                    return Some(Err(BinaryReadError::MemoryError(String::from(
                        "No Vec<u8> not found, big problem",
                    ))));
                }

                if let Err(err) = ALLOCATIONS
                    .lock()
                    .unwrap_or_else(foundation_nostd::comp::basic::PoisonError::into_inner)
                    .deallocate(mem_id)
                {
                    return Some(Err(err.into()));
                }

                let memory_vec = memory_vec_container.unwrap();
                let memory_size = memory_vec.len();

                // if the mode of 2 (size in bytes/u8) is not zero then
                // then its an invalid u16 array converted to u8
                if !memory_size.is_multiple_of(TOTAL_U8_IN_U32) {
                    return Some(Err(BinaryReadError::MemoryError(String::from(
                            "Vec<u8> of u16 send as u8 should have even lengths, because u16 in u8 is four u8",
                        ))));
                }

                let arr_size = memory_size / TOTAL_U8_IN_U32;
                let mut arr_content: Vec<i32> = Vec::with_capacity(arr_size);

                let mut move_index = 0;
                while move_index < arr_size {
                    let portion_end = move_index + TOTAL_U8_IN_U32;
                    let portion = &memory_vec[move_index..portion_end];
                    let mut arr: [u8; TOTAL_U8_IN_U32] = Default::default();
                    arr.copy_from_slice(portion);
                    arr_content.push(i32::from_le_bytes(arr));
                    move_index = portion_end;
                }

                self.index = end;

                Ok(ReturnValues::Int32Array(arr_content))
            }
            ReturnTypeId::Int64ArrayBuffer => {
                const TOTAL_U8_IN_U64: usize = 8;

                let end = index + MOVE_SIXTY_FOUR_BYTES;
                let portion = &bin[index..end];

                let mut section: [u8; 8] = Default::default();
                section.copy_from_slice(portion);

                let alloc_id = u64::from_le_bytes(section);
                let mem_id = MemoryId::from_u64(alloc_id);

                let memory_result = ALLOCATIONS
                    .lock()
                    .unwrap_or_else(foundation_nostd::comp::basic::PoisonError::into_inner)
                    .get(mem_id);
                if let Err(err) = memory_result {
                    return Some(Err(err.into()));
                }
                let mut memory = memory_result.unwrap();
                let memory_vec_container = memory.take();
                if memory_vec_container.is_none() {
                    return Some(Err(BinaryReadError::MemoryError(String::from(
                        "No Vec<u8> not found, big problem",
                    ))));
                }

                if let Err(err) = ALLOCATIONS
                    .lock()
                    .unwrap_or_else(foundation_nostd::comp::basic::PoisonError::into_inner)
                    .deallocate(mem_id)
                {
                    return Some(Err(err.into()));
                }

                let memory_vec = memory_vec_container.unwrap();
                let memory_size = memory_vec.len();

                // if the mode of 2 (size in bytes/u8) is not zero then
                // then its an invalid u16 array converted to u8
                if !memory_size.is_multiple_of(TOTAL_U8_IN_U64) {
                    return Some(Err(BinaryReadError::MemoryError(String::from(
                            "Vec<u8> of u16 send as u8 should have even lengths, because u16 in u8 is eight's u8",
                        ))));
                }

                let arr_size = memory_size / TOTAL_U8_IN_U64;
                let mut arr_content: Vec<i64> = Vec::with_capacity(arr_size);

                let mut move_index = 0;
                while move_index < arr_size {
                    let portion_end = move_index + TOTAL_U8_IN_U64;
                    let portion = &memory_vec[move_index..portion_end];
                    let mut arr: [u8; TOTAL_U8_IN_U64] = Default::default();
                    arr.copy_from_slice(portion);
                    arr_content.push(i64::from_le_bytes(arr));
                    move_index = portion_end;
                }

                self.index = end;

                Ok(ReturnValues::Int64Array(arr_content))
            }
            ReturnTypeId::Float32ArrayBuffer => {
                const TOTAL_U8_IN_F32: usize = 4;

                let end = index + MOVE_SIXTY_FOUR_BYTES;
                let portion = &bin[index..end];

                let mut section: [u8; 8] = Default::default();
                section.copy_from_slice(portion);

                let alloc_id = u64::from_le_bytes(section);
                let mem_id = MemoryId::from_u64(alloc_id);

                let memory_result = ALLOCATIONS
                    .lock()
                    .unwrap_or_else(foundation_nostd::comp::basic::PoisonError::into_inner)
                    .get(mem_id);
                if let Err(err) = memory_result {
                    return Some(Err(err.into()));
                }
                let mut memory = memory_result.unwrap();
                let memory_vec_container = memory.take();
                if memory_vec_container.is_none() {
                    return Some(Err(BinaryReadError::MemoryError(String::from(
                        "No Vec<u8> not found, big problem",
                    ))));
                }

                if let Err(err) = ALLOCATIONS
                    .lock()
                    .unwrap_or_else(foundation_nostd::comp::basic::PoisonError::into_inner)
                    .deallocate(mem_id)
                {
                    return Some(Err(err.into()));
                }

                let memory_vec = memory_vec_container.unwrap();
                let memory_size = memory_vec.len();

                // if the mode of 2 (size in bytes/u8) is not zero then
                // then its an invalid u16 array converted to u8
                if !memory_size.is_multiple_of(TOTAL_U8_IN_F32) {
                    return Some(Err(BinaryReadError::MemoryError(String::from(
                            "Vec<u8> of u16 send as u8 should have even lengths, because u16 in u8 is four's u8",
                        ))));
                }

                let arr_size = memory_size / TOTAL_U8_IN_F32;
                let mut arr_content: Vec<f32> = Vec::with_capacity(arr_size);

                let mut move_index = 0;
                while move_index < arr_size {
                    let portion_end = move_index + TOTAL_U8_IN_F32;
                    let portion = &memory_vec[move_index..portion_end];
                    let mut arr: [u8; TOTAL_U8_IN_F32] = Default::default();
                    arr.copy_from_slice(portion);
                    arr_content.push(f32::from_le_bytes(arr));
                    move_index = portion_end;
                }

                self.index = end;

                Ok(ReturnValues::Float32Array(arr_content))
            }
            ReturnTypeId::Float64ArrayBuffer => {
                const TOTAL_U8_IN_F64: usize = 8;

                let end = index + MOVE_SIXTY_FOUR_BYTES;
                let portion = &bin[index..end];

                let mut section: [u8; 8] = Default::default();
                section.copy_from_slice(portion);

                let alloc_id = u64::from_le_bytes(section);
                let mem_id = MemoryId::from_u64(alloc_id);

                let memory_result = ALLOCATIONS
                    .lock()
                    .unwrap_or_else(foundation_nostd::comp::basic::PoisonError::into_inner)
                    .get(mem_id);
                if let Err(err) = memory_result {
                    return Some(Err(err.into()));
                }
                let mut memory = memory_result.unwrap();
                let memory_vec_container = memory.take();
                if memory_vec_container.is_none() {
                    return Some(Err(BinaryReadError::MemoryError(String::from(
                        "No Vec<u8> not found, big problem",
                    ))));
                }

                if let Err(err) = ALLOCATIONS
                    .lock()
                    .unwrap_or_else(foundation_nostd::comp::basic::PoisonError::into_inner)
                    .deallocate(mem_id)
                {
                    return Some(Err(err.into()));
                }

                let memory_vec = memory_vec_container.unwrap();
                let memory_size = memory_vec.len();

                // if the mode of 2 (size in bytes/u8) is not zero then
                // then its an invalid u16 array converted to u8
                if !memory_size.is_multiple_of(TOTAL_U8_IN_F64) {
                    return Some(Err(BinaryReadError::MemoryError(String::from(
                            "Vec<u8> of u16 send as u8 should have even lengths, because u16 in u8 is eight's u8",
                        ))));
                }

                let arr_size = memory_size / TOTAL_U8_IN_F64;
                let mut arr_content: Vec<f64> = Vec::with_capacity(arr_size);

                let mut move_index = 0;
                while move_index < arr_size {
                    let portion_end = move_index + TOTAL_U8_IN_F64;
                    let portion = &memory_vec[move_index..portion_end];
                    let mut arr: [u8; TOTAL_U8_IN_F64] = Default::default();
                    arr.copy_from_slice(portion);
                    arr_content.push(f64::from_le_bytes(arr));
                    move_index = portion_end;
                }

                self.index = end;

                Ok(ReturnValues::Float64Array(arr_content))
            }
            ReturnTypeId::Text8 => {
                let end = index + MOVE_SIXTY_FOUR_BYTES;
                let portion = &bin[index..end];

                let mut section: [u8; 8] = Default::default();
                section.copy_from_slice(portion);

                let alloc_id = u64::from_le_bytes(section);
                let mem_id = MemoryId::from_u64(alloc_id);

                let memory_result = ALLOCATIONS
                    .lock()
                    .unwrap_or_else(foundation_nostd::comp::basic::PoisonError::into_inner)
                    .get(mem_id);
                if let Err(err) = memory_result {
                    return Some(Err(err.into()));
                }
                let mut memory = memory_result.unwrap();
                let memory_vec_container = memory.take();
                if memory_vec_container.is_none() {
                    return Some(Err(BinaryReadError::MemoryError(String::from(
                        "No Vec<u8> not found, big problem",
                    ))));
                }

                if let Err(err) = ALLOCATIONS
                    .lock()
                    .unwrap_or_else(foundation_nostd::comp::basic::PoisonError::into_inner)
                    .deallocate(mem_id)
                {
                    return Some(Err(err.into()));
                }

                let memory_vec = memory_vec_container.unwrap();

                let value = match String::from_utf8(memory_vec) {
                    Ok(content) => ReturnValues::Text8(content),
                    Err(_) => {
                        return Some(Err(BinaryReadError::ExpectedStringInCode(
                            ReturnTypeId::Text8 as u8,
                        )));
                    }
                };

                self.index = end;

                Ok(value)
            }
        };

        Some(result)
    }
}

// -- As an iterator

impl Iterator for ReturnValueParserIter<'_> {
    type Item = BinaryReaderResult<ReturnValues>;

    fn next(&mut self) -> Option<Self::Item> {
        match self.parse_next()? {
            Ok(item) => {
                let item_value_type_id = item.to_return_value_type();

                match self.hint.clone() {
                    ReturnTypeHints::One(state) => match state {
                        crate::ThreeState::One(return_type_id) => {
                            if item_value_type_id != return_type_id {
                                return Some(Err(BinaryReadError::NotMatchingTypeHint(
                                    state,
                                    item_value_type_id,
                                )));
                            }
                        }
                        crate::ThreeState::Two(p1, p2) => {
                            if item_value_type_id != p1 && item_value_type_id != p2 {
                                return Some(Err(BinaryReadError::NotMatchingTypeHint(
                                    state,
                                    item_value_type_id,
                                )));
                            }
                        }
                        crate::ThreeState::Three(p1, p2, p3) => {
                            if item_value_type_id != p1
                                && item_value_type_id != p2
                                && item_value_type_id != p3
                            {
                                return Some(Err(BinaryReadError::NotMatchingTypeHint(
                                    state,
                                    item_value_type_id,
                                )));
                            }
                        }
                    },
                    ReturnTypeHints::Multi(states) => {
                        let state = states[self.item_index].clone();
                        match state {
                            crate::ThreeState::One(return_type_id) => {
                                if item_value_type_id != return_type_id {
                                    return Some(Err(BinaryReadError::NotMatchingTypeHint(
                                        state,
                                        item_value_type_id,
                                    )));
                                }
                            }
                            crate::ThreeState::Two(p1, p2) => {
                                if item_value_type_id != p1 && item_value_type_id != p2 {
                                    return Some(Err(BinaryReadError::NotMatchingTypeHint(
                                        state,
                                        item_value_type_id,
                                    )));
                                }
                            }
                            crate::ThreeState::Three(p1, p2, p3) => {
                                if item_value_type_id != p1
                                    && item_value_type_id != p2
                                    && item_value_type_id != p3
                                {
                                    return Some(Err(BinaryReadError::NotMatchingTypeHint(
                                        state,
                                        item_value_type_id,
                                    )));
                                }
                            }
                        }
                        self.item_index += 1;
                    }
                    ReturnTypeHints::List(state) => match state {
                        crate::ThreeState::One(return_type_id) => {
                            if item_value_type_id != return_type_id {
                                return Some(Err(BinaryReadError::NotMatchingTypeHint(
                                    state,
                                    item_value_type_id,
                                )));
                            }
                        }
                        crate::ThreeState::Two(p1, p2) => {
                            if item_value_type_id != p1 && item_value_type_id != p2 {
                                return Some(Err(BinaryReadError::NotMatchingTypeHint(
                                    state,
                                    item_value_type_id,
                                )));
                            }
                        }
                        crate::ThreeState::Three(p1, p2, p3) => {
                            if item_value_type_id != p1
                                && item_value_type_id != p2
                                && item_value_type_id != p3
                            {
                                return Some(Err(BinaryReadError::NotMatchingTypeHint(
                                    state,
                                    item_value_type_id,
                                )));
                            }
                        }
                    },
                    ReturnTypeHints::None => unreachable!("Should never be called"),
                }

                Some(Ok(item))
            }
            Err(err) => Some(Err(err)),
        }
    }
}

impl FromBinary for ReturnTypeHints {
    type T = Vec<ReturnValues>;

    fn from_binary(self, input_bin: &[u8]) -> BinaryReaderResult<Self::T> {
        if input_bin[0] != (ReturnValueMarker::Begin as u8) {
            return Err(BinaryReadError::WrongStarterCode(input_bin[0]));
        }

        let length = input_bin.len();
        if input_bin[length - 1] != (ReturnValueMarker::End as u8) {
            return Err(BinaryReadError::WrongEndingCode(input_bin[length - 1]));
        }

        let value_start = 1;
        let value_end = length - 1;

        let bin = &input_bin[value_start..value_end];

        let mut decoded = Vec::with_capacity(1);
        let parser = ReturnValueParserIter::new(self, bin);
        for parsed_item in parser {
            match parsed_item {
                Ok(item) => {
                    decoded.push(item);
                    continue;
                }
                Err(err) => {
                    return Err(err);
                }
            }
        }

        Ok(decoded)
    }
}

/// [`GroupReturnTypeHints`] represents conversion of
/// underlying type which is a grouping of return values
/// from the host where it represent a batch of return values
/// that should be generated/materialized.
#[derive(Default)]
pub struct GroupReturnTypeHints;

impl FromBinary for GroupReturnTypeHints {
    type T = Vec<Returns>;

    fn from_binary(self, input_bin: &[u8]) -> BinaryReaderResult<Self::T> {
        if input_bin[0] != (GroupReturnHintMarker::Start as u8) {
            return Err(BinaryReadError::WrongStarterCode(input_bin[0]));
        }

        let length = input_bin.len();
        if input_bin[length - 1] != (GroupReturnHintMarker::Stop as u8) {
            return Err(BinaryReadError::WrongEndingCode(input_bin[length - 1]));
        }

        let value_start = 1;
        let value_end = length - 1;

        let bin = &input_bin[value_start..value_end];
        // panic!("Received binary info: {:?}", bin);

        let mut decoded = Vec::with_capacity(2);

        let mut index = 0;

        while index < bin.len() {
            let reply_type: ReturnIds = u8::from_le(bin[index]).into();
            index += MOVE_ONE_BYTE;

            let return_hint: ReturnTypeHints = match reply_type {
                ReturnIds::One => {
                    let state_type: ThreeStateId = u8::from_le(bin[index]).into();
                    index += MOVE_ONE_BYTE;

                    ReturnTypeHints::One(match state_type {
                        ThreeStateId::One => {
                            let value_type: ReturnTypeId = u8::from_le(bin[index]).into();
                            index += MOVE_ONE_BYTE;

                            ThreeState::One(value_type)
                        }
                        ThreeStateId::Two => {
                            let p1: ReturnTypeId = u8::from_le(bin[index]).into();
                            index += MOVE_ONE_BYTE;

                            let p2: ReturnTypeId = u8::from_le(bin[index]).into();
                            index += MOVE_ONE_BYTE;

                            ThreeState::Two(p1, p2)
                        }
                        ThreeStateId::Three => {
                            let p1: ReturnTypeId = u8::from_le(bin[index]).into();
                            index += MOVE_ONE_BYTE;

                            let p2: ReturnTypeId = u8::from_le(bin[index]).into();
                            index += MOVE_ONE_BYTE;

                            let p3: ReturnTypeId = u8::from_le(bin[index]).into();
                            index += MOVE_ONE_BYTE;

                            ThreeState::Three(p1, p2, p3)
                        }
                    })
                }
                ReturnIds::List => {
                    let state_type: ThreeStateId = u8::from_le(bin[index]).into();
                    index += MOVE_ONE_BYTE;

                    ReturnTypeHints::List(match state_type {
                        ThreeStateId::One => {
                            let value_type: ReturnTypeId = u8::from_le(bin[index]).into();
                            index += MOVE_ONE_BYTE;

                            ThreeState::One(value_type)
                        }
                        ThreeStateId::Two => {
                            let p1: ReturnTypeId = u8::from_le(bin[index]).into();
                            index += MOVE_ONE_BYTE;

                            let p2: ReturnTypeId = u8::from_le(bin[index]).into();
                            index += MOVE_ONE_BYTE;

                            ThreeState::Two(p1, p2)
                        }
                        ThreeStateId::Three => {
                            let p1: ReturnTypeId = u8::from_le(bin[index]).into();
                            index += MOVE_ONE_BYTE;

                            let p2: ReturnTypeId = u8::from_le(bin[index]).into();
                            index += MOVE_ONE_BYTE;

                            let p3: ReturnTypeId = u8::from_le(bin[index]).into();
                            index += MOVE_ONE_BYTE;

                            ThreeState::Three(p1, p2, p3)
                        }
                    })
                }
                ReturnIds::Multi => {
                    let item_count_start = index;
                    let item_count_end = index + MOVE_SIXTEEN_BYTES;
                    index = item_count_end;

                    let item_count_slice = &bin[item_count_start..item_count_end];
                    let mut item_count_arr: [u8; 2] = Default::default();
                    item_count_arr.copy_from_slice(item_count_slice);

                    let item_count = u16::from_le_bytes(item_count_arr);

                    let mut value_types = Vec::with_capacity(item_count as usize);
                    for _ in 0..item_count {
                        let state_type: ThreeStateId = u8::from_le(bin[index]).into();
                        index += MOVE_ONE_BYTE;

                        value_types.push(match state_type {
                            ThreeStateId::One => {
                                let value_type: ReturnTypeId = u8::from_le(bin[index]).into();
                                index += MOVE_ONE_BYTE;

                                ThreeState::One(value_type)
                            }
                            ThreeStateId::Two => {
                                let p1: ReturnTypeId = u8::from_le(bin[index]).into();
                                index += MOVE_ONE_BYTE;

                                let p2: ReturnTypeId = u8::from_le(bin[index]).into();
                                index += MOVE_ONE_BYTE;

                                ThreeState::Two(p1, p2)
                            }
                            ThreeStateId::Three => {
                                let p1: ReturnTypeId = u8::from_le(bin[index]).into();
                                index += MOVE_ONE_BYTE;

                                let p2: ReturnTypeId = u8::from_le(bin[index]).into();
                                index += MOVE_ONE_BYTE;

                                let p3: ReturnTypeId = u8::from_le(bin[index]).into();
                                index += MOVE_ONE_BYTE;

                                ThreeState::Three(p1, p2, p3)
                            }
                        });
                    }

                    ReturnTypeHints::Multi(value_types)
                }
                ReturnIds::None => unreachable!("should never get type of value from host"),
            };

            let end = index + MOVE_SIXTY_FOUR_BYTES;
            let portion = &bin[index..end];

            index = end;

            let mut section: [u8; 8] = Default::default();
            section.copy_from_slice(portion);

            let alloc_id = u64::from_le_bytes(section);
            let mem_id = MemoryId::from_u64(alloc_id);

            let memory_result = ALLOCATIONS
                .lock()
                .unwrap_or_else(foundation_nostd::comp::basic::PoisonError::into_inner)
                .get(mem_id);
            assert!(
                memory_result.is_ok(),
                "GroupReturnTypeHints: Received memoryId: {:?} -> {:?} -- result: {:?}",
                &mem_id,
                &return_hint,
                &memory_result
            );
            if let Err(err) = memory_result {
                return Err(err.into());
            }
            let memory = memory_result.unwrap();

            match memory.into_with(|mem| return_hint.clone().from_binary(mem.as_ref())) {
                Some(item_result) => {
                    let mut item = item_result?;

                    let value_item = match return_hint {
                        ReturnTypeHints::One(_) => {
                            if item.len() != 1 {
                                return Err(BinaryReadError::MemoryError(String::from(
                                    "more than one item for ReturnIds::One(_)",
                                )));
                            }
                            Returns::One(item.pop().expect("valid index"))
                        }
                        ReturnTypeHints::List(_) => Returns::List(item),
                        ReturnTypeHints::Multi(_) => Returns::Multi(item),
                        ReturnTypeHints::None => {
                            unreachable!("should never get return type from group")
                        }
                    };

                    decoded.push(value_item);
                }
                None => {
                    return Err(BinaryReadError::MemoryError(String::from(
                        "expected a valid returned value not None",
                    )));
                }
            }

            if let Err(err) = ALLOCATIONS
                .lock()
                .unwrap_or_else(foundation_nostd::comp::basic::PoisonError::into_inner)
                .deallocate(mem_id)
            {
                return Err(err.into());
            }
        }

        Ok(decoded)
    }
}
