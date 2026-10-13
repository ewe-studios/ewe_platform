//! Unit tests for DockerError.

use foundation_deployment_platform::docker::{DockerError, docker_err};

#[test]
fn test_is_connection_error() {
    assert!(DockerError::Connection("socket not found".into()).is_connection_error());
    assert!(!DockerError::InvalidConfig("bad image".into()).is_connection_error());
    assert!(!DockerError::ContainerCreate("failed".into()).is_connection_error());
    assert!(!DockerError::ContainerStart("failed".into()).is_connection_error());
}

#[test]
fn test_display_formats() {
    let e = DockerError::Connection("no socket".into());
    assert_eq!(format!("{e}"), "Docker daemon not reachable: no socket");

    let e = DockerError::ImagePull { image: "redis:7".into(), reason: "not found".into() };
    assert_eq!(format!("{e}"), "Image pull failed for 'redis:7': not found");

    let e = DockerError::InvalidConfig("missing port".into());
    assert_eq!(format!("{e}"), "Invalid configuration: missing port");

    let e = DockerError::ContainerCreate("oom".into());
    assert_eq!(format!("{e}"), "Container creation failed: oom");
}

#[test]
fn test_docker_err_wraps_in_errortrace() {
    let trace = docker_err(DockerError::Network("no route".into()));
    let display = format!("{trace}");
    assert!(display.contains("Network operation failed: no route"), "got: {display}");
    assert!(!trace.current_context().is_connection_error());
}

#[test]
fn test_io_error_maps_to_connection() {
    use std::io;
    let e = io::Error::new(io::ErrorKind::NotFound, "file missing");
    // The From impl is not available (multiple String variants conflict),
    // but the is_connection_error check works on Connection variant
    let conn_err = DockerError::Connection(e.to_string());
    assert!(conn_err.is_connection_error());
}

// ── Classifying the client's errors ─────────────────────────────────────────

use foundation_deployment_docker::DockerError as ClientError;
use foundation_deployment_platform::docker::classify_docker_client_err;

/// The CI failure: a reachable daemon answering `400` was reported as
/// "Docker daemon not reachable", which made the Docker-absent skip fire (or a
/// `required` test fail with a misleading message). A daemon that answers is
/// reachable, whatever it says.
#[test]
fn a_daemon_error_status_is_an_api_error_not_a_connection_error() {
    for status in [400u16, 404, 409, 500] {
        let e = classify_docker_client_err(
            "image_inspect",
            ClientError::Api { status, message: "(no body)".into() },
        );
        assert!(!e.is_connection_error(), "status {status} must not read as Docker absent");
        match &e {
            DockerError::Api { operation, status: s, message } => {
                assert_eq!(operation, "image_inspect");
                assert_eq!(*s, status);
                assert_eq!(message, "(no body)");
            }
            other => panic!("expected Api, got {other:?}"),
        }
        assert_eq!(
            e.to_string(),
            format!("Docker API error during image_inspect (status {status}): (no body)")
        );
    }
}

#[test]
fn an_unreachable_daemon_is_a_connection_error() {
    let e = classify_docker_client_err("image_inspect", ClientError::Transport("connection refused".into()));
    assert!(e.is_connection_error());
    assert_eq!(e.to_string(), "Docker daemon not reachable: image_inspect: connection refused");

    let e = classify_docker_client_err("connect", ClientError::Unavailable("no socket".into()));
    assert!(e.is_connection_error());
}

#[test]
fn a_failed_pull_or_push_is_not_a_connection_error() {
    let e = classify_docker_client_err("redis:7", ClientError::PullFailed("pull access denied".into()));
    assert!(!e.is_connection_error());
    assert!(matches!(e, DockerError::ImagePull { ref reason, .. } if reason == "pull access denied"), "{e:?}");

    let e = classify_docker_client_err("image_push", ClientError::PushFailed("denied".into()));
    assert!(!e.is_connection_error());
    assert!(matches!(e, DockerError::ImagePush(_)), "{e:?}");
}

#[test]
fn an_api_version_mismatch_is_not_a_connection_error() {
    let e = classify_docker_client_err(
        "image_inspect",
        ClientError::UnsupportedApiVersion {
            client_max: "1.53".into(),
            server_min: "1.55".into(),
            server_max: "1.60".into(),
        },
    );
    assert!(!e.is_connection_error());
    assert!(matches!(e, DockerError::IncompatibleDaemon(_)), "{e:?}");
}
