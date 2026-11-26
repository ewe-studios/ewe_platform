use crate::BehaviourId;

/// Names and identifiers of the behaviours that all platforms share.
pub const SHARED_BEHAVIOUR_NAMES: &[(&str, BehaviourId)] = &[
    ("dialog", BehaviourId::DIALOG),
    ("dismiss", BehaviourId::DISMISS),
    ("scroll_lock", BehaviourId::SCROLL_LOCK),
    ("transition", BehaviourId::TRANSITION),
];

/// Names of the behaviours that require a web mount.
pub const WEB_BEHAVIOUR_NAMES: &[&str] = &[
    "composite",
    "focus_trap",
    "gestures",
    "hover",
    "listbox",
    "measure",
    "position",
];

/// Return the identifier for a shared behaviour name.
#[must_use]
pub fn shared_behaviour(name: &str) -> Option<BehaviourId> {
    SHARED_BEHAVIOUR_NAMES
        .iter()
        .find(|(entry, _)| *entry == name)
        .map(|(_, id)| *id)
}

/// Return whether a name is in the web behaviour table.
#[must_use]
pub fn is_web_behaviour(name: &str) -> bool {
    WEB_BEHAVIOUR_NAMES.contains(&name)
}
