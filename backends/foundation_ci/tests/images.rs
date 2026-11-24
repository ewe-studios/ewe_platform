//! Exercise the installed image commands with local files and a local Docker substitute.
use std::{
    fs,
    path::Path,
    process::{Command, Output},
};

fn run(root: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_foundation-ci"))
        .current_dir(root)
        .args(args)
        .env("GITHUB_OUTPUT", root.join("actions-output"))
        .env("PLATFORM_IMAGES_REGISTRY_USERNAME", "test-user")
        .output()
        .unwrap()
}
fn success(output: &Output) -> serde_json::Value {
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&output.stdout).unwrap()
}
#[test]
fn image_inputs_preserve_argument_order_and_failed_staging_preserves_output() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path();
    fs::write(root.join("Dockerfile"), "FROM scratch").unwrap();
    fs::write(root.join("asset"), "asset").unwrap();
    fs::write(root.join("image.toml"),"[registry]\nnamespace='example'\nusername='configured'\n[base]\nname='base'\n[base.flavors.default]\nfrom='scratch'\n[image]\nname='app'\ndockerfile='Dockerfile'\nfiles=['asset']\n[image.build_args]\nZ='first'\nA='second'\n").unwrap();
    let args = [
        "image-prepare",
        "--config",
        "image.toml",
        "--layer",
        "image",
        "--parent",
        "base:latest",
        "--out",
        "out",
        "--github-output",
    ];
    let first = success(&run(root, &args));
    assert_eq!(
        first["build_args"],
        "Z=first\nA=second\nBASE_IMAGE=base:latest"
    );
    assert_eq!(first["registry_username"], "test-user");
    assert!(fs::read_to_string(root.join("actions-output"))
        .unwrap()
        .contains("build_args<<"));
    let path = root.join("out/context/files/asset");
    let before = path.metadata().unwrap().modified().unwrap();
    assert_eq!(first, success(&run(root, &args)));
    assert_eq!(before, path.metadata().unwrap().modified().unwrap());
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(root.join("asset"), fs::Permissions::from_mode(0o755)).unwrap();
        let changed = success(&run(root, &args));
        assert_ne!(first["hash"], changed["hash"]);
        assert_ne!(path.metadata().unwrap().permissions().mode() & 0o111, 0);
        fs::remove_file(root.join("asset")).unwrap();
        std::os::unix::fs::symlink("Dockerfile", root.join("asset")).unwrap();
        success(&run(root, &args));
        assert_eq!(path.read_link().unwrap(), Path::new("Dockerfile"));
    }
    let plan = fs::read(root.join("out/plan.json")).unwrap();
    fs::remove_file(root.join("asset")).unwrap();
    assert!(!run(root, &args).status.success());
    assert_eq!(fs::read(root.join("out/plan.json")).unwrap(), plan);
    assert!(fs::symlink_metadata(path).is_ok());
    let settings = success(&run(
        root,
        &["image-settings", "--config", "image.toml", "--credentials"],
    ));
    assert_eq!(settings["pull-username"], "test-user");
    assert_eq!(settings["push"], true);
}

#[cfg(unix)]
#[test]
fn registry_reports_missing_images_and_preserves_registry_ports() {
    use std::os::unix::fs::PermissionsExt;
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path();
    let docker = root.join("docker");
    fs::write(&docker,r#"#!/bin/sh
case "$3" in
  create) printf '%s\n' "$@" > tags ;;
  inspect)
    case "$7" in
      found) echo '{"digest":"sha256:012345"}' ;;
      retry) if test -f retried; then echo '{"digest":"sha256:abc"}'; else touch retried; echo 'Too many requests' >&2; exit 1; fi ;;
      denied) echo 'unauthorized' >&2; exit 1 ;;
      missing) echo 'manifest unknown' >&2; exit 1 ;;
      *) echo 'network unavailable' >&2; exit 17 ;;
    esac ;;
  *) exit 2 ;;
esac
"#).unwrap();
    fs::set_permissions(docker, fs::Permissions::from_mode(0o755)).unwrap();
    let mut paths = vec![root.to_path_buf()];
    paths.extend(std::env::split_paths(&std::env::var_os("PATH").unwrap()));
    let path = std::env::join_paths(paths).unwrap();
    let cmd = |args: &[&str]| {
        Command::new(env!("CARGO_BIN_EXE_foundation-ci"))
            .current_dir(root)
            .env("PATH", &path)
            .args(args)
            .output()
            .unwrap()
    };
    for image in ["missing", "denied"] {
        assert_eq!(success(&cmd(&["image-exists", image]))["exists"], false);
    }
    assert_eq!(
        success(&cmd(&["image-exists", "found"]))["digest"],
        "sha256:012345"
    );
    assert_eq!(
        success(&cmd(&["image-exists", "retry", "--retry-seconds", "0"]))["digest"],
        "sha256:abc"
    );
    let error = cmd(&["image-exists", "network"]);
    assert_eq!(error.status.code(), Some(17));
    assert!(String::from_utf8_lossy(&error.stderr).contains("network unavailable"));
    assert!(cmd(&[
        "image-promote",
        "localhost:9876/ns/name:old@sha256:aaa",
        "--tags",
        "latest qa"
    ])
    .status
    .success());
    let tags = fs::read_to_string(root.join("tags")).unwrap();
    assert!(tags.contains("-t\nlocalhost:9876/ns/name:latest\n-t\nlocalhost:9876/ns/name:qa\n"));
}

#[cfg(unix)]
#[test]
fn failed_image_export_preserves_the_previous_local_image() {
    use std::os::unix::fs::PermissionsExt;
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path();
    fs::write(root.join("Dockerfile"), "FROM scratch").unwrap();
    fs::write(root.join("image.toml"),"[registry]\nnamespace='fixture'\nusername='local'\n[base]\nname='base'\n[base.flavors.default]\nfrom='scratch'\n[image]\nname='app'\ndockerfile='Dockerfile'\n").unwrap();
    let docker = root.join("docker");
    fs::write(
        &docker,
        r#"#!/bin/sh
set -eu
test "$1 $2" = 'buildx build'
shift 2
while [ "$#" -gt 0 ]; do
    case "$1" in
        --metadata-file) shift; meta=$1 ;;
    esac
    shift
done
dest=$(dirname "$meta")
if test -f fail; then echo partial > "$dest/image.tar"; exit 34; fi
echo 'verified image' > "$dest/image.tar"
printf '{"containerimage.digest":"sha256:local"}' > "$meta"
"#,
    )
    .unwrap();
    fs::set_permissions(docker, fs::Permissions::from_mode(0o755)).unwrap();
    let paths = std::iter::once(root.to_owned())
        .chain(std::env::split_paths(&std::env::var_os("PATH").unwrap()))
        .collect::<Vec<_>>();
    let path = std::env::join_paths(paths).unwrap();
    let run = || {
        Command::new(env!("CARGO_BIN_EXE_foundation-ci"))
            .current_dir(root)
            .env("PATH", &path)
            .args([
                "image-ensure",
                "--config",
                "image.toml",
                "--layer",
                "image",
                "--parent",
                "scratch",
                "--out",
                "out",
                "--force",
            ])
            .output()
            .unwrap()
    };
    let result = success(&run());
    assert_eq!(result["source"], "local");
    let old = fs::read(root.join("out/plan.json")).unwrap();
    fs::write(root.join("fail"), "").unwrap();
    fs::write(root.join("Dockerfile"), "FROM other").unwrap();
    assert_eq!(run().status.code(), Some(34));
    assert_eq!(old, fs::read(root.join("out/plan.json")).unwrap());
    assert_eq!(
        fs::read_to_string(root.join("out/image.tar")).unwrap(),
        "verified image\n"
    );
}

#[cfg(unix)]
#[test]
fn vm_control_uses_the_selected_container_id_and_preserves_failures() {
    use std::{fs, os::unix::fs::PermissionsExt, process::Command};
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path();
    let docker = root.join("docker");
    fs::write(
        &docker,
        r#"#!/bin/sh
printf '<%s>' "$@" >> calls
printf '\n' >> calls
if test "$1" = container; then
  test -z "$FAIL" || exit "$FAIL"
  test -z "$STATE" || printf '%s %s\n' "$ID" "$STATE"
fi
"#,
    )
    .unwrap();
    fs::set_permissions(docker, fs::Permissions::from_mode(0o755)).unwrap();
    let id = "1".repeat(64);
    let invoke = |args: &[&str], state: &str, fail: &str| {
        Command::new(env!("CARGO_BIN_EXE_foundation-ci"))
            .current_dir(root)
            .env("PATH", root)
            .env("ID", &id)
            .env("STATE", state)
            .env("FAIL", fail)
            .args(["task", "run"])
            .args(args)
            .output()
            .unwrap()
    };
    let down = [
        "images:vm-down",
        "--name",
        "fixture.vm",
        "--stop-seconds",
        "5",
    ];
    assert!(invoke(&down, "", "").status.success());
    assert_eq!(invoke(&down, "", "42").status.code(), Some(42));
    assert!(invoke(&down, "running", "").status.success());
    let calls = fs::read_to_string(root.join("calls")).unwrap();
    assert!(calls.contains("<name=^/fixture\\.vm$>"));
    assert!(calls.contains(&format!("<stop><--time><5><{id}>\n<rm><{id}>")));
    fs::write(root.join("guest.pub"), "ssh-ed25519 public-fixture guest\n").unwrap();
    let up = [
        "images:vm-up",
        "--name",
        "fixture.vm",
        "--os",
        "macos",
        "--storage",
        "disk with spaces",
        "--image",
        "fixture/image:local",
        "--key",
        "guest",
        "--kvm-device",
        "/dev/null",
        "--tun-device",
        "/dev/null",
        "--version",
        "sonoma",
        "--web-port",
        "8708",
    ];
    assert!(!invoke(&up, "running", "").status.success());
    assert!(invoke(&up, "exited", "").status.success());
    let calls = fs::read_to_string(root.join("calls")).unwrap();
    assert!(calls.contains(&format!("<rm><{id}>\n<run><--detach><--name><fixture.vm>")));
    assert!(calls.contains("<127.0.0.1:2222:22>") && calls.contains("<127.0.0.1:8708:8006>"));
    assert!(calls.contains("disk with spaces:/storage>") && calls.contains("<VERSION=sonoma>"));
    assert!(calls.contains("<PI_SSH_AUTHORIZED_KEY=ssh-ed25519 public-fixture guest>"));
    assert!(!calls.contains("<RAM_SIZE=>"));
}

#[cfg(unix)]
#[test]
fn vm_runner_distinguishes_guest_exit_from_ssh_failure() {
    use std::os::unix::fs::PermissionsExt;
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path();
    let ssh = root.join("ssh");
    fs::write(
        &ssh,
        r#"#!/bin/sh
printf '<%s>' "$@" >> calls
case "$RESULT" in
  sequence) if test -f ready; then exit 44; else touch ready; exit 0; fi ;;
  *) exit "$RESULT" ;;
esac
"#,
    )
    .unwrap();
    fs::set_permissions(ssh, fs::Permissions::from_mode(0o755)).unwrap();
    let path = std::env::join_paths(
        std::iter::once(root.to_owned())
            .chain(std::env::split_paths(&std::env::var_os("PATH").unwrap())),
    )
    .unwrap();
    let invoke = |task: &str, os: &str, code: &str, args: &[&str]| {
        Command::new(env!("CARGO_BIN_EXE_foundation-ci"))
            .current_dir(root)
            .env("PATH", &path)
            .env("PI_VM_USER", "ci user")
            .env("RESULT", code)
            .args([
                "task",
                "run",
                task,
                "--key",
                "key with spaces",
                "--os",
                os,
                "--port",
                "2345",
            ])
            .args(args)
            .output()
            .unwrap()
    };
    let alive = "images:vm-runner-alive";
    let wait = "images:vm-wait-runner";
    for (os, code, expected) in [("windows", "0", "true"), ("macos", "44", "false")] {
        let output = invoke(alive, os, code, &[]);
        assert!(output.status.success());
        assert_eq!(String::from_utf8(output.stdout).unwrap().trim(), expected);
    }
    for failed in [1, 3, 127, 255] {
        let output = invoke(wait, "macos", &failed.to_string(), &[]);
        assert_eq!(output.status.code(), Some(failed));
        assert!(!String::from_utf8_lossy(&output.stdout).contains("runner finished"));
    }
    assert!(
        !invoke(wait, "windows", "0", &["--timeout-ms", "0"])
            .status
            .success()
    );
    assert!(
        invoke(wait, "macos", "sequence", &["--interval-ms", "1"])
            .status
            .success()
    );
    let calls = fs::read_to_string(root.join("calls")).unwrap();
    assert!(calls.contains("key with spaces><-p><2345>"));
    assert!(calls.contains("<-l><ci user><127.0.0.1>"));
    assert!(calls.contains("Get-Process") && calls.contains("pgrep -f"));
}
