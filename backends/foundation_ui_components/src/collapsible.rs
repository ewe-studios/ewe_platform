//! # Collapsible (F3 — Disclosure)
//!
//! WHY: A trigger that shows/hides a panel — the disclosure primitive
//! (spec-42 §F3). The headless, signal-driven form (the `<details>` recipe is
//! the no-runtime alternative; this one survives morphs and arbitrary panel
//! placement).
//!
//! WHAT: [`collapsible`] renders Root `<div>` → Trigger `<button>` + Panel
//! `<div>`. The `open` signal drives `aria-expanded`, the `data-open`/
//! `data-closed` triad (root + trigger + panel), and the panel's native
//! `hidden` (visibility model — panels stay MOUNTED).
//!
//! HOW: Native button = Enter/Space for free; `aria-controls`/`aria-expanded`
//! tie trigger→panel. The panel embeds the M7 [`transition_behavior`] (enter/
//! leave styles) and the [`panel_size_behavior`] (publishes
//! `--collapsible-panel-height/-width` from its scroll size so CSS can animate
//! `height` between 0 and the measured value).

use alloc::borrow::Cow;
use alloc::vec::Vec;

use foundation_signals::{Context, SignalGetter, SignalSetter};
use foundation_ui_traits::Html;
use foundation_wasm_ui::{html, SharedInstructionReceiver, Slot};

use crate::machinery::measure::panel_size_behavior;
use crate::machinery::transition::transition_behavior;

/// Static config for a collapsible.
pub struct CollapsibleConfig {
    /// Whether the trigger is disabled.
    pub disabled: bool,
    /// Use `hidden="until-found"` so find-in-page can reveal the panel
    /// (browser fires `beforematch`; the host should flip `open`).
    pub hidden_until_found: bool,
    /// Class override for the root (default: `"collapsible"`).
    pub class: Option<Cow<'static, str>>,
}

impl Default for CollapsibleConfig {
    fn default() -> Self {
        Self { disabled: false, hidden_until_found: false, class: None }
    }
}

/// Slots for a collapsible: the trigger content and the panel content.
pub struct CollapsibleSlots {
    /// Trigger (button) content.
    pub trigger: Vec<Slot>,
    /// Panel content.
    pub children: Vec<Slot>,
}

impl Default for CollapsibleSlots {
    fn default() -> Self {
        Self { trigger: Vec::new(), children: Vec::new() }
    }
}

/// Collapsible component — a disclosure with a signal-driven panel.
///
/// Signal: `open` (toggled by the trigger). Static: `disabled`,
/// `hidden_until_found`, `class`.
#[must_use]
pub fn collapsible(
    ctx: &Context,
    rcv: &SharedInstructionReceiver,
    config: CollapsibleConfig,
    open: &SignalGetter<bool>,
    set_open: SignalSetter<bool>,
    slots: CollapsibleSlots,
) -> Html {
    let class = config.class.unwrap_or(Cow::Borrowed("collapsible"));
    let disabled = config.disabled;
    let panel_id = alloc::format!("collapsible-panel-{}", ctx.allocate_id_block(1));
    let panel_id_attr: Cow<'static, str> = Cow::Owned(panel_id.clone());
    let controls: Cow<'static, str> = Cow::Owned(panel_id);

    let trigger_html: Vec<Html> = slots.trigger.into_iter().map(|s| s.render(ctx, rcv)).collect();
    let panel_html: Vec<Html> = slots.children.into_iter().map(|s| s.render(ctx, rcv)).collect();

    let toggle = {
        let open = open.clone();
        ctx.callback(move |_| set_open.set(!open.get()))
    };

    // hidden: when closed, native `hidden` (or `until-found`); presence attr.
    let hidden_value: Cow<'static, str> =
        if config.hidden_until_found { Cow::Borrowed("until-found") } else { Cow::Borrowed("") };

    // One getter clone per reactive attribute (each `{…}` is its own closure).
    let r_open = open.clone();
    let r_closed = open.clone();
    let t_expanded = open.clone();
    let t_panel = open.clone();
    let t_open = open.clone();
    let t_closed = open.clone();
    let p_open = open.clone();
    let p_closed = open.clone();
    let p_hidden = open.clone();

    html! { ctx, rcv,
        <div class=[class]
             data-open={r_open.get().then_some("")}
             data-closed={(!r_closed.get()).then_some("")}>
            <button type="button" class="collapsible-trigger"
                    aria-expanded={t_expanded.get()}
                    aria-controls=[controls]
                    disabled={disabled.then_some("")}
                    data-disabled={disabled.then_some("")}
                    data-panel-open={t_panel.get().then_some("")}
                    data-open={t_open.get().then_some("")}
                    data-closed={(!t_closed.get()).then_some("")}
                    primal:onclick={toggle}>
                <Fragment>{trigger_html.clone()}</Fragment>
            </button>
            <div class="collapsible-panel" id=[panel_id_attr]
                 role="region"
                 data-size-var="--collapsible-panel"
                 data-open={p_open.get().then_some("")}
                 data-closed={(!p_closed.get()).then_some("")}
                 hidden={(!p_hidden.get()).then_some(hidden_value.clone())}>
                <Fragment>{panel_html.clone()}</Fragment>
                <Fragment>{transition_behavior()}</Fragment>
                <Fragment>{panel_size_behavior()}</Fragment>
            </div>
        </div>
    }
}
