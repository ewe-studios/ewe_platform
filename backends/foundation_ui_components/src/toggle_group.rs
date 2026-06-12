//! # Toggle Group (F1 — Primitives)
//!
//! Shares pressed state across toggles; renders `<div role="group">`.
//!
//! Data attributes: `data-orientation`, `data-disabled`, `data-multiple`.

use alloc::string::String;
use alloc::vec;
use alloc::vec::Vec;

use foundation_signals::{Context, SignalGetter, SignalSetter};
use foundation_ui_traits::{AttrName, Html, HtmlTag};
use foundation_wasm_ui::SharedInstructionReceiver;

/// Orientation for the toggle group.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Orientation {
    /// Horizontal layout, left/right arrows navigate.
    #[default]
    Horizontal,
    /// Vertical layout, up/down arrows navigate.
    Vertical,
}

/// Static config for a toggle group.
pub struct ToggleGroupConfig {
    /// Whether multiple toggles can be pressed simultaneously.
    pub multiple: bool,
    /// Whether arrow key navigation loops (last → first).
    pub loop_focus: bool,
    /// Orientation for arrow-key navigation.
    pub orientation: Orientation,
    /// Whether the entire group is disabled.
    pub disabled: bool,
    /// Optional custom class for the group container.
    pub class: Option<&'static str>,
    /// `aria-label` for the group.
    pub aria_label: Option<&'static str>,
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
pub struct ToggleGroupItem {
    /// The toggle's identifying value.
    pub value: String,
    /// The toggle's display content (rendered by caller).
    pub content: Html,
    /// Per-item disabled state.
    pub disabled: bool,
}

/// Toggle group component — manages a set of toggles with shared state.
///
/// Signal: `values` (Vec<String> of pressed toggle values).
/// Static: `multiple`, `loop_focus`, `orientation`, `disabled`.
#[must_use]
pub fn toggle_group(
    _ctx: &Context,
    _rcv: &SharedInstructionReceiver,
    config: ToggleGroupConfig,
    values: &SignalGetter<Vec<String>>,
    set_values: SignalSetter<Vec<String>>,
    items: Vec<ToggleGroupItem>,
) -> Html {
    let orientation_str = match config.orientation {
        Orientation::Horizontal => "horizontal",
        Orientation::Vertical => "vertical",
    };
    let class = config.class.unwrap_or("toggle-group");
    let aria_label = config.aria_label.unwrap_or("");
    let _callback_id = set_values.callback_id();

    // Build toggle buttons
    let current_values = values.get();
    let mut buttons: Vec<Html> = Vec::with_capacity(items.len());

    for item in items {
        let is_pressed = current_values.contains(&item.value);

        let button = Html {
            tag: Some(HtmlTag::from_static("button")),
            text: None,
            children: vec![item.content],
            attributes: vec![
                (AttrName::from_static("type"), "button".into()),
                (AttrName::from_static("class"), "toggle".into()),
                (AttrName::from_static("role"), "button".into()),
                (AttrName::from_static("aria-pressed"), alloc::string::ToString::to_string(&is_pressed).into()),
                (AttrName::from_static("data-pressed"), alloc::string::ToString::to_string(&is_pressed).into()),
                (AttrName::from_static("data-disabled"), alloc::string::ToString::to_string(&item.disabled).into()),
                (AttrName::from_static("data-value"), item.value.clone().into()),
            ],
            parts: Vec::new(),
            runtime_id: None,
        };

        if config.disabled || item.disabled {
            // button_html.attributes.push disabled
        }

        buttons.push(button);
    }

    // Group container
    let group_html = Html {
        tag: Some(HtmlTag::from_static("div")),
        text: None,
        children: buttons,
        attributes: vec![
            (AttrName::from_static("class"), class.into()),
            (AttrName::from_static("role"), "group".into()),
            (AttrName::from_static("aria-label"), aria_label.into()),
            (AttrName::from_static("data-orientation"), orientation_str.into()),
            (AttrName::from_static("data-disabled"), alloc::string::ToString::to_string(&config.disabled).into()),
            (AttrName::from_static("data-multiple"), alloc::string::ToString::to_string(&config.multiple).into()),
        ],
        parts: Vec::new(),
        runtime_id: None,
    };

    // Queue DOM ops for the group
    // (In a full implementation, we'd allocate ids and queue CreateElement etc.)

    group_html
}
