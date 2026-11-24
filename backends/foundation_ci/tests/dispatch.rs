//! Start new binary jobs once and resume after a failure with a temporary Git repository.
#[cfg(unix)]
#[test]
fn dispatch_preserves_successful_tags_and_retries_the_failed_tag() {
    use std::{fs, os::unix::fs::PermissionsExt, process::Command};
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path();
    let git = |args: &[&str]| {
        let output = Command::new("git")
            .current_dir(root)
            .args(args)
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
    };
    git(&["init", "-q"]);
    git(&["config", "tag.gpgSign", "false"]);
    git(&["config", "commit.gpgSign", "false"]);
    git(&[
        "-c",
        "user.name=Test",
        "-c",
        "user.email=local@example.invalid",
        "commit",
        "-qm",
        "initial",
        "--allow-empty",
    ]);
    git(&["tag", "ewe_platform-v1"]);
    let fake = root.join("gh");
    fs::write(
        &fake,
        r#"#!/bin/sh
set -eu
case "$7" in
foundation_wasm_ui-v*) workflow=browser-runtime-release.yaml ;;
*) workflow=release.yaml ;;
esac
test "$1 $2 $3 $4 $5 $6" = "workflow run $workflow --repo fixture/test --ref"
if test -f fail && test "$7" = ewe_platform-v3; then exit 27; fi
echo "$7" >> dispatched
"#,
    )
    .unwrap();
    fs::set_permissions(fake, fs::Permissions::from_mode(0o755)).unwrap();
    let paths = std::iter::once(root.to_owned())
        .chain(std::env::split_paths(&std::env::var_os("PATH").unwrap()))
        .collect::<Vec<_>>();
    let path = std::env::join_paths(paths).unwrap();
    let run = |task: &str, remote: &str| {
        Command::new(env!("CARGO_BIN_EXE_foundation-ci"))
            .current_dir(root)
            .env("PATH", &path)
            .env("GITHUB_REPOSITORY", "fixture/test")
            .args([task, "--remote", remote, "--state", "state"])
            .output()
            .unwrap()
    };
    assert!(run("record-release-tags", ".").status.success());
    git(&["tag", "ewe_platform-v2"]);
    git(&[
        "-c",
        "user.name=Test",
        "-c",
        "user.email=local@example.invalid",
        "tag",
        "-am",
        "three",
        "ewe_platform-v3",
    ]);
    git(&["tag", "unrelated"]);
    let state = fs::read(root.join("state")).unwrap();
    assert!(!run("record-release-tags", "missing").status.success());
    assert_eq!(fs::read(root.join("state")).unwrap(), state);
    fs::write(root.join("fail"), "").unwrap();
    assert_eq!(run("dispatch-binary-releases", ".").status.code(), Some(27));
    fs::remove_file(root.join("fail")).unwrap();
    assert!(run("dispatch-binary-releases", ".").status.success());
    assert!(run("dispatch-binary-releases", ".").status.success());
    assert_eq!(
        fs::read_to_string(root.join("dispatched")).unwrap(),
        "ewe_platform-v2\newe_platform-v3\n"
    );
    let browser = |task: &str| {
        Command::new(env!("CARGO_BIN_EXE_foundation-ci"))
            .current_dir(root)
            .env("PATH", &path)
            .env("GITHUB_REPOSITORY", "fixture/test")
            .args([
                task,
                "--remote",
                ".",
                "--state",
                "browser-tags",
                "--product",
                "browser",
            ])
            .output()
            .unwrap()
    };
    git(&["tag", "foundation_wasm_ui-v1"]);
    assert!(browser("record-release-tags").status.success());
    git(&["tag", "foundation_wasm_ui-v2"]);
    git(&["tag", "ewe_platform-v4"]);
    assert!(browser("dispatch-binary-releases").status.success());
    assert!(browser("dispatch-binary-releases").status.success());
    assert_eq!(
        fs::read_to_string(root.join("dispatched")).unwrap(),
        "ewe_platform-v2\newe_platform-v3\nfoundation_wasm_ui-v2\n"
    );
}
