//! WHY: HTTP servers, SSE endpoints, WebSocket servers, CLI tools, and the WASM
//! runtime all need to turn `DomOp` batches into bytes. Putting the codec contract
//! in a pure, dependency-free, `no_std` crate lets every consumer share the exact
//! same wire format without dragging in WASM, FFI, or an allocator arena.
//!
//! WHAT: Layer 1 of the three-layer protocol architecture (decisions 014/022/028/
//! 030): the [`ProtocolEncoder`] trait, the [`DecodeError`]/[`DecodeResult`]
//! contract, the protocol discriminants, and the canonical decision-010 [`Row`]
//! mapping every encoder shares.
//!
//! HOW: Each encoder is a pure `Vec<DomOp> -> Vec<u8>` (and back) transform. No
//! `MemoryAllocations`, no FFI, no `host_apply`. The WASM transport layer
//! (`foundation_wasm`) and the composed protocol impls (`foundation_wasm_ui`)
//! build on top by adding arena memory and the FFI handoff.

use alloc::borrow::Cow;
use alloc::format;
use alloc::string::{String, ToString};
use alloc::vec::Vec;
use core::fmt;

use crate::{AttrName, DomOp, HtmlTag, MorphAction, TargetSelector};

// ─── Protocol bytes (decision 014/022) ────────────────────────────────────────

/// Protocol discriminant for the Custom Binary format (the `foundation_wasm`
/// batch-instructions stream — see decision 022; implemented in
/// `foundation_wasm_ui::BatchInstructionsV1`, not here).
pub const PROTOCOL_CUSTOM_BINARY: u8 = 0;
/// Protocol discriminant for the Arrow columnar format.
pub const PROTOCOL_ARROW: u8 = 1;
/// Protocol discriminant for the JSON format.
pub const PROTOCOL_JSON: u8 = 2;

/// Current version emitted by every encoder in this crate (spec: starts at 1).
pub const PROTOCOL_VERSION: u8 = 1;

// ─── Operation codes (decision 010 — Arrow batch schema) ───────────────────────
//
// These `u8` operation codes are the canonical cross-language identity of a
// `DomOp`. The Arrow columnar layout, the JSON `operation` field, and the byte-0
// batch stream all use them, and the JS applicator switches on the same numbers.

pub(crate) const OP_CREATE_ELEMENT: u8 = 0;
pub(crate) const OP_CREATE_TEXT_NODE: u8 = 1;
pub(crate) const OP_SET_TEXT_CONTENT: u8 = 2;
pub(crate) const OP_SET_ATTRIBUTE: u8 = 3;
pub(crate) const OP_REMOVE_ATTRIBUTE: u8 = 4;
pub(crate) const OP_SET_PROPERTY: u8 = 5;
pub(crate) const OP_ADD_EVENT_LISTENER: u8 = 6;
pub(crate) const OP_REMOVE_EVENT_LISTENER: u8 = 7;
pub(crate) const OP_APPEND_CHILD: u8 = 8;
pub(crate) const OP_REMOVE_CHILD: u8 = 9;
pub(crate) const OP_REMOVE_NODE: u8 = 10;
pub(crate) const OP_INSERT_BEFORE: u8 = 11;
pub(crate) const OP_REPLACE_NODE: u8 = 12;
pub(crate) const OP_SET_STYLE: u8 = 13;
pub(crate) const OP_ADD_CLASS: u8 = 14;
pub(crate) const OP_REMOVE_CLASS: u8 = 15;
pub(crate) const OP_MORPH_NODE: u8 = 16;
pub(crate) const OP_REGISTER_NODE: u8 = 17;
pub(crate) const OP_UNREGISTER_NODE: u8 = 18;

/// Number of distinct operations (ops `0..OP_COUNT`).
pub const OP_COUNT: u8 = 19;

// ─── DecodeError ───────────────────────────────────────────────────────────────

/// WHY: Decoding bytes that came off a socket, a fetch body, or an arena slot can
/// fail in many small ways; callers need a single typed error to branch on, with
/// enough context (row, column) to debug a corrupt batch.
///
/// WHAT: Every failure mode shared by the decoders (feature 01 spec section 1).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DecodeError {
    /// The byte slice ended before a fixed-width field could be read.
    TruncatedBuffer { expected_min: usize, actual: usize },
    /// An operation code (decision 010 `u8`) was not recognised.
    UnknownOperation { op_id: u8, row_index: usize },
    /// Bytes that were expected to be UTF-8 were not.
    InvalidUtf8 {
        column_name: &'static str,
        row_index: usize,
    },
    /// The columnar layout disagreed with the six-column schema (bad offsets,
    /// lengths, or column shape).
    SchemaMismatch { detail: String },
    /// The JSON payload was malformed.
    JsonParseError { detail: String },
    /// Anything else (bad numeric reference, invalid morph packing, ...).
    Other { detail: String },
}

impl fmt::Display for DecodeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TruncatedBuffer {
                expected_min,
                actual,
            } => write!(
                f,
                "truncated buffer: needed at least {expected_min} bytes, had {actual}"
            ),
            Self::UnknownOperation { op_id, row_index } => {
                write!(f, "unknown operation code {op_id} at row {row_index}")
            }
            Self::InvalidUtf8 {
                column_name,
                row_index,
            } => write!(
                f,
                "invalid UTF-8 in column `{column_name}` at row {row_index}"
            ),
            Self::SchemaMismatch { detail } => write!(f, "schema mismatch: {detail}"),
            Self::JsonParseError { detail } => write!(f, "JSON parse error: {detail}"),
            Self::Other { detail } => f.write_str(detail),
        }
    }
}

/// Result of decoding a payload.
///
/// The feature-01 spec sketches a bespoke `Success`/`Error` enum; this alias keeps
/// the identical two-state contract while staying `?`-compatible Rust.
pub type DecodeResult<T> = Result<T, DecodeError>;

// ─── ProtocolEncoder trait ─────────────────────────────────────────────────────

/// WHY: The runtime, HTTP servers, and tests all want to swap encoders behind one
/// interface so the choice of wire format is a configuration detail, not a
/// rewrite.
///
/// WHAT: A bidirectional, pure codec for a payload type `T`. Implementors carry no
/// state — they are zero-sized markers. Implemented only for concrete payload
/// types (G11): no blanket impls.
///
/// HOW: `encode` produces the protocol-specific payload bytes (no envelope);
/// `decode` reverses it. The `protocol_byte`/`version` pair identifies the format
/// inside an [`crate::Envelope`].
pub trait ProtocolEncoder<T> {
    /// The protocol discriminant written into the envelope header.
    fn protocol_byte(&self) -> u8;
    /// The protocol version written into the envelope header.
    fn version(&self) -> u8;
    /// Encode `data` into protocol-specific payload bytes (envelope not included).
    /// Encoding well-formed data is infallible.
    fn encode(&self, data: T) -> Vec<u8>;
    /// Decode a protocol-specific payload back into the original data.
    ///
    /// # Errors
    /// Returns [`DecodeError`] when the bytes are truncated, malformed, or carry
    /// an unknown operation code.
    fn decode(&self, payload: &[u8]) -> DecodeResult<T>;
}

// ─── Byte cursor helpers ───────────────────────────────────────────────────────

/// Minimal forward-only reader over a byte slice. Keeps the decoders readable and
/// keeps every bounds check in one place.
pub(crate) struct Cursor<'a> {
    bytes: &'a [u8],
    pos: usize,
}

impl<'a> Cursor<'a> {
    pub(crate) fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, pos: 0 }
    }

    fn truncated(&self, needed: usize) -> DecodeError {
        DecodeError::TruncatedBuffer {
            expected_min: needed,
            actual: self.bytes.len(),
        }
    }

    pub(crate) fn u8(&mut self) -> DecodeResult<u8> {
        let b = *self
            .bytes
            .get(self.pos)
            .ok_or_else(|| self.truncated(self.pos + 1))?;
        self.pos += 1;
        Ok(b)
    }

    pub(crate) fn u32(&mut self) -> DecodeResult<u32> {
        let end = self.pos.checked_add(4).ok_or_else(|| {
            DecodeError::SchemaMismatch {
                detail: "offset overflow".into(),
            }
        })?;
        let slice = self
            .bytes
            .get(self.pos..end)
            .ok_or_else(|| self.truncated(end))?;
        self.pos = end;
        Ok(u32::from_le_bytes([slice[0], slice[1], slice[2], slice[3]]))
    }

    pub(crate) fn bytes(&mut self, len: usize) -> DecodeResult<&'a [u8]> {
        let end = self.pos.checked_add(len).ok_or_else(|| {
            DecodeError::SchemaMismatch {
                detail: "length overflow".into(),
            }
        })?;
        let slice = self
            .bytes
            .get(self.pos..end)
            .ok_or_else(|| self.truncated(end))?;
        self.pos = end;
        Ok(slice)
    }
}

/// Convert a `usize` length / count / offset into the `u32` the wire format uses.
///
/// WHY: every length, row count, and string offset in these formats is a 4-byte
/// `u32` field — the same convention Apache Arrow IPC uses with its i32 offsets.
/// That caps a single batch, payload, or string column at `u32::MAX` (~4 GiB),
/// far beyond any real DOM update; a checked conversion fails loudly instead of
/// silently truncating on a 64-bit target.
///
/// # Panics
/// Panics if `value > u32::MAX` — a >4 GiB DOM batch, which never occurs in
/// practice and would indicate a bug upstream.
#[inline]
pub(crate) fn to_u32(value: usize) -> u32 {
    u32::try_from(value).expect("wire-format field exceeds u32::MAX (~4 GiB)")
}

/// Parse a stringified `u32` reference (child/ref/new id).
fn parse_ref(s: &str, what: &str, row_index: usize) -> DecodeResult<u32> {
    s.parse::<u32>().map_err(|_| DecodeError::Other {
        detail: format!("row {row_index}: {what} is not a valid u32 reference: `{s}`"),
    })
}

// ─── Canonical row view ────────────────────────────────────────────────────────
//
// The Arrow columnar layout, the flat JSON objects, and the byte-0 batch stream
// all serialise a `DomOp` through the same canonical decision-010 shape:
// `(operation, node_id, attribute, value, text_val)`. Centralising the mapping
// keeps every encoder bug-for-bug consistent with each other and with JS.
//
// Column conventions (feature 01 spec section 1):
//   * `HtmlTag` / `AttrName`: known ids as `"id:<n>"`, unknown names raw.
//   * Secondary node ids (child/ref/new): plain decimal strings.
//   * MorphNode (op 16): `node_id` carries the target id for
//     `TargetSelector::NodeId` (0 otherwise); `attribute` packs
//     `"<action>:<kind>:<selector>"` where action is `MorphAction::as_u8`, kind
//     is the selector discriminant (0 node-id / 1 #id / 2 .class / 3 query), and
//     selector is the raw string (empty for node-id). The selector may itself
//     contain `:` — parsers must split only the first two colons.

/// The canonical decision-010 row form of a [`DomOp`]. Public so protocol impls
/// (Arrow's columns, the flat JSON objects, the batch-instructions custom
/// protocol) share ONE `DomOp` ⇄ row mapping. `None` column values become Arrow
/// nulls / JSON `null`s.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Row {
    /// Operation code (decision 010 table).
    pub operation: u8,
    /// Target node id (primal id).
    pub node_id: u32,
    /// Attribute name slot.
    pub attribute: Option<String>,
    /// Value slot.
    pub value: Option<String>,
    /// Text-content slot.
    pub text_val: Option<String>,
}

impl Row {
    /// Map a [`DomOp`] to its row form.
    // One arm per op keeps this a literal transcription of the decision-010
    // table; splitting it into helpers would obscure the 1:1 mapping.
    #[allow(clippy::too_many_lines)]
    #[must_use]
    pub fn from_op(op: &DomOp) -> Self {
        let mut row = Self::default();
        match op {
            DomOp::CreateElement {
                node_id,
                tag,
                class,
            } => {
                row.operation = OP_CREATE_ELEMENT;
                row.node_id = *node_id;
                row.attribute = Some(tag.to_wire_string());
                row.value = Some(class.to_string());
            }
            DomOp::CreateTextNode { node_id, content } => {
                row.operation = OP_CREATE_TEXT_NODE;
                row.node_id = *node_id;
                row.text_val = Some(content.to_string());
            }
            DomOp::SetText { node_id, text } => {
                row.operation = OP_SET_TEXT_CONTENT;
                row.node_id = *node_id;
                row.text_val = Some(text.to_string());
            }
            DomOp::SetAttribute {
                node_id,
                name,
                value,
            } => {
                row.operation = OP_SET_ATTRIBUTE;
                row.node_id = *node_id;
                row.attribute = Some(name.to_wire_string());
                row.value = Some(value.to_string());
            }
            DomOp::RemoveAttribute { node_id, name } => {
                row.operation = OP_REMOVE_ATTRIBUTE;
                row.node_id = *node_id;
                row.attribute = Some(name.to_wire_string());
            }
            DomOp::SetProperty {
                node_id,
                name,
                value,
            } => {
                row.operation = OP_SET_PROPERTY;
                row.node_id = *node_id;
                row.attribute = Some(name.to_wire_string());
                row.value = Some(value.to_string());
            }
            DomOp::AddEventListener {
                node_id,
                event_name,
            } => {
                row.operation = OP_ADD_EVENT_LISTENER;
                row.node_id = *node_id;
                row.value = Some(event_name.to_wire_string());
            }
            DomOp::RemoveEventListener {
                node_id,
                event_name,
            } => {
                row.operation = OP_REMOVE_EVENT_LISTENER;
                row.node_id = *node_id;
                row.value = Some(event_name.to_wire_string());
            }
            DomOp::AppendChild {
                parent_id,
                child_id,
            } => {
                row.operation = OP_APPEND_CHILD;
                row.node_id = *parent_id;
                row.attribute = Some(child_id.to_string());
            }
            DomOp::RemoveChild {
                parent_id,
                child_id,
            } => {
                row.operation = OP_REMOVE_CHILD;
                row.node_id = *parent_id;
                row.attribute = Some(child_id.to_string());
            }
            DomOp::RemoveNode { node_id } => {
                row.operation = OP_REMOVE_NODE;
                row.node_id = *node_id;
            }
            DomOp::InsertBefore {
                parent_id,
                child_id,
                ref_id,
            } => {
                row.operation = OP_INSERT_BEFORE;
                row.node_id = *parent_id;
                row.attribute = Some(child_id.to_string());
                row.text_val = Some(ref_id.to_string());
            }
            DomOp::ReplaceNode { old_id, new_id } => {
                row.operation = OP_REPLACE_NODE;
                row.node_id = *old_id;
                row.attribute = Some(new_id.to_string());
            }
            DomOp::SetStyle {
                node_id,
                prop,
                value,
            } => {
                row.operation = OP_SET_STYLE;
                row.node_id = *node_id;
                row.attribute = Some(prop.to_wire_string());
                row.value = Some(value.to_string());
            }
            DomOp::AddClass { node_id, class } => {
                row.operation = OP_ADD_CLASS;
                row.node_id = *node_id;
                row.value = Some(class.to_string());
            }
            DomOp::RemoveClass { node_id, class } => {
                row.operation = OP_REMOVE_CLASS;
                row.node_id = *node_id;
                row.value = Some(class.to_string());
            }
            DomOp::MorphNode {
                target,
                action,
                content,
            } => {
                row.operation = OP_MORPH_NODE;
                let (node_id, kind, selector): (u32, u8, &str) = match target {
                    TargetSelector::NodeId(id) => (*id, 0, ""),
                    TargetSelector::Id(s) => (0, 1, s.as_ref()),
                    TargetSelector::Class(s) => (0, 2, s.as_ref()),
                    TargetSelector::Query(s) => (0, 3, s.as_ref()),
                };
                row.node_id = node_id;
                row.attribute = Some(format!("{}:{kind}:{selector}", action.as_u8()));
                row.text_val = Some(content.to_string());
            }
            DomOp::RegisterNode { node_id } => {
                row.operation = OP_REGISTER_NODE;
                row.node_id = *node_id;
            }
            DomOp::UnregisterNode { node_id } => {
                row.operation = OP_UNREGISTER_NODE;
                row.node_id = *node_id;
            }
        }
        row
    }

    /// Map the row form back to a [`DomOp`]. `row_index` is reported in errors.
    ///
    /// # Errors
    /// Returns [`DecodeError::UnknownOperation`] for an unrecognised code and
    /// [`DecodeError::Other`] for malformed references / morph packing.
    // One arm per op keeps this a literal transcription of the decision-010
    // table; splitting it into helpers would obscure the 1:1 mapping.
    #[allow(clippy::too_many_lines)]
    pub fn into_op(self, row_index: usize) -> DecodeResult<DomOp> {
        // Missing nullable columns decode as empty strings: the op code alone
        // determines which slots are meaningful, so absent and empty coincide.
        let attribute = self.attribute.unwrap_or_default();
        let value = self.value.unwrap_or_default();
        let text_val = self.text_val.unwrap_or_default();
        let node_id = self.node_id;

        let op = match self.operation {
            OP_CREATE_ELEMENT => DomOp::CreateElement {
                node_id,
                tag: HtmlTag::from_wire_str(&attribute),
                class: Cow::Owned(value),
            },
            OP_CREATE_TEXT_NODE => DomOp::CreateTextNode {
                node_id,
                content: Cow::Owned(text_val),
            },
            OP_SET_TEXT_CONTENT => DomOp::SetText {
                node_id,
                text: Cow::Owned(text_val),
            },
            OP_SET_ATTRIBUTE => DomOp::SetAttribute {
                node_id,
                name: AttrName::from_wire_str(&attribute),
                value: Cow::Owned(value),
            },
            OP_REMOVE_ATTRIBUTE => DomOp::RemoveAttribute {
                node_id,
                name: AttrName::from_wire_str(&attribute),
            },
            OP_SET_PROPERTY => DomOp::SetProperty {
                node_id,
                name: AttrName::from_wire_str(&attribute),
                value: Cow::Owned(value),
            },
            OP_ADD_EVENT_LISTENER => DomOp::AddEventListener {
                node_id,
                event_name: AttrName::from_wire_str(&value),
            },
            OP_REMOVE_EVENT_LISTENER => DomOp::RemoveEventListener {
                node_id,
                event_name: AttrName::from_wire_str(&value),
            },
            OP_APPEND_CHILD => DomOp::AppendChild {
                parent_id: node_id,
                child_id: parse_ref(&attribute, "child_id", row_index)?,
            },
            OP_REMOVE_CHILD => DomOp::RemoveChild {
                parent_id: node_id,
                child_id: parse_ref(&attribute, "child_id", row_index)?,
            },
            OP_REMOVE_NODE => DomOp::RemoveNode { node_id },
            OP_INSERT_BEFORE => DomOp::InsertBefore {
                parent_id: node_id,
                child_id: parse_ref(&attribute, "child_id", row_index)?,
                ref_id: parse_ref(&text_val, "ref_id", row_index)?,
            },
            OP_REPLACE_NODE => DomOp::ReplaceNode {
                old_id: node_id,
                new_id: parse_ref(&attribute, "new_id", row_index)?,
            },
            OP_SET_STYLE => DomOp::SetStyle {
                node_id,
                prop: AttrName::from_wire_str(&attribute),
                value: Cow::Owned(value),
            },
            OP_ADD_CLASS => DomOp::AddClass {
                node_id,
                class: Cow::Owned(value),
            },
            OP_REMOVE_CLASS => DomOp::RemoveClass {
                node_id,
                class: Cow::Owned(value),
            },
            OP_MORPH_NODE => {
                let (target, action) = parse_morph_packing(&attribute, node_id, row_index)?;
                DomOp::MorphNode {
                    target,
                    action,
                    content: Cow::Owned(text_val),
                }
            }
            OP_REGISTER_NODE => DomOp::RegisterNode { node_id },
            OP_UNREGISTER_NODE => DomOp::UnregisterNode { node_id },
            other => {
                return Err(DecodeError::UnknownOperation {
                    op_id: other,
                    row_index,
                })
            }
        };
        Ok(op)
    }
}

/// Parse the op-16 `"<action>:<kind>:<selector>"` attribute packing.
fn parse_morph_packing(
    packed: &str,
    node_id: u32,
    row_index: usize,
) -> DecodeResult<(TargetSelector, MorphAction)> {
    let bad = |what: &str| DecodeError::Other {
        detail: format!("row {row_index}: malformed morph packing ({what}): `{packed}`"),
    };
    // Split only the first two colons — CSS query selectors may contain `:`.
    let (action_str, rest) = packed.split_once(':').ok_or_else(|| bad("no action"))?;
    let (kind_str, selector) = rest.split_once(':').ok_or_else(|| bad("no kind"))?;
    let action_num = action_str.parse::<u8>().map_err(|_| bad("action"))?;
    let action = MorphAction::from_u8(action_num).ok_or_else(|| bad("action range"))?;
    let target = match kind_str {
        "0" => TargetSelector::NodeId(node_id),
        "1" => TargetSelector::Id(Cow::Owned(selector.to_string())),
        "2" => TargetSelector::Class(Cow::Owned(selector.to_string())),
        "3" => TargetSelector::Query(Cow::Owned(selector.to_string())),
        _ => return Err(bad("kind")),
    };
    Ok((target, action))
}
