//! Construct platform commands as generated wire operations.
use foundation_ui_wire::{
    MobileOperationT, NativeArgT, NativeCommandId, NativeCommandT, OperationT, PropValueT,
    TextValueT,
};

fn command(kind: NativeCommandId, argument: Option<(&str, &str)>) -> OperationT {
    let mut cmd = NativeCommandT::default();
    cmd.command = kind;
    cmd.args = argument.map(|(name, text)| {
        let mut value = TextValueT::default();
        value.v = Some(text.to_owned());
        let mut arg = NativeArgT::default();
        arg.name = Some(name.to_owned());
        arg.value = PropValueT::Text(Box::new(value));
        vec![arg]
    });
    let mut op = OperationT::default();
    op.mobile = MobileOperationT::NativeCommand(Box::new(cmd));
    op
}
#[must_use]
pub fn haptic() -> OperationT {
    command(NativeCommandId::HAPTIC, None)
}
#[must_use]
pub fn open_url(url: &str) -> OperationT {
    command(NativeCommandId::OPEN_URL, Some(("url", url)))
}
#[must_use]
pub fn share(text: &str) -> OperationT {
    command(NativeCommandId::SHARE, Some(("text", text)))
}
