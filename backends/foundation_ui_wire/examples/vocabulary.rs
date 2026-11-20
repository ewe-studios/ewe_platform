use foundation_ui_traits::{AttrName, DomOp};
use foundation_ui_wire::{
    vocab::{check_prop, prop_kind, vocab_version, web_form, PropKind, WireForm},
    Length, LengthUnit, LengthValueT, MountSurface, PropId, PropValueT, UiOperationT,
    WebOperationT,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut value = LengthValueT::default();
    value.v = Some(Length::new(8.0, LengthUnit::Point).unpack());
    assert_eq!(prop_kind(PropId::PADDING), Some(PropKind::Length));
    check_prop(PropId::PADDING, &PropValueT::Length(Box::new(value)))?;

    let peer = vocab_version(1).ok_or("Unknown vocabulary version")?;
    assert!(PropId::PADDING.0 <= peer.max_prop_id);

    let shared = web_form(&DomOp::SetText {
        node_id: 42,
        text: "Ready".into(),
    });
    let web_only = web_form(&DomOp::SetAttribute {
        node_id: 42,
        name: AttrName::from_static("class"),
        value: "ready".into(),
    });
    assert!(matches!(shared, WireForm::Ui(UiOperationT::SetText(_))));
    assert!(matches!(
        web_only,
        WireForm::Web(WebOperationT::WebSetAttribute(_))
    ));
    assert!(supports(MountSurface::Native, &shared));
    assert!(!supports(MountSurface::Native, &web_only));
    assert!(supports(MountSurface::Web, &web_only));
    println!(
        "Version {} accepts padding and distinguishes shared and web forms.",
        peer.version
    );
    Ok(())
}

fn supports(surface: MountSurface, form: &WireForm) -> bool {
    surface == MountSurface::Web || matches!(form, WireForm::Ui(_))
}
