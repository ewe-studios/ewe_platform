//! I7 and the exit-status invariant.

use foundation_context::Ctx;
use foundation_platform_sandbox::shared::command::ExitStatus;
use foundation_platform_sandbox::shared::error::SandboxError;
use foundation_platform_sandbox::shared::host::SandboxHost;

use super::common;

/// I7: a missing side-effect bundle gives `HostNotConfigured`, with no
/// fall-back to the real system.
#[test]
fn host_not_configured() {
    let ctx = Ctx::fixed(common::clock());
    let Err(error) = SandboxHost::from_ctx(&ctx) else {
        panic!("the context holds no bundle");
    };
    assert!(matches!(
        error.current_context(),
        SandboxError::HostNotConfigured
    ));
}

/// Exactly one of the exit code and the signal is present.
#[test]
fn exit_status_one_of() {
    let exited = ExitStatus::code(0);
    assert_eq!(exited.exit_code(), Some(0));
    assert_eq!(exited.terminating_signal(), None);

    let signalled = ExitStatus::signal(9);
    assert_eq!(signalled.exit_code(), None);
    assert_eq!(signalled.terminating_signal(), Some(9));

    let json = serde_json::to_string(&exited).expect("encode");
    let decoded: ExitStatus = serde_json::from_str(&json).expect("decode");
    assert_eq!(decoded, exited);

    assert!(serde_json::from_str::<ExitStatus>(r#"{"code":0,"signal":9}"#).is_err());
    assert!(serde_json::from_str::<ExitStatus>(r#"{"code":null,"signal":null}"#).is_err());
}
