//! The file scenarios: the atomic write, the spans, the end of an upload and
//! the reads.

use std::time::Duration;

use proptest::prelude::*;
use proptest::test_runner::RngSeed;

use foundation_sandbox::shared::error::SlaveErrorCode;

use super::support::{slave_code, SlaveProcess, TempDir};

/// The bytes of a file below the root of a prepared slave.
fn root_file(root: &TempDir, guest: &str) -> std::path::PathBuf {
    root.join(guest.trim_start_matches('/'))
}

/// Makes the directory of a guest path below the root.
fn make_parent(root: &TempDir, guest: &str) {
    let path = root_file(root, guest);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).expect("the parent directory");
    }
}

/// A file write is atomic.
#[test]
fn file_atomic() {
    let (slave, root) = SlaveProcess::start_prepared("file-atomic");
    make_parent(&root, "/tmp/f");
    std::fs::write(root_file(&root, "/tmp/f"), b"old").expect("the old file");

    let upload = slave
        .caller
        .file_write_begin("/tmp/f", 0o644)
        .expect("the upload starts");
    slave
        .caller
        .file_write_chunk(&upload.upload_id, 0, b"new".to_vec())
        .expect("the chunk");
    assert_eq!(
        std::fs::read(root_file(&root, "/tmp/f")).expect("the old file"),
        b"old"
    );
    slave
        .caller
        .file_write_commit(&upload.upload_id, 3)
        .expect("the commit");
    assert_eq!(
        std::fs::read(root_file(&root, "/tmp/f")).expect("the new file"),
        b"new"
    );
}

/// A chunk is applied one time.
#[test]
fn file_chunk_duplicate() {
    let (slave, root) = SlaveProcess::start_prepared("file-duplicate");
    make_parent(&root, "/tmp/f");
    let upload = slave
        .caller
        .file_write_begin("/tmp/f", 0o600)
        .expect("the upload starts");
    let first = slave
        .caller
        .file_write_chunk(&upload.upload_id, 0, b"new".to_vec())
        .expect("the chunk");
    assert_eq!(first.next_offset, 3);
    let again = slave
        .caller
        .file_write_chunk(&upload.upload_id, 0, b"new".to_vec())
        .expect("the repeat");
    assert_eq!(again.next_offset, 3);
    slave
        .caller
        .file_write_commit(&upload.upload_id, 3)
        .expect("the commit");
    assert_eq!(
        std::fs::read(root_file(&root, "/tmp/f")).expect("the file"),
        b"new"
    );
}

/// A gap is refused and the upload stays.
#[test]
fn file_chunk_gap() {
    let (slave, root) = SlaveProcess::start_prepared("file-gap");
    make_parent(&root, "/tmp/f");
    let upload = slave
        .caller
        .file_write_begin("/tmp/f", 0o600)
        .expect("the upload starts");
    let error = slave
        .caller
        .file_write_chunk(&upload.upload_id, 3, b"x".to_vec())
        .expect_err("the gap is refused");
    assert!(matches!(
        super::support::sandbox_error(&error),
        foundation_sandbox::shared::error::SandboxError::SequenceGap
    ));
    // The upload still accepts the first span.
    let answer = slave
        .caller
        .file_write_chunk(&upload.upload_id, 0, b"x".to_vec())
        .expect("the first span");
    assert_eq!(answer.next_offset, 1);
    let _ = root;
}

/// A wrong commit size is refused, and the upload is gone.
#[test]
fn file_commit_wrong_size() {
    let (slave, root) = SlaveProcess::start_prepared("file-wrong-size");
    make_parent(&root, "/tmp/f");
    let upload = slave
        .caller
        .file_write_begin("/tmp/f", 0o600)
        .expect("the upload starts");
    slave
        .caller
        .file_write_chunk(&upload.upload_id, 0, vec![b'x'; 10])
        .expect("the chunk");
    let error = slave
        .caller
        .file_write_commit(&upload.upload_id, 9)
        .expect_err("the size is wrong");
    assert_eq!(
        slave_code(&error).map(|(code, _)| code),
        Some(SlaveErrorCode::BadRequest.to_string())
    );
    assert!(
        !root_file(&root, "/tmp/f").exists(),
        "no file at the target"
    );
    let error = slave
        .caller
        .file_write_chunk(&upload.upload_id, 10, vec![b'y'])
        .expect_err("the upload is gone");
    assert_eq!(
        slave_code(&error).map(|(code, _)| code),
        Some(SlaveErrorCode::NotFound.to_string())
    );
}

/// An abort removes the upload, and a second abort is accepted.
#[test]
fn file_abort() {
    let (slave, root) = SlaveProcess::start_prepared("file-abort");
    make_parent(&root, "/tmp/f");
    let upload = slave
        .caller
        .file_write_begin("/tmp/f", 0o600)
        .expect("the upload starts");
    slave
        .caller
        .file_write_chunk(&upload.upload_id, 0, b"data".to_vec())
        .expect("the chunk");
    slave
        .caller
        .file_write_abort(&upload.upload_id)
        .expect("the abort");
    slave
        .caller
        .file_write_abort(&upload.upload_id)
        .expect("the second abort");
    assert!(!root_file(&root, "/tmp/f").exists());
    assert!(
        std::fs::read_dir(root.join("tmp"))
            .expect("the directory")
            .next()
            .is_none(),
        "no temporary file stays"
    );
}

/// An idle upload expires.
#[test]
fn file_expire() {
    let root = TempDir::new("file-expire-root");
    let slave = SlaveProcess::start("file-expire", Some(root.path().to_path_buf()), 4);
    let mut prepare = super::support::plain_prepare("/work");
    prepare.limits.upload_idle_timeout_ms = 200;
    slave.prepare(&prepare);
    make_parent(&root, "/tmp/f");
    let upload = slave
        .caller
        .file_write_begin("/tmp/f", 0o600)
        .expect("the upload starts");
    slave
        .caller
        .file_write_chunk(&upload.upload_id, 0, b"data".to_vec())
        .expect("the chunk");
    std::thread::sleep(Duration::from_millis(300));
    let error = slave
        .caller
        .file_write_chunk(&upload.upload_id, 4, b"more".to_vec())
        .expect_err("the upload expired");
    assert_eq!(
        slave_code(&error).map(|(code, _)| code),
        Some(SlaveErrorCode::NotFound.to_string())
    );
    assert!(
        std::fs::read_dir(root.join("tmp"))
            .expect("the directory")
            .next()
            .is_none(),
        "no temporary file stays"
    );
}

/// A file is read in order.
#[test]
fn file_read_roundtrip() {
    let (slave, root) = SlaveProcess::start_prepared("file-read");
    for size in [0_usize, 1, 200_000] {
        let guest = format!("/tmp/f{size}");
        make_parent(&root, &guest);
        let content: Vec<u8> = (0..size)
            .map(|index| u8::try_from(index % 251).expect("a byte value"))
            .collect();
        std::fs::write(root_file(&root, &guest), &content).expect("the file");
        let chunks = slave.caller.file_read(&guest).expect("the read");
        assert!(!chunks.is_empty(), "at least one chunk");
        let mut offset = 0_u64;
        let mut bytes = Vec::new();
        for (index, chunk) in chunks.iter().enumerate() {
            assert_eq!(chunk.offset, offset, "the offsets grow without a gap");
            offset += chunk.data.len() as u64;
            bytes.extend_from_slice(&chunk.data);
            assert_eq!(
                chunk.eof,
                index + 1 == chunks.len(),
                "only the last chunk has eof"
            );
        }
        assert_eq!(bytes, content, "the data of {size} bytes");
    }
}

/// A directory is refused with `BadRequest`.
#[test]
fn file_read_directory() {
    let (slave, root) = SlaveProcess::start_prepared("file-directory");
    std::fs::create_dir_all(root.join("tmp")).expect("the directory");
    let error = slave
        .caller
        .file_read("/tmp")
        .expect_err("a directory is refused");
    assert_eq!(
        slave_code(&error),
        Some((SlaveErrorCode::BadRequest.to_string(), Some(21)))
    );
}

/// A path cannot leave the root.
#[test]
fn path_escape() {
    let (slave, _root) = SlaveProcess::start_prepared("path-escape");
    for path in ["/../etc/passwd", "/a/../../x", "relative", "/a/../.."] {
        let error = slave
            .caller
            .file_read(path)
            .expect_err("the path is refused");
        assert_eq!(
            slave_code(&error).map(|(code, _)| code),
            Some(SlaveErrorCode::BadRequest.to_string()),
            "the path {path}"
        );
    }
}

/// A symbolic link resolves inside the root.
#[test]
fn symlink_resolves_in_root() {
    let (slave, root) = SlaveProcess::start_prepared("symlink");
    std::os::unix::fs::symlink("/etc/passwd", root.join("l")).expect("the link");
    let error = slave
        .caller
        .file_read("/l")
        .expect_err("the link resolves inside the root");
    assert_eq!(
        slave_code(&error).map(|(code, _)| code),
        Some(SlaveErrorCode::NotFound.to_string())
    );
}

proptest! {
    #![proptest_config(ProptestConfig {
        rng_seed: RngSeed::Fixed(0x5eed_0003),
        cases: 6,
        .. ProptestConfig::default()
    })]

    /// A random series of upload calls matches a model of the file content.
    #[test]
    fn upload_model(
            parts in proptest::collection::vec(proptest::collection::vec(any::<u8>(), 1..6), 1..8),
            repeat_last in any::<bool>(),
            abort in any::<bool>(),
        ) {
            let (slave, root) = SlaveProcess::start_prepared("upload-model");
            make_parent(&root, "/tmp/model");
            let upload = slave
                .caller
                .file_write_begin("/tmp/model", 0o600)
                .expect("the upload starts");
            let mut model = Vec::new();
            let mut offset = 0_u64;
            for part in &parts {
                let before = model.len() as u64;
                let answer = slave
                    .caller
                    .file_write_chunk(&upload.upload_id, offset, part.clone())
                    .expect("the chunk");
                model.extend_from_slice(part);
                offset += part.len() as u64;
                prop_assert_eq!(answer.next_offset, offset);
                if repeat_last {
                    let answer = slave
                        .caller
                        .file_write_chunk(&upload.upload_id, before, part.clone())
                        .expect("the repeat");
                    prop_assert_eq!(answer.next_offset, offset);
                }
            }
        if abort {
            slave.caller.file_write_abort(&upload.upload_id).expect("the abort");
            prop_assert!(!root_file(&root, "/tmp/model").exists());
        } else {
            slave
                .caller
                .file_write_commit(&upload.upload_id, offset)
                .expect("the commit");
            prop_assert_eq!(
                std::fs::read(root_file(&root, "/tmp/model")).expect("the file"),
                model
            );
        }
    }
}
