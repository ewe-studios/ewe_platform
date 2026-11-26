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

#[cfg(unix)]
#[test]
fn vm_wait_checks_the_same_guest_until_it_is_ready() {
    use std::os::unix::fs::PermissionsExt;
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path();
    for (name, script) in [
        (
            "docker",
            r#"#!/bin/sh
case "$1" in
  logs) echo "$@" >> log-calls ;;
  container)
    test "$MODE" != daemon || exit 41
    test "$MODE" != missing || exit 0
    id=$ORIGINAL
    if test "$MODE" = replace && test -f polled; then id=$SECOND; fi
    printf '%s %s\n' "$id" "$STATE" ;;
esac
"#,
        ),
        (
            "ssh",
            r#"#!/bin/sh
printf '<%s>' "$@" >> probes
case "$MODE" in
  pending|replace) if test -f polled; then exit 0; fi; touch polled; exit 2 ;;
  *) exit "$RESULT" ;;
esac
"#,
        ),
    ] {
        let executable = root.join(name);
        fs::write(&executable, script).unwrap();
        fs::set_permissions(executable, fs::Permissions::from_mode(0o755)).unwrap();
    }
    let path = std::env::join_paths(
        std::iter::once(root.to_owned())
            .chain(std::env::split_paths(&std::env::var_os("PATH").unwrap())),
    )
    .unwrap();
    let original = "a".repeat(64);
    let invoke = |os: &str, mode: &str, state: &str, result: &str, args: &[&str]| {
        Command::new(env!("CARGO_BIN_EXE_foundation-ci"))
            .current_dir(root)
            .env("PATH", &path)
            .env("ORIGINAL", &original)
            .env("SECOND", "b".repeat(64))
            .env("MODE", mode)
            .env("STATE", state)
            .env("RESULT", result)
            .args([
                "task",
                "run",
                "images:vm-wait",
                "--os",
                os,
                "--key",
                "guest",
                "--interval-ms",
                "1",
            ])
            .args(args)
            .output()
            .unwrap()
    };
    assert!(
        invoke("macos", "pending", "running", "0", &[])
            .status
            .success()
    );
    assert!(
        invoke("windows", "", "running", "0", &["--ssh-only"])
            .status
            .success()
    );
    let failure = invoke("windows", "", "running", "3", &[]);
    assert!(String::from_utf8_lossy(&failure.stderr).contains("Provisioning failed"));
    let calls = fs::read_to_string(root.join("probes")).unwrap();
    assert!(calls.contains("/usr/local/platform-images/status/provisioned.ok"));
    assert!(calls.contains("C:/OEM/status/provision.failed") && calls.contains("<exit 0>"));
    for (mode, code) in [("daemon", 41), ("", 127)] {
        assert_eq!(
            invoke("windows", mode, "running", "127", &[]).status.code(),
            Some(code)
        );
    }
    assert!(
        !invoke("macos", "missing", "running", "0", &[])
            .status
            .success()
    );
    assert!(!invoke("macos", "", "exited", "0", &[]).status.success());
    assert!(
        fs::read_to_string(root.join("log-calls"))
            .unwrap()
            .contains(&format!("logs --tail 50 {original}"))
    );
    let timeout = invoke("macos", "", "running", "255", &["--timeout-ms", "0"]);
    assert!(String::from_utf8_lossy(&timeout.stderr).contains("SSH status 255"));
    fs::remove_file(root.join("polled")).unwrap();
    let replaced = invoke("macos", "replace", "running", "0", &[]);
    assert!(String::from_utf8_lossy(&replaced.stderr).contains("was replaced or removed"));
}

#[cfg(unix)]
#[test]
fn vm_logs_preserve_completed_files_and_distinguish_absence() {
    use std::os::unix::fs::PermissionsExt;
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path();
    for (name, script) in [
        (
            "docker",
            r#"#!/bin/sh
if test "$1" = container; then printf '%064d running\n' 0; else printf host; printf diagnostic >&2; fi
"#,
        ),
        (
            "ssh",
            r#"#!/bin/sh
printf '%s\n' "$@" >> probes
if test "$RESPONSE" != good; then printf partial; exit "$RESPONSE"; fi
case "$*" in *versions.txt*) exit 44 ;; *) printf 'guest\000log\377'; printf diag >&2 ;; esac
"#,
        ),
    ] {
        let path = root.join(name);
        fs::write(&path, script).unwrap();
        fs::set_permissions(path, fs::Permissions::from_mode(0o755)).unwrap();
    }
    let path = std::env::join_paths(
        std::iter::once(root.to_owned())
            .chain(std::env::split_paths(&std::env::var_os("PATH").unwrap())),
    )
    .unwrap();
    let invoke = |os: &str, response: &str| {
        Command::new(env!("CARGO_BIN_EXE_foundation-ci"))
            .current_dir(root)
            .env("PATH", &path)
            .env("RESPONSE", response)
            .args([
                "task",
                "run",
                "images:vm-logs",
                "--os",
                os,
                "--key",
                "fixture",
                "--out",
                "output",
            ])
            .output()
            .unwrap()
    };
    let out = root.join("output");
    fs::create_dir(&out).unwrap();
    fs::write(out.join("versions.txt"), "old").unwrap();
    for os in ["windows", "macos"] {
        let result = invoke(os, "good");
        assert!(
            result.status.success(),
            "{}",
            String::from_utf8_lossy(&result.stderr)
        );
        for name in ["provision.log", "runner.log"] {
            assert_eq!(fs::read(out.join(name)).unwrap(), b"guest\0log\xff");
        }
        assert!(!out.join("versions.txt").exists());
        let host = fs::read_to_string(out.join("container.log")).unwrap();
        assert!(host.contains("host") && host.contains("diagnostic"));
        assert_eq!(invoke(os, "255").status.code(), Some(255));
        assert_eq!(
            fs::read(out.join("provision.log")).unwrap(),
            b"guest\0log\xff"
        );
    }
    let probes = fs::read_to_string(root.join("probes")).unwrap();
    assert!(probes.contains("CopyTo") && probes.contains("exec cat"));
}

#[cfg(unix)]
#[test]
fn vm_transfers_keep_binary_input_intact_and_preserve_files_after_failure() {
    use std::os::unix::fs::PermissionsExt;
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path();
    for (name, script) in [
        (
            "ssh",
            r#"#!/bin/sh
printf '<%s>' "$@" >> probes
cat > archive
"#,
        ),
        (
            "scp",
            r#"#!/bin/sh
printf '<%s>' "$@" >> copies
for last do :; done
case "$last" in
  127.0.0.1:*) ;;
  *) mkdir -p "$last/folder"; echo complete > "$last/folder/readme" ;;
esac
exit "$RESULT"
"#,
        ),
    ] {
        let path = root.join(name);
        fs::write(&path, script).unwrap();
        fs::set_permissions(path, fs::Permissions::from_mode(0o755)).unwrap();
    }
    let path = std::env::join_paths(
        std::iter::once(root.to_owned())
            .chain(std::env::split_paths(&std::env::var_os("PATH").unwrap())),
    )
    .unwrap();
    let run = |task: &str, os: &str, result: &str, args: &[&str]| {
        Command::new(env!("CARGO_BIN_EXE_foundation-ci"))
            .current_dir(root)
            .env("PATH", &path)
            .env("RESULT", result)
            .args(["task", "run", task, "--os", os, "--key", "guest"])
            .args(args)
            .output()
            .unwrap()
    };
    fs::create_dir_all(root.join("src/target")).unwrap();
    fs::write(root.join("src/target/cache"), "excluded").unwrap();
    fs::write(root.join("src/file"), b"\x00\xff\x11").unwrap();
    let push = ["--src", "src", "--dest", "a user's files"];
    assert!(run("images:vm-push", "macos", "0", &push).status.success());
    check_guest_archive(&root.join("archive"));
    assert!(
        run("images:vm-push", "windows", "0", &push)
            .status
            .success()
    );
    assert_eq!(
        run("images:vm-push", "windows", "37", &push).status.code(),
        Some(37)
    );
    let probes = fs::read_to_string(root.join("probes")).unwrap();
    assert!(probes.contains("a user'\\''s files") && probes.contains("a user''s files"));
    assert!(
        fs::read_to_string(root.join("copies"))
            .unwrap()
            .contains("127.0.0.1:ewe-guest-")
    );
    fs::create_dir_all(root.join("downloads/folder")).unwrap();
    fs::write(root.join("downloads/folder/readme"), "previous").unwrap();
    fs::write(root.join("downloads/folder/extra"), "keep").unwrap();
    let pull = ["--src", "folder", "--dest", "downloads"];
    assert_eq!(
        run("images:vm-pull", "windows", "37", &pull).status.code(),
        Some(37)
    );
    assert_eq!(
        fs::read(root.join("downloads/folder/readme")).unwrap(),
        b"previous"
    );
    assert!(run("images:vm-pull", "macos", "0", &pull).status.success());
    assert_eq!(
        fs::read(root.join("downloads/folder/readme")).unwrap(),
        b"complete\n"
    );
    assert_eq!(
        fs::read(root.join("downloads/folder/extra")).unwrap(),
        b"keep"
    );
}

#[cfg(unix)]
fn check_guest_archive(path: &std::path::Path) {
    let mut archive = tar::Archive::new(flate2::read::GzDecoder::new(
        fs::File::open(path).unwrap(),
    ));
    let mut found = false;
    for entry in archive.entries().unwrap() {
        use std::io::Read;
        let mut entry = entry.unwrap();
        assert!(!entry.path().unwrap().starts_with("./target"));
        if entry.path().unwrap().ends_with("file") {
            let mut bytes = Vec::new();
            entry.read_to_end(&mut bytes).unwrap();
            assert_eq!(bytes, b"\x00\xff\x11");
            found = true;
        }
    }
    assert!(found);
}

#[cfg(unix)]
#[test]
fn vm_provision_selects_layers_and_preserves_failures() {
    use std::os::unix::fs::PermissionsExt;
    let directory = tempfile::tempdir().unwrap();
    let root = directory.path();
    for (name, script) in [
        (
            "docker",
            r#"#!/bin/sh
case "$1" in
    container) printf '%s %s\n' "$ID" "$STATE" ;;
    exec) test "$2" = "$ID" || exit 99; exit "$LAYERS" ;;
    cp) case "$2" in
            "$ID":/opt/platform-images/guest) cp -R guest "$3" ;;
            "$ID":/opt/platform-images/layers) cp -R layers "$3" ;;
            *) exit 98 ;;
        esac ;;
    *) exit 97 ;;
esac
"#,
        ),
        (
            "ssh",
            r#"#!/bin/sh
printf '%s\n' "$@" > connection
tar -tzf - > manifest || exit 96
exit "$INSTALL_STATUS"
"#,
        ),
    ] {
        let path = root.join(name);
        fs::write(&path, script).unwrap();
        fs::set_permissions(&path, fs::Permissions::from_mode(0o755)).unwrap();
    }
    fs::create_dir(root.join("guest")).unwrap();
    fs::create_dir(root.join("layers")).unwrap();
    fs::write(root.join("guest/provision.sh"), "#!/bin/sh\nexit 0\n").unwrap();
    fs::write(root.join("layers/base"), "selected layer").unwrap();
    let path = std::env::join_paths(
        std::iter::once(root.to_owned())
            .chain(std::env::split_paths(&std::env::var_os("PATH").unwrap())),
    )
    .unwrap();
    let run = |os, state, layers, result| {
        Command::new(env!("CARGO_BIN_EXE_foundation-ci"))
            .current_dir(root)
            .env("PATH", &path)
            .env("ID", "a".repeat(64))
            .env("STATE", state)
            .env("LAYERS", layers)
            .env("INSTALL_STATUS", result)
            .args([
                "task",
                "run",
                "images:vm-provision",
                "--os",
                os,
                "--key",
                "guest-key",
            ])
            .output()
            .unwrap()
    };
    assert!(run("macos", "running", "0", "0").status.success());
    let manifest = fs::read_to_string(root.join("manifest")).unwrap();
    assert!(manifest.contains("guest/provision.sh") && manifest.contains("guest/layers/base"));
    assert!(
        fs::read_to_string(root.join("connection"))
            .unwrap()
            .contains("127.0.0.1\n")
    );
    assert!(run("macos", "running", "44", "0").status.success());
    assert!(
        !fs::read_to_string(root.join("manifest"))
            .unwrap()
            .contains("layers")
    );
    assert_eq!(run("macos", "running", "36", "0").status.code(), Some(36));
    assert_eq!(run("macos", "running", "0", "42").status.code(), Some(42));
    assert!(!run("macos", "exited", "0", "0").status.success());
    assert!(!run("windows", "running", "0", "0").status.success());
}

#[cfg(unix)]
#[test]
fn vm_exec_writes_the_prologue_and_script_and_streams_the_result() {
    use std::os::unix::fs::PermissionsExt;
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path();
    let ssh = root.join("ssh");
    fs::write(
        &ssh,
        r#"#!/bin/sh
printf '<%s>' "$@" >> calls
printf '\n' >> calls
case "$*" in
  *'Set-Content -Path'*|*"cat > '"*) cat > script; exit "$WRITE" ;;
  *) exit "$RESULT" ;;
esac
"#,
    )
    .unwrap();
    fs::set_permissions(&ssh, fs::Permissions::from_mode(0o755)).unwrap();
    let path = std::env::join_paths(
        std::iter::once(root.to_owned())
            .chain(std::env::split_paths(&std::env::var_os("PATH").unwrap())),
    )
    .unwrap();
    let body = "echo one\necho two\n";
    fs::write(root.join("commands"), body).unwrap();
    let invoke = |os: &str, write: &str, result: &str| {
        Command::new(env!("CARGO_BIN_EXE_foundation-ci"))
            .current_dir(root)
            .env("PATH", &path)
            .env("WRITE", write)
            .env("RESULT", result)
            .args([
                "task",
                "run",
                "images:vm-exec",
                "--os",
                os,
                "--key",
                "guest",
                "--port",
                "2345",
                "--cwd",
                "work dir",
                "--script",
                "commands",
            ])
            .output()
            .unwrap()
    };
    for (os, remote, command, prologue) in [
        (
            "windows",
            "C:/platform-images/exec.ps1",
            "pwsh -NoProfile -File",
            "$ErrorActionPreference = 'Stop'\n$PSNativeCommandUseErrorActionPreference = $true\nSet-Location -LiteralPath 'work dir'\n",
        ),
        (
            "macos",
            "/tmp/platform-images-exec.sh",
            "bash -l",
            "set -euo pipefail\ncd 'work dir'\n",
        ),
    ] {
        let output = invoke(os, "0", "42");
        assert_eq!(
            output.status.code(),
            Some(42),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert_eq!(
            fs::read_to_string(root.join("script")).unwrap(),
            format!("{prologue}{body}")
        );
        let calls = fs::read_to_string(root.join("calls")).unwrap();
        assert!(calls.contains(remote), "{calls}");
        assert!(calls.contains(command), "{calls}");
        fs::remove_file(root.join("script")).unwrap();
        fs::remove_file(root.join("calls")).unwrap();
    }
}

#[cfg(unix)]
#[test]
fn vm_exec_reports_a_write_failure() {
    use std::os::unix::fs::PermissionsExt;
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path();
    let ssh = root.join("ssh");
    fs::write(
        &ssh,
        r#"#!/bin/sh
case "$*" in
  *'Set-Content -Path'*|*"cat > '"*) echo 'write denied' >&2; exit 3 ;;
  *) touch ran; exit 0 ;;
esac
"#,
    )
    .unwrap();
    fs::set_permissions(&ssh, fs::Permissions::from_mode(0o755)).unwrap();
    let path = std::env::join_paths(
        std::iter::once(root.to_owned())
            .chain(std::env::split_paths(&std::env::var_os("PATH").unwrap())),
    )
    .unwrap();
    fs::write(root.join("commands"), "echo one\n").unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_foundation-ci"))
        .current_dir(root)
        .env("PATH", &path)
        .args([
            "task",
            "run",
            "images:vm-exec",
            "--os",
            "macos",
            "--key",
            "guest",
            "--cwd",
            "/tmp",
            "--script",
            "commands",
        ])
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("write denied"));
    assert!(!root.join("ran").exists());
}

#[cfg(unix)]
#[test]
fn vm_runner_rejects_an_empty_jit_config() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path();
    for content in ["", "  \n\t\n"] {
        fs::write(root.join("jit"), content).unwrap();
        let output = Command::new(env!("CARGO_BIN_EXE_foundation-ci"))
            .current_dir(root)
            .args([
                "task",
                "run",
                "images:vm-runner",
                "--os",
                "macos",
                "--key",
                "guest",
                "--jitconfig-file",
                "jit",
            ])
            .output()
            .unwrap();
        assert!(!output.status.success());
        assert!(String::from_utf8_lossy(&output.stderr).contains("is empty"));
    }
}

#[cfg(unix)]
#[test]
fn vm_runner_reports_a_runner_that_exits_at_once() {
    use std::os::unix::fs::PermissionsExt;
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path();
    let ssh = root.join("ssh");
    fs::write(
        &ssh,
        r#"#!/bin/sh
printf '<%s>' "$@" >> calls
case "$*" in
  *pgrep*) exit 44 ;;
  *'tail -30'*) echo 'runner crashed at once' ;;
  *) exit 0 ;;
esac
"#,
    )
    .unwrap();
    fs::set_permissions(&ssh, fs::Permissions::from_mode(0o755)).unwrap();
    let path = std::env::join_paths(
        std::iter::once(root.to_owned())
            .chain(std::env::split_paths(&std::env::var_os("PATH").unwrap())),
    )
    .unwrap();
    fs::write(root.join("jit"), "encoded-config\n").unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_foundation-ci"))
        .current_dir(root)
        .env("PATH", &path)
        .args([
            "task",
            "run",
            "images:vm-runner",
            "--os",
            "macos",
            "--key",
            "guest",
            "--jitconfig-file",
            "jit",
            "--start-wait-ms",
            "0",
        ])
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("runner crashed at once"));
    let calls = fs::read_to_string(root.join("calls")).unwrap();
    assert!(calls.contains("nohup"), "{calls}");
    assert!(calls.contains("tail -30"), "{calls}");
}

#[cfg(unix)]
#[test]
fn vm_runner_distinguishes_an_ssh_failure() {
    use std::os::unix::fs::PermissionsExt;
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path();
    let ssh = root.join("ssh");
    fs::write(&ssh, "#!/bin/sh\nexit 255\n").unwrap();
    fs::set_permissions(&ssh, fs::Permissions::from_mode(0o755)).unwrap();
    let path = std::env::join_paths(
        std::iter::once(root.to_owned())
            .chain(std::env::split_paths(&std::env::var_os("PATH").unwrap())),
    )
    .unwrap();
    fs::write(root.join("jit"), "encoded-config\n").unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_foundation-ci"))
        .current_dir(root)
        .env("PATH", &path)
        .args([
            "task",
            "run",
            "images:vm-runner",
            "--os",
            "macos",
            "--key",
            "guest",
            "--jitconfig-file",
            "jit",
        ])
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(255));
    assert!(String::from_utf8_lossy(&output.stderr).contains("exited with status 255"));
}
