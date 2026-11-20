use crate::{UiOperationT, WebOperationT};

/// A shared or web form that can represent a DOM operation.
#[derive(Clone, Debug, PartialEq)]
pub enum WireForm {
    Ui(UiOperationT),
    Web(WebOperationT),
}
