//! WHY: Protocol byte 0 — Custom Binary — IS the `foundation_wasm` Instructions
//! format (decision 022): the batch-operations system (`Operations` opcodes,
//! quantized V2 params, texts pool) that processes selectively use to send
//! messages. It is an instruction STREAM the host executes, not a value encoding,
//! so it composes differently from Arrow/JSON: payloads are built by
//! [`BatchMessage`] implementors and executed by the JS `BatchInstructions`
//! runtime.
//!
//! WHAT: [`BatchInstructionsV1`] — the byte-0 [`ProtocolHandler`] +
//! [`ProtocolMethods`] built on top of the batch system — plus [`DomOpsBatch`]
//! (`DomOp`s as a registered batch operation, [`BATCH_OP_APPLY_DOM`]) so DOM
//! batches CAN ride this protocol (Arrow/JSON remain their default transports).
//!
//! HOW: `send_message` builds an [`Instructions`] batch (ops + texts arena slots),
//! packs both into ONE envelope slot — `[texts_off:u32 LE][texts_len:u32 LE]
//! [ops stream][texts pool]` — and ships it via the uniform `host_apply`. The JS
//! side splits the payload at `texts_off` and feeds the live-memory pointers to
//! the existing `BatchInstructions` machinery (`batchProtocolHandler` in
//! foundation-wasm.js); DOM handling registers [`BATCH_OP_APPLY_DOM`] via
//! `registerOperation` (foundation-wasm-ui.js). `handle_received` decodes the
//! byte-0 payload back to `DomOp`s natively so round-trips stay testable
//! without a JS host.

use alloc::string::String;
use alloc::vec::Vec;

use foundation_ui_traits::{DecodeError, DomOp, Row, PROTOCOL_CUSTOM_BINARY, PROTOCOL_VERSION};
use foundation_wasm::{
    ArgumentOperations, BatchMessage, Instructions, MemoryAllocations, MemoryId, Operations,
    ParamTypeId, Params, ProtocolHandler, TypeOptimization,
};

use super::{ship, ship_payload, HandleResult, ProtocolMethods, SendResult};

/// The registered batch opcode carrying one `DomOp` row
/// (`[opcode][ArgStart (params…) ArgStop][Operations::End]`). Outside the core
/// `Operations` table (0–3, 254, 255) — registered selectively on the JS side via
/// `BatchInstructions.registerOperation(BATCH_OP_APPLY_DOM, …)`.
pub const BATCH_OP_APPLY_DOM: u8 = 10;

// ─── DomOp as a batch message ───────────────────────────────────────────────────

/// A `DomOp` batch riding the instructions system: one [`BATCH_OP_APPLY_DOM`]
/// operation per op, with the canonical [`Row`] fields as quantized params
/// (strings through the texts pool as `Text8`).
pub struct DomOpsBatch<'a>(pub &'a [DomOp]);

impl BatchMessage for DomOpsBatch<'_> {
    fn encode_batch(
        &self,
        batch: &Instructions,
    ) -> foundation_wasm::MemoryWriterResult<()> {
        use foundation_wasm::BatchEncodable;
        for op in self.0 {
            let row = Row::from_op(op);
            batch.data(&[BATCH_OP_APPLY_DOM])?;
            batch.encode_params(Some(&[
                Params::Uint8(row.operation),
                Params::Uint32(row.node_id),
                Params::Text8(&row.attribute),
                Params::Text8(&row.value),
                Params::Text8(&row.text_val),
            ]))?;
            batch.end()?;
        }
        Ok(())
    }
}

// ─── BatchInstructionsV1 (protocol byte 0) ───────────────────────────────────────

/// Custom Binary v1 — protocol byte `0`, the batch-instructions transport.
#[derive(Clone, Copy, Debug, Default)]
pub struct BatchInstructionsV1;

impl BatchInstructionsV1 {
    /// Construct the byte-0 protocol handler.
    #[must_use]
    pub const fn new() -> Self {
        Self
    }

    /// Build an [`Instructions`] batch from `message`, pack it into ONE envelope
    /// slot (`[texts_off:u32][texts_len:u32][ops][texts]`), and ship it.
    ///
    /// This is the generic entry any [`BatchMessage`] type uses; `DomOp` batches
    /// go through it via [`ProtocolMethods::encode_and_send`].
    ///
    /// # Panics
    /// Panics if the arena cannot allocate or address a slot — an allocation
    /// failure here means a fundamental arena leak (decision 028, Error Cases).
    pub fn send_message<T: BatchMessage>(
        &self,
        message: &T,
        memory: &mut MemoryAllocations,
    ) -> SendResult {
        let batch = memory
            .batch_for(64, 64, true)
            .expect("arena batch_for failed");
        message.encode_batch(&batch).expect("batch encode failed");
        let completed = batch.complete().expect("batch complete failed");

        let ops_bytes = memory
            .get(completed.ops_id)
            .expect("arena get ops failed")
            .clone_memory()
            .expect("read ops bytes");
        let text_bytes = memory
            .get(completed.text_id)
            .expect("arena get texts failed")
            .clone_memory()
            .expect("read text bytes");
        let _ = memory.deallocate(completed.ops_id);
        let _ = memory.deallocate(completed.text_id);

        let texts_off = 8 + ops_bytes.len();
        let mut payload = Vec::with_capacity(texts_off + text_bytes.len());
        payload.extend_from_slice(&u32::try_from(texts_off).expect("ops too large").to_le_bytes());
        payload
            .extend_from_slice(&u32::try_from(text_bytes.len()).expect("texts too large").to_le_bytes());
        payload.extend_from_slice(&ops_bytes);
        payload.extend_from_slice(&text_bytes);

        ship_payload(self, &payload, memory)
    }
}

impl ProtocolHandler for BatchInstructionsV1 {
    fn protocol_byte(&self) -> u8 {
        PROTOCOL_CUSTOM_BINARY
    }

    fn version(&self) -> u8 {
        PROTOCOL_VERSION
    }

    fn send_to_js(&self, memory_id: MemoryId, ptr: *const u8, len: usize) {
        ship(memory_id, ptr, len);
    }

    fn handle_from_js(&self, memory_id: MemoryId, ptr: *const u8, len: usize) {
        let _ = self.handle_received(memory_id, ptr, len);
    }
}

impl ProtocolMethods<Vec<DomOp>> for BatchInstructionsV1 {
    fn encode_and_send(&self, ops: Vec<DomOp>, memory: &mut MemoryAllocations) -> SendResult {
        self.send_message(&DomOpsBatch(&ops), memory)
    }

    fn handle_received(&self, _memory_id: MemoryId, ptr: *const u8, len: usize) -> HandleResult {
        // SAFETY: the caller guarantees (ptr, len) describe a readable region.
        let payload = unsafe { core::slice::from_raw_parts(ptr, len) };
        decode_dom_batch(payload)
    }
}

// ─── Native decoder for the byte-0 DomOp payload ─────────────────────────────────
//
// The batch stream is normally EXECUTED by the JS host; this native reader exists
// so byte-0 DomOp round-trips stay testable without a JS runtime. It covers exactly
// the encodings `DomOpsBatch` emits (Uint8 raw, Uint32 quantized, Text8 via the
// texts pool) — the full V2 table lives on the JS side (`BatchParameterParser`).

struct Reader<'a> {
    bytes: &'a [u8],
    at: usize,
}

impl<'a> Reader<'a> {
    fn u8(&mut self) -> Result<u8, DecodeError> {
        let b = *self.bytes.get(self.at).ok_or(DecodeError::UnexpectedEnd)?;
        self.at += 1;
        Ok(b)
    }

    fn take(&mut self, n: usize) -> Result<&'a [u8], DecodeError> {
        let end = self.at.checked_add(n).ok_or(DecodeError::InvalidLength)?;
        let slice = self.bytes.get(self.at..end).ok_or(DecodeError::UnexpectedEnd)?;
        self.at = end;
        Ok(slice)
    }

    fn expect(&mut self, marker: u8) -> Result<(), DecodeError> {
        let b = self.u8()?;
        if b == marker {
            Ok(())
        } else {
            Err(DecodeError::UnknownOperation(b))
        }
    }

    /// Read one quantized unsigned value: `[TypeOptimization][bytes…]`.
    fn quantized_uint(&mut self) -> Result<u64, DecodeError> {
        let tq = self.u8()?;
        let tq = TypeOptimization::from(tq);
        let value = match tq {
            TypeOptimization::QuantizedUint16AsU8
            | TypeOptimization::QuantizedUint32AsU8
            | TypeOptimization::QuantizedUint64AsU8
            | TypeOptimization::QuantizedPtrAsU8 => u64::from(self.u8()?),
            TypeOptimization::QuantizedUint32AsU16
            | TypeOptimization::QuantizedUint64AsU16
            | TypeOptimization::QuantizedPtrAsU16 => {
                let b = self.take(2)?;
                u64::from(u16::from_le_bytes([b[0], b[1]]))
            }
            TypeOptimization::QuantizedUint64AsU32 | TypeOptimization::QuantizedPtrAsU32 => {
                let b = self.take(4)?;
                u64::from(u32::from_le_bytes([b[0], b[1], b[2], b[3]]))
            }
            TypeOptimization::None => {
                // Width depends on the declared param type; DomOpsBatch only emits
                // Uint32 (4) and Text8 locations (u64, 8) unquantized. The caller
                // passes the width through `quantized_uint_none_width`.
                return Err(DecodeError::InvalidLength);
            }
            _ => return Err(DecodeError::InvalidLength),
        };
        Ok(value)
    }

    /// Like [`Reader::quantized_uint`], but with the full width for
    /// `TypeOptimization::None` known from the declared param type.
    fn quantized_uint_with_width(&mut self, none_width: usize) -> Result<u64, DecodeError> {
        let tq_byte = *self.bytes.get(self.at).ok_or(DecodeError::UnexpectedEnd)?;
        if TypeOptimization::from(tq_byte) == TypeOptimization::None {
            self.at += 1;
            let b = self.take(none_width)?;
            let mut value = 0u64;
            for (i, byte) in b.iter().enumerate() {
                value |= u64::from(*byte) << (8 * i);
            }
            return Ok(value);
        }
        self.quantized_uint()
    }
}

/// Decode a byte-0 payload (`[texts_off][texts_len][ops][texts]`) built by
/// [`DomOpsBatch`] back into `DomOp`s.
fn decode_dom_batch(payload: &[u8]) -> HandleResult {
    if payload.len() < 8 {
        return Err(DecodeError::UnexpectedEnd);
    }
    let texts_off = u32::from_le_bytes([payload[0], payload[1], payload[2], payload[3]]) as usize;
    let texts_len = u32::from_le_bytes([payload[4], payload[5], payload[6], payload[7]]) as usize;
    let ops = payload.get(8..texts_off).ok_or(DecodeError::InvalidLength)?;
    let texts_end = texts_off.checked_add(texts_len).ok_or(DecodeError::InvalidLength)?;
    let texts = payload
        .get(texts_off..texts_end)
        .ok_or(DecodeError::InvalidLength)?;

    let mut r = Reader { bytes: ops, at: 0 };
    r.expect(Operations::Begin as u8)?;

    let mut decoded = Vec::new();
    loop {
        let opcode = r.u8()?;
        if opcode == Operations::Stop as u8 {
            break;
        }
        if opcode != BATCH_OP_APPLY_DOM {
            return Err(DecodeError::UnknownOperation(opcode));
        }

        r.expect(ArgumentOperations::Start as u8)?;
        let mut fields: Vec<ParamField> = Vec::with_capacity(5);
        loop {
            let marker = r.u8()?;
            if marker == ArgumentOperations::Stop as u8 {
                break;
            }
            if marker != ArgumentOperations::Begin as u8 {
                return Err(DecodeError::UnknownOperation(marker));
            }
            let ty = r.u8()?;
            let field = if ty == ParamTypeId::Uint8 as u8 {
                ParamField::Uint(u64::from(r.u8()?))
            } else if ty == ParamTypeId::Uint32 as u8 {
                ParamField::Uint(r.quantized_uint_with_width(4)?)
            } else if ty == ParamTypeId::Text8 as u8 {
                let index = r.quantized_uint_with_width(8)? as usize;
                let len = r.quantized_uint_with_width(8)? as usize;
                let end = index.checked_add(len).ok_or(DecodeError::InvalidLength)?;
                let bytes = texts.get(index..end).ok_or(DecodeError::InvalidLength)?;
                let text =
                    core::str::from_utf8(bytes).map_err(|_| DecodeError::InvalidUtf8)?;
                ParamField::Text(String::from(text))
            } else {
                return Err(DecodeError::UnknownOperation(ty));
            };
            fields.push(field);
            r.expect(ArgumentOperations::End as u8)?;
        }
        r.expect(Operations::End as u8)?;

        decoded.push(row_from_fields(fields)?.into_op()?);
    }

    Ok(decoded)
}

enum ParamField {
    Uint(u64),
    Text(String),
}

fn row_from_fields(fields: Vec<ParamField>) -> Result<Row, DecodeError> {
    let mut it = fields.into_iter();
    let operation = match it.next() {
        Some(ParamField::Uint(v)) => u8::try_from(v).map_err(|_| DecodeError::InvalidNumber)?,
        _ => return Err(DecodeError::UnexpectedEnd),
    };
    let node_id = match it.next() {
        Some(ParamField::Uint(v)) => u32::try_from(v).map_err(|_| DecodeError::InvalidNumber)?,
        _ => return Err(DecodeError::UnexpectedEnd),
    };
    let mut text = |slot: Option<ParamField>| match slot {
        Some(ParamField::Text(t)) => Ok(t),
        _ => Err(DecodeError::UnexpectedEnd),
    };
    Ok(Row {
        operation,
        node_id,
        attribute: text(it.next())?,
        value: text(it.next())?,
        text_val: text(it.next())?,
    })
}
