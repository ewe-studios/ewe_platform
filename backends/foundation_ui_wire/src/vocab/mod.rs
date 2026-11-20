//! Property, platform and version tables for the wire vocabulary.

mod behaviour;
mod dom_op;
mod forms;
mod props;
mod tokens;
mod version;
mod web;

pub use behaviour::{
    is_web_behaviour, shared_behaviour, SHARED_BEHAVIOUR_NAMES, WEB_BEHAVIOUR_NAMES,
};
pub use dom_op::{dom_op, web_form};
pub use forms::WireForm;
pub use props::{check_prop, prop_kind, value_kind, PropKind, VocabError};
pub use tokens::TokenTable;
pub use version::{vocab_version, VocabVersion, VOCABULARY_VERSION, VOCAB_VERSIONS};
pub use web::{vocab_js, WebKind, CORE_CSS, WEB_EVENTS, WEB_KINDS};
