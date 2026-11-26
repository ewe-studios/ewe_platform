//! `Info`, the state rules and the environment.

use foundation_sandbox::shared::error::{SandboxError, SlaveErrorCode};
use foundation_sandbox::shared::spec::RootMode;

use super::support::{
    drain, exec_request, hex, plain_prepare, sandbox_error, slave_code, SlaveProcess,
};

/// The slave says who it is.
#[test]
fn info() {
    let slave = SlaveProcess::start("info", None, 4);
    let mut prepare = plain_prepare("/tmp");
    prepare.env = vec![("A".to_string(), "1".to_string())];
    let answer = slave.prepare(&prepare);
    assert_eq!(answer.root_mode, RootMode::Overlay);
    let info = slave.caller.info().expect("Info answers");
    assert_ne!(info.slave_version, "");
    assert!(info.arch == "x86_64" || info.arch == "aarch64");
    assert_ne!(info.kernel, "");
    assert!(!info.is_pid1);
    assert!(info.prepared);
    assert!(!info.can_control);
}

/// The error of a call that must fail.
///
/// # Panics
/// When the call succeeds.
fn expect_refusal<T>(
    result: Result<T, foundation_errstacks::ErrorTrace<foundation_connectrpc::ConnectError>>,
) -> foundation_errstacks::ErrorTrace<foundation_connectrpc::ConnectError> {
    match result {
        Ok(_) => panic!("the call must fail"),
        Err(error) => error,
    }
}

/// Work before `Prepare` fails with `NotPrepared`.
#[test]
fn not_prepared() {
    let slave = SlaveProcess::start("not-prepared", None, 4);
    let caller = &slave.caller;
    let input = foundation_sandbox::shared::rpc::messages::ExecInputRequest {
        exec_id: hex(1),
        seq: 0,
        data: Vec::new(),
        eof: true,
    };
    let signal = foundation_sandbox::shared::rpc::messages::ExecSignalRequest {
        exec_id: hex(1),
        signal: 9,
    };
    let cases = vec![
        match caller.exec_error(exec_request(&hex(1), &["/bin/echo"])) {
            SandboxError::SlaveError { code, errno, .. } => Some((code.to_string(), errno)),
            _ => None,
        },
        slave_code(&expect_refusal(caller.exec_input(input))),
        slave_code(&expect_refusal(caller.exec_signal(signal))),
        slave_code(&expect_refusal(caller.file_write_begin("/tmp/x", 0o644))),
        slave_code(&expect_refusal(caller.file_write_chunk(
            &hex(2),
            0,
            vec![1],
        ))),
        slave_code(&expect_refusal(caller.file_write_commit(&hex(2), 1))),
        slave_code(&expect_refusal(caller.file_write_abort(&hex(2)))),
        slave_code(&expect_refusal(caller.file_read("/etc/hostname"))),
    ];
    for case in cases {
        let (code, _) = case.expect("a slave error");
        assert_eq!(code, SlaveErrorCode::NotPrepared.to_string());
    }
    // `Info` and `Shutdown` work before `Prepare`.
    assert!(caller.info().is_ok());
    assert!(caller.shutdown(0).is_ok());
}

/// `Prepare` is safe to repeat.
#[test]
fn prepare_idempotent() {
    let slave = SlaveProcess::start("prepare-idempotent", None, 4);
    let request = plain_prepare("/tmp");
    let first = slave.caller.prepare(&request).expect("the first Prepare");
    let second = slave.caller.prepare(&request).expect("the second Prepare");
    assert_eq!(first, second);

    let mut other = request.clone();
    other.workdir = "/var/tmp".to_string();
    let error = slave
        .caller
        .prepare(&other)
        .expect_err("another request fails");
    match sandbox_error(&error) {
        foundation_sandbox::shared::error::SandboxError::SlaveError { code, .. } => {
            assert_eq!(code, SlaveErrorCode::AlreadyExists);
        }
        other => panic!("not a slave error: {other:?}"),
    }
    // The first answer is still the answer.
    assert_eq!(
        slave.caller.prepare(&request).expect("the first answer"),
        first
    );
}

/// The environment of a program has no entry of the slave process.
#[test]
fn env_is_explicit() {
    let slave = SlaveProcess::start("env", None, 4);
    slave.prepare(&plain_prepare("/tmp"));
    let stream = slave
        .caller
        .exec(exec_request(&hex(3), &["env"]))
        .expect("the program starts");
    let events = drain(stream);
    let stdout = super::support::stdout_of(&events);
    let text = String::from_utf8_lossy(&stdout);
    assert!(
        !text.contains("SLAVE_SECRET"),
        "the slave environment reached the program: {text}"
    );
    assert!(text.contains("PATH="), "the slave adds a PATH");
}
