//! The `sandbox-krun` runner tests (feature 09, section 7).
//!
//! These tests run the real runner program. They need no libkrun: the two
//! cases stop before a library call.

fn runner(args: &[&str]) -> std::process::Output {
    std::process::Command::new(env!("CARGO_BIN_EXE_sandbox-krun"))
        .args(args)
        .output()
        .expect("the sandbox-krun program starts")
}

#[test]
fn config_parse_error_exit_64() {
    let output = runner(&["/nonexistent/krun.json"]);
    assert_eq!(output.status.code(), Some(64));
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("sandbox-krun:"),
        "the runner explains the bad configuration: {stderr}"
    );
}

#[test]
fn probe_without_library_exit_71() {
    let output = runner(&["--probe", "--libkrun", "/nonexistent/libkrun.dylib"]);
    assert_eq!(output.status.code(), Some(71));
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("sandbox-krun:"),
        "the runner explains the missing library: {stderr}"
    );
}
