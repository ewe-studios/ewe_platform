//! The `newc` archive tests.

use std::path::PathBuf;

use foundation_sandbox::native::artifacts::cpio::{initramfs_entries, write_newc, CpioEntry};

use super::support;

/// The fixed input of the golden file.
const GOLDEN_INIT: &[u8] = b"#!/bin/sh\n";

/// The fixed entries of the golden file.
fn golden_entries() -> [CpioEntry<'static>; 5] {
    initramfs_entries(GOLDEN_INIT)
}

/// The golden file of this crate.
fn golden_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/artifacts/initramfs-golden.newc")
}

#[test]
fn cpio_golden() {
    let mut bytes = Vec::new();
    write_newc(&golden_entries(), &mut bytes).expect("the archive is written");
    let golden = std::fs::read(golden_path()).expect("the golden file exists");
    assert_eq!(bytes, golden, "the newc output changed");
}

#[test]
fn cpio_is_deterministic() {
    let mut first = Vec::new();
    let mut second = Vec::new();
    write_newc(&golden_entries(), &mut first).expect("the first archive is written");
    write_newc(&golden_entries(), &mut second).expect("the second archive is written");
    assert_eq!(first, second);
    assert_eq!(first.len() % 512, 0, "the archive ends on a 512 block");
}

#[test]
fn cpio_read_by_gnu_cpio() {
    let available = std::process::Command::new("cpio")
        .arg("--version")
        .output()
        .is_ok();
    if !available {
        eprintln!("skipping: the cpio program is not installed");
        return;
    }
    let directory = support::TempDir::new("cpio-gnu");
    let archive = directory.path().join("initramfs.cpio");
    let mut bytes = Vec::new();
    write_newc(&golden_entries(), &mut bytes).expect("the archive is written");
    std::fs::write(&archive, &bytes).expect("the archive is on disk");
    let output = std::process::Command::new("cpio")
        .arg("-id")
        .current_dir(directory.path())
        .stdin(std::fs::File::open(&archive).expect("the archive opens"))
        .output()
        .expect("the cpio program starts");
    assert!(
        output.status.success(),
        "cpio -id failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    for name in ["dev", "proc", "sys", "run"] {
        assert!(
            directory.path().join(name).is_dir(),
            "cpio did not make {name}"
        );
    }
    let init = std::fs::read(directory.path().join("init")).expect("cpio wrote init");
    assert_eq!(init, GOLDEN_INIT);
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mode = std::fs::metadata(directory.path().join("init"))
            .expect("init metadata")
            .permissions()
            .mode();
        assert_eq!(mode & 0o777, 0o755, "cpio did not keep the file mode");
    }
}

#[test]
fn cpio_headers_follow_the_rules() {
    let entries = golden_entries();
    let mut bytes = Vec::new();
    write_newc(&entries, &mut bytes).expect("the archive is written");
    // The first entry: magic, inode 1, mode 040755, link count 2, time 0,
    // name size 4 for "dev\0".
    let header = std::str::from_utf8(&bytes[..110]).expect("the header is ASCII");
    assert_eq!(&header[..6], "070701");
    assert_eq!(&header[6..14], "00000001", "the inode is the entry index");
    assert_eq!(&header[14..22], "000041ED", "the directory mode");
    assert_eq!(&header[38..46], "00000002", "a directory has two links");
    assert_eq!(&header[46..54], "00000000", "the modification time is 0");
    assert_eq!(
        &header[94..102],
        "00000004",
        "the name size includes the zero"
    );
}

#[test]
fn cpio_rejects_a_name_with_a_zero_byte() {
    let entries = [CpioEntry {
        path: "a\0b",
        mode: 0o100_755,
        data: b"x",
    }];
    let mut bytes = Vec::new();
    let error = write_newc(&entries, &mut bytes).expect_err("the name is rejected");
    assert_eq!(error.kind(), std::io::ErrorKind::InvalidInput);
}
