//! Lower a template to wire operations and collect its state attachment targets.

use crate::action::{ActionError, Origin};
use foundation_errstacks::ErrorTrace;
use foundation_ui_wire::ErrorCode;
use std::fmt;

pub use foundation_ui_traits as template_types;
pub use foundation_ui_wire::{vocab::TokenTable, PlacementKind};

macro_rules! table {
    ($name:ident { $($field:ident : $value:expr),* $(,)? }) => {{
        let mut table = $name::default();
        $(table.$field = $value;)*
        table
    }};
}

mod lower;
mod properties;
mod render;

pub use lower::lower;
pub use properties::{core_kind, property};
pub use render::{full_view, mount, place, render, NodeIdAllocator, Target, ViewIds};

/// Assigns a stable node identifier to a region targeted by later actions.
/// Use an identifier above two, outside other live regions in this mount.
#[must_use]
pub fn anchor(id: u32, mut tree: template_types::Html) -> template_types::Html {
    tree.attributes
        .push(("primal:anchor".into(), id.to_string().into()));
    tree
}

/// The root offset and shared token names for a pure template.
#[derive(Debug, Clone, Copy)]
pub struct LowerOptions<'a> {
    pub id_base: u32,
    pub tokens: &'a TokenTable,
}

/// A template that cannot be represented as wire operations.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LowerError {
    MissingId,
    BadProp {
        node_id: u32,
        prop: String,
        value: String,
    },
    UnknownToken {
        node_id: u32,
        name: String,
    },
    NoRootElement,
    NoComponentRoot,
    IdOverflow,
    MissingState,
    ReservedAttribute,
    BadTarget,
}

impl fmt::Display for LowerError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingId => f.write_str("view element has no valid template identifier"),
            Self::BadProp {
                node_id,
                prop,
                value,
            } => write!(f, "node {node_id} has invalid {prop} value {value:?}"),
            Self::UnknownToken { node_id, name } => {
                write!(f, "node {node_id} uses unknown token {name:?}")
            }
            Self::NoRootElement => f.write_str("view needs one root element"),
            Self::NoComponentRoot => f.write_str("view has no component root"),
            Self::IdOverflow => f.write_str("view exhausted its node identifiers"),
            Self::MissingState => f.write_str("a marked element requires component state"),
            Self::ReservedAttribute => f.write_str("the runtime owns the state attribute"),
            Self::BadTarget => f.write_str("invalid placement target or content"),
        }
    }
}

impl std::error::Error for LowerError {}

impl From<LowerError> for ErrorTrace<ActionError> {
    fn from(error: LowerError) -> Self {
        let message = error.to_string();
        ErrorTrace::new(error).change_context(ActionError::new(ErrorCode::ActionFailed, message))
    }
}

/// Returns the action reference stored by a typed event binding.
#[must_use]
pub fn action_reference(origin: Option<Origin>, name: &str) -> String {
    let prefix = if origin == Some(Origin::Server) {
        "server"
    } else {
        "host"
    };
    format!("{prefix}:{name}")
}
