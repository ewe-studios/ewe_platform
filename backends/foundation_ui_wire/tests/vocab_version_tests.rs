use foundation_ui_traits::{ATTR_NAMES, TAG_NAMES};
use foundation_ui_wire::{
    vocab::{vocab_version, VocabVersion, VOCABULARY_VERSION, VOCAB_VERSIONS},
    BehaviourId, EventId, MobileBehaviourId, NativeCommandId, NodeKind, PropId, WebBehaviourId,
};

#[test]
fn last_row_matches_the_enums() {
    let current = VocabVersion {
        version: VOCABULARY_VERSION,
        max_node_kind: NodeKind::ENUM_MAX,
        max_prop_id: PropId::ENUM_MAX,
        max_event_id: EventId::ENUM_MAX,
        max_tag_id: u16::try_from(TAG_NAMES.len()).unwrap(),
        max_attr_id: u16::try_from(ATTR_NAMES.len()).unwrap(),
        max_native_command: NativeCommandId::ENUM_MAX,
        max_behaviour: BehaviourId::ENUM_MAX,
        max_web_behaviour: WebBehaviourId::ENUM_MAX,
        max_mobile_behaviour: MobileBehaviourId::ENUM_MAX,
    };
    assert_eq!(VOCAB_VERSIONS.last(), Some(&current));
    assert_eq!(vocab_version(VOCABULARY_VERSION), Some(&current));
}

#[test]
fn unknown_version_has_no_row() {
    assert_eq!(vocab_version(0), None);
    assert_eq!(vocab_version(99), None);
}

#[test]
fn rows_never_decrease() {
    assert_eq!(
        vocab_version(1),
        Some(&VocabVersion {
            version: 1,
            max_node_kind: 10,
            max_prop_id: 16,
            max_event_id: 5,
            max_tag_id: 114,
            max_attr_id: 81,
            max_native_command: 3,
            max_behaviour: 0,
            max_web_behaviour: 0,
            max_mobile_behaviour: 0,
        }),
    );
    for rows in VOCAB_VERSIONS.windows(2) {
        let previous = &rows[0];
        let next = &rows[1];
        assert!(previous.version < next.version);
        assert!(previous.max_node_kind <= next.max_node_kind);
        assert!(previous.max_prop_id <= next.max_prop_id);
        assert!(previous.max_event_id <= next.max_event_id);
        assert!(previous.max_tag_id <= next.max_tag_id);
        assert!(previous.max_attr_id <= next.max_attr_id);
        assert!(previous.max_native_command <= next.max_native_command);
        assert!(previous.max_behaviour <= next.max_behaviour);
        assert!(previous.max_web_behaviour <= next.max_web_behaviour);
        assert!(previous.max_mobile_behaviour <= next.max_mobile_behaviour);
    }
}
