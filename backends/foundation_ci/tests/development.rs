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

#[cfg(unix)]
#[test]
fn nfs_reports_cargo_failures_and_empty_test_runs() {
    use std::{fs, os::unix::fs::PermissionsExt, process::Command};
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    let cargo = root.join("cargo");
    fs::write(&cargo, "#!/bin/sh\nprintf '%s' \"$*\" > args\nprintf '%s\\n' \"$LINE\"\nprintf 'cargo diagnostic\\n' >&2\nexit \"$CODE\"\n").unwrap();
    fs::set_permissions(&cargo, fs::Permissions::from_mode(0o755)).unwrap();
    for (line, code, expected) in [
        ("test reads_file ... ok", 0, 0),
        ("test reads_file ... FAILED", 25, 25),
        ("running 0 tests", 0, 1),
    ] {
        let out = Command::new(env!("CARGO_BIN_EXE_foundation-ci"))
            .current_dir(root)
            .env("PATH", root)
            .env("LINE", line)
            .env("CODE", code.to_string())
            .args(["task", "run", "workspace:test-nfs", "--log", "results.log"])
            .output()
            .unwrap();
        assert_eq!(out.status.code(), Some(expected));
        let log = fs::read_to_string(root.join("results.log")).unwrap();
        assert!(log.contains(line) && log.contains("cargo diagnostic"));
        assert!(String::from_utf8(out.stdout).unwrap().contains(line));
        assert!(String::from_utf8(out.stderr)
            .unwrap()
            .contains("cargo diagnostic"));
    }
    assert_eq!(fs::read_to_string(root.join("args")).unwrap(),"test --locked --package foundation_nativeapis --features vfs-native,vfs-nfs --test nfs_vfs");
}
