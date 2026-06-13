//! # Radio group (F2 — Selection controls)
//!
//! WHY: A single-choice control where exactly one option is selected, a real
//! form field per option, with the WAI-ARIA radiogroup semantics (spec-42 §F2).
//!
//! WHAT: [`radio_group`] renders `<div role="radiogroup">` over items, each a
//! visually-hidden native `<input type="radio">` (shared `name` → native
//! grouping, carries the submitted value) plus a visual indicator.
//!
//! HOW: Items render through `<For>` (correct ids + live `data-checked`).
//! Selecting writes the group's `Option<String>` value via `ctx.callback`
//! (an `Option<String>` setter has no default event mapping). The container
//! carries `data-composite` so the M5 roving-focus module can attach the
//! arrow-key "move AND select" radio pattern (that keyboard nav is M5's job).

use alloc::borrow::Cow;
use alloc::string::String;
use alloc::vec::Vec;

use foundation_signals::{Context, SignalGetter, SignalSetter};
use foundation_ui_traits::Html;
use foundation_wasm_ui::{html, SharedInstructionReceiver};

use crate::toggle_group::Orientation;

/// A radio option within a group.
#[derive(Clone)]
pub struct RadioItem {
    /// The submitted value + `<For>` key + `data-value`.
    pub value: String,
    /// The option's display content (label, etc.).
    pub content: Html,
    /// Per-item disabled state.
    pub disabled: bool,
}

/// Static config for a radio group. Text fields are `Cow<'static, str>`.
pub struct RadioGroupConfig {
    /// Shared submitted field name (native radio grouping).
    pub name: Cow<'static, str>,
    /// Whether the whole group is disabled.
    pub disabled: bool,
    /// Whether a selection is required.
    pub required: bool,
    /// Orientation (arrow-key axis for M5).
    pub orientation: Orientation,
    /// Class override (default: `"radio-group"`).
    pub class: Option<Cow<'static, str>>,
    /// `aria-label` for the group.
    pub aria_label: Option<Cow<'static, str>>,
}

impl Default for RadioGroupConfig {
    fn default() -> Self {
        Self {
            name: Cow::Borrowed(""),
            disabled: false,
            required: false,
            orientation: Orientation::Horizontal,
            class: None,
            aria_label: None,
        }
    }
}

/// Radio group component — single selection over `value` (`Option<String>`).
///
/// Data attrs: group `data-orientation`/`data-disabled`/`data-composite`; per
/// item `data-checked`/`data-disabled`, `data-value`.
#[must_use]
pub fn radio_group(
    ctx: &Context,
    rcv: &SharedInstructionReceiver,
    config: RadioGroupConfig,
    value: &SignalGetter<Option<String>>,
    set_value: SignalSetter<Option<String>>,
    items: Vec<RadioItem>,
) -> Html {
    let class = config.class.unwrap_or(Cow::Borrowed("radio-group"));
    let orientation = config.orientation.as_str();
    let group_disabled = config.disabled;
    let group_name = config.name;
    let value_for_render = value.clone();

    let render = move |c: &Context, r: &SharedInstructionReceiver, item: &RadioItem| -> Html {
        let disabled = group_disabled || item.disabled;
        let content = item.content.clone();
        let item_value: Cow<'static, str> = Cow::Owned(item.value.clone());
        let name = group_name.clone();

        let select = {
            let set = set_value.clone();
            let v = item.value.clone();
            c.callback(move |_| set.set(Some(v.clone())))
        };

        let checked_root = value_for_render.clone();
        let cr_value = item.value.clone();
        let checked_input = value_for_render.clone();
        let ci_value = item.value.clone();
        html! { c, r,
            <span class="radio"
                  data-value=[item_value.clone()]
                  data-checked={(checked_root.get().as_deref() == Some(cr_value.as_str())).then_some("")}
                  data-disabled=[disabled.then_some("")]>
                <input type="radio" class="radio-input"
                       name=[name]
                       value=[item_value]
                       checked={(checked_input.get().as_deref() == Some(ci_value.as_str())).then_some("")}
                       disabled=[disabled.then_some("")]
                       primal:onchange={select} />
                <span class="radio-indicator"></span>
                <Fragment>{content.clone()}</Fragment>
            </span>
        }
    };

    html! { ctx, rcv,
        <div class=[class] role="radiogroup"
             aria-label=[config.aria_label]
             data-composite="true"
             data-orientation={orientation}
             data-required=[config.required.then_some("")]
             data-disabled=[group_disabled.then_some("")]>
            <For each={items.clone()} key={|item: &RadioItem| item.value.clone()} render={render} />
        </div>
    }
}
