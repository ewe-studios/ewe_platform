use super::{LowerError, TokenTable};
use foundation_ui_wire::vocab::{prop_kind, PropKind};
use foundation_ui_wire::{
    Alignment, AlignmentValueT, Axis, AxisValueT, BoolValueT, ColorTokenT, FontTokenT, LengthT,
    LengthUnit, LengthValueT, NodeKind, PropId, PropValueT, SetPropT, TextValueT,
};

/// Finds the shared control for a template tag.
#[must_use]
pub fn core_kind(tag: &str) -> Option<NodeKind> {
    Some(match tag {
        "Stack" => NodeKind::STACK,
        "Text" => NodeKind::TEXT,
        "Image" => NodeKind::IMAGE,
        "Button" => NodeKind::BUTTON,
        "TextInput" => NodeKind::TEXT_INPUT,
        "Toggle" => NodeKind::TOGGLE,
        "List" => NodeKind::LIST,
        "Scroll" => NodeKind::SCROLL,
        "Spacer" => NodeKind::SPACER,
        "Overlay" => NodeKind::OVERLAY,
        "Modal" => NodeKind::MODAL,
        "BottomSheet" => NodeKind::BOTTOM_SHEET,
        _ => return None,
    })
}

/// Converts a template attribute to its typed control property.
///
/// # Errors
/// Returns the invalid property, value, or token name.
pub fn property(
    id: u32,
    name: &str,
    value: &str,
    tokens: &TokenTable,
) -> Result<SetPropT, LowerError> {
    let bad = || LowerError::BadProp {
        node_id: id,
        prop: name.into(),
        value: value.into(),
    };
    let unknown = || LowerError::UnknownToken {
        node_id: id,
        name: value.into(),
    };
    let wire_name = if name == "class" { "utility_classes" } else { name };
    let prop = PropId::ENUM_VALUES
        .iter()
        .copied()
        .find(|p| {
            p.variant_name()
                .is_some_and(|n| n.eq_ignore_ascii_case(wire_name))
        })
        .ok_or_else(bad)?;
    let v = match prop_kind(prop).ok_or_else(bad)? {
        PropKind::Text => PropValueT::Text(Box::new(table!(TextValueT {
            v: Some(value.to_owned())
        }))),
        PropKind::Bool => PropValueT::Bool(Box::new(table!(BoolValueT {
            v: value.parse().map_err(|_| bad())?
        }))),
        PropKind::ColorToken => PropValueT::ColorToken(Box::new(table!(ColorTokenT {
            id: tokens.color_id(value).ok_or_else(unknown)?
        }))),
        PropKind::FontToken => PropValueT::FontToken(Box::new(table!(FontTokenT {
            id: tokens.font_id(value).ok_or_else(unknown)?
        }))),
        PropKind::Axis => {
            let axis = match value {
                "vertical" => Axis::Vertical,
                "horizontal" => Axis::Horizontal,
                _ => return Err(bad()),
            };
            PropValueT::Axis(Box::new(table!(AxisValueT { v: axis })))
        }
        PropKind::Alignment => {
            let align = match value {
                "start" => Alignment::Start,
                "end" => Alignment::End,
                "center" => Alignment::Center,
                "stretch" => Alignment::Stretch,
                _ => return Err(bad()),
            };
            PropValueT::Alignment(Box::new(table!(AlignmentValueT { v: align })))
        }
        PropKind::Length => {
            let (number, unit) = if value == "auto" {
                ("0", LengthUnit::Auto)
            } else if let Some(percent) = value.strip_suffix('%') {
                (percent, LengthUnit::Percent)
            } else {
                (value, LengthUnit::Point)
            };
            let number = number
                .parse::<f32>()
                .ok()
                .filter(|v| v.is_finite())
                .ok_or_else(bad)?;
            let len = table!(LengthT {
                value: number,
                unit: unit
            });
            PropValueT::Length(Box::new(table!(LengthValueT { v: Some(len) })))
        }
    };
    Ok(table!(SetPropT {
        node_id: id,
        prop: prop,
        value: v
    }))
}
