//! Verify workspace recipes with a substitute for Cargo.
#[cfg(unix)]
#[test]
fn matrix_stops_at_failure_and_publish_validates_the_full_selection() {
    use std::{fs, os::unix::fs::PermissionsExt, process::Command};
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path();
    let cargo = root.join("cargo");
    fs::write(&cargo, r#"#!/bin/sh
set -eu
echo "$* ${CARGO_PROFILE_DEV_CODEGEN_BACKEND:-default}" >> calls
case "$1" in
metadata) echo '{"packages":[{"name":"one","publish":null},{"name":"two","publish":["private"]},{"name":"hidden","publish":[]}]}' ;;
build) test "$5" != wasm32-wasip1 ;;
esac
"#).unwrap();
    fs::set_permissions(&cargo, fs::Permissions::from_mode(0o755)).unwrap();
    let run = |args: &[&str]| {
        Command::new(env!("CARGO_BIN_EXE_foundation-ci"))
            .current_dir(root)
            .env("PATH", root)
            .env_remove("CARGO_PROFILE_DEV_CODEGEN_BACKEND")
            .args(["task", "run"])
            .args(args)
            .output()
            .unwrap()
    };
    let calls = || fs::read_to_string(root.join("calls")).unwrap();
    assert!(!run(&["workspace:build-wasm-matrix"]).status.success());
    assert_eq!(calls().lines().count(), 3);
    assert!(calls().contains("wasm32-wasip1 llvm"));
    assert!(!calls().contains("wasm32-wasip2"));
    for invalid in ["missing", "hidden"] {
        assert!(!run(&["workspace:publish", "one", invalid]).status.success());
    }
    assert!(!calls().lines().any(|l| l.starts_with("publish ")));
    assert!(
        run(&["workspace:publish", "one", "two", "one", "--dry-run"])
            .status
            .success()
    );
    assert!(calls().contains("publish --package one --package two --dry-run default"));
    assert!(run(&["workspace:doc"]).status.success());
    assert!(calls().contains("doc --no-deps --all-features default"));
}
