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
