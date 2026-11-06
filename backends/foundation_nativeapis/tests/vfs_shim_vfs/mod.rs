#![cfg(all(feature = "vfs-preload", target_os = "linux"))]

use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::OnceLock;

/// Locate the `LD_PRELOAD` shim, building it from this crate's source first.
///
/// WHY: the shim is the crate's `cdylib` artifact. `cargo test` builds the
/// `rlib` and the test binaries, never the `cdylib`, so a bare
/// `cargo test --test vfs_shim_vfs` finds no `.so` and the tests cannot run at
/// all. Depending on someone having run `cargo build --lib` first makes the
/// suite order-dependent and silently red on a clean checkout.
///
/// WHY ALWAYS BUILD: a `.so` in the profile directory may come from an earlier
/// build with a different feature set. Returning it without asking cargo
/// produced a shim without `vfs-preload` interposition, so the child `open`
/// failed and the test died with a bare `ExitStatus(256)`. Cargo rebuilds when
/// the feature set differs from its last build, and does nothing when it does
/// not, so the call is cheap in the common case and correct in the stale one.
///
/// WHAT: the path to `libfoundation_nativeapis.so` in this profile's target dir.
///
/// HOW: build the `cdylib` once per test run (all tests share the result), then
/// look for it. The profile is recovered from the target directory name, so
/// `--profile uat` and `--release` both work.
fn find_shim_so() -> String {
    static SHIM: OnceLock<String> = OnceLock::new();
    SHIM.get_or_init(|| {
        let target_dir = profile_dir();
        build_shim(&target_dir, shim_features());
        existing_shim(&target_dir).unwrap_or_else(|| {
            panic!(
                "shim .so not found in {} after building the cdylib",
                target_dir.display()
            )
        })
    })
    .clone()
}

/// The features the `cdylib` needs: `vfs-preload` always, plus `vfs-d1` when
/// this test binary carries it, so the D1 delta is inside the shim too.
/// `vfs-d1` is a superset of `vfs-preload`, and the shim selects the delta
/// backend from `FOUNDATION_VFS_DELTA` at run time, so one wider shim serves
/// every test here.
fn shim_features() -> &'static str {
    if cfg!(feature = "vfs-d1") {
        "vfs-preload,vfs-d1"
    } else {
        "vfs-preload"
    }
}

/// Build the `cdylib` for `features` into `target_dir`'s profile.
fn build_shim(target_dir: &Path, features: &str) {
    let profile = target_dir
        .file_name()
        .and_then(|s| s.to_str())
        .expect("target directory has no name");
    // `cargo build` spells the dev profile "debug" on disk.
    let profile_flag = if profile == "debug" { "dev" } else { profile };

    let manifest_dir = env!("CARGO_MANIFEST_DIR");
    let status = Command::new(std::env::var("CARGO").unwrap_or_else(|_| "cargo".into()))
        .current_dir(manifest_dir)
        .args([
            "build",
            "-p",
            "foundation_nativeapis",
            "--features",
            features,
            "--profile",
            profile_flag,
            "--lib",
        ])
        .status()
        .expect("failed to run cargo build for the shim");
    assert!(
        status.success(),
        "cargo build --lib failed for the shim with features {features}"
    );
}

/// The `<target>/<profile>` directory that holds this run's artifacts.
///
/// WHY: the artifact layout is not fixed. With cargo's new build-directory
/// layout a test binary runs from
/// `<target>/<profile>/build/<package>/<hash>/out/<binary>`, so the
/// executable's grandparent is `<hash>`, not the profile directory. Deriving
/// the profile from it passed the hash to `--profile` and the build failed with
/// `profile '<hash>' is not defined`.
///
/// WHAT: the nearest ancestor of the running executable that holds cargo's
/// `.cargo-lock`, which cargo writes directly into the profile directory in
/// every layout.
///
/// HOW: walk the executable's ancestors. Panic with the executable path when no
/// ancestor holds the lock file, so the cause is readable rather than a build
/// error about a profile name.
fn profile_dir() -> PathBuf {
    let exe = std::env::current_exe().expect("could not find the test executable");
    exe.ancestors()
        .skip(1) // the executable itself
        .find(|dir| dir.join(".cargo-lock").is_file())
        .map_or_else(
            || {
                panic!(
                    "no target profile directory above {}: it must hold `.cargo-lock`",
                    exe.display()
                )
            },
            Path::to_path_buf,
        )
}

fn existing_shim(target_dir: &Path) -> Option<String> {
    let entries = std::fs::read_dir(target_dir).ok()?;
    for entry in entries.flatten() {
        let name = entry.file_name();
        let name = name.to_string_lossy();
        if name.starts_with("libfoundation_nativeapis") && name.ends_with(".so") {
            return Some(entry.path().to_string_lossy().into_owned());
        }
    }
    None
}

#[test]
fn test_shim_writes_to_virtual_path() {
    let so = find_shim_so();
    let tmpdir = std::env::temp_dir();
    let c_path = tmpdir.join("shim_test.c");
    let exe_path = tmpdir.join("shim_test");
    std::fs::write(
        &c_path,
        r#"
#include <stdio.h>
#include <fcntl.h>
#include <unistd.h>
int main() {
    int fd = open("/virtual/test.txt", O_WRONLY|O_CREAT|O_TRUNC, 0644);
    if (fd < 0) return 1;
    write(fd, "hello\n", 6);
    close(fd);
    fd = open("/virtual/test.txt", O_RDONLY);
    if (fd < 0) return 2;
    char buf[32];
    int n = read(fd, buf, sizeof(buf)-1);
    buf[n] = 0;
    printf("%s", buf);
    close(fd);
    return 0;
}
"#,
    )
    .unwrap();

    let status = std::process::Command::new("gcc")
        .arg(&c_path)
        .arg("-o")
        .arg(&exe_path)
        .status()
        .unwrap();
    assert!(status.success(), "gcc failed");

    let output = std::process::Command::new(&exe_path)
        .env("LD_PRELOAD", &so)
        .env("FOUNDATION_VFS_PREFIX", "/virtual")
        .env("FOUNDATION_VFS_DELTA", "memory")
        .output()
        .expect("command failed");
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        output.status.success(),
        "exit: {:?}, stdout: {}, stderr: {}",
        output.status,
        stdout,
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(stdout.contains("hello"), "stdout: {stdout}");

    let _ = std::fs::remove_file(&c_path);
    let _ = std::fs::remove_file(&exe_path);
}

#[test]
fn test_shim_real_path_passthrough() {
    let so = find_shim_so();
    let output = Command::new("/bin/sh")
        .env("LD_PRELOAD", &so)
        .env("FOUNDATION_VFS_PREFIX", "/virtual")
        .env("FOUNDATION_VFS_DELTA", "memory")
        .arg("-c")
        .arg("cat /etc/hostname")
        .output()
        .expect("command failed");
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(!stdout.is_empty(), "should read real file, got: {stdout}");
}

/// The local Cloudflare emulator endpoint and database id, when reachable.
/// Mirrors the probe in `tests/d1_delta_vfs`; the D1 shim test skips without it.
#[cfg(feature = "vfs-d1")]
fn local_d1() -> Option<(String, String)> {
    if std::env::var("CF_INTEGRATION_TEST").ok().as_deref() != Some("1") {
        return None;
    }
    let base = std::env::var("LOCAL_CF_API_BASE")
        .unwrap_or_else(|_| "http://localhost:8789/cdn-cgi/local/explorer/api".to_string());
    let db_id =
        std::env::var("LOCAL_D1_DATABASE_ID").unwrap_or_else(|_| "test-db-preview".to_string());
    let up = Command::new("curl")
        .args([
            "-s",
            "-o",
            "/dev/null",
            "-w",
            "%{http_code}",
            &format!("{base}/d1/database"),
        ])
        .output()
        .ok()
        .map(|o| {
            let code = String::from_utf8_lossy(&o.stdout);
            code.trim() == "200"
        })
        .unwrap_or(false);
    if up {
        // A real D1 database does not create the delta's table on demand.
        let body = r#"{"sql":"CREATE TABLE IF NOT EXISTS vfs_kv (key TEXT PRIMARY KEY, value TEXT NOT NULL, created_at INTEGER DEFAULT (strftime('%s', 'now') * 1000), updated_at INTEGER DEFAULT (strftime('%s', 'now') * 1000))"}"#;
        let created = Command::new("curl")
            .args(["-s", "-o", "/dev/null", "-w", "%{http_code}", "-X", "POST"])
            .args(["-H", "Content-Type: application/json", "-d", body])
            .arg(format!("{base}/d1/database/{db_id}/raw"))
            .output()
            .expect("curl");
        assert_eq!(
            String::from_utf8_lossy(&created.stdout).trim(),
            "200",
            "create vfs_kv"
        );
        Some((base, db_id))
    } else {
        None
    }
}

/// A write through the shim with `FOUNDATION_VFS_DELTA=d1` lands in D1.
#[cfg(feature = "vfs-d1")]
#[test]
fn shim_config_d1_uses_d1_delta() {
    let Some((base, db_id)) = local_d1() else {
        return;
    };
    let _guard = foundation_core::valtron::initialize_pool(42, Some(3));
    let so = find_shim_so();

    let key = format!("/d1_shim_{}.txt", std::process::id());
    let vpath = format!("/virtual{key}");

    let tmpdir = std::env::temp_dir();
    let c_path = tmpdir.join("shim_d1_test.c");
    let exe_path = tmpdir.join("shim_d1_test");
    std::fs::write(
        &c_path,
        r#"
#include <fcntl.h>
#include <unistd.h>
int main(int argc, char **argv) {
    if (argc < 2) return 1;
    int fd = open(argv[1], O_WRONLY|O_CREAT|O_TRUNC, 0644);
    if (fd < 0) return 2;
    const char *msg = "shim-to-d1\n";
    if (write(fd, msg, 11) != 11) return 3;
    close(fd);
    return 0;
}
"#,
    )
    .unwrap();

    let status = Command::new("gcc")
        .arg(&c_path)
        .arg("-o")
        .arg(&exe_path)
        .status()
        .unwrap();
    assert!(status.success(), "gcc failed");

    let output = Command::new(&exe_path)
        .arg(&vpath)
        .env("LD_PRELOAD", &so)
        .env("FOUNDATION_VFS_PREFIX", "/virtual")
        .env("FOUNDATION_VFS_DELTA", "d1")
        .env("LOCAL_CF_API_BASE", &base)
        .env("LOCAL_D1_DATABASE_ID", &db_id)
        .env("CF_API_TOKEN", "test-token")
        .env("CF_ACCOUNT_ID", "test-account")
        .output()
        .expect("command failed");
    assert!(
        output.status.success(),
        "exit: {:?}, stderr: {}",
        output.status,
        String::from_utf8_lossy(&output.stderr)
    );

    use foundation_nativeapis::native::vfs::d1_delta::D1Delta;
    use foundation_nativeapis::shared::vfs::sync_bridge::SyncFs;
    use foundation_nativeapis::shared::vfs::VfsFileSystem;
    let delta = D1Delta::new_with_base_url("test-token", "test-account", &db_id, Some(&base))
        .expect("D1Delta::new_with_base_url");
    let read = SyncFs::new(delta)
        .read_file(&key)
        .unwrap_or_else(|e| panic!("read back failed: {e}"));
    assert_eq!(String::from_utf8_lossy(&read).as_ref(), "shim-to-d1\n");

    let _ = std::fs::remove_file(&c_path);
    let _ = std::fs::remove_file(&exe_path);
}
