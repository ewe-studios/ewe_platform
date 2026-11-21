use foundation_errstacks::ErrorTrace;
use foundation_ui_wire::{ActionErrorT, ErrorCode};

#[test]
fn action_error_is_an_error() {
    let mut context = table!(ActionErrorT {
        code: ErrorCode::ActionFailed,
        message: Some("failed to save".into()),
        row_index: 0,
    });
    assert_eq!(context.to_string(), "ActionFailed: failed to save");
    assert!(ErrorTrace::new(context.clone())
        .to_string()
        .contains("failed to save"));
    context.message = None;
    assert_eq!(context.to_string(), "ActionFailed");
}
