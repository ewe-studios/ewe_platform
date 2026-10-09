//! # Toggle (F1 — Primitives)
//!
//! WHY: A two-state button — pressed/unpressed — with the correct ARIA
//! (`aria-pressed`) and the data-attribute styling contract (spec-42 §F1).
//!
//! WHAT: [`toggle`] renders `<button aria-pressed>`; clicking flips the
//! `pressed` signal. No `role` attribute — a native `<button>` plus
//! `aria-pressed` IS the toggle contract (base-ui adds none).
//!
//! HOW: The click is a [`Callback`] (`ctx.callback`) that flips the signal —
//! the escape hatch for a button click, which carries no value the default
//! setter could read. State rides reactive/Option-valued attributes; no manual
//! effects. Data attributes: `data-pressed`, `data-disabled`.

use alloc::borrow::Cow;
use alloc::vec::Vec;

use foundation_signals::{Context, SignalGetter, SignalSetter};
use foundation_ui_traits::Html;
use foundation_wasm_ui::{html, SharedInstructionReceiver, Slot};

/// Static config for a toggle. Text fields are `Cow<'static, str>`.
pub struct ToggleConfig {
    /// Identifies this toggle inside a toggle-group.
    pub value: Option<Cow<'static, str>>,
    /// Whether the toggle is disabled.
    pub disabled: bool,
    /// Class override (default: `"toggle"`).
    pub class: Option<Cow<'static, str>>,
    /// `aria-label` override.
    pub aria_label: Option<Cow<'static, str>>,
}

impl Default for ToggleConfig {
    fn default() -> Self {
        Self {
            value: None,
            disabled: false,
            class: None,
            aria_label: None,
        }
    }
}

/// Slots for the toggle — its content.
pub struct ToggleSlots {
    /// Toggle content (label/icon).
    pub children: Vec<Slot>,
}

impl Default for ToggleSlots {
    fn default() -> Self {
        Self {
            children: Vec::new(),
        }
    }
}

/// Toggle component — a two-state button.
///
/// Signal: `pressed`; clicking (or Enter/Space, native) flips it via
/// `set_pressed`. Static: `disabled`, `value`, `class`, `aria_label`.
#[must_use]
pub fn toggle(
    ctx: &Context,
    rcv: &SharedInstructionReceiver,
    config: ToggleConfig,
    pressed: &SignalGetter<bool>,
    set_pressed: SignalSetter<bool>,
    slots: ToggleSlots,
) -> Html {
    let class = config.class.unwrap_or(Cow::Borrowed("toggle"));
    let children: Vec<Html> = slots
        .children
        .into_iter()
        .map(|slot| slot.render(ctx, rcv))
        .collect();

    // Click flips the signal — a button click carries no value, so the default
    // setter can't help; a Callback that reads-and-writes is the escape hatch.
    let flip = {
        let pressed = pressed.clone();
        ctx.callback(move |_| set_pressed.set(!pressed.get()))
    };

    let aria = pressed.clone();
    let data = pressed.clone();
    html! { ctx, rcv,
        <button type="button" class=[class]
                aria-label=[config.aria_label]
                aria-pressed={aria.get()}
                data-pressed={data.get().then_some("")}
                disabled={config.disabled.then_some("")}
                data-disabled={config.disabled.then_some("")}
                primal:onclick={flip}>
            <Fragment>{children.clone()}</Fragment>
        </button>
    }
}
