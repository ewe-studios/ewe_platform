#![cfg(all(feature = "vfs-preload", target_os = "linux"))]

use std::path::Path;
use std::process::Command;

/// Locate the `LD_PRELOAD` shim, building it if this run hasn't produced it yet.
///
/// WHY: the shim is the crate's `cdylib` artifact. `cargo test` builds the
/// `rlib` and the test binaries, never the `cdylib`, so a bare
/// `cargo test --test vfs_shim_vfs` finds no `.so` and the tests cannot run at
/// all. Depending on someone having run `cargo build --lib` first makes the
/// suite order-dependent and silently red on a clean checkout.
///
/// WHAT: the path to `libfoundation_nativeapis.so` in this profile's target dir.
///
/// HOW: look for it; if absent, invoke `cargo build --lib` for this crate,
/// feature and profile, then look again. The profile is recovered from the
/// target directory name, so `--profile uat` and `--release` both work.
fn find_shim_so() -> String {
    // Test binary lives at <target>/<profile>/deps/<bin>; the cdylib is one level up.
    let target_dir = std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(Path::to_path_buf)) // deps/
        .and_then(|p| p.parent().map(Path::to_path_buf)) // <profile>/
        .expect("could not find target directory");

    if let Some(so) = existing_shim(&target_dir) {
        return so;
    }

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
            "vfs-preload",
            "--profile",
            profile_flag,
            "--lib",
        ])
        .status()
        .expect("failed to run cargo build for the vfs-preload shim");
    assert!(status.success(), "cargo build --lib failed for the vfs-preload shim");

    existing_shim(&target_dir).unwrap_or_else(|| {
        panic!("shim .so still not found in {target_dir:?} after building the cdylib")
    })
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
    std::fs::write(&c_path, r#"
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
"#).unwrap();

    let status = std::process::Command::new("gcc")
        .arg(&c_path).arg("-o").arg(&exe_path)
        .status().unwrap();
    assert!(status.success(), "gcc failed");

    let output = std::process::Command::new(&exe_path)
        .env("LD_PRELOAD", &so)
        .env("FOUNDATION_VFS_PREFIX", "/virtual")
        .env("FOUNDATION_VFS_DELTA", "memory")
        .output()
        .expect("command failed");
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(output.status.success(), "exit: {:?}, stdout: {}, stderr: {}",
        output.status, stdout, String::from_utf8_lossy(&output.stderr));
    assert!(stdout.contains("hello"), "stdout: {}", stdout);

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
    assert!(!stdout.is_empty(), "should read real file, got: {}", stdout);
}
