//! # Button (F1 — Primitives)
//!
//! Source: base-ui `types.md` (spec-42 feature 05 §F1).
//!
//! Single part, renders `<button type="button">`. Native `<button>` gives
//! Enter/Space activation, focusability, form semantics for free — no JS
//! keyboard code. `focusable_when_disabled` renders `aria-disabled` without
//! the `disabled` attribute so the button stays in tab order.
//!
//! Data attributes: `data-disabled`, `data-loading` (ours, additive).

use alloc::vec;
use alloc::vec::Vec;

use foundation_signals::{Context, SignalGetter};
use foundation_ui_traits::{AttrName, DomOp, Html, HtmlTag};
use foundation_wasm_ui::{MaybeCallback, SharedInstructionReceiver, Slot};

/// Static config for a button.
pub struct ButtonConfig {
    /// Whether the button is disabled.
    pub disabled: bool,
    /// Stay focusable when disabled (for tooltip/AT discoverability).
    pub focusable_when_disabled: bool,
    /// Custom class for the button element.
    pub class: Option<&'static str>,
    /// `aria-label` override.
    pub aria_label: Option<&'static str>,
    /// Optional `aria-describedby` for tooltip linking.
    pub aria_describedby: Option<&'static str>,
}

impl Default for ButtonConfig {
    fn default() -> Self {
        Self {
            disabled: false,
            focusable_when_disabled: false,
            class: None,
            aria_label: None,
            aria_describedby: None,
        }
    }
}

/// Slots for the button component.
pub struct ButtonSlots {
    /// Button content (label, icon, etc.).
    pub children: Vec<Slot>,
}

impl ButtonSlots {
    pub fn single(slot: impl Into<Slot>) -> Self {
        Self {
            children: vec![slot.into()],
        }
    }
}

/// Button component — renders `<button type="button">`.
///
/// Static: `disabled`, `focusable_when_disabled`, `class`, `aria_*`.
/// Optional signal: `loading` → adds `data-loading` and `aria-busy`.
#[must_use]
pub fn button(
    ctx: &Context,
    rcv: &SharedInstructionReceiver,
    config: ButtonConfig,
    slots: ButtonSlots,
    loading: Option<SignalGetter<bool>>,
) -> Html {
    let id = ctx.allocate_id_block(1);
    let class = config.class.unwrap_or("button");
    let is_disabled = config.disabled || config.focusable_when_disabled;
    let aria_label = config.aria_label.unwrap_or("");
    let aria_describedby = config.aria_describedby.unwrap_or("");

    // Loading effect: data-loading + aria-busy
    if let Some(loading) = loading {
        let rcv = rcv.clone();
        let loading = loading.clone();
        ctx.effect(move || {
            let is_loading = loading.get();
            if is_loading {
                rcv.queue(DomOp::SetAttribute {
                    node_id: id,
                    name: AttrName::from_static("data-loading"),
                    value: "".into(),
                });
                rcv.queue(DomOp::SetAttribute {
                    node_id: id,
                    name: AttrName::from_static("aria-busy"),
                    value: "true".into(),
                });
            } else {
                rcv.queue(DomOp::RemoveAttribute {
                    node_id: id,
                    name: AttrName::from_static("data-loading"),
                });
                rcv.queue(DomOp::SetAttribute {
                    node_id: id,
                    name: AttrName::from_static("aria-busy"),
                    value: "false".into(),
                });
            }
        });
    }

    // Build the button element
    let mut button_html = Html {
        tag: Some(HtmlTag::from_static("button")),
        text: None,
        children: Vec::new(),
        attributes: vec![
            (AttrName::from_static("primal-id"), alloc::string::ToString::to_string(&id).into()),
            (AttrName::from_static("type"), "button".into()),
            (AttrName::from_static("class"), class.into()),
            (AttrName::from_static("aria-disabled"), alloc::string::ToString::to_string(&is_disabled).into()),
            (AttrName::from_static("aria-label"), aria_label.into()),
            (AttrName::from_static("aria-describedby"), aria_describedby.into()),
            (AttrName::from_static("data-disabled"), alloc::string::ToString::to_string(&is_disabled).into()),
        ],
        parts: Vec::new(),
        runtime_id: None,
    };

    if config.disabled {
        button_html.attributes.push((
            AttrName::from_static("disabled"),
            "true".into(),
        ));
    }

    // Render slot children and append them
    let _child_ids: Vec<u32> = Vec::new();
    for slot in slots.children {
        let child = slot.render(ctx, rcv);
        button_html.children.push(child);
    }

    // Queue DOM ops
    rcv.queue(DomOp::CreateElement {
        node_id: id,
        tag: HtmlTag::from_static("button"),
        class: class.into(),
    });
    rcv.queue(DomOp::RegisterNode { node_id: id });
    for (name, value) in &button_html.attributes {
        if name.name() != Some("primal-id") {
            rcv.queue(DomOp::SetAttribute {
                node_id: id,
                name: name.clone(),
                value: value.clone(),
            });
        }
    }

    button_html
}

/// Convenience: a button that also wires a click handler.
///
/// The handler can be a `SignalSetter<()>` (two-way binding via G21) or
/// any `Fn(&EventData)`.
#[must_use]
pub fn button_with_click<H>(
    ctx: &Context,
    rcv: &SharedInstructionReceiver,
    config: ButtonConfig,
    slots: ButtonSlots,
    loading: Option<SignalGetter<bool>>,
    on_click: H,
) -> Html
where
    H: MaybeCallback + 'static,
{
    let id = ctx.allocate_id_block(1);
    let class = config.class.unwrap_or("button");
    let is_disabled = config.disabled || config.focusable_when_disabled;
    let aria_label = config.aria_label.unwrap_or("");

    // Loading effect
    if let Some(loading) = loading {
        let rcv = rcv.clone();
        let loading = loading.clone();
        ctx.effect(move || {
            let is_loading = loading.get();
            if is_loading {
                rcv.queue(DomOp::SetAttribute {
                    node_id: id,
                    name: AttrName::from_static("data-loading"),
                    value: "".into(),
                });
                rcv.queue(DomOp::SetAttribute {
                    node_id: id,
                    name: AttrName::from_static("aria-busy"),
                    value: "true".into(),
                });
            } else {
                rcv.queue(DomOp::RemoveAttribute {
                    node_id: id,
                    name: AttrName::from_static("data-loading"),
                });
                rcv.queue(DomOp::SetAttribute {
                    node_id: id,
                    name: AttrName::from_static("aria-busy"),
                    value: "false".into(),
                });
            }
        });
    }

    // Build button HTML manually
    let mut button_html = Html {
        tag: Some(HtmlTag::from_static("button")),
        text: None,
        children: Vec::new(),
        attributes: vec![
            (AttrName::from_static("primal-id"), alloc::string::ToString::to_string(&id).into()),
            (AttrName::from_static("type"), "button".into()),
            (AttrName::from_static("class"), class.into()),
            (AttrName::from_static("aria-disabled"), alloc::string::ToString::to_string(&is_disabled).into()),
            (AttrName::from_static("aria-label"), aria_label.into()),
            (AttrName::from_static("data-disabled"), alloc::string::ToString::to_string(&is_disabled).into()),
            (AttrName::from_static("primal:onclick"), "true".into()),
        ],
        parts: Vec::new(),
        runtime_id: None,
    };

    if config.disabled {
        button_html.attributes.push((
            AttrName::from_static("disabled"),
            "true".into(),
        ));
    }

    // Event handler wiring
    let callback_id = on_click.maybe_callback_id();
    if let Some(cb_id) = callback_id {
        button_html.attributes.push((
            AttrName::from_static("primal:setter"),
            alloc::string::ToString::to_string(&cb_id).into(),
        ));
    }

    // Render slot children
    for slot in slots.children {
        let child = slot.render(ctx, rcv);
        button_html.children.push(child);
    }

    // Queue DOM ops
    rcv.queue(DomOp::CreateElement {
        node_id: id,
        tag: HtmlTag::from_static("button"),
        class: class.into(),
    });
    rcv.queue(DomOp::RegisterNode { node_id: id });
    for (name, value) in &button_html.attributes {
        if name.name() != Some("primal-id") && name.name() != Some("primal:onclick") && name.name() != Some("primal:setter") {
            rcv.queue(DomOp::SetAttribute {
                node_id: id,
                name: name.clone(),
                value: value.clone(),
            });
        }
    }

    button_html
}
