//! Exercise the installed service controller with two independent projects.
#![cfg(unix)]
use std::{fs, os::unix::fs::PermissionsExt, path::Path, process::Command};

fn install_tool(root: &Path, name: &str, script: &str) {
    let path = root.join(name);
    fs::write(&path, script).unwrap();
    fs::set_permissions(path, fs::Permissions::from_mode(0o755)).unwrap();
}

fn run(root: &Path, tools: &Path, task: &str, status: i32) {
    let result = Command::new(env!("CARGO_BIN_EXE_foundation-ci"))
        .current_dir(root)
        .env("PATH", tools)
        .args([
            "task",
            "run",
            task,
            "--timeout-seconds",
            "15",
            "--poll-milliseconds",
            "20",
        ])
        .output()
        .unwrap();
    assert!(
        result.status.code() == Some(status),
        "{task}: {}",
        String::from_utf8_lossy(&result.stderr)
    );
}

#[test]
fn local_services_restart_and_stop_within_their_project() {
    let tools = tempfile::tempdir().unwrap();
    install_tool(
        tools.path(),
        "npx",
        "#!/bin/sh\nprintf '%s\\n' \"$$\" > child\nexec /bin/sleep 120\n",
    );
    install_tool(
        tools.path(),
        "curl",
        "#!/bin/sh\nread -r pid < child && kill -0 \"$pid\" || exit 7\nprintf 200\n",
    );
    let first = tempfile::tempdir().unwrap();
    let second = tempfile::tempdir().unwrap();
    let tasks = |dir: &Path, name| run(dir, tools.path(), name, 0);
    tasks(first.path(), "cf:start");
    tasks(second.path(), "cf:start");
    let is_alive = |pid: &str| {
        Command::new("/bin/kill")
            .args(["-0", pid.trim()])
            .output()
            .unwrap()
            .status
            .success()
    };
    let child = |dir: &Path| fs::read_to_string(dir.join("child")).unwrap();
    let old = child(first.path());
    let other = child(second.path());
    tasks(first.path(), "cf:start");
    let new = child(first.path());
    assert_ne!(old, new);
    assert!(!is_alive(&old));
    assert!(is_alive(&other));
    tasks(first.path(), "cf:stop");
    tasks(first.path(), "cf:stop");
    assert!(!is_alive(&new));
    assert!(is_alive(&other));
    tasks(second.path(), "cf:stop");
    assert!(!is_alive(&other));
    install_tool(
        tools.path(),
        "cargo",
        r#"#!/bin/sh
printf '%s|%s|%s|%s|%s\n' "$*" "$CF_INTEGRATION_TEST" "$LOCAL_CF_API_BASE" "$LOCAL_R2_BUCKET" "$LOCAL_D1_DATABASE_ID" >> tests-run
test ! -f fail || exit 37
"#,
    );
    tasks(first.path(), "cf:test:vfs");
    assert!(!is_alive(&child(first.path())));
    let calls = fs::read_to_string(first.path().join("tests-run")).unwrap();
    assert_eq!(calls.lines().count(), 2);
    assert!(calls.contains("--features vfs-native,vfs-r2 --test r2_delta_vfs"));
    assert!(calls.contains("--features vfs-native,vfs-d1 --test d1_delta_vfs"));
    assert!(calls.contains(
        "|1|http://127.0.0.1:8789/cdn-cgi/local/explorer/api|test-bucket-preview|test-db-preview"
    ));
    fs::write(first.path().join("fail"), []).unwrap();
    run(first.path(), tools.path(), "cf:test", 37);
    assert!(!is_alive(&child(first.path())));
    let calls = fs::read_to_string(first.path().join("tests-run")).unwrap();
    assert_eq!(calls.lines().count(), 3);
    assert!(calls.contains("--package foundation_db --test r2_blobstore_tests"));
}
