use foundation_ui_wire::{
    vocab::{check_prop, prop_kind, value_kind, PropKind, VocabError},
    Length, LengthUnit, LengthValueT, PropId, PropValueT, TextValueT,
};

const PROPS: &[(PropId, PropKind)] = &[
    (PropId::PADDING, PropKind::Length),
    (PropId::SPACING, PropKind::Length),
    (PropId::ALIGN, PropKind::Alignment),
    (PropId::COLOR, PropKind::ColorToken),
    (PropId::FONT, PropKind::FontToken),
    (PropId::ENABLED, PropKind::Bool),
    (PropId::A11Y_LABEL, PropKind::Text),
    (PropId::AXIS, PropKind::Axis),
    (PropId::VALUE, PropKind::Text),
    (PropId::CHECKED, PropKind::Bool),
    (PropId::SOURCE, PropKind::Text),
    (PropId::PLACEHOLDER, PropKind::Text),
    (PropId::BACKGROUND, PropKind::ColorToken),
    (PropId::WIDTH, PropKind::Length),
    (PropId::HEIGHT, PropKind::Length),
    (PropId::HIDDEN, PropKind::Bool),
    (PropId::UTILITY_CLASSES, PropKind::Text),
    (PropId::ID, PropKind::Text),
];

#[test]
fn every_supported_prop_has_a_kind() {
    assert_eq!(prop_kind(PropId::UNKNOWN), None);
    assert_eq!(prop_kind(PropId::PRESENTATION), None);
    assert_eq!(prop_kind(PropId(u16::MAX)), None);
    assert_eq!(
        PropId::ENUM_VALUES
            .iter()
            .copied()
            .filter(|p| *p != PropId::PRESENTATION)
            .collect::<Vec<_>>(),
        [PropId::UNKNOWN]
            .into_iter()
            .chain(PROPS.iter().map(|(prop, _)| *prop))
            .collect::<Vec<_>>(),
    );
    for &(prop, kind) in PROPS {
        assert_eq!(prop_kind(prop), Some(kind));
    }
}

#[test]
fn check_prop_accepts_only_the_table_kind() {
    let padding = PropValueT::Length(Box::new(table!(LengthValueT {
        v: Some(Length::new(8.0, LengthUnit::Point).unpack()),
    })));
    let cases = [
        (None, PropValueT::NONE),
        (Some(PropKind::Length), padding),
        (
            Some(PropKind::ColorToken),
            PropValueT::ColorToken(Box::default()),
        ),
        (
            Some(PropKind::FontToken),
            PropValueT::FontToken(Box::default()),
        ),
        (
            Some(PropKind::Alignment),
            PropValueT::Alignment(Box::default()),
        ),
        (Some(PropKind::Axis), PropValueT::Axis(Box::default())),
        (Some(PropKind::Bool), PropValueT::Bool(Box::default())),
        (
            Some(PropKind::Text),
            PropValueT::Text(Box::new(table!(TextValueT {
                v: Some("8px".into())
            }))),
        ),
    ];
    for (found, value) in &cases {
        assert_eq!(value_kind(value), *found);
        for &(prop, expected) in PROPS {
            let result = if Some(expected) == *found {
                Ok(())
            } else {
                Err(VocabError::PropKindMismatch {
                    prop,
                    expected,
                    found: *found,
                })
            };
            assert_eq!(check_prop(prop, value), result);
        }
        for prop in [PropId::UNKNOWN, PropId(u16::MAX)] {
            assert_eq!(
                check_prop(prop, value),
                Err(VocabError::UnknownProp { prop: prop.0 })
            );
        }
    }
}
