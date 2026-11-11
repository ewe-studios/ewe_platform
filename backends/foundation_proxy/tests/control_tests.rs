//! Control-socket wire format tests (moved from `src/control.rs`).

use foundation_proxy::control::{Request, Response};

#[test]
fn request_parse_valid() {
    let req: Request =
        serde_json::from_str(r#"{"command":"status","params":{}}"#).expect("valid request");
    assert_eq!(req.command, "status");
}

#[test]
fn request_parse_invalid_returns_err() {
    let err = serde_json::from_str::<Request>("not json").unwrap_err();
    assert!(!err.to_string().is_empty(), "should produce error message");
}

#[test]
fn response_ok_format() {
    let resp = Response {
        ok: true,
        error: None,
        data: Some(serde_json::json!({"count": 3})),
    };
    let json = serde_json::to_string(&resp).expect("serialize");
    assert!(json.contains("\"ok\":true"));
    assert!(json.contains("\"data\""));
}

#[test]
fn response_error_format() {
    let resp = Response {
        ok: false,
        error: Some("bad command".into()),
        data: None,
    };
    let json = serde_json::to_string(&resp).expect("serialize");
    assert!(json.contains("\"ok\":false"));
    assert!(json.contains("bad command"));
    assert!(!json.contains("\"data\""));
}
