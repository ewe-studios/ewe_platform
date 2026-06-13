//! # Popover / tooltip / preview-card (F4 — Overlays)
//!
//! WHY: An anchored, non-modal (by default) surface tied to a trigger —
//! popover (click), tooltip (hover/focus, `role="tooltip"`), preview-card
//! (hover link preview). They share ONE machinery stack: M1 positioning, M3
//! dismiss + hover, M7 transitions, optionally M2/M4 when modal (spec-42 §F4).
//!
//! WHAT: [`popover`] (the base over a `(open, set_open)` signal) and the
//! [`tooltip`]/[`preview_card`] specializations (hover defaults + role fixed).
//!
//! HOW: Trigger `<button>` carries `aria-expanded`/`aria-controls` (or
//! `aria-describedby` for tooltip), `data-popup-open`/`data-pressed`, and a
//! click that toggles `open`; with hover config it embeds
//! [`hover_behavior`] + delay attrs and sibling `[data-hover-open/close]`
//! actions. A positioner `<div>` (`primal:anchor` = trigger id) runs
//! [`position_behavior`] and holds the popup, which embeds
//! [`transition_behavior`] + [`dismiss_behavior`] (Escape/outside →
//! `[data-dismiss-action]`) and, when modal, [`scroll_lock_behavior`] +
//! [`focus_trap_behavior`].

use alloc::borrow::Cow;
use alloc::string::String;
use alloc::vec::Vec;

use foundation_signals::{Context, SignalGetter, SignalSetter};
use foundation_ui_traits::Html;
use foundation_wasm_ui::{html, SharedInstructionReceiver, Slot};

use crate::machinery::dismiss::dismiss_behavior;
use crate::machinery::focus_trap::focus_trap_behavior;
use crate::machinery::hover::hover_behavior;
use crate::machinery::position::position_behavior;
use crate::machinery::scroll_lock::scroll_lock_behavior;
use crate::machinery::transition::transition_behavior;
use crate::positioning::{PlacementAlign, PlacementSide};

/// Hover-intent timings (ms) for an overlay opened on hover/focus.
#[derive(Clone, Copy, Debug)]
pub struct HoverConfig {
    /// Open delay.
    pub delay: u32,
    /// Close delay.
    pub close_delay: u32,
}

/// Static config for a popover.
pub struct PopoverConfig {
    /// Preferred side.
    pub side: PlacementSide,
    /// Preferred alignment.
    pub align: PlacementAlign,
    /// Offset from the anchor (px).
    pub offset: i32,
    /// Modal: activates scroll-lock + focus-trap (popover default false).
    pub modal: bool,
    /// Open on hover/focus with these delays instead of click.
    pub hover: Option<HoverConfig>,
    /// Popup `role` (default `"dialog"`; tooltip uses `"tooltip"`).
    pub role: Cow<'static, str>,
    /// Trigger links the popup via `aria-describedby` instead of
    /// `aria-controls`/`aria-expanded` (tooltip semantics).
    pub describe: bool,
    /// Class override for the popup (default: `"popover"`).
    pub class: Option<Cow<'static, str>>,
}

impl Default for PopoverConfig {
    fn default() -> Self {
        Self {
            side: PlacementSide::Bottom,
            align: PlacementAlign::Center,
            offset: 4,
            modal: false,
            hover: None,
            role: Cow::Borrowed("dialog"),
            describe: false,
            class: None,
        }
    }
}

/// Slots for a popover.
pub struct PopoverSlots {
    /// Trigger content.
    pub trigger: Vec<Slot>,
    /// Popup content.
    pub children: Vec<Slot>,
    /// Optional arrow content (positioned via `--transform-origin`).
    pub arrow: Option<Slot>,
}

impl Default for PopoverSlots {
    fn default() -> Self {
        Self { trigger: Vec::new(), children: Vec::new(), arrow: None }
    }
}

/// Popover component — an anchored surface over `(open, set_open)`.
#[must_use]
pub fn popover(
    ctx: &Context,
    rcv: &SharedInstructionReceiver,
    config: PopoverConfig,
    open: &SignalGetter<bool>,
    set_open: SignalSetter<bool>,
    slots: PopoverSlots,
) -> Html {
    let id_n = ctx.allocate_id_block(1);
    let trigger_id: String = alloc::format!("popover-trigger-{id_n}");
    let popup_id: String = alloc::format!("popover-popup-{id_n}");
    let class = config.class.unwrap_or(Cow::Borrowed("popover"));
    let modal = config.modal;
    let describe = config.describe;

    let trigger_html: Vec<Html> = slots.trigger.into_iter().map(|s| s.render(ctx, rcv)).collect();
    let popup_html: Vec<Html> = slots.children.into_iter().map(|s| s.render(ctx, rcv)).collect();
    let arrow_html = slots.arrow.map(|s| s.render(ctx, rcv));

    // Click toggles (popover); hover triggers use the hidden hover actions.
    let toggle = {
        let open = open.clone();
        let set_open = set_open.clone();
        ctx.callback(move |_| set_open.set(!open.get()))
    };
    let dismiss = {
        let set_open = set_open.clone();
        ctx.callback(move |_| set_open.set(false))
    };
    let hover_open = {
        let set_open = set_open.clone();
        ctx.callback(move |_| set_open.set(true))
    };
    let hover_close = {
        let set_open = set_open.clone();
        ctx.callback(move |_| set_open.set(false))
    };

    let trigger_id_attr: Cow<'static, str> = Cow::Owned(trigger_id.clone());
    let anchor_attr: Cow<'static, str> = Cow::Owned(trigger_id.clone());
    let anchor_attr_popup: Cow<'static, str> = Cow::Owned(trigger_id);
    let popup_id_attr: Cow<'static, str> = Cow::Owned(popup_id.clone());
    let controls_attr: Option<Cow<'static, str>> =
        (!describe).then(|| Cow::Owned(popup_id.clone()));
    let describedby_attr: Option<Cow<'static, str>> =
        describe.then(|| Cow::Owned(popup_id.clone()));
    let side_attr: Cow<'static, str> = Cow::Borrowed(config.side.as_str());
    let align_attr: Cow<'static, str> = Cow::Borrowed(config.align.as_str());
    let offset_attr: Cow<'static, str> = Cow::Owned(alloc::format!("{}", config.offset));

    let hover_delay: Option<Cow<'static, str>> =
        config.hover.map(|h| Cow::Owned(alloc::format!("{}", h.delay)));
    let hover_close_delay: Option<Cow<'static, str>> =
        config.hover.map(|h| Cow::Owned(alloc::format!("{}", h.close_delay)));
    let is_hover = config.hover.is_some();

    // Reactive attrs (one getter clone each).
    let t_expanded = open.clone();
    let t_popup_open = open.clone();
    let t_pressed = open.clone();
    let p_open = open.clone();
    let p_closed = open.clone();

    html! { ctx, rcv,
        <div class="popover-root">
            <button type="button" class="popover-trigger"
                    id=[trigger_id_attr]
                    aria-haspopup=[(!describe).then_some("dialog")]
                    aria-expanded={(!describe).then(|| t_expanded.get())}
                    aria-controls=[controls_attr]
                    aria-describedby=[describedby_attr]
                    data-hover-delay=[hover_delay]
                    data-hover-close-delay=[hover_close_delay]
                    data-popup-open={t_popup_open.get().then_some("")}
                    data-pressed={t_pressed.get().then_some("")}
                    primal:onclick={toggle}>
                <Fragment>{trigger_html.clone()}</Fragment>
                <Fragment>{is_hover.then(hover_behavior)}</Fragment>
            </button>
            <button type="button" hidden="" data-hover-open="true" primal:onclick={hover_open} />
            <button type="button" hidden="" data-hover-close="true" primal:onclick={hover_close} />
            <div class="popover-positioner"
                 data-anchor=[anchor_attr]
                 data-prefer-side=[side_attr]
                 data-prefer-align=[align_attr]
                 data-offset=[offset_attr]>
                <div class=[class] id=[popup_id_attr]
                     role=[config.role]
                     data-anchor=[anchor_attr_popup]
                     data-hover-popup="true"
                     data-dismiss="true"
                     data-scroll-lock=[modal.then_some("true")]
                     data-focus-trap=[modal.then_some("true")]
                     data-open={p_open.get().then_some("")}
                     data-closed={(!p_closed.get()).then_some("")}>
                    <Fragment>{popup_html.clone()}</Fragment>
                    <Fragment>{arrow_html.clone().map(|a| html! { ctx, rcv,
                        <span class="popover-arrow" aria-hidden="true"><Fragment>{a.clone()}</Fragment></span>
                    })}</Fragment>
                    <button type="button" hidden="" data-dismiss-action="true" primal:onclick={dismiss} />
                    <Fragment>{transition_behavior()}</Fragment>
                    <Fragment>{dismiss_behavior()}</Fragment>
                    <Fragment>{modal.then(scroll_lock_behavior)}</Fragment>
                    <Fragment>{modal.then(focus_trap_behavior)}</Fragment>
                </div>
                <Fragment>{position_behavior()}</Fragment>
            </div>
        </div>
    }
}

/// Tooltip — a hover/focus popover, `role="tooltip"`, `aria-describedby` link,
/// never modal. Defaults: `delay = 600`, `closeDelay = 0` (machinery.md §M3).
#[must_use]
pub fn tooltip(
    ctx: &Context,
    rcv: &SharedInstructionReceiver,
    open: &SignalGetter<bool>,
    set_open: SignalSetter<bool>,
    slots: PopoverSlots,
) -> Html {
    popover(
        ctx, rcv,
        PopoverConfig {
            side: PlacementSide::Top,
            role: Cow::Borrowed("tooltip"),
            describe: true,
            hover: Some(HoverConfig { delay: 600, close_delay: 0 }),
            class: Some(Cow::Borrowed("tooltip")),
            ..PopoverConfig::default()
        },
        open, set_open, slots,
    )
}

/// Preview-card — a hover popover for link previews. Defaults:
/// `delay = 600`, `closeDelay = 300`; not modal, no focus management.
#[must_use]
pub fn preview_card(
    ctx: &Context,
    rcv: &SharedInstructionReceiver,
    open: &SignalGetter<bool>,
    set_open: SignalSetter<bool>,
    slots: PopoverSlots,
) -> Html {
    popover(
        ctx, rcv,
        PopoverConfig {
            hover: Some(HoverConfig { delay: 600, close_delay: 300 }),
            class: Some(Cow::Borrowed("preview-card")),
            ..PopoverConfig::default()
        },
        open, set_open, slots,
    )
}
