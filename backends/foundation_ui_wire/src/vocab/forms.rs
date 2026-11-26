use crate::{UiOperationT, WebOperationT};

/// A shared or web form that can represent a DOM operation.
#[derive(Clone, Debug, PartialEq)]
pub enum WireForm {
    Ui(UiOperationT),
    Web(WebOperationT),
}

impl From<WireForm> for crate::OperationT {
    fn from(form: WireForm) -> Self {
        let mut operation = Self::default();
        match form {
            WireForm::Ui(ui) => operation.ui = ui,
            WireForm::Web(web) => operation.web = web,
        }
        operation
    }
}

impl TryFrom<crate::OperationT> for WireForm {
    type Error = ();
    fn try_from(op: crate::OperationT) -> Result<Self, Self::Error> {
        if op.columnar.is_some() || op.mobile != crate::MobileOperationT::NONE {
            return Err(());
        }
        match (op.ui, op.web) {
            (UiOperationT::NONE, WebOperationT::NONE) => Err(()),
            (UiOperationT::NONE, web) => Ok(Self::Web(web)),
            (ui, WebOperationT::NONE) => Ok(Self::Ui(ui)),
            _ => Err(()),
        }
    }
}
