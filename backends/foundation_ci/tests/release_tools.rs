//! Verify compiler selection and cached release tools with local executables.
#[cfg(unix)]
#[test]
fn release_tools_select_stable_or_nightly_and_check_the_cached_version() {
    use std::{fs, os::unix::fs::PermissionsExt, process::Command};
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path();
    fs::create_dir(root.join("ci")).unwrap();
    fs::write(root.join("ci/release-rust-toolchain"), "1.98.0\n").unwrap();
    fs::write(
        root.join("rust-toolchain.toml"),
        "[toolchain]\nchannel='nightly-2026-09-20'\n",
    )
    .unwrap();
    for (name, body) in [
        ("rustup", "#!/bin/sh\necho \"$*\" >> rustup-calls\n"),
        (
            "cargo",
            r#"#!/bin/sh
set -eu
test "$RUSTUP_TOOLCHAIN" = 1.98.0
echo "$*" >> cargo-calls
while [ "$1" != --root ];do shift; done
shift
mkdir -p "$1/bin"
printf '#!/bin/sh\necho release-plz 0.3.169\n' > "$1/bin/release-plz"
chmod +x "$1/bin/release-plz"
"#,
        ),
    ] {
        let file = root.join(name);
        fs::write(&file, body).unwrap();
        fs::set_permissions(file, fs::Permissions::from_mode(0o755)).unwrap();
    }
    let paths = std::iter::once(root.to_owned())
        .chain(std::env::split_paths(&std::env::var_os("PATH").unwrap()))
        .collect::<Vec<_>>();
    let path = std::env::join_paths(paths).unwrap();
    let run = |mode: &str| {
        Command::new(env!("CARGO_BIN_EXE_foundation-ci"))
            .current_dir(root)
            .env("PATH", &path)
            .env("GITHUB_ENV", root.join("env"))
            .env("GITHUB_PATH", root.join("path"))
            .args([
                "toolchain-release",
                "--mode",
                mode,
                "--cache",
                "tool cache",
                "--version",
                "0.3.169",
                "--github-env",
            ])
            .output()
            .unwrap()
    };
    let result = run("pull-request");
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    assert_eq!(
        fs::read_to_string(root.join("env")).unwrap(),
        "RUSTUP_TOOLCHAIN=nightly-2026-09-20\nCARGO_UNSTABLE_CODEGEN_BACKEND=true\n"
    );
    fs::remove_file(root.join("env")).unwrap();
    assert!(run("publish").status.success());
    assert_eq!(
        fs::read_to_string(root.join("env")).unwrap(),
        "RUSTUP_TOOLCHAIN=1.98.0\n"
    );
    assert_eq!(
        fs::read_to_string(root.join("cargo-calls"))
            .unwrap()
            .lines()
            .count(),
        1
    );
    assert_eq!(
        fs::read_to_string(root.join("rustup-calls"))
            .unwrap()
            .lines()
            .count(),
        3
    );
}
