use core::fmt;

use crate::{PropId, PropValueT};

/// The value type that one property accepts.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PropKind {
    Length,
    ColorToken,
    FontToken,
    Alignment,
    Axis,
    Bool,
    Text,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum VocabError {
    UnknownProp {
        prop: u16,
    },
    PropKindMismatch {
        prop: PropId,
        expected: PropKind,
        found: Option<PropKind>,
    },
}

impl fmt::Display for VocabError {
    fn fmt(&self, output: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnknownProp { prop } => write!(output, "Unknown property number {prop}"),
            Self::PropKindMismatch {
                prop,
                expected,
                found: Some(found),
            } => write!(
                output,
                "Property {prop:?} requires {expected:?}, found {found:?}"
            ),
            Self::PropKindMismatch {
                prop,
                expected,
                found: None,
            } => write!(
                output,
                "Property {prop:?} requires {expected:?}, found no value"
            ),
        }
    }
}

impl core::error::Error for VocabError {}

/// Return the value type for a known property number.
#[must_use]
pub fn prop_kind(prop: PropId) -> Option<PropKind> {
    let kind = match prop {
        PropId::PADDING | PropId::SPACING | PropId::WIDTH | PropId::HEIGHT => PropKind::Length,
        PropId::ALIGN => PropKind::Alignment,
        PropId::COLOR | PropId::BACKGROUND => PropKind::ColorToken,
        PropId::FONT => PropKind::FontToken,
        PropId::ENABLED | PropId::CHECKED | PropId::HIDDEN => PropKind::Bool,
        PropId::A11Y_LABEL
        | PropId::VALUE
        | PropId::SOURCE
        | PropId::PLACEHOLDER
        | PropId::PRESENTATION
        | PropId::UTILITY_CLASSES => PropKind::Text,
        PropId::AXIS => PropKind::Axis,
        _ => return None,
    };
    Some(kind)
}

/// Return the type of a value. An unset value has no type.
#[must_use]
pub fn value_kind(value: &PropValueT) -> Option<PropKind> {
    let kind = match value {
        PropValueT::NONE => return None,
        PropValueT::Length(_) => PropKind::Length,
        PropValueT::ColorToken(_) => PropKind::ColorToken,
        PropValueT::FontToken(_) => PropKind::FontToken,
        PropValueT::Alignment(_) => PropKind::Alignment,
        PropValueT::Axis(_) => PropKind::Axis,
        PropValueT::Bool(_) => PropKind::Bool,
        PropValueT::Text(_) => PropKind::Text,
    };
    Some(kind)
}

/// Check a value against its property type.
///
/// # Errors
///
/// Returns an error for an unknown property or a different value type.
pub fn check_prop(prop: PropId, value: &PropValueT) -> Result<(), VocabError> {
    let expected = prop_kind(prop).ok_or(VocabError::UnknownProp { prop: prop.0 })?;
    let found = value_kind(value);
    if found == Some(expected) {
        Ok(())
    } else {
        Err(VocabError::PropKindMismatch {
            prop,
            expected,
            found,
        })
    }
}
