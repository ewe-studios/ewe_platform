//! # Toggle (F1 — Primitives)
//!
//! A two-state button. Renders `<button aria-pressed>`.
//!
//! Data attributes: `data-pressed`, `data-disabled`.

use alloc::string::ToString;
use alloc::vec;
use alloc::vec::Vec;

use foundation_signals::{Context, SignalGetter, SignalSetter};
use foundation_ui_traits::{AttrName, DomOp, Html, HtmlTag};
use foundation_wasm_ui::{SharedInstructionReceiver, Slot};

/// Static config for a toggle.
pub struct ToggleConfig {
    /// Identifies this toggle inside a toggle-group.
    pub value: Option<&'static str>,
    /// Whether the toggle is disabled.
    pub disabled: bool,
    /// Optional custom class.
    pub class: Option<&'static str>,
    /// `aria-label` override.
    pub aria_label: Option<&'static str>,
}

impl Default for ToggleConfig {
    fn default() -> Self {
        Self {
            value: None,
            disabled: false,
            class: None,
            aria_label: None,
        }
    }
}

/// Toggle component — a two-state button.
///
/// Signal: `pressed` (bool), `set_pressed` toggled on click/Enter/Space.
/// Static: `disabled`, `value`, `class`, `aria_label`.
#[must_use]
pub fn toggle(
    ctx: &Context,
    rcv: &SharedInstructionReceiver,
    config: ToggleConfig,
    pressed: &SignalGetter<bool>,
    set_pressed: SignalSetter<bool>,
    slots: ToggleSlots,
) -> Html {
    let id = ctx.allocate_id_block(1);
    let class = config.class.unwrap_or("toggle");
    let aria_label = config.aria_label.unwrap_or("");
    let callback_id = set_pressed.callback_id();

    // Effect: aria-pressed + data-pressed tracking
    {
        let rcv = rcv.clone();
        let pressed = pressed.clone();
        ctx.effect(move || {
            let is_pressed = pressed.get();
            rcv.queue(DomOp::SetAttribute {
                node_id: id,
                name: AttrName::from_static("aria-pressed"),
                value: if is_pressed { "true" } else { "false" }.into(),
            });
            if is_pressed {
                rcv.queue(DomOp::SetAttribute {
                    node_id: id,
                    name: AttrName::from_static("data-pressed"),
                    value: "".into(),
                });
            } else {
                rcv.queue(DomOp::RemoveAttribute {
                    node_id: id,
                    name: AttrName::from_static("data-pressed"),
                });
            }
        });
    }

    // Build initial markup
    let mut toggle_html = Html {
        tag: Some(HtmlTag::from_static("button")),
        text: None,
        children: Vec::new(),
        attributes: vec![
            (AttrName::from_static("primal-id"), id.to_string().into()),
            (AttrName::from_static("type"), "button".into()),
            (AttrName::from_static("class"), class.into()),
            (AttrName::from_static("role"), "button".into()),
            (AttrName::from_static("aria-label"), aria_label.into()),
            (AttrName::from_static("data-disabled"), alloc::string::ToString::to_string(&config.disabled).into()),
            (AttrName::from_static("primal:onclick"), "true".into()),
            (AttrName::from_static("primal:setter"), callback_id.to_string().into()),
        ],
        parts: Vec::new(),
        runtime_id: None,
    };

    if config.disabled {
        toggle_html.attributes.push((
            AttrName::from_static("disabled"),
            "true".into(),
        ));
    }

    // Render slot children
    for slot in slots.children {
        let child = slot.render(ctx, rcv);
        toggle_html.children.push(child);
    }

    // Queue DOM ops
    rcv.queue(DomOp::CreateElement {
        node_id: id,
        tag: HtmlTag::from_static("button"),
        class: class.into(),
    });
    rcv.queue(DomOp::RegisterNode { node_id: id });
    for (name, value) in &toggle_html.attributes {
        if name.name() != Some("primal-id") && name.name() != Some("primal:onclick") && name.name() != Some("primal:setter") {
            rcv.queue(DomOp::SetAttribute {
                node_id: id,
                name: name.clone(),
                value: value.clone(),
            });
        }
    }

    toggle_html
}

/// Slots for the toggle.
pub struct ToggleSlots {
    pub children: Vec<Slot>,
}
