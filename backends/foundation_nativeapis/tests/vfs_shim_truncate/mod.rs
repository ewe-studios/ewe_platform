#![cfg(all(feature = "vfs-preload", target_os = "linux"))]

//! Integration tests for the `LD_PRELOAD` shim's truncate entry points.
//!
//! Tests:
//! 1. `truncate` rejects a negative length with `EINVAL` and keeps the file.
//! 2. `ftruncate` rejects a negative length with `EINVAL` and keeps the file.
//!
//! The shim reads `FOUNDATION_VFS_PREFIX` and `FOUNDATION_VFS_ROOT` once, at
//! its first call. The test sets both variables before that call. The target
//! holds one test, so no other test reads the environment at the same time.

use foundation_nativeapis::native::vfs::shim;

/// The size of a file on disk.
fn real_size(path: &std::path::Path) -> u64 {
    std::fs::metadata(path).expect("stat file").len()
}

#[test]
fn negative_truncate_length_is_rejected() {
    let dir = tempfile::TempDir::new().expect("create temp dir");
    let file = dir.path().join("file");
    std::fs::write(&file, b"four").expect("write the real file");

    // The shim reads the configuration once, at the first call below.
    std::env::set_var("FOUNDATION_VFS_PREFIX", "/virtual");
    std::env::set_var("FOUNDATION_VFS_ROOT", dir.path());

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
