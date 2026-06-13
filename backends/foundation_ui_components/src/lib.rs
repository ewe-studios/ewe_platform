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

// F3 — Disclosure
pub mod collapsible;
pub mod accordion;
pub mod tabs;

// F4 — Overlays
pub mod dialog;
pub mod popover;
pub mod toast;

// F7 — Form
pub mod input;
pub mod number_field;
pub mod otp_field;

// F8 — Indicators & surfaces
pub mod progress;
pub mod slider;
pub mod surfaces;

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
pub use collapsible::{collapsible, CollapsibleConfig, CollapsibleSlots};
pub use accordion::{accordion, AccordionConfig, AccordionItem};
pub use tabs::{tabs, TabDef, TabsConfig};
pub use dialog::{
    alert_dialog, dialog, drawer, DialogConfig, DialogSlots, DrawerSide,
};
pub use popover::{
    popover, preview_card, tooltip, HoverConfig, PopoverConfig, PopoverSlots,
};
pub use toast::{toast_viewport, Toast, ToastManager};
pub use input::{
    fieldset, form, input, FieldsetConfig, FieldsetSlots, FormConfig, InputConfig,
};
pub use number_field::{number_field, NumberFieldConfig};
pub use otp_field::{otp_field, OtpConfig};
pub use progress::{meter, progress, ProgressConfig};
pub use slider::{slider, SliderConfig};
pub use surfaces::{scroll_area, skeleton, ScrollAreaConfig, SkeletonShape};
pub use field_state::{
    field, FieldBinding, FieldConfig, FieldSlots, FieldState, ValidationMode, Validator,
};
