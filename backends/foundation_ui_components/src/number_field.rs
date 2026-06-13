//! # Number field (F7 — Form)
//!
//! WHY: A numeric input with decrement/increment affordances and step
//! semantics — the reason it exists over `<input type=number>` is custom
//! stepping/format (spec-42 §F7). Scrub + hold-repeat are the M8 follow-up.
//!
//! WHAT: [`number_field`] renders Root → Decrement / Input / Increment over a
//! `f64` signal. Buttons step-and-clamp; typing parses (typed text may exceed
//! the range per base-ui; step interactions always clamp).
//!
//! HOW: The input is `inputmode="decimal"` TEXT (not `type=number`, whose
//! native spinner is what we're replacing). Decrement/Increment are Rust
//! callbacks doing the clamped step math; a scoped script maps ArrowUp/Down on
//! the input to those buttons (`.click()`). `on_commit` fires alongside single
//! keyboard steps; blur-commit + hold-repeat + scrub are the M8 follow-up.

use alloc::borrow::Cow;

use foundation_signals::{Context, SignalGetter, SignalSetter};
use foundation_ui_traits::Html;
use foundation_wasm_ui::{html, SharedInstructionReceiver};

use crate::machinery::scoped_script;

/// Static config for a number field.
pub struct NumberFieldConfig {
    /// Minimum value (inclusive).
    pub min: Option<f64>,
    /// Maximum value (inclusive).
    pub max: Option<f64>,
    /// Step for arrow/button changes (default 1.0).
    pub step: f64,
    /// Disabled.
    pub disabled: bool,
    /// `name` for form submission.
    pub name: Option<Cow<'static, str>>,
    /// Class override (default `"number-field"`).
    pub class: Option<Cow<'static, str>>,
    /// `aria-label`.
    pub aria_label: Option<Cow<'static, str>>,
}

impl Default for NumberFieldConfig {
    fn default() -> Self {
        Self { min: None, max: None, step: 1.0, disabled: false, name: None, class: None, aria_label: None }
    }
}

fn clamp(v: f64, min: Option<f64>, max: Option<f64>) -> f64 {
    let v = match min {
        Some(m) if v < m => m,
        _ => v,
    };
    match max {
        Some(m) if v > m => m,
        _ => v,
    }
}

/// Arrow-key stepping: ArrowUp/Down on the input click the inc/dec buttons.
const NUMBER_FIELD_JS: &str = r#"function(scope){
  var root = scope.parent();
  var input = root.querySelector('[data-nf-input]');
  if (!input || input.__nf) return; input.__nf = true;
  scope.addEvent(input, 'keydown', function(e){
    if (e.key === 'ArrowUp') { e.preventDefault(); var i = root.querySelector('[data-nf-increment]'); if (i) i.click(); }
    else if (e.key === 'ArrowDown') { e.preventDefault(); var d = root.querySelector('[data-nf-decrement]'); if (d) d.click(); }
  });
}"#;

/// Number-field component over a `f64` signal.
#[must_use]
pub fn number_field(
    ctx: &Context,
    rcv: &SharedInstructionReceiver,
    config: NumberFieldConfig,
    value: &SignalGetter<f64>,
    set_value: SignalSetter<f64>,
) -> Html {
    let class = config.class.unwrap_or(Cow::Borrowed("number-field"));
    let (min, max, step) = (config.min, config.max, config.step);
    let disabled = config.disabled;

    let decrement = {
        let value = value.clone();
        let set = set_value.clone();
        ctx.callback(move |_| set.set(clamp(value.get() - step, min, max)))
    };
    let increment = {
        let value = value.clone();
        let set = set_value.clone();
        ctx.callback(move |_| set.set(clamp(value.get() + step, min, max)))
    };
    // Typing: parse to f64; typed text MAY exceed the range (no clamp here).
    let on_input = {
        let set = set_value.clone();
        ctx.callback(move |data| {
            if let Some(text) = data.value.as_deref() {
                if let Ok(n) = text.parse::<f64>() {
                    set.set(n);
                }
            }
        })
    };

    let v_attr = value.clone();
    let aria_now = value.clone();
    html! { ctx, rcv,
        <div class=[class] role="group"
             aria-label=[config.aria_label]
             data-disabled={disabled.then_some("")}>
            <button type="button" class="number-field-decrement"
                    data-nf-decrement="true" aria-label="Decrease"
                    disabled={disabled.then_some("")}
                    primal:onclick={decrement}>{"−"}</button>
            <input class="number-field-input" type="text" inputmode="decimal"
                   data-nf-input="true"
                   name=[config.name]
                   role="spinbutton"
                   aria-valuenow={aria_now.get()}
                   value={v_attr.get()}
                   disabled={disabled.then_some("")}
                   primal:onchange={on_input} />
            <button type="button" class="number-field-increment"
                    data-nf-increment="true" aria-label="Increase"
                    disabled={disabled.then_some("")}
                    primal:onclick={increment}>{"+"}</button>
            <Fragment>{scoped_script(NUMBER_FIELD_JS)}</Fragment>
        </div>
    }
}
