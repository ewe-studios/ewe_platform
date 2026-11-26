use foundation_ui_wire::{
    vocab::{is_web_behaviour, shared_behaviour, SHARED_BEHAVIOUR_NAMES, WEB_BEHAVIOUR_NAMES},
    BehaviourId, MobileBehaviourId, WebBehaviourId,
};

#[test]
fn each_behaviour_name_is_in_one_set() {
    assert_eq!(
        SHARED_BEHAVIOUR_NAMES,
        [
            ("dialog", BehaviourId::DIALOG),
            ("dismiss", BehaviourId::DISMISS),
            ("scroll_lock", BehaviourId::SCROLL_LOCK),
            ("transition", BehaviourId::TRANSITION),
        ],
    );
    assert_eq!(
        WEB_BEHAVIOUR_NAMES,
        [
            "composite",
            "focus_trap",
            "gestures",
            "hover",
            "listbox",
            "measure",
            "position"
        ],
    );
    assert_eq!(WebBehaviourId::ENUM_VALUES, [WebBehaviourId::UNKNOWN]);
    assert_eq!(MobileBehaviourId::ENUM_VALUES, [MobileBehaviourId::UNKNOWN]);
    assert_eq!(
        SHARED_BEHAVIOUR_NAMES
            .iter()
            .map(|(_, behaviour)| *behaviour)
            .collect::<Vec<_>>(),
        &BehaviourId::ENUM_VALUES[1..],
    );
    for &(name, behaviour) in SHARED_BEHAVIOUR_NAMES {
        assert_eq!(shared_behaviour(name), Some(behaviour));
        assert!(!is_web_behaviour(name));
        assert!(name.chars().all(|c| c.is_ascii_lowercase() || c == '_'));
    }
    for &name in WEB_BEHAVIOUR_NAMES {
        assert_eq!(shared_behaviour(name), None);
        assert!(is_web_behaviour(name));
        assert!(name.chars().all(|c| c.is_ascii_lowercase() || c == '_'));
    }
    assert_eq!(shared_behaviour("unknown"), None);
    assert!(!is_web_behaviour("unknown"));
}
