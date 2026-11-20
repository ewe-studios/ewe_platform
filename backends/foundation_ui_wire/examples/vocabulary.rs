use foundation_ui_traits::AttrName;
use foundation_ui_wire::{
    ops,
    vocab::{check_prop, prop_kind, vocab_version, PropKind},
    Length, LengthUnit, LengthValueT, MountSurface, OperationT, PropId, PropValueT, UiOperationT,
    WebOperationT,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut value = LengthValueT::default();
    value.v = Some(Length::new(8.0, LengthUnit::Point).unpack());
    assert_eq!(prop_kind(PropId::PADDING), Some(PropKind::Length));
    check_prop(PropId::PADDING, &PropValueT::Length(Box::new(value)))?;

    let peer = vocab_version(1).ok_or("Unknown vocabulary version")?;
    assert!(PropId::PADDING.0 <= peer.max_prop_id);

    let shared = ops::set_text(42, "Ready");
    let web_only = ops::set_attribute(42, &AttrName::from_static("class"), "ready");
    assert!(matches!(shared.ui, UiOperationT::SetText(_)));
    assert!(matches!(web_only.web, WebOperationT::WebSetAttribute(_)));
    assert!(supports(MountSurface::Native, &shared));
    assert!(!supports(MountSurface::Native, &web_only));
    assert!(supports(MountSurface::Web, &web_only));
    println!(
        "Version {} accepts padding and distinguishes shared and web forms.",
        peer.version
    );
    Ok(())
}

fn supports(surface: MountSurface, form: &OperationT) -> bool {
    surface == MountSurface::Web || form.ui != UiOperationT::NONE
}
