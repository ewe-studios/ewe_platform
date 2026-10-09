//! # Toggle Group (F1 — Primitives)
//!
//! WHY: A set of toggles sharing one selection (single or multiple), e.g. a
//! text-alignment or view-mode switcher (spec-42 §F1).
//!
//! WHAT: `<div role="group">` over toggle `<button>`s. The selection is a
//! `Vec<String>` signal of pressed values; single mode keeps at most one.
//!
//! HOW: Items render through `<For>` (feature 01) — correct per-item ids and
//! LIVE `data-pressed` (each item's effect reads the shared `values` signal).
//! Clicks update the Vec via `ctx.callback`. The container carries
//! `data-composite` so the M5 roving-focus JS module (arrow-key navigation)
//! can attach; M5 keyboard nav itself is that module's concern.
//!
//! Data attributes: `data-orientation`, `data-disabled`, `data-multiple`,
//! `data-composite`; per item `data-pressed`, `data-value`, `data-disabled`.

use alloc::borrow::Cow;
use alloc::string::String;
use alloc::vec::Vec;

use foundation_signals::{Context, SignalGetter, SignalSetter};
use foundation_ui_traits::Html;
use foundation_wasm_ui::{html, SharedInstructionReceiver};

use crate::machinery::composite::composite_behavior;

/// Orientation for the toggle group (drives arrow-key axis in M5 + CSS).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Orientation {
    /// Horizontal layout, left/right arrows navigate.
    #[default]
    Horizontal,
    /// Vertical layout, up/down arrows navigate.
    Vertical,
}

impl Orientation {
    /// The `data-orientation` token.
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Orientation::Horizontal => "horizontal",
            Orientation::Vertical => "vertical",
        }
    }
}

/// Static config for a toggle group. Text fields are `Cow<'static, str>`.
pub struct ToggleGroupConfig {
    /// Whether multiple toggles can be pressed simultaneously.
    pub multiple: bool,
    /// Whether arrow-key navigation loops (last → first). M5 parameter.
    pub loop_focus: bool,
    /// Orientation for arrow-key navigation.
    pub orientation: Orientation,
    /// Whether the entire group is disabled.
    pub disabled: bool,
    /// Class override for the container (default: `"toggle-group"`).
    pub class: Option<Cow<'static, str>>,
    /// `aria-label` for the group.
    pub aria_label: Option<Cow<'static, str>>,
}

impl Default for ToggleGroupConfig {
    fn default() -> Self {
        Self {
            multiple: false,
            loop_focus: true,
            orientation: Orientation::Horizontal,
            disabled: false,
            class: None,
            aria_label: None,
        }
    }
}

/// A toggle item within a group.
#[derive(Clone)]
pub struct ToggleGroupItem {
    /// The toggle's identifying value (used as the `<For>` key and `data-value`).
    pub value: String,
    /// The toggle's display content.
    pub content: Html,
    /// Per-item disabled state.
    pub disabled: bool,
}

/// Toggle group component — manages a set of toggles with shared state.
///
/// Signal: `values` (`Vec<String>` of pressed toggle values). Static:
/// `multiple`, `loop_focus`, `orientation`, `disabled`.
#[must_use]
pub fn toggle_group(
    ctx: &Context,
    rcv: &SharedInstructionReceiver,
    config: ToggleGroupConfig,
    values: &SignalGetter<Vec<String>>,
    set_values: SignalSetter<Vec<String>>,
    items: Vec<ToggleGroupItem>,
) -> Html {
    let class = config.class.unwrap_or(Cow::Borrowed("toggle-group"));
    let orientation = config.orientation.as_str();
    let multiple = config.multiple;
    let group_disabled = config.disabled;
    let loop_focus = config.loop_focus;

    // Per-item render: live data-pressed (reads `values`) + click that updates
    // the Vec (toggle off if present; single mode replaces, multiple appends).
    let values_for_render = values.clone();
    let render = move |c: &Context, r: &SharedInstructionReceiver, item: &ToggleGroupItem| -> Html {
        let value: Cow<'static, str> = Cow::Owned(item.value.clone());
        let content = item.content.clone();
        let disabled = group_disabled || item.disabled;

        let on_click = {
            let set = set_values.clone();
            let get = values_for_render.clone();
            let value = item.value.clone();
            c.callback(move |_| {
                let mut current = get.get();
                if let Some(pos) = current.iter().position(|v| v == &value) {
                    current.remove(pos);
                } else if multiple {
                    current.push(value.clone());
                } else {
                    current.clear();
                    current.push(value.clone());
                }
                set.set(current);
            })
        };

        let pressed = values_for_render.clone();
        let pressed_value = item.value.clone();
        let aria = values_for_render.clone();
        let aria_value = item.value.clone();
        html! { c, r,
            <button type="button" class="toggle"
                    data-composite-item="true"
                    data-value=[value]
                    aria-pressed={aria.get().iter().any(|v| v == &aria_value)}
                    data-pressed={pressed.get().iter().any(|v| v == &pressed_value).then_some("")}
                    disabled={disabled.then_some("")}
                    data-disabled={disabled.then_some("")}
                    primal:onclick={on_click}>
                <Fragment>{content.clone()}</Fragment>
            </button>
        }
    };

    html! { ctx, rcv,
        <div class=[class] role="group"
             aria-label=[config.aria_label]
             data-composite="true"
             data-orientation={orientation}
             data-loop=[(!loop_focus).then_some("false")]
             data-disabled={group_disabled.then_some("")}
             data-multiple={multiple.then_some("")}>
            <For each={items.clone()} key={|item: &ToggleGroupItem| item.value.clone()} render={render} />
            <Fragment>{composite_behavior()}</Fragment>
        </div>
    }
}
