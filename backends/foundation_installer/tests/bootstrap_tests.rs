//! The bootstrap script: the manifest round trip, the target choice and the
//! pinned installer checksum.

use base64::Engine as _;
use foundation_installer::{BootstrapInput, BootstrapScript, InstallError, InstallerBuild};

const MARKER_START: &str = "<<'EWE_MANIFEST_BASE64'\n";
const MARKER_END: &str = "\nEWE_MANIFEST_BASE64";

fn sha(byte: char) -> String {
    std::iter::repeat_n(byte, 64).collect()
}

fn build(target: &str, url: &str, sha256: String) -> InstallerBuild {
    InstallerBuild {
        target: target.to_string(),
        url: url.to_string(),
        sha256,
    }
}

fn input() -> BootstrapInput {
    BootstrapInput::new(
        r#"{"manifest_version":1,"entries":[]}"#,
        vec![
            build(
                "x86_64-unknown-linux-gnu",
                "https://example.com/ewe-installer-x86_64",
                sha('a'),
            ),
            build(
                "aarch64-unknown-linux-gnu",
                "https://example.com/ewe-installer-aarch64",
                sha('b'),
            ),
        ],
    )
}

fn embedded_manifest(script: &str) -> String {
    let start = script
        .find(MARKER_START)
        .map(|index| index + MARKER_START.len())
        .expect("the manifest marker");
    let end = script[start..]
        .find(MARKER_END)
        .map(|index| start + index)
        .expect("the manifest end marker");
    let decoded = base64::engine::general_purpose::STANDARD
        .decode(&script.as_bytes()[start..end])
        .expect("the embedded manifest is base64");
    String::from_utf8(decoded).expect("the embedded manifest is UTF-8")
}

#[test]
fn the_embedded_manifest_decodes_back_to_the_same_json() {
    let input = input();
    let script = BootstrapScript::render(&input).expect("the script renders");
    assert_eq!(embedded_manifest(&script), input.manifest_json);
}

#[test]
fn the_script_chooses_a_build_for_each_target() {
    let script = BootstrapScript::render(&input()).expect("the script renders");
    assert!(
        script.contains("Linux/x86_64)"),
        "unexpected script:\n{script}"
    );
    assert!(
        script.contains("Linux/aarch64)"),
        "unexpected script:\n{script}"
    );
    assert!(script.contains("'https://example.com/ewe-installer-x86_64'"));
    assert!(script.contains("'https://example.com/ewe-installer-aarch64'"));
}

#[test]
fn the_script_pins_the_installer_checksum() {
    let script = BootstrapScript::render(&input()).expect("the script renders");
    assert!(script.contains("sha256sum -c"));
    assert!(script.contains(&format!("'{}'", sha('a'))));
    assert!(!script.contains(&format!("'{}'", sha('c'))));
}

#[test]
fn a_non_https_installer_url_is_refused() {
    let input = BootstrapInput::new(
        "{}",
        vec![build(
            "x86_64-unknown-linux-gnu",
            "http://example.com/ewe-installer",
            sha('a'),
        )],
    );
    match BootstrapScript::render(&input) {
        Err(InstallError::Parse(faults)) => {
            assert_eq!(faults[0].path, "/installers/0/url");
        }
        other => panic!("expected a parse error, got {other:?}"),
    }
}

#[test]
fn a_short_checksum_is_refused() {
    let input = BootstrapInput::new(
        "{}",
        vec![build(
            "x86_64-unknown-linux-gnu",
            "https://example.com/ewe-installer",
            "abc".to_string(),
        )],
    );
    match BootstrapScript::render(&input) {
        Err(InstallError::Parse(faults)) => {
            assert_eq!(faults[0].path, "/installers/0/sha256");
        }
        other => panic!("expected a parse error, got {other:?}"),
    }
}

#[test]
fn a_target_without_a_uname_mapping_is_refused() {
    let input = BootstrapInput::new(
        "{}",
        vec![build(
            "sparc-unknown-linux-gnu",
            "https://example.com/ewe-installer",
            sha('a'),
        )],
    );
    match BootstrapScript::render(&input) {
        Err(InstallError::Parse(faults)) => {
            assert!(faults[0].message.contains("uname"));
        }
        other => panic!("expected a parse error, got {other:?}"),
    }
}

#[test]
fn an_empty_installer_list_is_refused() {
    let input = BootstrapInput::new("{}", Vec::new());
    match BootstrapScript::render(&input) {
        Err(InstallError::Parse(faults)) => {
            assert_eq!(faults[0].path, "/installers");
        }
        other => panic!("expected a parse error, got {other:?}"),
    }
}

#[test]
fn the_manifest_path_is_shell_quoted() {
    let mut input = input();
    input.manifest_path = std::path::PathBuf::from("/tmp/a path/it's.json");
    let script = BootstrapScript::render(&input).expect("the script renders");
    assert!(
        script.contains("MANIFEST_PATH='/tmp/a path/it'\\''s.json'"),
        "unexpected script:\n{script}"
    );
}

#[test]
fn two_builds_of_one_os_arch_keep_the_first() {
    let input = BootstrapInput::new(
        "{}",
        vec![
            build(
                "x86_64-unknown-linux-gnu",
                "https://example.com/gnu",
                sha('a'),
            ),
            build(
                "x86_64-unknown-linux-musl",
                "https://example.com/musl",
                sha('b'),
            ),
        ],
    );
    let script = BootstrapScript::render(&input).expect("the script renders");
    assert!(script.contains("'https://example.com/gnu'"));
    assert!(!script.contains("'https://example.com/musl'"));
}

/// The SHA-256 of `bytes` as lowercase hexadecimal.
fn sha256_hex(bytes: &[u8]) -> String {
    use core::fmt::Write as _;
    use sha2::{Digest, Sha256};

    let mut hasher = Sha256::new();
    hasher.update(bytes);
    let mut out = String::with_capacity(64);
    for byte in hasher.finalize() {
        // Writing to a String cannot fail.
        let _ = write!(out, "{byte:02x}");
    }
    out
}

/// A shell single-quoted value.
fn shell_quote(value: &str) -> String {
    format!("'{}'", value.replace('\'', "'\\''"))
}

/// Write a fake command and make it executable.
#[cfg(unix)]
fn write_command(path: &std::path::Path, body: &str) {
    use std::os::unix::fs::PermissionsExt;

    std::fs::write(path, body).expect("the fake command");
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o755))
        .expect("the fake command mode");
}

/// Run the rendered script with fake `uname`, `curl`, `base64` and `sha256sum`
/// commands. It returns the exit status and whether the fake installer ran.
#[cfg(unix)]
fn run_script(installer_sha: &str, marker: &std::path::Path) -> (bool, bool) {
    let directory = tempfile::tempdir().expect("a temporary directory");
    let bin = directory.path().join("bin");
    std::fs::create_dir_all(&bin).expect("the fake bin directory");
    write_command(
        &bin.join("uname"),
        "#!/bin/sh\ncase \"$1\" in -s) echo Linux ;; -m) echo x86_64 ;; *) exit 1 ;; esac\n",
    );
    write_command(
        &bin.join("base64"),
        "#!/bin/sh\nexec openssl base64 -d -A\n",
    );
    write_command(
        &bin.join("sha256sum"),
        "#!/bin/sh\nset -eu\nset -- $(cat \"$2\")\nexpected=$1\npath=$2\nactual=$(openssl dgst -sha256 -r \"$path\" | cut -d' ' -f1)\n[ \"$expected\" = \"$actual\" ]\n",
    );

    let installer = format!(
        "#!/bin/sh\ntouch {}\nexit 0\n",
        shell_quote(&marker.display().to_string())
    );
    let curl = format!(
        "#!/bin/sh\nset -eu\nout=\nwhile [ $# -gt 0 ]; do case \"$1\" in -o) out=\"$2\"; shift 2 ;; *) shift ;; esac; done\nprintf '%s' {} > \"$out\"\nchmod +x \"$out\"\n",
        shell_quote(&installer)
    );
    write_command(&bin.join("curl"), &curl);

    let mut input = BootstrapInput::new(
        r#"{"manifest_version":1,"entries":[]}"#,
        vec![build(
            "x86_64-unknown-linux-gnu",
            "https://example.com/ewe-installer",
            installer_sha.to_string(),
        )],
    );
    input.manifest_path = directory.path().join("etc/ewe/manifest.json");
    let script = BootstrapScript::render(&input).expect("the script renders");
    let script_path = directory.path().join("bootstrap.sh");
    std::fs::write(&script_path, script).expect("the script file");

    let path = format!(
        "{}:{}",
        bin.display(),
        std::env::var("PATH").unwrap_or_default()
    );
    let status = std::process::Command::new("sh")
        .arg(&script_path)
        .env("PATH", path)
        .output()
        .expect("the script runs");
    (status.status.success(), marker.exists())
}

#[cfg(unix)]
#[test]
fn the_script_runs_the_installer_with_the_right_checksum() {
    let directory = tempfile::tempdir().expect("a temporary directory");
    let marker = directory.path().join("installer-ran");
    let installer = format!(
        "#!/bin/sh\ntouch {}\nexit 0\n",
        shell_quote(&marker.display().to_string())
    );
    let (success, ran) = run_script(&sha256_hex(installer.as_bytes()), &marker);
    assert!(success, "the script should run the installer");
    assert!(ran, "the installer should have run");
}

#[cfg(unix)]
#[test]
fn the_script_refuses_a_wrong_installer_checksum() {
    let directory = tempfile::tempdir().expect("a temporary directory");
    let marker = directory.path().join("installer-ran");
    let (success, ran) = run_script(&"0".repeat(64), &marker);
    assert!(!success, "the script should stop on a checksum mismatch");
    assert!(!ran, "the installer must not run");
}
