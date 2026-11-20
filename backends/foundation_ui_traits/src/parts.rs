//! WHY: The `html!` macro must tell the signal runtime WHERE dynamic expressions
//! live in the rendered tree so each one can become an `Effect` that re-renders
//! just that slot (decision 005 — part granularity).
//!
//! WHAT: [`Part`] and its three descriptor structs. Note: `ChildPart` was REMOVED
//! (feature 01 spec) — every child position generates a `Part::Text`; `Vec<Html>`
//! expressions are handled at runtime via [`crate::IntoHtml`], which wraps them as
//! a tagless node.
//!
//! HOW: Plain data carried inside [`crate::Html::parts`]; node ids are the
//! `primal-id`s assigned by the macro (decision 006).

use alloc::string::String;

/// Describes a dynamic slot in an [`crate::Html`] tree.
///
/// Each `Part` becomes one `Effect` that reads a signal and queues a frame operation.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Part {
    /// Dynamic text content — `{signal}` inside an element.
    Text(TextPart),
    /// Dynamic attribute — `class={expr}`, `id={expr}`, etc.
    Attribute(AttrPart),
    /// Event handler — `primal:onclick={handler}`, etc.
    Event(EventPart),
}

impl Part {
    /// Request signed state on this node. The template stores this request in its metadata.
    #[must_use]
    pub fn state_marker(node_id: u32) -> Self {
        Self::Attribute(AttrPart {
            node_id,
            attr_name: String::from("primal::state"),
        })
    }

    /// Check whether this part requests signed state for the given local node.
    #[must_use]
    pub fn marks_state_on(&self, id: u32) -> bool {
        matches!(self, Self::Attribute(part) if part.node_id == id && part.attr_name == "primal::state")
    }
}

/// A dynamic text content slot.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TextPart {
    /// The `primal-id` of the element containing this text slot.
    pub node_id: u32,
}

/// A dynamic attribute slot.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AttrPart {
    /// The `primal-id` of the element.
    pub node_id: u32,
    /// The attribute name being set dynamically.
    pub attr_name: String,
}

/// An event handler slot.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EventPart {
    /// The `primal-id` of the element.
    pub node_id: u32,
    /// The event name (e.g., "click", "change", "submit").
    pub event_name: String,
}
