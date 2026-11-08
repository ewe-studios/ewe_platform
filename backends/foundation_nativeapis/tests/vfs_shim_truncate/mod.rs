#![cfg(all(feature = "vfs-preload", target_os = "linux"))]

//! Integration tests for the `LD_PRELOAD` shim's truncate entry points.
//!
//! Tests:
//! 1. `truncate` rejects a negative length with `EINVAL` and keeps the file.
//! 2. `ftruncate` rejects a negative length with `EINVAL` and keeps the file.
//!
//! The shim reads `FOUNDATION_VFS_PREFIX` and `FOUNDATION_VFS_ROOT` once, at
//! its first call (`CONFIG` is a `LazyLock`). The shim's exported symbols also
//! interpose inside this test binary, so a file operation in the test body can
//! make that first call before the variables are set. The assertions therefore
//! run in a child process of this test binary, where the variables are in the
//! environment from process start.

use std::path::{Path, PathBuf};
use std::process::Command;

use foundation_nativeapis::native::vfs::shim;

/// Set in the child process; tells the test whether it is the parent or the child.
const CHILD_MARKER: &str = "FOUNDATION_VFS_SHIM_TRUNCATE_CHILD";

/// The size of a file on disk.
fn real_size(path: &Path) -> u64 {
    std::fs::metadata(path).expect("stat file").len()
}

#[test]
fn negative_truncate_length_is_rejected() {
    if std::env::var_os(CHILD_MARKER).is_some() {
        child_assertions();
        return;
    }

    let dir = tempfile::TempDir::new().expect("create temp dir");
    std::fs::write(dir.path().join("file"), b"four").expect("write the real file");

    // The child starts with both variables set, so its first shim call reads
    // the configuration the test needs.
    let exe = std::env::current_exe().expect("find the test binary");
    let output = Command::new(exe)
        .args([
            "--exact",
            "negative_truncate_length_is_rejected",
            "--nocapture",
        ])
        .env("FOUNDATION_VFS_PREFIX", "/virtual")
        .env("FOUNDATION_VFS_ROOT", dir.path())
        .env(CHILD_MARKER, "1")
        .output()
        .expect("run the child test");
    assert!(
        output.status.success(),
        "the child test failed ({:?})\nstdout:\n{}\nstderr:\n{}",
        output.status,
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

/// Assertions that need the shim variables in the process environment.
fn child_assertions() {
    let root = std::env::var("FOUNDATION_VFS_ROOT").expect("FOUNDATION_VFS_ROOT in the child");
    let file = PathBuf::from(root).join("file");

    let path = c"/virtual/file";

    // `truncate` with a negative length is `EINVAL`, and the file stays as is.
    // SAFETY: `path` is a valid NUL-terminated C string.
    let rc = unsafe { shim::truncate(path.as_ptr(), -1) };
    assert_eq!(rc, -1, "truncate with a negative length must fail");
    assert_eq!(
        std::io::Error::last_os_error().raw_os_error(),
        Some(libc::EINVAL),
        "truncate with a negative length must report EINVAL"
    );
    assert_eq!(real_size(&file), 4, "truncate must not change the file");

    // SAFETY: `path` is a valid NUL-terminated C string.
    let fd = unsafe { shim::open(path.as_ptr(), libc::O_RDWR, 0) };
    assert!(
        fd >= 10_000,
        "open of a virtual path must return a virtual fd, got {fd}"
    );

    // `ftruncate` with a negative length is `EINVAL`, and the file stays as is.
    // SAFETY: `fd` is the open virtual descriptor returned above.
    let rc = unsafe { shim::ftruncate(fd, -1) };
    assert_eq!(rc, -1, "ftruncate with a negative length must fail");
    assert_eq!(
        std::io::Error::last_os_error().raw_os_error(),
        Some(libc::EINVAL),
        "ftruncate with a negative length must report EINVAL"
    );

    // SAFETY: `fd` is the open virtual descriptor returned above.
    let rc = unsafe { shim::close(fd) };
    assert_eq!(rc, 0, "close of a virtual fd must succeed");
    assert_eq!(real_size(&file), 4, "ftruncate must not change the file");
}
