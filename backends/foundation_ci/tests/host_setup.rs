//! Check package selection without changing the host package database.
#![cfg(unix)]
use std::{fs, os::unix::fs::PermissionsExt, path::Path, process::Command};

fn tool(root: &Path, name: &str) {
    let file = root.join(name);
    fs::write(
        &file,
        "#!/bin/sh\nprintf '%s %s\\n' \"${0##*/}\" \"$*\" >> calls\nexit \"$STATUS\"\n",
    )
    .unwrap();
    fs::set_permissions(&file, fs::Permissions::from_mode(0o755)).unwrap();
}

#[test]
fn testbed_packages_follow_the_distribution_and_stop_after_failure() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    let invoke = |task: &str, args: &[&str], status: &str| {
        Command::new(env!("CARGO_BIN_EXE_foundation-ci"))
            .current_dir(root)
            .env("PATH", root)
            .env("STATUS", status)
            .args(["task", "run", task])
            .args(args)
            .output()
            .unwrap()
    };
    tool(root, "quickget");
    assert!(invoke("images:download-macos", &[], "0").status.success());
    assert_eq!(
        fs::read_to_string(root.join("calls")).unwrap(),
        "quickget macos sonoma\n"
    );
    assert_eq!(
        invoke("images:download-macos", &["ventura"], "27")
            .status
            .code(),
        Some(27)
    );
    fs::remove_file(root.join("calls")).unwrap();
    let file = root.join("os-release");
    fs::write(
        &file,
        "ID=example\nID_LIKE=\"unrelated debian\"\nCOMMENT=$(touch unexpected)\n",
    )
    .unwrap();
    let detect = ["--os-release", file.to_str().unwrap()];
    tool(root, "sudo");
    assert!(invoke("toolchain:install-testbed", &detect, "0")
        .status
        .success());
    let log = root.join("calls");
    let calls = fs::read_to_string(&log).unwrap();
    assert!(calls.starts_with(
        "sudo apt-get update\nsudo apt-get install -y --no-install-recommends qemu-system-x86 "
    ));
    assert!(calls.contains("openssh-client"));
    assert!(!root.join("unexpected").exists());
    fs::write(&log, []).unwrap();
    assert_eq!(
        invoke(
            "toolchain:install-testbed",
            &["--distribution", "ubuntu"],
            "19"
        )
        .status
        .code(),
        Some(19)
    );
    assert_eq!(fs::read_to_string(&log).unwrap(), "sudo apt-get update\n");
    fs::write(&file, "ID=arch\n").unwrap();
    tool(root, "pacman");
    assert!(invoke("toolchain:install-quickemu", &detect, "0")
        .status
        .success());
    assert!(fs::read_to_string(&log)
        .unwrap()
        .ends_with("sudo pacman -Syu --needed --noconfirm quickemu jq python3\n"));
    tool(root, "yay");
    assert!(invoke("toolchain:install-testbed", &detect, "0")
        .status
        .success());
    assert!(fs::read_to_string(&log)
        .unwrap()
        .contains("yay -Syu --needed --noconfirm qemu-base "));
    fs::write(&file, "ID=custom\nID_LIKE=unsupported\n").unwrap();
    let unsupported = invoke("toolchain:install-testbed", &detect, "0");
    assert!(!unsupported.status.success());
    assert!(String::from_utf8(unsupported.stderr)
        .unwrap()
        .contains("Unsupported Linux distribution custom"));
}
