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
pub mod machinery;

// F1 — Primitives
pub mod button;
pub mod separator;
pub mod toggle;
pub mod toggle_group;
pub mod avatar;

// F2 — Selection controls
pub mod switch;
pub mod checkbox;
pub mod radio;

// Re-exports for convenience. `Orientation` is intentionally NOT re-exported
// at the crate root (defined by both `separator` and `toggle_group`); reach it
// via the module path.
pub use button::{button, button_with_click, ButtonConfig, ButtonSlots};
pub use separator::{separator, SeparatorConfig};
pub use toggle::{toggle, ToggleConfig, ToggleSlots};
pub use toggle_group::{toggle_group, ToggleGroupConfig, ToggleGroupItem};
pub use avatar::{avatar, AvatarConfig, AvatarSlots, ImageLoadingStatus};
pub use switch::{switch, SwitchConfig};
pub use checkbox::{
    checkbox, checkbox_group, parent_check_state, CheckState, CheckboxConfig, CheckboxGroupConfig,
    CheckboxSlots,
};
pub use radio::{radio_group, RadioGroupConfig, RadioItem};
pub use field_state::{
    field, FieldBinding, FieldConfig, FieldSlots, FieldState, ValidationMode, Validator,
};
