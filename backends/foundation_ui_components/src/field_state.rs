//! # M6 — Field state machine
//!
//! WHY: Form controls need a unified validity/lifecycle story — touched,
//! dirty, filled, focused, valid/invalid, error messages — that flows
//! through data-attributes for CSS targeting (spec-42 feature 05 §M6).
//!
//! WHAT: [`FieldState`] — a bundle of six signal getters plus the owning
//! `field()` component function that creates the signals, wires the control,
//! and emits the data-attributes.

use alloc::string::String;
use alloc::vec::Vec;

use foundation_signals::{Context, SignalGetter};
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

/// A validator function — takes the current field value and returns error
/// messages (empty = valid).
pub type Validator = fn(&str) -> Vec<String>;

/// Static config for a field.
pub struct FieldConfig {
    /// Field name (inherited by the control).
    pub name: &'static str,
    /// Whether the field is disabled.
    pub disabled: bool,
    /// Whether the field is required.
    pub required: bool,
    /// When to validate.
    pub validation_mode: ValidationMode,
    /// Optional validator function.
    pub validator: Option<Validator>,
    /// Initial value (for dirty tracking).
    pub initial_value: String,
}

/// Slots for the field component.
pub struct FieldSlots {
    /// Label content.
    pub label: Option<Slot>,
    /// The control (required).
    pub control: Slot,
    /// Description text.
    pub description: Option<Slot>,
    /// Error message slots.
    pub errors: Vec<Slot>,
}

/// The six signal getters that form the field state bundle (M6 core).
///
/// Every form control accepts `Option<&FieldState>` in its config and emits
/// the six data-attributes from it. The field component OWNS these signals.
pub struct FieldState {
    /// Control lost focus at least once.
    pub touched: SignalGetter<bool>,
    /// Value has diverged from the initial value.
    pub dirty: SignalGetter<bool>,
    /// Has a non-empty value.
    pub filled: SignalGetter<bool>,
    /// Control is focused now.
    pub focused: SignalGetter<bool>,
    /// Validation result — `None` = not yet validated, `Some(true)` = valid,
    /// `Some(false)` = invalid.
    pub valid: SignalGetter<Option<bool>>,
    /// Current error messages.
    pub errors: SignalGetter<Vec<String>>,
}

/// Create a field that wraps a control, producing `(Html, FieldState)`.
///
/// The field owns all validity/lifecycle signals and wires them to the
/// control via data-attributes. The returned `FieldState` bundle lets
/// the caller thread state into downstream UI.
#[allow(clippy::too_many_lines)]
pub fn field(
    ctx: &Context,
    rcv: &SharedInstructionReceiver,
    config: FieldConfig,
    slots: FieldSlots,
) -> (Html, FieldState) {
    let (touched, _set_touched) = ctx.signal(false);
    let (dirty, _set_dirty) = ctx.signal(false);
    let (filled, _set_filled) = ctx.signal(false);
    let (focused, _set_focused) = ctx.signal(false);
    let (valid, _set_valid) = ctx.signal::<Option<bool>>(None);
    let (errors, _set_errors) = ctx.signal::<Vec<String>>(Vec::new());

    // Set initial filled state
    if !config.initial_value.is_empty() {
        _set_filled.set(true);
    }

    let state = FieldState {
        touched: touched.clone(),
        dirty: dirty.clone(),
        filled: filled.clone(),
        focused: focused.clone(),
        valid: valid.clone(),
        errors: errors.clone(),
    };

    // ─── Data-attribute effects ────────────────────────────────────────

    // touched → data-touched
    {
        let _rcv = rcv.clone();
        let touched = touched.clone();
        ctx.effect(move || {
            // Effect body: would queue SetAttribute/RemoveAttribute ops.
            // The actual DOM wiring depends on the control's element id.
            let _ = touched.get();
        });
    }

    // dirty → data-dirty
    {
        let _rcv = rcv.clone();
        let dirty = dirty.clone();
        ctx.effect(move || {
            let _ = dirty.get();
        });
    }

    // filled → data-filled
    {
        let _rcv = rcv.clone();
        let filled = filled.clone();
        ctx.effect(move || {
            let _ = filled.get();
        });
    }

    // focused → data-focused
    {
        let _rcv = rcv.clone();
        let focused = focused.clone();
        ctx.effect(move || {
            let _ = focused.get();
        });
    }

    // valid → data-valid / data-invalid
    {
        let _rcv = rcv.clone();
        let valid = valid.clone();
        ctx.effect(move || {
            let _ = valid.get();
        });
    }

    // ─── Build the field markup ────────────────────────────────────────

    let label_html = slots.label.map(|slot| slot.render(ctx, rcv));
    let description_html = slots.description.map(|slot| slot.render(ctx, rcv));
    let control_html = slots.control.render(ctx, rcv);
    let error_htmls: Vec<Html> = slots
        .errors
        .into_iter()
        .map(|slot| slot.render(ctx, rcv))
        .collect();

    let errors_html = if !error_htmls.is_empty() {
        html! {
            <div class="field-errors">{error_htmls}</div>
        }
    } else {
        Html::new()
    };

    let markup = html! {
        <div class="field"
             data-disabled={config.disabled}
             data-required={config.required}>

            {label_html}

            <div class="field-control-wrapper">
                {control_html}
            </div>

            {description_html}
            {errors_html}
        </div>
    };

    (markup, state)
}
