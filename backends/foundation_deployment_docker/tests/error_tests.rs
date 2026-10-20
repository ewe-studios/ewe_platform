//! Classification of failed requests into [`DockerError`].
//!
//! WHY: Callers decide whether to retry, surface, or report "Docker is not
//! running" from the error kind, so a daemon that answered with an error
//! status must never look unreachable, and a failed send must.
//!
//! WHAT: Pure conversions — `From<ApiError>` and `is_unreachable` — with no
//! network.

#![cfg(feature = "docker")]

use foundation_deployment_docker::generated::shared::ApiError;
use foundation_deployment_docker::DockerError;

fn status(code: u16, body: Option<&str>) -> ApiError {
    ApiError::HttpStatus {
        code,
        headers: foundation_netio::shared::http::SimpleHeaders::new(),
        body: body.map(str::to_string),
    }
}

#[test]
fn a_4xx_is_an_api_error_not_an_unreachable_daemon() {
    // What an older daemon answers a too-new API version with.
    let e = DockerError::from(status(
        400,
        Some(
            r#"{"message":"client version 1.53 is too new. Maximum supported API version is 1.48"}"#,
        ),
    ));
    match &e {
        DockerError::Api { status, message } => {
            assert_eq!(*status, 400);
            assert!(message.contains("too new"), "message: {message}");
        }
        other => panic!("expected Api, got {other:?}"),
    }
    assert!(!e.is_unreachable());
}

#[test]
fn an_error_status_without_a_body_is_still_an_api_error() {
    let e = DockerError::from(status(400, None));
    assert!(matches!(e, DockerError::Api { status: 400, ref message } if message == "(no body)"));
    assert!(!e.is_unreachable());
}

#[test]
fn a_5xx_is_an_api_error() {
    let e = DockerError::from(status(500, Some("plain text")));
    assert!(matches!(e, DockerError::Api { status: 500, ref message } if message == "plain text"));
    assert!(!e.is_unreachable());
}

#[test]
fn a_failed_send_is_a_transport_error_and_unreachable() {
    let e = DockerError::from(ApiError::RequestSendFailed(
        "connection refused".to_string(),
    ));
    assert!(matches!(e, DockerError::Transport(_)));
    assert!(e.is_unreachable());
    assert!(DockerError::Unavailable("no socket".to_string()).is_unreachable());
}

#[test]
fn a_version_mismatch_is_not_unreachable() {
    let e = DockerError::UnsupportedApiVersion {
        client_max: "1.53".into(),
        server_min: "1.55".into(),
        server_max: "1.60".into(),
    };
    assert!(!e.is_unreachable());
    assert!(e.to_string().contains("1.55..=1.60"), "{e}");
}
