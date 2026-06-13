//! # Switch (F2 — Selection controls)
//!
//! WHY: A styled on/off control that is STILL a real form field for forms and
//! assistive tech (spec-42 feature 05 §F2).
//!
//! WHAT: A visually-hidden native `<input type="checkbox" role="switch">` (the
//! actual focusable control, carrying `name`/`value`/`checked` for forms) next
//! to a visual `<span>` track + thumb that CSS styles off the data-attributes.
//!
//! HOW: The hidden input IS the control — native focus + Space toggle for free,
//! and its `change` event delivers `checked` straight to the `SignalSetter`
//! (the default two-way binding; no `ctx.callback` needed). The visual span
//! carries `data-checked`/`data-unchecked` (presence pair) for styling.

use alloc::borrow::Cow;

use foundation_signals::{Context, SignalGetter, SignalSetter};
use foundation_ui_traits::Html;
use foundation_wasm_ui::{html, SharedInstructionReceiver};

/// Static config for a switch. Text fields are `Cow<'static, str>`.
pub struct SwitchConfig {
    /// Submitted field name.
    pub name: Cow<'static, str>,
    /// Value submitted when on (native default: `"on"`).
    pub value: Cow<'static, str>,
    /// Disabled (leaves tab order).
    pub disabled: bool,
    /// Read-only (focusable + announced; mutation blocked by CSS/consumer).
    pub readonly: bool,
    /// Required.
    pub required: bool,
    /// Class override for the root (default: `"switch"`).
    pub class: Option<Cow<'static, str>>,
    /// `id` for the hidden input (label `for` target).
    pub id: Option<Cow<'static, str>>,
}

impl Default for SwitchConfig {
    fn default() -> Self {
        Self {
            name: Cow::Borrowed(""),
            value: Cow::Borrowed("on"),
            disabled: false,
            readonly: false,
            required: false,
            class: None,
            id: None,
        }
    }
}

/// Switch component — `role="switch"` hidden input + visual track/thumb.
///
/// Signal: `checked`; the hidden input's `change` writes it via `set_checked`.
/// Static: everything else. Data attrs (on root): `data-checked`/
/// `data-unchecked`, `data-disabled`, `data-readonly`, `data-required`.
#[must_use]
pub fn switch(
    ctx: &Context,
    rcv: &SharedInstructionReceiver,
    config: SwitchConfig,
    checked: &SignalGetter<bool>,
    set_checked: SignalSetter<bool>,
) -> Html {
    let class = config.class.unwrap_or(Cow::Borrowed("switch"));
    let c_root = checked.clone();
    let c_unchecked = checked.clone();
    let c_input = checked.clone();
    html! { ctx, rcv,
        <span class=[class]
              data-checked={c_root.get().then_some("")}
              data-unchecked={(!c_unchecked.get()).then_some("")}
              data-disabled=[config.disabled.then_some("")]
              data-readonly=[config.readonly.then_some("")]
              data-required=[config.required.then_some("")]>
            <input type="checkbox" role="switch" class="switch-input"
                   id=[config.id]
                   name=[config.name]
                   value=[config.value]
                   checked={c_input.get().then_some("")}
                   disabled=[config.disabled.then_some("")]
                   required=[config.required.then_some("")]
                   primal:onchange={set_checked} />
            <span class="switch-thumb"></span>
        </span>
    }
}
