//! Project captured operations onto the reference vocabulary for the original browser fixtures.

use foundation_ui_traits::DomOp;
use foundation_ui_wire::{
    vocab::{dom_op, WireForm},
    OperationT,
};

pub fn reference(op: &OperationT) -> DomOp {
    dom_op(&WireForm::try_from(op.clone()).expect("fixture uses one web or shared operation"))
        .expect("fixture covers a reference browser operation")
}
