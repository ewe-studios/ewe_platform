//! # Dialog / alert-dialog / drawer (F4 — Overlays)
//!
//! WHY: A modal surface — native `<dialog>` gives the top layer, focus trap,
//! scroll lock, `::backdrop` and page inertness for free (F4 verdict: "native
//! `<dialog>`, fully"). The headless job is signal⇄native sync + the ARIA and
//! data-attribute contract.
//!
//! WHAT: [`dialog`] (the base), [`alert_dialog`] (always modal, no light
//! dismiss, `role="alertdialog"`), and [`drawer`] (a side-anchored dialog;
//! swipe/snap gestures are the M8 follow-up). Open is one `(open, set_open)`
//! signal.
//!
//! HOW: Renders `<dialog primal:dialog>` driven by [`dialog_behavior`]
//! (`data-open` ⇄ `showModal()`/`close()`, `cancel`/backdrop → `set_open(false)`).
//! Title/Description slots auto-wire `aria-labelledby`/`aria-describedby` to
//! generated ids. A hidden `[data-dialog-close]` element carries the close
//! callback for the Escape/backdrop routes; the optional Close slot button is
//! wired the same way. M7 transitions ride the `data-open` toggle.

use alloc::borrow::Cow;
use alloc::vec::Vec;

use foundation_signals::{Context, SignalGetter, SignalSetter};
use foundation_ui_traits::Html;
use foundation_wasm_ui::{html, SharedInstructionReceiver, Slot};

use crate::machinery::dialog::dialog_behavior;
use crate::machinery::gestures::{snap_behavior, swipe_behavior};
use crate::machinery::transition::transition_behavior;

/// Which edge a drawer anchors to.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum DrawerSide {
    /// Slide in from the right (default).
    #[default]
    Right,
    /// Slide in from the left.
    Left,
    /// Slide in from the top.
    Top,
    /// Slide in from the bottom.
    Bottom,
}

impl DrawerSide {
    fn as_str(self) -> &'static str {
        match self {
            DrawerSide::Right => "right",
            DrawerSide::Left => "left",
            DrawerSide::Top => "top",
            DrawerSide::Bottom => "bottom",
        }
    }
}

/// Static config for a dialog.
pub struct DialogConfig {
    /// Modal (`showModal()` — top layer, scroll lock, inert page) vs non-modal
    /// (`show()`). Default modal.
    pub modal: bool,
    /// Allow backdrop/Escape light dismissal (default true; alert-dialog fixes
    /// this false).
    pub dismissable: bool,
    /// `role="alertdialog"` instead of `dialog`.
    pub alert: bool,
    /// Class override for the dialog element (default: `"dialog"`).
    pub class: Option<Cow<'static, str>>,
    /// `aria-label` when no Title slot is provided.
    pub aria_label: Option<Cow<'static, str>>,
}

impl Default for DialogConfig {
    fn default() -> Self {
        Self { modal: true, dismissable: true, alert: false, class: None, aria_label: None }
    }
}

/// Slots for a dialog.
pub struct DialogSlots {
    /// Title (→ `aria-labelledby`).
    pub title: Option<Slot>,
    /// Description (→ `aria-describedby`).
    pub description: Option<Slot>,
    /// Body content.
    pub children: Vec<Slot>,
    /// Optional close-button content (wired to `set_open(false)`).
    pub close_label: Option<Slot>,
}

impl Default for DialogSlots {
    fn default() -> Self {
        Self { title: None, description: None, children: Vec::new(), close_label: None }
    }
}

/// Dialog component — a native `<dialog>` kept in sync with `open`.
#[must_use]
pub fn dialog(
    ctx: &Context,
    rcv: &SharedInstructionReceiver,
    config: DialogConfig,
    open: &SignalGetter<bool>,
    set_open: SignalSetter<bool>,
    slots: DialogSlots,
) -> Html {
    dialog_impl(ctx, rcv, config, open, set_open, slots, None, None)
}

/// Alert-dialog — always modal, never light-dismissable, `role="alertdialog"`.
#[must_use]
pub fn alert_dialog(
    ctx: &Context,
    rcv: &SharedInstructionReceiver,
    mut config: DialogConfig,
    open: &SignalGetter<bool>,
    set_open: SignalSetter<bool>,
    slots: DialogSlots,
) -> Html {
    config.modal = true;
    config.dismissable = false;
    config.alert = true;
    dialog_impl(ctx, rcv, config, open, set_open, slots, None, None)
}

/// Drawer — a side-anchored modal dialog (swipe/snap deferred to M8). Adds
/// `data-side` for the slide-in stylesheet.
#[must_use]
pub fn drawer(
    ctx: &Context,
    rcv: &SharedInstructionReceiver,
    mut config: DialogConfig,
    side: DrawerSide,
    open: &SignalGetter<bool>,
    set_open: SignalSetter<bool>,
    slots: DialogSlots,
) -> Html {
    if config.class.is_none() {
        config.class = Some(Cow::Borrowed("drawer"));
    }
    dialog_impl(ctx, rcv, config, open, set_open, slots, Some(side), None)
}

/// Drawer with snap points (OUR design — base-ui has no drawer). `snap_points`
/// are fractional-open heights (`1.0` = fully open); dragging rests at the
/// nearest (or sequential) point, and below the smallest dismisses. The resting
/// index flows to `set_snap_point`.
#[must_use]
#[allow(clippy::too_many_arguments)]
pub fn drawer_with_snap(
    ctx: &Context,
    rcv: &SharedInstructionReceiver,
    mut config: DialogConfig,
    side: DrawerSide,
    snap_points: Vec<f64>,
    sequential: bool,
    set_snap_point: SignalSetter<usize>,
    open: &SignalGetter<bool>,
    set_open: SignalSetter<bool>,
    slots: DialogSlots,
) -> Html {
    if config.class.is_none() {
        config.class = Some(Cow::Borrowed("drawer"));
    }
    let on_snap = ctx.callback(move |data| {
        if let Some(n) = data.value.as_deref().and_then(|t| t.parse::<usize>().ok()) {
            set_snap_point.set(n);
        }
    });
    dialog_impl(ctx, rcv, config, open, set_open, slots, Some(side), Some((snap_points, sequential, on_snap)))
}

#[allow(clippy::too_many_lines, clippy::too_many_arguments, clippy::type_complexity)]
fn dialog_impl(
    ctx: &Context,
    rcv: &SharedInstructionReceiver,
    config: DialogConfig,
    open: &SignalGetter<bool>,
    set_open: SignalSetter<bool>,
    slots: DialogSlots,
    drawer_side: Option<DrawerSide>,
    snap: Option<(Vec<f64>, bool, foundation_signals::Callback)>,
) -> Html {
    let class = config
        .class
        .unwrap_or(if config.alert { Cow::Borrowed("alert-dialog") } else { Cow::Borrowed("dialog") });
    let role: Cow<'static, str> =
        if config.alert { Cow::Borrowed("alertdialog") } else { Cow::Borrowed("dialog") };
    let mode: Cow<'static, str> =
        if config.modal { Cow::Borrowed("modal") } else { Cow::Borrowed("nonmodal") };
    let id_n = ctx.allocate_id_block(1);
    let title_id = alloc::format!("dialog-title-{id_n}");
    let desc_id = alloc::format!("dialog-desc-{id_n}");

    let labelledby: Option<Cow<'static, str>> =
        slots.title.as_ref().map(|_| Cow::Owned(title_id.clone()));
    let describedby: Option<Cow<'static, str>> =
        slots.description.as_ref().map(|_| Cow::Owned(desc_id.clone()));

    let title_html = slots.title.map(|s| s.render(ctx, rcv));
    let description_html = slots.description.map(|s| s.render(ctx, rcv));
    let body_html: Vec<Html> = slots.children.into_iter().map(|s| s.render(ctx, rcv)).collect();
    let close_label_html = slots.close_label.map(|s| s.render(ctx, rcv));

    let close = {
        let set_open = set_open.clone();
        ctx.callback(move |_| set_open.set(false))
    };
    let close_hidden = {
        let set_open = set_open.clone();
        ctx.callback(move |_| set_open.set(false))
    };

    let title_id_attr: Cow<'static, str> = Cow::Owned(title_id);
    let desc_id_attr: Cow<'static, str> = Cow::Owned(desc_id);
    let side_attr: Option<Cow<'static, str>> = drawer_side.map(|s| Cow::Borrowed(s.as_str()));
    let dismissable_attr: Option<&'static str> = (!config.dismissable).then_some("false");
    // Drawers swipe toward their anchored edge to dismiss (M8).
    let swipe_dir: Option<Cow<'static, str>> = drawer_side.map(|s| Cow::Borrowed(s.as_str()));
    let swipe_prefix: Option<&'static str> = drawer_side.map(|_| "--drawer");
    let is_drawer = drawer_side.is_some();
    // Snap config (our design): comma-joined fractions + sequential flag.
    let (snap_points, sequential, on_snap) = match snap {
        Some((pts, seq, cb)) => (Some(pts), seq, Some(cb)),
        None => (None, false, None),
    };
    let has_snap = snap_points.is_some();
    let snap_points_attr: Option<Cow<'static, str>> = snap_points.map(|pts| {
        Cow::Owned(
            pts.iter()
                .map(alloc::string::ToString::to_string)
                .collect::<Vec<_>>()
                .join(","),
        )
    });
    let sequential_attr: Option<&'static str> = (has_snap && sequential).then_some("true");

    let d_open = open.clone();
    let d_closed = open.clone();

    html! { ctx, rcv,
        <dialog class=[class]
                role=[role]
                data-dialog-mode=[mode]
                data-side=[side_attr]
                data-dialog-dismissable=[dismissable_attr]
                data-swipe-direction=[swipe_dir]
                data-swipe-prefix=[swipe_prefix]
                data-snap-points=[snap_points_attr]
                data-snap-sequential=[sequential_attr]
                aria-label=[config.aria_label]
                aria-labelledby=[labelledby]
                aria-describedby=[describedby]
                data-open={d_open.get().then_some("")}
                data-closed={(!d_closed.get()).then_some("")}>
            <Fragment>{title_html.clone().map(|t| html! { ctx, rcv,
                <h2 class="dialog-title" id=[title_id_attr.clone()]><Fragment>{t.clone()}</Fragment></h2>
            })}</Fragment>
            <Fragment>{description_html.clone().map(|d| html! { ctx, rcv,
                <p class="dialog-description" id=[desc_id_attr.clone()]><Fragment>{d.clone()}</Fragment></p>
            })}</Fragment>
            <div class="dialog-body"><Fragment>{body_html.clone()}</Fragment></div>
            <Fragment>{close_label_html.clone().map(|c| html! { ctx, rcv,
                <button type="button" class="dialog-close" primal:onclick={close.clone()}>
                    <Fragment>{c.clone()}</Fragment>
                </button>
            })}</Fragment>
            <button type="button" hidden="" data-dialog-close="true" data-swipe-dismiss="true" primal:onclick={close_hidden} />
            <Fragment>{on_snap.map(|cb| html! { ctx, rcv,
                <input type="hidden" data-snap-input="true" primal:onchange={cb} />
            })}</Fragment>
            <Fragment>{transition_behavior()}</Fragment>
            <Fragment>{dialog_behavior()}</Fragment>
            // Snap drawers use the snap gesture; plain drawers use swipe-dismiss.
            <Fragment>{if has_snap { Some(snap_behavior()) } else if is_drawer { Some(swipe_behavior()) } else { None }}</Fragment>
        </dialog>
    }
}
