//! I9, the socket path bound and the container identifier shape.

use std::path::PathBuf;
use std::str::FromStr;

use foundation_context::Ctx;
use foundation_platform_sandbox::shared::container::ContainerId;
use foundation_platform_sandbox::shared::error::SandboxError;
use foundation_platform_sandbox::shared::ids::{check_socket_path, ExecId, SandboxId, UploadId};

use super::common;

fn all_lower_hex(value: &str) -> bool {
    !value.is_empty()
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || matches!(byte, b'a'..=b'f'))
}

/// I9: two contexts with the same seed give the same identifier sequence.
#[test]
fn ids_are_deterministic() {
    let first = Ctx::with_seed(common::clock(), 42);
    let second = Ctx::with_seed(common::clock(), 42);

    for _ in 0..32 {
        let first_id = SandboxId::generate(&first);
        let second_id = SandboxId::generate(&second);
        assert_eq!(first_id, second_id);
        assert_eq!(first_id.as_str().len(), 12);
        assert!(all_lower_hex(first_id.as_str()));

        let first_exec = ExecId::generate(&first);
        let second_exec = ExecId::generate(&second);
        assert_eq!(first_exec, second_exec);
        assert_eq!(first_exec.as_str().len(), 16);
        assert!(all_lower_hex(first_exec.as_str()));

        let first_upload = UploadId::generate(&first);
        let second_upload = UploadId::generate(&second);
        assert_eq!(first_upload, second_upload);
        assert_eq!(first_upload.as_str().len(), 16);
        assert!(all_lower_hex(first_upload.as_str()));
    }
}

/// An identifier accepts only its exact lower-case hexadecimal shape.
#[test]
fn ids_shape() {
    assert!(SandboxId::from_str("0123456789ab").is_ok());
    assert!(SandboxId::from_str("0123456789AB").is_err());
    assert!(SandboxId::from_str("0123456789a").is_err());
    assert!(SandboxId::from_str("0123456789abc").is_err());
    assert!(SandboxId::from_str("0123456789ag").is_err());

    assert!(ExecId::from_str("0123456789abcdef").is_ok());
    assert!(UploadId::from_str("0123456789abcdef").is_ok());
    assert!(ExecId::from_str("0123456789abcde").is_err());
}

/// The socket path limit: 103 bytes pass, 104 bytes fail.
#[test]
fn socket_path_limit() {
    let accepted = PathBuf::from("a".repeat(103));
    assert!(check_socket_path(&accepted).is_ok());

    let refused = PathBuf::from("a".repeat(104));
    let error = check_socket_path(&refused).expect_err("104 bytes must fail");
    match error.current_context() {
        SandboxError::SocketPathTooLong { path, max } => {
            assert_eq!(path.len(), 104);
            assert_eq!(*max, 103);
        }
        other => panic!("expected SocketPathTooLong, got {other:?}"),
    }
}

/// A container identifier is 1 to 64 characters of `[A-Za-z0-9_.-]`.
#[test]
fn container_id_shape() {
    assert!(ContainerId::new("abc".to_string()).is_ok());
    assert!(ContainerId::new("A.b_c-9".to_string()).is_ok());
    assert!(ContainerId::new("a".repeat(64)).is_ok());

    for refused in [
        String::new(),
        "a".repeat(65),
        "a/b".to_string(),
        "a b".to_string(),
        "a\\b".to_string(),
    ] {
        let error = ContainerId::new(refused).expect_err("the value must fail");
        match error.current_context() {
            SandboxError::InvalidSpec { field, .. } => assert_eq!(field, "container_id"),
            other => panic!("expected InvalidSpec, got {other:?}"),
        }
    }

    assert_eq!(
        ContainerId::from_str("abc")
            .expect("a valid identifier")
            .as_str(),
        "abc"
    );
}
