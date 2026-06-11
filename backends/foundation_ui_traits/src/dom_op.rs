//! WHY: Every transport (Arrow, JSON, the byte-0 batch-instructions stream) and
//! every consumer (WASM runtime, HTTP/SSE/WS servers, tests) needs ONE canonical
//! vocabulary of DOM mutations. The variants map 1:1 to the decision-010 `u8`
//! operation codes, which the JS applicator switches on.
//!
//! WHAT: [`DomOp`] (19 variants, ops 0-18), plus the [`TargetSelector`] and
//! [`MorphAction`] types `MorphNode` carries (decision 027 — DOM morphing).
//!
//! HOW: Pure data. Tag/attribute names use [`HtmlTag`]/[`AttrName`] so known names
//! ride the wire as compact ids; free-form values are `Cow<'static, str>` so
//! static templates encode without allocation.
//!
//! # `NodeRegistry` semantics (ops 17/18)
//!
//! The JS side keeps an explicit `node_id -> DOM element` registry:
//!
//! - [`DomOp::RegisterNode`] adds the mapping. The element must already exist —
//!   created by a prior `CreateElement`/`CreateTextNode`, or found in the document
//!   (e.g. via `[primal-id="<n>"]`) for pre-existing elements. Re-registering an
//!   id is a no-op.
//! - [`DomOp::UnregisterNode`] removes the mapping only — the DOM element is NOT
//!   removed. Unknown ids are a no-op.
//! - `CreateElement` / `CreateTextNode` do NOT auto-register; the caller must emit
//!   `RegisterNode` before any op references the id. Explicit ownership keeps the
//!   registry testable.
//! - `ReplaceNode` implicitly unregisters `old_id` and registers `new_id`;
//!   `RemoveNode` implicitly unregisters. No separate ops needed.

use alloc::borrow::Cow;

use crate::{AttrName, HtmlTag};

/// Compact wire representation of "which element" for [`DomOp::MorphNode`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TargetSelector {
    /// Direct registry lookup (selector kind 0).
    NodeId(u32),
    /// `#id` lookup (kind 1).
    Id(Cow<'static, str>),
    /// `.class` — first match (kind 2).
    Class(Cow<'static, str>),
    /// Arbitrary CSS query (kind 3).
    Query(Cow<'static, str>),
}

/// How [`DomOp::MorphNode`] applies its content — a small enum instead of a
/// string, saving wire bytes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MorphAction {
    /// Morph the target's children (0).
    ReplaceChildren,
    /// Replace the target element itself (1).
    ReplaceElement,
    /// Insert before the target (2).
    InsertBefore,
    /// Insert after the target (3).
    InsertAfter,
    /// Append as a sibling (4).
    AppendSibling,
}

impl MorphAction {
    /// The wire discriminant.
    #[must_use]
    pub fn as_u8(self) -> u8 {
        match self {
            Self::ReplaceChildren => 0,
            Self::ReplaceElement => 1,
            Self::InsertBefore => 2,
            Self::InsertAfter => 3,
            Self::AppendSibling => 4,
        }
    }

    /// Parse the wire discriminant.
    #[must_use]
    pub fn from_u8(value: u8) -> Option<Self> {
        match value {
            0 => Some(Self::ReplaceChildren),
            1 => Some(Self::ReplaceElement),
            2 => Some(Self::InsertBefore),
            3 => Some(Self::InsertAfter),
            4 => Some(Self::AppendSibling),
            _ => None,
        }
    }
}

/// A single DOM operation. Each variant's comment shows its decision-010 `u8` code.
#[derive(Clone, Debug, PartialEq)]
pub enum DomOp {
    /// Create an element (op 0). Does NOT auto-register — see module docs.
    CreateElement {
        node_id: u32,
        tag: HtmlTag,
        class: Cow<'static, str>,
    },
    /// Create a text node (op 1). Does NOT auto-register.
    CreateTextNode {
        node_id: u32,
        content: Cow<'static, str>,
    },
    /// Set text content (op 2).
    SetText {
        node_id: u32,
        text: Cow<'static, str>,
    },
    /// Set an attribute (op 3).
    SetAttribute {
        node_id: u32,
        name: AttrName,
        value: Cow<'static, str>,
    },
    /// Remove an attribute (op 4).
    RemoveAttribute { node_id: u32, name: AttrName },
    /// Set a JS property (op 5). `value` is the serialized property value.
    SetProperty {
        node_id: u32,
        name: AttrName,
        value: Cow<'static, str>,
    },
    /// Attach an event listener (op 6).
    AddEventListener { node_id: u32, event_name: AttrName },
    /// Detach an event listener (op 7).
    RemoveEventListener { node_id: u32, event_name: AttrName },
    /// Append `child_id` to `parent_id` (op 8).
    AppendChild { parent_id: u32, child_id: u32 },
    /// Remove `child_id` from `parent_id` (op 9).
    RemoveChild { parent_id: u32, child_id: u32 },
    /// Remove a node from the DOM (op 10). Implicitly unregisters.
    RemoveNode { node_id: u32 },
    /// Insert `child_id` into `parent_id` before `ref_id` (op 11).
    InsertBefore {
        parent_id: u32,
        child_id: u32,
        ref_id: u32,
    },
    /// Hard-replace `old_id` with `new_id` (op 12) — no state preservation.
    /// Implicitly unregisters `old_id` and registers `new_id`.
    ReplaceNode { old_id: u32, new_id: u32 },
    /// Set a style property (op 13).
    SetStyle {
        node_id: u32,
        prop: AttrName,
        value: Cow<'static, str>,
    },
    /// Add a class (op 14).
    AddClass {
        node_id: u32,
        class: Cow<'static, str>,
    },
    /// Remove a class (op 15).
    RemoveClass {
        node_id: u32,
        class: Cow<'static, str>,
    },
    /// Morph `content` into/around `target` per `action` (op 16, decision 027).
    /// Self-contained: the wire row carries selector, action, and content.
    MorphNode {
        target: TargetSelector,
        action: MorphAction,
        content: Cow<'static, str>,
    },
    /// Add `node_id -> element` to the `NodeRegistry` (op 17). See module docs.
    RegisterNode { node_id: u32 },
    /// Remove `node_id` from the `NodeRegistry` (op 18). DOM untouched.
    UnregisterNode { node_id: u32 },
}
