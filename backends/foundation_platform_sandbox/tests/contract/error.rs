//! I5: every error variant keeps its type and its Connect code over RPC.

use foundation_connectrpc::{Code, ConnectError};
use foundation_platform_sandbox::shared::driver::{Capabilities, HostCheck, HostReport};
use foundation_platform_sandbox::shared::error::{SandboxError, SlaveErrorCode};
use foundation_platform_sandbox::shared::ids::DriverId;
use foundation_platform_sandbox::shared::platform::PlatformKind;
use proptest::prelude::*;

use super::common;

/// A host report for `NoDriver`.
fn host_report() -> HostReport {
    HostReport {
        driver: DriverId::Firecracker,
        usable: false,
        checks: vec![HostCheck {
            name: "kvm".to_string(),
            ok: false,
            message: "missing".to_string(),
        }],
        capabilities: Capabilities::default(),
    }
}

/// Builds one value of every error variant.
#[allow(
    clippy::too_many_lines,
    reason = "one place holds one value of every error variant"
)]
fn every_error(text: &str, number: u64, data: &[u8]) -> Vec<SandboxError> {
    vec![
        SandboxError::HostNotConfigured,
        SandboxError::NoDriver {
            reports: vec![host_report()],
        },
        SandboxError::DriverUnavailable {
            driver: DriverId::Libkrun,
            reason: text.to_string(),
        },
        SandboxError::BinaryNotFound {
            name: text.to_string(),
        },
        SandboxError::LibraryNotFound {
            name: text.to_string(),
        },
        SandboxError::LibraryTooOld {
            symbol: text.to_string(),
        },
        SandboxError::NotSigned,
        SandboxError::InvalidSpec {
            field: text.to_string(),
            reason: text.to_string(),
        },
        SandboxError::UnsupportedRootFs,
        SandboxError::UnsupportedNetwork,
        SandboxError::UnsupportedMount,
        SandboxError::UnsupportedCapability {
            capability: text.to_string(),
        },
        SandboxError::SocketPathTooLong {
            path: text.to_string(),
            max: 103,
        },
        SandboxError::SubstrateExited {
            code: Some(1),
            log_tail: data.to_vec(),
        },
        SandboxError::SubstrateApi {
            status: 500,
            message: text.to_string(),
        },
        SandboxError::BootTimeout {
            stage: text.to_string(),
            log_tail: data.to_vec(),
        },
        SandboxError::SlaveHandshake,
        SandboxError::NotFound {
            what: text.to_string(),
        },
        SandboxError::SandboxStopped,
        SandboxError::ExecTimeout,
        SandboxError::SlaveError {
            code: SlaveErrorCode::BadRequest,
            errno: Some(2),
            message: text.to_string(),
        },
        SandboxError::SequenceGap,
        SandboxError::InputQueueFull,
        SandboxError::StreamLost,
        SandboxError::TooManySessions,
        SandboxError::ArtifactHashMismatch {
            expected: text.to_string(),
            actual: text.to_string(),
        },
        SandboxError::ArtifactUnsafePath {
            path: text.to_string(),
        },
        SandboxError::ArtifactTooLarge {
            limit: number,
            message: text.to_string(),
        },
        SandboxError::ImageArchMismatch {
            image: text.to_string(),
            host: text.to_string(),
        },
        SandboxError::BuildStepFailed {
            line: 1,
            exit: Some(1),
            signal: None,
        },
        SandboxError::Rpc {
            code: "not_found".to_string(),
            message: text.to_string(),
        },
        SandboxError::Unauthorized,
        SandboxError::Io {
            op: text.to_string(),
            message: text.to_string(),
        },
        SandboxError::PlatformMismatch {
            expected: PlatformKind::Native,
            actual: PlatformKind::Docker,
        },
    ]
}

proptest! {
    #![proptest_config(common::config(0x5eed_0002))]

    /// I5 and the "an error keeps its type across RPC" scenario.
    #[test]
    fn error_wire_roundtrip(
        text in "[a-z]{0,16}",
        number in any::<u64>(),
        data in proptest::collection::vec(any::<u8>(), 0..16),
    ) {
        for error in every_error(&text, number, &data) {
            let wire = error.to_connect(1024 * 1024);
            prop_assert_eq!(wire.code(), error.code());
            prop_assert_eq!(SandboxError::from_connect(&wire), error);
        }
    }
}

/// The code table: each variant maps to exactly one Connect code.
#[allow(clippy::too_many_lines, reason = "the table lists one row per variant")]
#[test]
fn error_code_table() {
    let text = "text";
    let errors = every_error(text, 1, &[1, 2, 3]);
    for error in &errors {
        let expected = match error {
            SandboxError::InvalidSpec { .. }
            | SandboxError::UnsupportedRootFs
            | SandboxError::UnsupportedNetwork
            | SandboxError::UnsupportedMount
            | SandboxError::SocketPathTooLong { .. } => Code::InvalidArgument,
            SandboxError::NotFound { .. } => Code::NotFound,
            SandboxError::SandboxStopped
            | SandboxError::HostNotConfigured
            | SandboxError::NoDriver { .. }
            | SandboxError::DriverUnavailable { .. }
            | SandboxError::BinaryNotFound { .. }
            | SandboxError::LibraryNotFound { .. }
            | SandboxError::LibraryTooOld { .. }
            | SandboxError::NotSigned
            | SandboxError::UnsupportedCapability { .. }
            | SandboxError::PlatformMismatch { .. } => Code::FailedPrecondition,
            SandboxError::BootTimeout { .. } | SandboxError::ExecTimeout => Code::DeadlineExceeded,
            SandboxError::TooManySessions | SandboxError::InputQueueFull => Code::ResourceExhausted,
            SandboxError::SequenceGap => Code::OutOfRange,
            SandboxError::StreamLost => Code::Aborted,
            SandboxError::Unauthorized => Code::Unauthenticated,
            SandboxError::SubstrateExited { .. }
            | SandboxError::SubstrateApi { .. }
            | SandboxError::SlaveHandshake
            | SandboxError::SlaveError { .. }
            | SandboxError::ArtifactHashMismatch { .. }
            | SandboxError::ArtifactUnsafePath { .. }
            | SandboxError::ArtifactTooLarge { .. }
            | SandboxError::ImageArchMismatch { .. }
            | SandboxError::BuildStepFailed { .. }
            | SandboxError::Io { .. } => Code::Internal,
            SandboxError::Rpc { code, .. } => Code::from_str(code).unwrap_or(Code::Unknown),
        };
        assert_eq!(error.code(), expected, "wrong code for {error:?}");
    }

    // The slave code decides between two Connect codes.
    assert_eq!(
        SandboxError::SlaveError {
            code: SlaveErrorCode::NotFound,
            errno: Some(2),
            message: "missing".to_string(),
        }
        .code(),
        Code::NotFound
    );
    assert_eq!(
        SandboxError::SlaveError {
            code: SlaveErrorCode::PermissionDenied,
            errno: Some(13),
            message: "denied".to_string(),
        }
        .code(),
        Code::PermissionDenied
    );

    // An unknown remote code is `Unknown`.
    assert_eq!(
        SandboxError::Rpc {
            code: "nonsense".to_string(),
            message: "text".to_string(),
        }
        .code(),
        Code::Unknown
    );
}

/// A Connect error with no detail is still a typed error.
#[test]
fn error_without_detail() {
    let wire = ConnectError::internal("boom");
    assert_eq!(
        SandboxError::from_connect(&wire),
        SandboxError::Rpc {
            code: "internal".to_string(),
            message: "boom".to_string(),
        }
    );
}

/// Text fields are cut to the maximum in `to_connect`.
#[test]
fn error_text_is_bounded() {
    let long = "a".repeat(100);
    let error = SandboxError::InvalidSpec {
        field: long.clone(),
        reason: long.clone(),
    };
    let wire = error.to_connect(16);
    let decoded = SandboxError::from_connect(&wire);
    match decoded {
        SandboxError::InvalidSpec { field, reason } => {
            assert_eq!(field.len(), 16);
            assert_eq!(reason.len(), 16);
        }
        other => panic!("expected InvalidSpec, got {other:?}"),
    }
}
