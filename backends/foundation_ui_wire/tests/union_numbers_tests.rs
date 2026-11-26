use foundation_ui_wire::{
    ColumnData, Control, FrameKind, MobileOperation, PropValue, UiOperation, WebOperation,
};

macro_rules! assert_numbers {
    ($union:ident; $($variant:ident),+ $(,)?) => {{
        let variants = [$($union::$variant),+];
        assert_eq!($union::ENUM_VALUES, variants);
        for (number, variant) in variants.iter().enumerate() {
            assert_eq!(usize::from(variant.0), number, "{variant:?}");
        }
    }};
}

#[test]
fn union_numbers_are_frozen() {
    assert_numbers!(UiOperation;
        NONE, CreateNode, SetText, SetProp, AppendChild, InsertBefore, Remove, ReplaceSubtree,
        BindAction, UnbindAction, AttachState, LiveMount, UiBehaviour, Place,
    );
    assert_numbers!(WebOperation;
        NONE, WebElement, WebText, WebSetAttribute, WebRemoveAttribute, WebProperty, WebAddListener,
        WebRemoveListener, WebReplaceNode, WebStyle, WebAddClass, WebRemoveClass, HtmlMorph,
        WebRegisterNode, WebUnregisterNode, ScriptNode, WebBehaviour, WebDetachChild,
    );
    assert_numbers!(MobileOperation; NONE, MountWebView, NativeCommand, MobileBehaviour);
    assert_numbers!(ColumnData; NONE, I32, I64, F64, Bool, Utf8);
    assert_numbers!(PropValue; NONE, Length, ColorToken, FontToken, Alignment, Axis, Bool, Text);
    assert_numbers!(FrameKind; NONE, FrameRequest, FrameMessage);
    assert_eq!(
        [
            Control::VT_CAPS,
            Control::VT_STATE_EXPIRED,
            Control::VT_UPDATE_REQUIRED,
            Control::VT_ERROR,
        ],
        [4, 6, 8, 10],
    );
}
