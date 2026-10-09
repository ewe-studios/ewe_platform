//! # Tabs (F3 — Disclosure)
//!
//! WHY: A tablist where selecting a tab reveals its panel, with the WAI-ARIA
//! tabs pattern — roving focus, manual or automatic activation (spec-42 §F3).
//!
//! WHAT: [`tabs`] renders Root `<div>` → List (`role="tablist"`) → Tab
//! (`role="tab"` button) + Indicator `<span>`; Panels (`role="tabpanel"`).
//! Selection is an `Option<String>` of the active tab value (explicit values,
//! not indexes).
//!
//! HOW: The tablist embeds the M5 [`composite_behavior`] for roving focus
//! (`data-composite-select` in AUTOMATIC mode → arrows activate; manual mode →
//! arrows move focus, Enter/Space activate via the native button). The
//! [`tab_indicator_behavior`] publishes the active tab's box as `--active-tab-*`
//! for the animated underline. Selecting writes `selected` and an
//! `activation_dir` signal (`left|right|up|down|none`) reflected as
//! `data-activation-direction` for slide animations. Panels stay MOUNTED
//! (native `hidden` + `data-hidden`), each `tabindex=0`, tied to its tab via
//! `aria-labelledby`/`aria-controls`, embedding M7 transitions.

use alloc::borrow::Cow;
use alloc::string::String;
use alloc::vec::Vec;

use foundation_signals::{Context, SignalGetter, SignalSetter};
use foundation_ui_traits::Html;
use foundation_wasm_ui::{html, SharedInstructionReceiver};

use crate::machinery::composite::composite_behavior;
use crate::machinery::measure::tab_indicator_behavior;
use crate::machinery::transition::transition_behavior;
use crate::toggle_group::Orientation;

/// Static config for a tabs component.
pub struct TabsConfig {
    /// Orientation (arrow-key axis + `aria-orientation`).
    pub orientation: Orientation,
    /// Automatic activation: arrow focus also activates (default: manual —
    /// arrows move focus, Enter/Space activate).
    pub activate_on_focus: bool,
    /// Whether arrow navigation loops.
    pub loop_focus: bool,
    /// Class override for the root (default: `"tabs"`).
    pub class: Option<Cow<'static, str>>,
}

impl Default for TabsConfig {
    fn default() -> Self {
        Self {
            orientation: Orientation::Horizontal,
            activate_on_focus: false,
            loop_focus: true,
            class: None,
        }
    }
}

/// One tab + its panel.
#[derive(Clone)]
pub struct TabDef {
    /// The tab's identifying value (`<For>` key + selection).
    pub value: String,
    /// Tab (button) label content.
    pub label: Html,
    /// Panel content.
    pub panel: Html,
    /// Per-tab disabled state.
    pub disabled: bool,
}

/// Tabs component — a tablist over `selected` (`Option<String>`).
///
/// Signal: `selected`. Static: `orientation`, `activate_on_focus`, `loop_focus`.
#[must_use]
pub fn tabs(
    ctx: &Context,
    rcv: &SharedInstructionReceiver,
    config: TabsConfig,
    selected: &SignalGetter<Option<String>>,
    set_selected: SignalSetter<Option<String>>,
    tab_defs: Vec<TabDef>,
) -> Html {
    let class = config.class.unwrap_or(Cow::Borrowed("tabs"));
    let orientation = config.orientation;
    let orientation_str = orientation.as_str();
    let vertical = matches!(orientation, Orientation::Vertical);
    let order: Vec<String> = tab_defs.iter().map(|t| t.value.clone()).collect();

    // activation-direction tracks which way selection moved (slide animations).
    let (activation_dir, set_activation_dir) = ctx.signal::<&'static str>("none");

    let selected_for_tabs = selected.clone();
    let dir_for_tabs = activation_dir.clone();
    let order_for_tabs = order.clone();

    // ─── Tabs (the tablist) ─────────────────────────────────────────────────
    let tab_render = move |c: &Context, r: &SharedInstructionReceiver, tab: &TabDef| -> Html {
        let value = tab.value.clone();
        let label = tab.label.clone();
        let disabled = tab.disabled;
        let tab_id: Cow<'static, str> = Cow::Owned(alloc::format!("tab-{value}"));
        let panel_ref: Cow<'static, str> = Cow::Owned(alloc::format!("tabpanel-{value}"));

        let on_click = {
            let set_sel = set_selected.clone();
            let set_dir = set_activation_dir.clone();
            let get_sel = selected_for_tabs.clone();
            let order = order_for_tabs.clone();
            let value = value.clone();
            c.callback(move |_| {
                let new_idx = order.iter().position(|v| v == &value);
                let prev_idx = get_sel
                    .get()
                    .and_then(|cur| order.iter().position(|v| v == &cur));
                let dir = match (prev_idx, new_idx) {
                    (Some(p), Some(n)) if n > p => if vertical { "down" } else { "right" },
                    (Some(p), Some(n)) if n < p => if vertical { "up" } else { "left" },
                    _ => "none",
                };
                set_dir.set(dir);
                set_sel.set(Some(value.clone()));
            })
        };

        let is_sel = {
            let get = selected_for_tabs.clone();
            let value = value.clone();
            move || get.get().as_deref() == Some(value.as_str())
        };
        let s_aria = is_sel.clone();
        let s_active = is_sel.clone();
        let s_comp_active = is_sel.clone();
        let d_dir = dir_for_tabs.clone();
        html! { c, r,
            <button type="button" class="tabs-tab" role="tab"
                    id=[tab_id]
                    aria-selected={s_aria()}
                    aria-controls=[panel_ref]
                    data-composite-item="true"
                    data-composite-active={s_comp_active().then_some("")}
                    data-active={s_active().then_some("")}
                    data-disabled={disabled.then_some("")}
                    data-orientation={orientation_str}
                    data-activation-direction={d_dir.get()}
                    disabled={disabled.then_some("")}
                    primal:onclick={on_click}>
                <Fragment>{label.clone()}</Fragment>
            </button>
        }
    };

    // ─── Panels ─────────────────────────────────────────────────────────────
    let selected_for_panels = selected.clone();
    let dir_for_panels = activation_dir.clone();
    let panel_render = move |c: &Context, r: &SharedInstructionReceiver, tab: &TabDef| -> Html {
        let value = tab.value.clone();
        let panel = tab.panel.clone();
        let panel_id: Cow<'static, str> = Cow::Owned(alloc::format!("tabpanel-{value}"));
        let tab_ref: Cow<'static, str> = Cow::Owned(alloc::format!("tab-{value}"));

        let is_sel = {
            let get = selected_for_panels.clone();
            let value = value.clone();
            move || get.get().as_deref() == Some(value.as_str())
        };
        let p_open = is_sel.clone();
        let p_closed = is_sel.clone();
        let p_hidden = is_sel.clone();
        let p_data_hidden = is_sel.clone();
        let d_dir = dir_for_panels.clone();
        html! { c, r,
            <div class="tabs-panel" role="tabpanel"
                 id=[panel_id]
                 aria-labelledby=[tab_ref]
                 tabindex="0"
                 data-orientation={orientation_str}
                 data-activation-direction={d_dir.get()}
                 data-open={p_open().then_some("")}
                 data-closed={(!p_closed()).then_some("")}
                 data-hidden={(!p_data_hidden()).then_some("")}
                 hidden={(!p_hidden()).then_some("")}>
                <Fragment>{panel.clone()}</Fragment>
                <Fragment>{transition_behavior()}</Fragment>
            </div>
        }
    };

    let select_mode = config.activate_on_focus;
    let loop_focus = config.loop_focus;
    let list_dir = activation_dir.clone();
    let tab_defs_panels = tab_defs.clone();
    html! { ctx, rcv,
        <div class=[class] data-orientation={orientation_str}>
            <div class="tabs-list" role="tablist"
                 aria-orientation={orientation_str}
                 data-orientation={orientation_str}
                 data-activation-direction={list_dir.get()}
                 data-composite="true"
                 data-composite-select=[select_mode.then_some("true")]
                 data-loop=[(!loop_focus).then_some("false")]>
                <For each={tab_defs.clone()} key={|t: &TabDef| t.value.clone()} render={tab_render} />
                <span class="tabs-indicator" aria-hidden="true"></span>
                <Fragment>{composite_behavior()}</Fragment>
                <Fragment>{tab_indicator_behavior()}</Fragment>
            </div>
            <For each={tab_defs_panels.clone()} key={|t: &TabDef| t.value.clone()} render={panel_render} />
        </div>
    }
}
