//! # `foundation_ui_components`
//!
//! WHY: Headless, composable UI components over the `foundation_wasm_ui` stack —
//! designed for our architecture (typed `Html`, signals, DOM ops over a wire).
//! We learn from base-ui but do NOT port the React model (spec-42 feature 05).
//!
//! WHAT: Component functions (`fn(...) -> Html`) with config/signal/slot
//! structs. No component trait — composition is function composition. State
//! flows through data-attributes (`data-checked`, `data-disabled`, …) that
//! consumer CSS targets via attribute selectors.
//!
//! HOW: Every component documents static vs signal config (plan.md reminder:
//! not everything is a signal). Pure components (no ctx/rcv needed) exist
//! for static markup (e.g. `separator`). Reactive components take
//! `&Context` + `&SharedInstructionReceiver` and wire effects/bindings.

#![cfg_attr(not(test), no_std)]
#![allow(clippy::module_name_repetitions)]

extern crate alloc;

// Machinery modules
pub mod field_state;
pub mod positioning;

// F1 — Primitives
pub mod button;
pub mod separator;
pub mod toggle;
pub mod toggle_group;
pub mod avatar;

// Re-exports for convenience
pub use button::{button, ButtonConfig, ButtonSlots};
pub use separator::{separator, SeparatorConfig};
pub use toggle::{toggle, ToggleConfig};
pub use toggle_group::{toggle_group, ToggleGroupConfig};
pub use avatar::{avatar, AvatarConfig, AvatarSlots};
pub use field_state::{field, FieldConfig, FieldSlots, FieldState};
