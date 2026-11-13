//! Feature 67: the public fallible API returns an errstacks trace.

use foundation_sshkit::powershell::ps_exec;
use foundation_sshkit::SshKitError;

#[test]
fn api_error_is_an_errstacks_trace_with_sshkit_context() {
    let session = ssh2::Session::new().expect("create ssh2 session");
    let trace = ps_exec(&session, "Write-Host hello").expect_err("an unconnected session fails");

    assert!(
        matches!(trace.current_context(), SshKitError::Operation(_)),
        "current context must be the crate error variant, got {:?}",
        trace.current_context()
    );
}

#[test]
fn attached_context_survives_a_layer_crossing() {
    let session = ssh2::Session::new().expect("create ssh2 session");
    let trace = ps_exec(&session, "Write-Host hello")
        .expect_err("an unconnected session fails")
        .attach("host=build-01")
        .attach("operation=ps_exec");

    let rendered = format!("{trace:?}");
    assert!(
        rendered.contains("host=build-01"),
        "host context lost: {rendered}"
    );
    assert!(
        rendered.contains("operation=ps_exec"),
        "operation context lost: {rendered}"
    );
}
