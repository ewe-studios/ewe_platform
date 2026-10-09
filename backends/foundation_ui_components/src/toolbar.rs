//! # Toolbar (F5 — Menus)
//!
//! WHY: A group of mixed interactive widgets (buttons, toggles, inputs, links)
//! that is a SINGLE tab stop, navigated with arrows — the WAI-ARIA toolbar
//! pattern (spec-42 §F5). No popup; pure M5 composite roving.
//!
//! WHAT: [`toolbar`] renders `<div role="toolbar">` over child slots, marking
//! each child as a composite member and embedding the M5
//! [`composite_behavior`].
//!
//! HOW: Each child slot is rendered, then tagged `data-composite-item` on its
//! root (so callers don't wire ARIA), and the container carries `data-composite`
//! + `data-orientation`/`data-loop`. Tab reaches the toolbar once; arrows rove,
//! Home/End jump.

use alloc::borrow::Cow;
use alloc::vec::Vec;

use foundation_ui_traits::{AttrName, Html};
use foundation_signals::Context;
use foundation_wasm_ui::{html, SharedInstructionReceiver, Slot};

use crate::machinery::composite::composite_behavior;
use crate::toggle_group::Orientation;

/// Static config for a toolbar.
pub struct ToolbarConfig {
    /// Orientation (arrow axis).
    pub orientation: Orientation,
    /// Whether roving loops.
    pub loop_focus: bool,
    /// Disabled.
    pub disabled: bool,
    /// Class override (default `"toolbar"`).
    pub class: Option<Cow<'static, str>>,
    /// `aria-label`.
    pub aria_label: Option<Cow<'static, str>>,
}

impl Default for ToolbarConfig {
    fn default() -> Self {
        Self {
            orientation: Orientation::Horizontal,
            loop_focus: true,
            disabled: false,
            class: None,
            aria_label: None,
        }
    }
}

/// Toolbar component — roving focus over mixed interactive children.
#[must_use]
pub fn toolbar(
    ctx: &Context,
    rcv: &SharedInstructionReceiver,
    config: ToolbarConfig,
    children: Vec<Slot>,
) -> Html {
    let class = config.class.unwrap_or(Cow::Borrowed("toolbar"));
    let orientation = config.orientation.as_str();
    let loop_focus = config.loop_focus;

    // Render children, marking each root as a composite member.
    let items: Vec<Html> = children
        .into_iter()
        .map(|slot| {
            let mut h = slot.render(ctx, rcv);
            h.attributes.push((AttrName::from_static("data-composite-item"), Cow::Borrowed("true")));
            h
        })
        .collect();

    html! { ctx, rcv,
        <div class=[class] role="toolbar"
             aria-label=[config.aria_label]
             aria-orientation={orientation}
             data-orientation={orientation}
             data-disabled={config.disabled.then_some("")}
             data-composite="true"
             data-loop=[(!loop_focus).then_some("false")]>
            <Fragment>{items.clone()}</Fragment>
            <Fragment>{composite_behavior()}</Fragment>
        </div>
    }
}
