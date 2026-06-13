//! # Button (F1 — Primitives)
//!
//! WHY: A native `<button>` gives Enter/Space activation, focusability and
//! form semantics for free — no JS keyboard code (spec-42 feature 05 §F1).
//!
//! WHAT: [`button`] (a styled primitive — content + optional `loading` signal)
//! and [`button_with_click`] (adds a click [`Callback`]). `focusable_when_disabled`
//! renders `aria-disabled` WITHOUT the `disabled` attribute so the button stays
//! in tab order (tooltip/AT discoverability).
//!
//! HOW: Pure `html!` reactive form — reactive + Option-valued attributes
//! (features.md §8.2) carry state; no hand-built `Html` or manual DomOp
//! queueing. Data attributes: `data-disabled`, `data-loading` (ours, additive).

use alloc::borrow::Cow;
use alloc::vec;
use alloc::vec::Vec;

use foundation_signals::{Callback, Context, SignalGetter};
use foundation_ui_traits::Html;
use foundation_wasm_ui::{html, SharedInstructionReceiver, Slot};

/// Static config for a button. Text fields are `Cow<'static, str>` so literals
/// cost nothing and owned `String`s are accepted (features.md §8.1).
pub struct ButtonConfig {
    /// Whether the button is disabled (renders the `disabled` attribute).
    pub disabled: bool,
    /// Stay focusable when disabled (`aria-disabled` only, no `disabled` attr).
    pub focusable_when_disabled: bool,
    /// Class for the button element (default: `"button"`).
    pub class: Option<Cow<'static, str>>,
    /// `aria-label` override.
    pub aria_label: Option<Cow<'static, str>>,
    /// `aria-describedby` (e.g. tooltip linking).
    pub aria_describedby: Option<Cow<'static, str>>,
}

impl Default for ButtonConfig {
    fn default() -> Self {
        Self {
            disabled: false,
            focusable_when_disabled: false,
            class: None,
            aria_label: None,
            aria_describedby: None,
        }
    }
}

/// Slots for the button — its content (label, icon, …).
pub struct ButtonSlots {
    /// Button content.
    pub children: Vec<Slot>,
}

impl ButtonSlots {
    /// A single content slot.
    pub fn single(slot: impl Into<Slot>) -> Self {
        Self {
            children: vec![slot.into()],
        }
    }
}

/// Button primitive — `<button type="button">` with content and an optional
/// `loading` signal (→ `data-loading` presence + `aria-busy`).
///
/// Static: `disabled`, `focusable_when_disabled`, `class`, `aria_*`.
#[must_use]
pub fn button(
    ctx: &Context,
    rcv: &SharedInstructionReceiver,
    config: ButtonConfig,
    slots: ButtonSlots,
    loading: Option<SignalGetter<bool>>,
) -> Html {
    let class = config.class.unwrap_or(Cow::Borrowed("button"));
    let is_disabled = config.disabled || config.focusable_when_disabled;
    let children = render_children(ctx, rcv, slots.children);
    let load_data = loading.clone();
    html! { ctx, rcv,
        <button type="button" class=[class]
                disabled={config.disabled.then_some("")}
                aria-disabled={is_disabled.then_some("true")}
                aria-label=[config.aria_label]
                aria-describedby=[config.aria_describedby]
                data-disabled={is_disabled.then_some("")}
                data-loading={load_data.as_ref().map_or(false, SignalGetter::get).then_some("")}
                aria-busy={loading.as_ref().map(SignalGetter::get)}>
            {children.clone()}
        </button>
    }
}

/// Button that also wires a click [`Callback`] (build it with
/// `ctx.callback(...)`). Same rendering as [`button`] plus `primal:onclick`.
#[must_use]
pub fn button_with_click(
    ctx: &Context,
    rcv: &SharedInstructionReceiver,
    config: ButtonConfig,
    slots: ButtonSlots,
    loading: Option<SignalGetter<bool>>,
    on_click: Callback,
) -> Html {
    let class = config.class.unwrap_or(Cow::Borrowed("button"));
    let is_disabled = config.disabled || config.focusable_when_disabled;
    let children = render_children(ctx, rcv, slots.children);
    let load_data = loading.clone();
    html! { ctx, rcv,
        <button type="button" class=[class]
                disabled={config.disabled.then_some("")}
                aria-disabled={is_disabled.then_some("true")}
                aria-label=[config.aria_label]
                aria-describedby=[config.aria_describedby]
                data-disabled={is_disabled.then_some("")}
                data-loading={load_data.as_ref().map_or(false, SignalGetter::get).then_some("")}
                aria-busy={loading.as_ref().map(SignalGetter::get)}
                primal:onclick={on_click}>
            {children.clone()}
        </button>
    }
}

/// Render slot children to `Html` for embedding (the established slot pattern).
fn render_children(ctx: &Context, rcv: &SharedInstructionReceiver, slots: Vec<Slot>) -> Vec<Html> {
    slots.into_iter().map(|slot| slot.render(ctx, rcv)).collect()
}
