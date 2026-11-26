//! Property, platform and version tables for the wire vocabulary.

mod behaviour;
mod forms;
mod props;
mod target;
mod tokens;
mod version;
mod web;

pub use behaviour::{
    is_web_behaviour, shared_behaviour, SHARED_BEHAVIOUR_NAMES, WEB_BEHAVIOUR_NAMES,
};
pub use forms::WireForm;
pub use props::{check_prop, prop_kind, value_kind, PropKind, VocabError};
pub use target::{portable_kind, portable_selector, valid_name};
pub use tokens::TokenTable;
pub use version::{
    vocab_version, VocabVersion, MANAGED_CONNECTION_VERSION, PORTABLE_TARGET_VERSION,
    STRUCTURED_INPUT_VERSION, VOCABULARY_VERSION, VOCAB_VERSIONS,
};
pub use web::{vocab_js, WebKind, CORE_CSS, WEB_EVENTS, WEB_KINDS};
