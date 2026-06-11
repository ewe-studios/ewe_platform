//! WHY: Two-way binding's JS leg (decision 029 / G17): a DOM event in the
//! browser must land on a Rust setter. JS serializes the event, calls
//! `invoke_callback(id, ...)` across the boundary; the id was minted when the
//! signal was created and stamped into the markup by the `html!` macro.
//!
//! WHAT: [`EventData`] — the wire shape of a delivered DOM event, matching the
//! JS `buildEventData` object in `foundation-wasm-ui.js` (feature 08) field for
//! field — plus [`Modifiers`].
//!
//! HOW: Plain serde structs; `Runtime::invoke_callback_json` deserializes and
//! dispatches. The registry itself lives on the `Runtime` (`BTreeMap`, monotonic
//! never-reused ids, stale lookups dropped silently).

use serde::{Deserialize, Serialize};

/// Modifier-key state at event time. Four independent keys, four bools — this
/// IS the domain shape (mirrors the JS `modifiers` object), not a state enum.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
#[allow(clippy::struct_excessive_bools)]
pub struct Modifiers {
    pub alt: bool,
    pub ctrl: bool,
    pub shift: bool,
    pub meta: bool,
}

/// A DOM event as delivered to a callback — mirrors the JS `buildEventData`
/// shape (`type`, `primalId`, `value`, `checked`, `keyCode`, `modifiers`).
/// Every field except `event_type` is optional: different event kinds carry
/// different payloads.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct EventData {
    /// The DOM event type ("click", "change", "input", ...).
    #[serde(rename = "type")]
    pub event_type: String,
    /// The element's `primal-id`, when present.
    pub primal_id: Option<String>,
    /// The element's current `value` (inputs, selects, textareas).
    pub value: Option<String>,
    /// The element's `checked` state (checkboxes, radios).
    pub checked: Option<bool>,
    /// Key code for keyboard events.
    pub key_code: Option<u32>,
    /// Modifier-key state.
    pub modifiers: Modifiers,
}

impl EventData {
    /// Convenience constructor for the common "value changed" shape.
    #[must_use]
    pub fn with_value(event_type: impl Into<String>, value: impl Into<String>) -> Self {
        Self {
            event_type: event_type.into(),
            value: Some(value.into()),
            ..Self::default()
        }
    }
}
