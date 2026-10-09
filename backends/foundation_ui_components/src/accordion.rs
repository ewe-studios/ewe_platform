//! # Accordion (F3 — Disclosure)
//!
//! WHY: Multiple collapsibles sharing one selection — `single` (opening one
//! closes the open one) or `multiple` (spec-42 §F3).
//!
//! WHAT: [`accordion`] renders Root `<div>` → Item `<div>` → Header `<h3>` +
//! Trigger `<button>` + Panel `<div role="region">`. Open set is a
//! `Vec<String>` signal of open item values.
//!
//! HOW: Items render via `<For>` (correct ids + live `data-open`). Post-APG,
//! triggers are PLAIN tab stops (no roving / M5 — base-ui removed it following
//! the APG update); Tab/Shift+Tab move, Enter/Space toggle (native button).
//! Single mode writes `[value]`/`[]` in ONE setter (glitch-free). Each panel
//! ties to its trigger via `aria-labelledby`/`aria-controls`, stays MOUNTED
//! (native `hidden`), and embeds M7 + panel-size measurement.

use alloc::borrow::Cow;
use alloc::string::String;
use alloc::vec::Vec;

use foundation_signals::{Context, SignalGetter, SignalSetter};
use foundation_ui_traits::Html;
use foundation_wasm_ui::{html, SharedInstructionReceiver};

use crate::machinery::measure::panel_size_behavior;
use crate::machinery::transition::transition_behavior;

/// Static config for an accordion.
pub struct AccordionConfig {
    /// Allow multiple items open at once (default: single).
    pub multiple: bool,
    /// Disable the whole accordion.
    pub disabled: bool,
    /// Class override for the root (default: `"accordion"`).
    pub class: Option<Cow<'static, str>>,
}

impl Default for AccordionConfig {
    fn default() -> Self {
        Self { multiple: false, disabled: false, class: None }
    }
}

/// One accordion item.
#[derive(Clone)]
pub struct AccordionItem {
    /// The item's identifying value (`<For>` key + open-set membership).
    pub value: String,
    /// Header (trigger) content.
    pub header: Html,
    /// Panel content.
    pub panel: Html,
    /// Per-item disabled state.
    pub disabled: bool,
}

/// Accordion component — a set of collapsibles over a shared open set.
///
/// Signal: `open_items` (`Vec<String>`). Static: `multiple`, `disabled`.
#[must_use]
pub fn accordion(
    ctx: &Context,
    rcv: &SharedInstructionReceiver,
    config: AccordionConfig,
    open_items: &SignalGetter<Vec<String>>,
    set_open_items: SignalSetter<Vec<String>>,
    items: Vec<AccordionItem>,
) -> Html {
    let class = config.class.unwrap_or(Cow::Borrowed("accordion"));
    let multiple = config.multiple;
    let group_disabled = config.disabled;
    let open_for_render = open_items.clone();

    let render = move |c: &Context, r: &SharedInstructionReceiver, item: &AccordionItem| -> Html {
        let value = item.value.clone();
        let disabled = group_disabled || item.disabled;
        let header = item.header.clone();
        let panel = item.panel.clone();
        let trigger_id: Cow<'static, str> = Cow::Owned(alloc::format!("accordion-trigger-{value}"));
        let trigger_ref = trigger_id.clone();
        let panel_id: Cow<'static, str> = Cow::Owned(alloc::format!("accordion-panel-{value}"));
        let panel_ref = panel_id.clone();

        let on_click = {
            let set = set_open_items.clone();
            let get = open_for_render.clone();
            let value = value.clone();
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

        let is_open = {
            let get = open_for_render.clone();
            let value = value.clone();
            move || get.get().iter().any(|v| v == &value)
        };
        // One closure clone per reactive attribute (each `{…}` is its own closure).
        let i_item_o = is_open.clone();
        let i_item_c = is_open.clone();
        let i_trig_e = is_open.clone();
        let i_trig_o = is_open.clone();
        let i_panel_o = is_open.clone();
        let i_panel_c = is_open.clone();
        let i_panel_h = is_open.clone();
        html! { c, r,
            <div class="accordion-item"
                 data-value=[Cow::<'static, str>::Owned(value.clone())]
                 data-disabled={disabled.then_some("")}
                 data-open={i_item_o().then_some("")}
                 data-closed={(!i_item_c()).then_some("")}>
                <h3 class="accordion-header">
                    <button type="button" class="accordion-trigger"
                            id=[trigger_id]
                            aria-expanded={i_trig_e()}
                            aria-controls=[panel_ref]
                            disabled={disabled.then_some("")}
                            data-disabled={disabled.then_some("")}
                            data-panel-open={i_trig_o().then_some("")}
                            primal:onclick={on_click}>
                        <Fragment>{header.clone()}</Fragment>
                    </button>
                </h3>
                <div class="accordion-panel" id=[panel_id]
                     role="region"
                     aria-labelledby=[trigger_ref]
                     data-size-var="--accordion-panel"
                     data-open={i_panel_o().then_some("")}
                     data-closed={(!i_panel_c()).then_some("")}
                     hidden={(!i_panel_h()).then_some("")}>
                    <Fragment>{panel.clone()}</Fragment>
                    <Fragment>{transition_behavior()}</Fragment>
                    <Fragment>{panel_size_behavior()}</Fragment>
                </div>
            </div>
        }
    };

    html! { ctx, rcv,
        <div class=[class]
             data-disabled={group_disabled.then_some("")}
             data-multiple={multiple.then_some("")}>
            <For each={items.clone()} key={|item: &AccordionItem| item.value.clone()} render={render} />
        </div>
    }
}
