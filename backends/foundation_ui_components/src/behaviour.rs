//! Named capabilities used by the component library.
use foundation_ui_wire::{vocab::shared_behaviour, BehaviourId};

/// A behaviour has a shared form or a named web form.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[repr(u16)]
pub enum Behaviour {
    Composite = 1,
    Dialog,
    Dismiss,
    FocusTrap,
    Gestures,
    Hover,
    Listbox,
    Measure,
    Position,
    ScrollLock,
    Transition,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BehaviourWire {
    Shared(BehaviourId),
    Web(&'static str),
}
impl Behaviour {
    pub const ALL: &'static [Self] = &[
        Self::Composite,
        Self::Dialog,
        Self::Dismiss,
        Self::FocusTrap,
        Self::Gestures,
        Self::Hover,
        Self::Listbox,
        Self::Measure,
        Self::Position,
        Self::ScrollLock,
        Self::Transition,
    ];
    #[must_use]
    pub fn name(self) -> &'static str {
        match self {
            Self::Composite => "composite",
            Self::Dialog => "dialog",
            Self::Dismiss => "dismiss",
            Self::FocusTrap => "focus_trap",
            Self::Gestures => "gestures",
            Self::Hover => "hover",
            Self::Listbox => "listbox",
            Self::Measure => "measure",
            Self::Position => "position",
            Self::ScrollLock => "scroll_lock",
            Self::Transition => "transition",
        }
    }
    #[must_use]
    pub fn from_name(name: &str) -> Option<Self> {
        Self::ALL.iter().copied().find(|value| value.name() == name)
    }
    #[must_use]
    pub fn wire(self) -> BehaviourWire {
        shared_behaviour(self.name()).map_or(BehaviourWire::Web(self.name()), BehaviourWire::Shared)
    }
}
/// Component modules and the behaviours used in their source.
pub static COMPONENT_BEHAVIOURS: &[(&str, &[Behaviour])] = &[
    ("accordion", crate::accordion::BEHAVIOURS),
    ("collapsible", crate::collapsible::BEHAVIOURS),
    ("dialog", crate::dialog::BEHAVIOURS),
    ("menu", crate::menu::BEHAVIOURS),
    ("nav_menu", crate::nav_menu::BEHAVIOURS),
    ("pickers", crate::pickers::BEHAVIOURS),
    ("popover", crate::popover::BEHAVIOURS),
    ("radio", crate::radio::BEHAVIOURS),
    ("slider", crate::slider::BEHAVIOURS),
    ("tabs", crate::tabs::BEHAVIOURS),
    ("toast", crate::toast::BEHAVIOURS),
    ("toggle_group", crate::toggle_group::BEHAVIOURS),
    ("toolbar", crate::toolbar::BEHAVIOURS),
];

/// Build a marker for a named behaviour. The browser supplies the implementation.
#[must_use]
pub fn behaviour_script(
    behaviour: Behaviour,
    _body: impl Into<alloc::borrow::Cow<'static, str>>,
) -> foundation_ui_traits::Html {
    marker(behaviour, None)
}

pub(crate) fn marker(
    behaviour: Behaviour,
    mode: Option<&'static str>,
) -> foundation_ui_traits::Html {
    use foundation_ui_traits::{AttrName, Html, HtmlTag};
    let mut attributes = alloc::vec![(
        AttrName::from_static("data-ewe-behaviour"),
        behaviour.name().into()
    )];
    if let Some(mode) = mode {
        attributes.push((AttrName::from_static("data-ewe-mode"), mode.into()));
    }
    Html {
        tag: Some(HtmlTag::from_static("span")),
        attributes,
        ..Html::default()
    }
}
