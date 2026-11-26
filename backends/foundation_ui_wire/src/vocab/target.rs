//! Portable names for view roots and shared elements.
use crate::{NodeKind, SelectorKind};

/// Names start with a letter or underscore. Remaining characters can also include digits and hyphens.
#[must_use]
pub fn valid_name(name: &str) -> bool {
    let mut bytes = name.bytes();
    bytes
        .next()
        .is_some_and(|b| b.is_ascii_alphabetic() || b == b'_')
        && bytes.all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-')
}

/// Returns the wire selector kind and its text. The caller handles browser selectors separately.
#[must_use]
pub fn portable_selector(value: &str) -> Option<(SelectorKind, &str)> {
    let (tag, name) = value.split_once('#')?;
    if !valid_name(name) {
        return None;
    }
    match tag {
        "" => Some((SelectorKind::Id, name)),
        "view" => Some((SelectorKind::View, name)),
        _ if valid_name(tag) => Some((SelectorKind::TypedId, value)),
        _ => None,
    }
}

/// Find the shared element kind. Separators and letter case do not change a tag.
#[must_use]
pub fn portable_kind(tag: &str) -> Option<NodeKind> {
    if tag.eq_ignore_ascii_case("sheet") {
        return Some(NodeKind::BOTTOM_SHEET);
    }
    NodeKind::ENUM_VALUES.iter().copied().find(|kind| {
        *kind != NodeKind::UNKNOWN
            && kind.variant_name().is_some_and(|name| {
                name.bytes()
                    .filter(|c| *c != b'_')
                    .map(|c| c.to_ascii_lowercase())
                    .eq(tag
                        .bytes()
                        .filter(|c| *c != b'_' && *c != b'-')
                        .map(|c| c.to_ascii_lowercase()))
            })
    })
}
