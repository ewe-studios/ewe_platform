/// The version of the vocabulary in a new frame.
pub const VOCABULARY_VERSION: u32 = 3;

/// The highest vocabulary numbers that one version supports.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct VocabVersion {
    pub version: u32,
    pub max_node_kind: u16,
    pub max_prop_id: u16,
    pub max_event_id: u16,
    pub max_tag_id: u16,
    pub max_attr_id: u16,
    pub max_native_command: u16,
    pub max_behaviour: u16,
    pub max_web_behaviour: u16,
    pub max_mobile_behaviour: u16,
}

/// Released rows stay fixed. Append a row when the vocabulary grows.
pub static VOCAB_VERSIONS: &[VocabVersion] = &[
    VocabVersion {
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
    },
    VocabVersion {
        version: 2,
        max_node_kind: 10,
        max_prop_id: 16,
        max_event_id: 5,
        max_tag_id: 114,
        max_attr_id: 81,
        max_native_command: 3,
        max_behaviour: 4,
        max_web_behaviour: 0,
        max_mobile_behaviour: 0,
    },
    VocabVersion {
        version: VOCABULARY_VERSION,
        max_node_kind: 10,
        max_prop_id: 16,
        max_event_id: 5,
        max_tag_id: 114,
        max_attr_id: 81,
        max_native_command: 3,
        max_behaviour: 4,
        max_web_behaviour: 0,
        max_mobile_behaviour: 0,
    },
];

/// Return the row for a known vocabulary version.
#[must_use]
pub fn vocab_version(version: u32) -> Option<&'static VocabVersion> {
    VOCAB_VERSIONS.iter().find(|row| row.version == version)
}
