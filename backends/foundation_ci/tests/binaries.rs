//! Verify a release archive through the independent command host.
use sha2::{Digest, Sha256};
use std::{fs, path::Path, process::Command};
#[test]
fn release_binary_archive_uses_cargo_artifacts_and_verifies_the_checksum() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path();
    fs::create_dir(root.join("src")).unwrap();
    fs::write(root.join("Cargo.toml"),"[package]\nname='release-fixture'\nversion='1.0.0'\nedition='2021'\n[[bin]]\nname='external-name'\npath='src/main.rs'\n").unwrap();
    fs::write(
        root.join("src/main.rs"),
        "fn main(){println!(\"binary fixture\");}",
    )
    .unwrap();
    let status = Command::new("cargo")
        .current_dir(root)
        .arg("generate-lockfile")
        .status()
        .unwrap();
    assert!(status.success());
    let run = |packages: &str| {
        Command::new(env!("CARGO_BIN_EXE_foundation-ci"))
            .current_dir(root)
            .env("CARGO_TARGET_DIR", root.join("custom target"))
            .env("GITHUB_OUTPUT", root.join("output"))
            .args([
                "release-binaries",
                "--packages",
                packages,
                "--tag",
                "branch/test",
                "--github-output",
            ])
            .output()
            .unwrap()
    };
    let output = run("release-fixture");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let name = String::from_utf8(output.stdout).unwrap();
    let name = name.trim();
    assert!(name.starts_with("ewe_platform-branch-test-"));
    let archive = root.join("dist").join(format!("{name}.tar.gz"));
    let extract = root.join("unpacked");
    fs::create_dir(&extract).unwrap();
    assert!(Command::new("tar")
        .current_dir(&extract)
        .arg("-xf")
        .arg(&archive)
        .status()
        .unwrap()
        .success());
    let bin = Path::new("external-name").with_extension(std::env::consts::EXE_EXTENSION);
    assert_eq!(
        Command::new(extract.join(name).join(bin))
            .output()
            .unwrap()
            .stdout,
        b"binary fixture\n"
    );
    let checksum =
        fs::read_to_string(root.join("dist").join(format!("{name}.tar.gz.sha256"))).unwrap();
    let bytes = fs::read(&archive).unwrap();
    assert_eq!(
        checksum.split_whitespace().next().unwrap(),
        hex::encode(Sha256::digest(&bytes))
    );
    assert!(run("release-fixture").status.success());
    assert_eq!(fs::read(&archive).unwrap(), bytes);
    assert!(!run("unknown").status.success());
    assert_eq!(fs::read(&archive).unwrap(), bytes);
    assert_eq!(
        fs::read_to_string(root.join("output")).unwrap(),
        format!("name={name}\nname={name}\n")
    );
}

#[cfg(unix)]
#[test]
fn attachments_verify_all_files_before_query_and_recover_after_failed_upload() {
    use std::os::unix::fs::PermissionsExt;
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path();
    let dir = root.join("dist");
    fs::create_dir(&dir).unwrap();
    fs::write(dir.join("app.tar.gz"), b"archive bytes").unwrap();
    let checksum = dir.join("app.tar.gz.sha256");
    fs::write(&checksum, "bad  dist/app.tar.gz\n").unwrap();
    let gh = root.join("gh");
    fs::write(
        &gh,
        r#"#!/bin/sh
printf '%s\n' "$*" >> calls
case "$1 $2" in
  'api graphql')
    if test -f denied; then exit 23; fi
    if test -f draft; then
      echo '{"data":{"repository":{"release":{"tagName":"v1"}}}}'
    else
      echo '{"data":{"repository":{"release":null}}}'
    fi ;;
  'release create') touch draft ;;
  'release upload') test -f broken && exit 13; exit 0 ;;
  *) exit 2 ;;
esac
"#,
    )
    .unwrap();
    fs::set_permissions(gh, fs::Permissions::from_mode(0o755)).unwrap();
    let paths = std::iter::once(root.to_owned())
        .chain(std::env::split_paths(&std::env::var_os("PATH").unwrap()))
        .collect::<Vec<_>>();
    let path = std::env::join_paths(paths).unwrap();
    let run = || {
        Command::new(env!("CARGO_BIN_EXE_foundation-ci"))
            .current_dir(root)
            .env("PATH", &path)
            .args(["attach-binaries", "--tag", "v1", "--repo", "test/local"])
            .output()
            .unwrap()
    };
    assert!(!run().status.success());
    assert!(!root.join("calls").exists());
    fs::write(
        checksum,
        format!(
            "{}  dist/app.tar.gz\n",
            hex::encode(Sha256::digest(b"archive bytes"))
        ),
    )
    .unwrap();
    fs::write(root.join("denied"), "").unwrap();
    assert_eq!(run().status.code(), Some(23));
    assert!(!root.join("draft").exists());
    fs::remove_file(root.join("denied")).unwrap();
    fs::write(root.join("broken"), "").unwrap();
    assert_eq!(run().status.code(), Some(13));
    assert!(root.join("draft").exists());
    fs::remove_file(root.join("broken")).unwrap();
    let output = run();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let calls = fs::read_to_string(root.join("calls")).unwrap();
    assert_eq!(
        calls.matches("release create --draft --verify-tag").count(),
        1
    );
    assert_eq!(
        calls
            .matches("release upload --repo test/local --clobber -- v1")
            .count(),
        2
    );
}
