//! # M6 — Field state machine
//!
//! WHY: Form controls need a unified validity/lifecycle story — touched,
//! dirty, filled, focused, valid/invalid, errors — flowing through
//! data-attributes for CSS targeting (spec-42 feature 05 §M6).
//!
//! WHAT: [`FieldState`] (the six signal getters), [`FieldBinding`] (the
//! ready-made event [`Callback`]s a control wires onto itself), and the
//! `field()` function that OWNS the signals, runs the validator, and reflects
//! the six data-attributes on the field root.
//!
//! HOW: `field()` builds the binding and hands it to a control-builder closure
//! (decision: the field doesn't own the control's markup — it hands the
//! control ready-made handlers to spread onto ANY element). The control's
//! `focus`/`blur`/`input` events drive the signals through those callbacks;
//! `focus`/`blur` carry no convertible value so they ride `ctx.callback`,
//! while `input` reads `EventData.value`.

use alloc::borrow::Cow;
use alloc::string::String;
use alloc::vec::Vec;

use foundation_signals::{Callback, Context, SignalGetter};
use foundation_ui_traits::Html;
use foundation_wasm_ui::{html, SharedInstructionReceiver, Slot};

/// Validation mode: when to run the validator.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ValidationMode {
    /// Validate only on form submit.
    #[default]
    OnSubmit,
    /// Validate when the control loses focus.
    OnBlur,
    /// Validate on every value change.
    OnChange,
}

/// A validator — takes the current value, returns error messages (empty = ok).
pub type Validator = fn(&str) -> Vec<String>;

/// Static config for a field. Text fields are `Cow<'static, str>`.
pub struct FieldConfig {
    /// Field name (the control inherits it).
    pub name: Cow<'static, str>,
    /// Whether the field is disabled.
    pub disabled: bool,
    /// Whether the field is required.
    pub required: bool,
    /// When to run the validator.
    pub validation_mode: ValidationMode,
    /// Optional validator.
    pub validator: Option<Validator>,
    /// Initial value — the dirty baseline.
    pub initial_value: String,
}

impl Default for FieldConfig {
    fn default() -> Self {
        Self {
            name: Cow::Borrowed(""),
            disabled: false,
            required: false,
            validation_mode: ValidationMode::default(),
            validator: None,
            initial_value: String::new(),
        }
    }
}

/// Slots for the field shell (the control is built by the closure passed to
/// [`field`], not a slot — it needs the [`FieldBinding`]).
pub struct FieldSlots {
    /// Label content.
    pub label: Option<Slot>,
    /// Description / help text.
    pub description: Option<Slot>,
    /// Error message content (shown by CSS under `[data-invalid]`).
    pub errors: Vec<Slot>,
}

impl Default for FieldSlots {
    fn default() -> Self {
        Self {
            label: None,
            description: None,
            errors: Vec::new(),
        }
    }
}

/// The six signal getters that form the field state bundle (M6 core). The
/// field OWNS the signals; downstream UI reads these.
pub struct FieldState {
    /// Control lost focus at least once.
    pub touched: SignalGetter<bool>,
    /// Value diverged from the initial value.
    pub dirty: SignalGetter<bool>,
    /// Has a non-empty value.
    pub filled: SignalGetter<bool>,
    /// Control is focused now.
    pub focused: SignalGetter<bool>,
    /// `None` = not yet validated, `Some(true)` = valid, `Some(false)` = invalid.
    pub valid: SignalGetter<Option<bool>>,
    /// Current error messages.
    pub errors: SignalGetter<Vec<String>>,
}

/// Ready-made handlers the control spreads onto its element, plus the id the
/// control should adopt (so the label's `for` matches). Wire all three events:
///
/// ```ignore
/// field(ctx, rcv, cfg, slots, |c, r, b| html! { c, r,
///     <input id=[b.control_id.clone()] value={v.get()}
///            primal:onchange={set_v}
///            primal:onfocus={b.on_focus}
///            primal:onblur={b.on_blur}
///            primal:oninput={b.on_input} />
/// })
/// ```
pub struct FieldBinding {
    /// `focus` → `focused = true`.
    pub on_focus: Callback,
    /// `blur` → `touched = true`, `focused = false` (+ validate on `OnBlur`).
    pub on_blur: Callback,
    /// `input` → `dirty`/`filled` from the value (+ validate on `OnChange`).
    pub on_input: Callback,
    /// The id the control should set as its element `id` (label `for` target).
    pub control_id: String,
}

/// Create a field that wraps a control, producing `(Html, FieldState)`.
///
/// `control` is a closure handed `(ctx, rcv, &binding)` — it returns the
/// control's `Html` with those handlers attached. Passing `ctx`/`rcv` in (vs.
/// capturing them) avoids a borrow conflict with `field`'s own `&ctx`/`&rcv`.
/// The field owns all validity/lifecycle signals and reflects them as
/// data-attributes on the field root.
#[must_use]
pub fn field<F>(
    ctx: &Context,
    rcv: &SharedInstructionReceiver,
    config: FieldConfig,
    slots: FieldSlots,
    control: F,
) -> (Html, FieldState)
where
    F: FnOnce(&Context, &SharedInstructionReceiver, &FieldBinding) -> Html,
{
    let (touched, set_touched) = ctx.signal(false);
    let (dirty, set_dirty) = ctx.signal(false);
    let (filled, set_filled) = ctx.signal(!config.initial_value.is_empty());
    let (focused, set_focused) = ctx.signal(false);
    let (valid, set_valid) = ctx.signal::<Option<bool>>(None);
    let (errors, set_errors) = ctx.signal::<Vec<String>>(Vec::new());

    let control_id = alloc::format!("field-control-{}", ctx.allocate_id_block(1));
    let mode = config.validation_mode;
    let validator = config.validator;
    let initial_value = config.initial_value.clone();

    // Run the validator over `value`, writing valid + errors. No-op if unset.
    let run_validate = {
        let set_valid = set_valid.clone();
        let set_errors = set_errors.clone();
        move |value: &str| {
            if let Some(validator) = validator {
                let errs = validator(value);
                set_valid.set(Some(errs.is_empty()));
                set_errors.set(errs);
            }
        }
    };

    let on_focus = {
        let set_focused = set_focused.clone();
        ctx.callback(move |_| set_focused.set(true))
    };
    let on_blur = {
        let set_touched = set_touched.clone();
        let set_focused = set_focused.clone();
        let validate = run_validate.clone();
        ctx.callback(move |data| {
            set_touched.set(true);
            set_focused.set(false);
            if mode == ValidationMode::OnBlur {
                validate(data.value.as_deref().unwrap_or(""));
            }
        })
    };
    let on_input = {
        let validate = run_validate.clone();
        let initial_value = initial_value.clone();
        ctx.callback(move |data| {
            let value = data.value.as_deref().unwrap_or("");
            set_filled.set(!value.is_empty());
            set_dirty.set(value != initial_value);
            if mode == ValidationMode::OnChange {
                validate(value);
            }
        })
    };

    let binding = FieldBinding {
        on_focus,
        on_blur,
        on_input,
        control_id: control_id.clone(),
    };

    // Render the slots + the caller's control.
    let label_html = slots.label.map(|slot| slot.render(ctx, rcv));
    let description_html = slots.description.map(|slot| slot.render(ctx, rcv));
    let control_html = control(ctx, rcv, &binding);
    let error_htmls: Vec<Html> = slots
        .errors
        .into_iter()
        .map(|slot| slot.render(ctx, rcv))
        .collect();

    let state = FieldState {
        touched: touched.clone(),
        dirty: dirty.clone(),
        filled: filled.clone(),
        focused: focused.clone(),
        valid: valid.clone(),
        errors: errors.clone(),
    };

    // Reactive data-attribute reflection (one cloned getter per attr).
    let d_touched = touched.clone();
    let d_dirty = dirty.clone();
    let d_filled = filled.clone();
    let d_focused = focused.clone();
    let d_valid = valid.clone();
    let d_invalid = valid.clone();
    let label_for: Cow<'static, str> = Cow::Owned(control_id);

    let markup = html! { ctx, rcv,
        <div class="field"
             data-disabled=[config.disabled.then_some("")]
             data-required=[config.required.then_some("")]
             data-touched={d_touched.get().then_some("")}
             data-dirty={d_dirty.get().then_some("")}
             data-filled={d_filled.get().then_some("")}
             data-focused={d_focused.get().then_some("")}
             data-valid={d_valid.get().and_then(|v| v.then_some(""))}
             data-invalid={d_invalid.get().and_then(|v| (!v).then_some(""))}>
            <label class="field-label" for=[label_for]><Fragment>{label_html.clone()}</Fragment></label>
            <div class="field-control"><Fragment>{control_html.clone()}</Fragment></div>
            <Fragment>{description_html.clone()}</Fragment>
            <div class="field-error"><Fragment>{error_htmls.clone()}</Fragment></div>
        </div>
    };

    (markup, state)
}
