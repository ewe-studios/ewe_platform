//! # Progress / meter (F8 — Indicators)
//!
//! WHY: A determinate (or indeterminate) completion bar and its static cousin
//! the meter — the headless versions exist for custom tracks native
//! `<progress>`/`<meter>` can't style (spec-42 §F8).
//!
//! WHAT: [`progress`] (`role="progressbar"` over `Option<f64>` — `None` =
//! indeterminate) and [`meter`] (`role="meter"` over `f64`, never
//! indeterminate). Both publish `--progress-value`/`--meter-value` (0–100) so
//! the Indicator sizes in pure CSS, plus the ARIA value attributes.
//!
//! HOW: An effect-free reactive form — `aria-valuenow` and the CSS-var inline
//! style read the value getter; `data-progressing`/`data-complete`/
//! `data-indeterminate` reflect the state for the stylesheet.

use alloc::borrow::Cow;

use foundation_signals::{Context, SignalGetter};
use foundation_ui_traits::Html;
use foundation_wasm_ui::{html, SharedInstructionReceiver, Slot};

/// Static config for a progress/meter.
pub struct ProgressConfig {
    /// Minimum (default 0.0).
    pub min: f64,
    /// Maximum (default 100.0).
    pub max: f64,
    /// Class override (default `"progress"` / `"meter"`).
    pub class: Option<Cow<'static, str>>,
    /// `aria-label`.
    pub aria_label: Option<Cow<'static, str>>,
}

impl Default for ProgressConfig {
    fn default() -> Self {
        Self { min: 0.0, max: 100.0, class: None, aria_label: None }
    }
}

fn percent(value: f64, min: f64, max: f64) -> f64 {
    if max <= min {
        return 0.0;
    }
    (((value - min) / (max - min)) * 100.0).clamp(0.0, 100.0)
}

/// Progress bar over `Option<f64>` (`None` = indeterminate).
#[must_use]
pub fn progress(
    ctx: &Context,
    rcv: &SharedInstructionReceiver,
    config: ProgressConfig,
    value: &SignalGetter<Option<f64>>,
    label: Option<Slot>,
) -> Html {
    let class = config.class.unwrap_or(Cow::Borrowed("progress"));
    let (min, max) = (config.min, config.max);
    let label_html = label.map(|s| s.render(ctx, rcv));

    let v_now = value.clone();
    let v_style = value.clone();
    let v_prog = value.clone();
    let v_complete = value.clone();
    let v_indet = value.clone();
    html! { ctx, rcv,
        <div class=[class] role="progressbar"
             aria-label=[config.aria_label]
             aria-valuemin={min}
             aria-valuemax={max}
             aria-valuenow={v_now.get()}
             data-indeterminate={v_indet.get().is_none().then_some("")}
             data-progressing={v_prog.get().map_or(false, |v| percent(v, min, max) < 100.0).then_some("")}
             data-complete={v_complete.get().map_or(false, |v| percent(v, min, max) >= 100.0).then_some("")}
             style={v_style.get().map(|v| alloc::format!("--progress-value:{}", percent(v, min, max)))}>
            <div class="progress-track"><div class="progress-indicator"></div></div>
            <Fragment>{label_html.clone()}</Fragment>
        </div>
    }
}

/// Meter over `f64` (always determinate), `role="meter"`.
#[must_use]
pub fn meter(
    ctx: &Context,
    rcv: &SharedInstructionReceiver,
    config: ProgressConfig,
    value: &SignalGetter<f64>,
    label: Option<Slot>,
) -> Html {
    let class = config.class.unwrap_or(Cow::Borrowed("meter"));
    let (min, max) = (config.min, config.max);
    let label_html = label.map(|s| s.render(ctx, rcv));

    let v_now = value.clone();
    let v_style = value.clone();
    html! { ctx, rcv,
        <div class=[class] role="meter"
             aria-label=[config.aria_label]
             aria-valuemin={min}
             aria-valuemax={max}
             aria-valuenow={v_now.get()}
             style={alloc::format!("--meter-value:{}", percent(v_style.get(), min, max))}>
            <div class="meter-track"><div class="meter-indicator"></div></div>
            <Fragment>{label_html.clone()}</Fragment>
        </div>
    }
}
