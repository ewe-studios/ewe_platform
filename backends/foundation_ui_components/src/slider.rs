//! # Slider (F8 — Indicators)
//!
//! WHY: A value-in-range control with the named CSS-var styling contract
//! (`--slider-value`/`--slider-percent`) that Track/Indicator/Thumb style in
//! pure CSS (spec-42 §F8). v1 ships keyboard + the hidden-range-input carrier;
//! pointer drag is the first pointer-gestures (M8) consumer.
//!
//! WHAT: [`slider`] over a single `f64` signal (range/multi-thumb deferred).
//! Root → Control → Track → Indicator + Thumb(hidden `<input type="range">`).
//!
//! HOW: The Thumb wraps a visually-hidden native `<input type="range">` — the
//! focus + form + AT carrier whose native arrows/Home/End drive the value
//! (`onchange` parses → `set_value`). Root publishes `--slider-value` and
//! `--slider-percent` (0–100); the Thumb is positioned by `--slider-percent`.
//! Pointer drag on the Control is the M8 follow-up.

use alloc::borrow::Cow;

use foundation_signals::{Context, SignalGetter, SignalSetter};
use foundation_ui_traits::Html;
use foundation_wasm_ui::{html, SharedInstructionReceiver};

use crate::machinery::gestures::slider_drag_behavior;
use crate::toggle_group::Orientation;

/// Static config for a slider.
pub struct SliderConfig {
    /// Minimum (default 0.0).
    pub min: f64,
    /// Maximum (default 100.0).
    pub max: f64,
    /// Step (default 1.0).
    pub step: f64,
    /// Large step for PageUp/PageDown (default 10.0).
    pub large_step: f64,
    /// Orientation.
    pub orientation: Orientation,
    /// Disabled.
    pub disabled: bool,
    /// `name` for the hidden range input (form serialization).
    pub name: Option<Cow<'static, str>>,
    /// Class override (default `"slider"`).
    pub class: Option<Cow<'static, str>>,
    /// `aria-label`.
    pub aria_label: Option<Cow<'static, str>>,
}

impl Default for SliderConfig {
    fn default() -> Self {
        Self {
            min: 0.0,
            max: 100.0,
            step: 1.0,
            large_step: 10.0,
            orientation: Orientation::Horizontal,
            disabled: false,
            name: None,
            class: None,
            aria_label: None,
        }
    }
}

fn percent(value: f64, min: f64, max: f64) -> f64 {
    if max <= min {
        return 0.0;
    }
    (((value - min) / (max - min)) * 100.0).clamp(0.0, 100.0)
}

/// Slider component over a single `f64` value.
#[must_use]
pub fn slider(
    ctx: &Context,
    rcv: &SharedInstructionReceiver,
    config: SliderConfig,
    value: &SignalGetter<f64>,
    set_value: SignalSetter<f64>,
) -> Html {
    let class = config.class.unwrap_or(Cow::Borrowed("slider"));
    let (min, max, step) = (config.min, config.max, config.step);
    let orientation = config.orientation.as_str();
    let disabled = config.disabled;

    let on_change = {
        let set = set_value.clone();
        ctx.callback(move |data| {
            if let Some(text) = data.value.as_deref() {
                if let Ok(n) = text.parse::<f64>() {
                    set.set(n);
                }
            }
        })
    };

    let large_step = config.large_step;
    let v_style = value.clone();
    let v_thumb = value.clone();
    let v_input = value.clone();
    let v_now = value.clone();
    html! { ctx, rcv,
        <div class=[class] data-orientation={orientation}
             aria-label=[config.aria_label]
             data-disabled={disabled.then_some("")}
             data-min={min} data-max={max} data-step={step} data-large-step={large_step}
             style={alloc::format!("--slider-value:{};--slider-percent:{}",
                    v_style.get(), percent(v_style.get(), min, max))}>
            <div class="slider-control" data-slider-control="true">
                <div class="slider-track"><div class="slider-indicator"></div></div>
                <div class="slider-thumb"
                     style={alloc::format!("--slider-percent:{}", percent(v_thumb.get(), min, max))}>
                    <input type="range" class="slider-input"
                           data-slider-input="true"
                           min={min} max={max} step={step}
                           name=[config.name]
                           aria-valuenow={v_now.get()}
                           value={v_input.get()}
                           disabled={disabled.then_some("")}
                           data-orientation={orientation}
                           primal:onchange={on_change} />
                </div>
            </div>
            <Fragment>{slider_drag_behavior()}</Fragment>
        </div>
    }
}
