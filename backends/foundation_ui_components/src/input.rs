//! # Input / fieldset / form (F7 — Form)
//!
//! WHY: The thin form widgets that ride the M6 field state machine — a native
//! `<input>`, a native `<fieldset>` (disabled inheritance for free), and a
//! `<form>` that owns the server-error seam (spec-42 §F7).
//!
//! WHAT: [`input`] (field-aware `<input>` over a `String` signal),
//! [`fieldset`] (pure), [`form`] (submit callback + `(errors, set_errors)`
//! server-roundtrip signal).
//!
//! HOW: `input` uses the G21 `primal:onchange={setter}` path and, when handed a
//! `&FieldState`, reflects the six validity data-attributes. `form` prevents
//! the default submit and calls the callback with no value (callers read their
//! own signals); the `errors` signal is the seam feature 04 paints from a
//! server response. `:user-valid`/`:user-invalid` are the documented zero-JS
//! CSS-first alternative.

use alloc::borrow::Cow;
use alloc::vec::Vec;

use foundation_signals::{Callback, Context, SignalGetter, SignalSetter};
use foundation_ui_traits::Html;
use foundation_wasm_ui::{html, SharedInstructionReceiver, Slot};

use crate::field_state::FieldState;

/// Static config for an [`input`].
pub struct InputConfig {
    /// `type` attribute (default `"text"`).
    pub input_type: Cow<'static, str>,
    /// `name` attribute.
    pub name: Option<Cow<'static, str>>,
    /// `placeholder`.
    pub placeholder: Option<Cow<'static, str>>,
    /// Disabled.
    pub disabled: bool,
    /// Required.
    pub required: bool,
    /// Read-only.
    pub readonly: bool,
    /// `id` (label `for` target).
    pub id: Option<Cow<'static, str>>,
    /// Class override (default `"input"`).
    pub class: Option<Cow<'static, str>>,
    /// `aria-label`.
    pub aria_label: Option<Cow<'static, str>>,
}

impl Default for InputConfig {
    fn default() -> Self {
        Self {
            input_type: Cow::Borrowed("text"),
            name: None,
            placeholder: None,
            disabled: false,
            required: false,
            readonly: false,
            id: None,
            class: None,
            aria_label: None,
        }
    }
}

/// Field-aware `<input>` over a `String` signal. When `field` is given, the six
/// M6 validity data-attributes ride along.
#[must_use]
pub fn input(
    ctx: &Context,
    rcv: &SharedInstructionReceiver,
    config: InputConfig,
    value: &SignalGetter<alloc::string::String>,
    set_value: SignalSetter<alloc::string::String>,
    field: Option<&FieldState>,
) -> Html {
    let class = config.class.unwrap_or(Cow::Borrowed("input"));
    // Field validity → reactive data-attrs (clone a getter per attribute).
    let (touched, dirty, filled, focused, valid_t, valid_f) = match field {
        Some(f) => (
            Some(f.touched.clone()),
            Some(f.dirty.clone()),
            Some(f.filled.clone()),
            Some(f.focused.clone()),
            Some(f.valid.clone()),
            Some(f.valid.clone()),
        ),
        None => (None, None, None, None, None, None),
    };
    let v = value.clone();
    html! { ctx, rcv,
        <input type=[config.input_type] class=[class]
               id=[config.id]
               name=[config.name]
               placeholder=[config.placeholder]
               value={v.get()}
               disabled={config.disabled.then_some("")}
               data-disabled={config.disabled.then_some("")}
               required={config.required.then_some("")}
               readonly={config.readonly.then_some("")}
               aria-label=[config.aria_label]
               data-touched={touched.clone().and_then(|s| s.get().then_some(""))}
               data-dirty={dirty.clone().and_then(|s| s.get().then_some(""))}
               data-filled={filled.clone().and_then(|s| s.get().then_some(""))}
               data-focused={focused.clone().and_then(|s| s.get().then_some(""))}
               data-valid={valid_t.clone().and_then(|s| s.get().and_then(|v| v.then_some("")))}
               data-invalid={valid_f.clone().and_then(|s| s.get().and_then(|v| (!v).then_some("")))}
               primal:onchange={set_value} />
    }
}

/// Static config for a [`fieldset`].
pub struct FieldsetConfig {
    /// Disable the whole group (native fieldset disables descendants).
    pub disabled: bool,
    /// Class override (default `"fieldset"`).
    pub class: Option<Cow<'static, str>>,
}

impl Default for FieldsetConfig {
    fn default() -> Self {
        Self { disabled: false, class: None }
    }
}

/// Slots for a [`fieldset`].
pub struct FieldsetSlots {
    /// Legend content.
    pub legend: Option<Slot>,
    /// Body content.
    pub children: Vec<Slot>,
}

impl Default for FieldsetSlots {
    fn default() -> Self {
        Self { legend: None, children: Vec::new() }
    }
}

/// Native `<fieldset>` + `<legend>` with disabled inheritance.
#[must_use]
pub fn fieldset(
    ctx: &Context,
    rcv: &SharedInstructionReceiver,
    config: FieldsetConfig,
    slots: FieldsetSlots,
) -> Html {
    let class = config.class.unwrap_or(Cow::Borrowed("fieldset"));
    let legend_html = slots.legend.map(|s| s.render(ctx, rcv));
    let body: Vec<Html> = slots.children.into_iter().map(|s| s.render(ctx, rcv)).collect();
    html! { ctx, rcv,
        <fieldset class=[class]
                  disabled={config.disabled.then_some("")}
                  data-disabled={config.disabled.then_some("")}>
            <Fragment>{legend_html.clone().map(|l| html! { ctx, rcv,
                <legend class="fieldset-legend"><Fragment>{l.clone()}</Fragment></legend>
            })}</Fragment>
            <Fragment>{body.clone()}</Fragment>
        </fieldset>
    }
}

/// Static config for a [`form`].
pub struct FormConfig {
    /// Class override (default `"form"`).
    pub class: Option<Cow<'static, str>>,
    /// `novalidate` (suppress native validation bubbles; use our messages).
    pub novalidate: bool,
}

impl Default for FormConfig {
    fn default() -> Self {
        Self { class: None, novalidate: true }
    }
}

/// `<form>` with a submit callback and the server-error seam. `on_submit` is a
/// [`Callback`] (build with `ctx.callback`) — the default submit is prevented;
/// the callback reads the caller's own field signals. `errors` is the signal
/// feature 04 paints from a server response.
#[must_use]
pub fn form(
    ctx: &Context,
    rcv: &SharedInstructionReceiver,
    config: FormConfig,
    _errors: &SignalGetter<Vec<(alloc::string::String, alloc::string::String)>>,
    on_submit: Callback,
    children: Vec<Slot>,
) -> Html {
    let class = config.class.unwrap_or(Cow::Borrowed("form"));
    let body: Vec<Html> = children.into_iter().map(|s| s.render(ctx, rcv)).collect();
    html! { ctx, rcv,
        <form class=[class]
              novalidate={config.novalidate.then_some("")}
              primal:onsubmit={on_submit}>
            <Fragment>{body.clone()}</Fragment>
        </form>
    }
}
