//! The fault table of section 6.8 (feature 12): fill an image.
//!
//! Four rows run without a virtual machine: the missing `mkfs.ext4`, a start
//! that fails, an entry that fails and a stream that ends early. A fifth row,
//! the work-file sweep, is `store::work_files_expire`. The unmount row and the
//! host death inside a guest need the micro-virtual machine (VM) tests.

use std::sync::Arc;

use foundation_sandbox::native::images::ImageStore;
use foundation_sandbox::shared::control::testing::{FakeHostFs, ScriptedSlave, SimDriver};
use foundation_sandbox::shared::control::SandboxProvider;
use foundation_sandbox::shared::error::SandboxError;
use foundation_sandbox::shared::host::{HostConfig, SandboxHost};
use foundation_sandbox::shared::limits::{ArtifactLimits, ImageLimits};
use foundation_sandbox::shared::spec::{GuestArch, ImageRef, RootFs, SandboxSpec};
use foundation_sandbox::shared::unpack::UnpackOptions;

use super::support;

const BLOCK: usize = 512;

/// An image store over an in-memory host with no `mkfs.ext4`.
fn fake_store(executable: Option<&str>) -> ImageStore {
    let (ctx, _clock) = crate::control::support::ctx(171);
    let fs = FakeHostFs::new();
    let config = HostConfig {
        state_dir: "/state".into(),
        cache_dir: "/cache".into(),
        slave_path: None,
        firecracker_path: None,
        krun_runner_path: None,
        libkrun_path: None,
        mkfs_ext4_path: executable.map(std::path::PathBuf::from),
        driver_order: Vec::new(),
        artifact_limits: ArtifactLimits::default(),
        image_limits: ImageLimits::default(),
    };
    let host = SandboxHost {
        processes: Arc::new(
            foundation_sandbox::shared::control::testing::FakeProcessSpawner::new(),
        ),
        fs: Arc::new(fs),
        connector: Arc::new(support::NoConnector),
        config: Arc::new(config),
    };
    ImageStore::new(ctx, host)
}

#[test]
fn mkfs_fault() {
    // `mkfs.ext4` missing: no file, and `BinaryNotFound`.
    let store = fake_store(Some("/missing/mkfs.ext4"));
    let error = crate::pool::block_on(store.create_blank(16)).expect_err("no mkfs program");
    assert!(matches!(
        error.current_context(),
        SandboxError::BinaryNotFound { .. }
    ));
}

#[test]
fn sandbox_start_fault() {
    let _serial = crate::control::support::serial();
    let (ctx, _clock) = crate::control::support::ctx(172);
    let work = support::TempDir::new("start-fault");
    let work_file = work.path().join("work.ext4");
    std::fs::write(&work_file, b"work").expect("the work file");
    let driver = Arc::new(SimDriver::new(support::driver_id()).with_launch_error(
        SandboxError::Io {
            op: "start".to_string(),
            message: "the substrate did not start".to_string(),
        },
    ));
    let slave = Arc::new(ScriptedSlave::new());
    let resolver = Arc::new(support::FixedResolver {
        rootfs: work_file.clone(),
        manifest: support::manifest(GuestArch::host()),
    });
    let manager = support::image_manager(ctx, driver, slave, resolver);
    let spec = SandboxSpec::new(RootFs::Image(ImageRef::Tag("tool".to_string())));
    let error = crate::control::support::expect_error(
        crate::pool::block_on(manager.create(spec)),
        "a sandbox that does not start",
    );
    assert!(matches!(error, SandboxError::Io { .. }));
    assert!(work_file.exists(), "the tool still holds the work file");
}

#[test]
fn entry_fault() {
    // An entry that cannot be applied reports its path and `errno`.
    let dir = support::TempDir::new("entry-fault");
    let mut archive = Vec::new();
    archive.extend_from_slice(&header("a", 0o644, 4, b'0', ""));
    archive.extend_from_slice(b"file");
    archive.extend(std::iter::repeat_n(0_u8, BLOCK - 4));
    archive.extend_from_slice(&header("a/b", 0o644, 7, b'0', ""));
    archive.extend_from_slice(b"blocked");
    archive.extend(std::iter::repeat_n(0_u8, BLOCK - 7));
    archive.extend(std::iter::repeat_n(0_u8, BLOCK * 2));
    let error = foundation_sandbox::native::unpack::unpack(
        archive.as_slice(),
        dir.path(),
        &UnpackOptions::default(),
        &foundation_sandbox::native::unpack::UnpackLimits::default(),
    )
    .expect_err("the entry under a regular file fails");
    match error {
        foundation_sandbox::native::unpack::UnpackError::Entry { path, errno, .. } => {
            assert_eq!(path, "a/b", "the error names the tar entry");
            assert!(
                errno.is_some(),
                "the error carries the operating system number"
            );
        }
        other => panic!("the error is an entry error: {other:?}"),
    }
}

#[test]
fn stream_fault() {
    let _serial = crate::control::support::serial();
    let (ctx, _clock) = crate::control::support::ctx(173);
    let driver = Arc::new(SimDriver::new(support::driver_id()));
    let slave = Arc::new(ScriptedSlave::new());
    let resolver = Arc::new(support::FixedResolver {
        rootfs: std::path::PathBuf::from("/images/tool.ext4"),
        manifest: support::manifest(GuestArch::host()),
    });
    let manager = support::image_manager(ctx, driver, slave, resolver);
    let spec = SandboxSpec::new(RootFs::Image(ImageRef::Tag("tool".to_string())));
    let handle = crate::pool::block_on(manager.create(spec)).expect("the sandbox");
    let tar = futures::stream::iter(vec![
        Ok(vec![0_u8; 8]),
        Err(SandboxError::Io {
            op: "the tool stream".to_string(),
            message: "the stream ends early".to_string(),
        }
        .into()),
    ]);
    let error = crate::control::support::expect_error(
        crate::pool::block_on(handle.unpack("/", tar, UnpackOptions::default())),
        "a stream that ends early",
    );
    assert!(matches!(
        error,
        SandboxError::Io { ref message, .. } if message.contains("ends early")
    ));
}

/// One ustar header.
fn header(name: &str, mode: u32, size: u64, typeflag: u8, link: &str) -> [u8; BLOCK] {
    let mut block = [0_u8; BLOCK];
    put(&mut block[0..100], name.as_bytes());
    put(&mut block[100..108], &octal(u64::from(mode), 8));
    // SAFETY: `geteuid` and `getegid` read the process credential and have no
    // side effect. The current owner can set the same owner on the test files.
    let uid = unsafe { libc::geteuid() };
    // SAFETY: as above.
    let gid = unsafe { libc::getegid() };
    put(&mut block[108..116], &octal(u64::from(uid), 8));
    put(&mut block[116..124], &octal(u64::from(gid), 8));
    put(&mut block[124..136], &octal(size, 12));
    put(&mut block[136..148], &octal(0, 12));
    block[148..156].fill(b' ');
    block[156] = typeflag;
    put(&mut block[157..257], link.as_bytes());
    block[257..263].copy_from_slice(b"ustar\0");
    block[263..265].copy_from_slice(b"00");
    let checksum: u64 = block.iter().map(|byte| u64::from(*byte)).sum();
    put(&mut block[148..156], &octal(checksum, 7));
    block[155] = b' ';
    block
}

fn put(field: &mut [u8], bytes: &[u8]) {
    let length = field.len().min(bytes.len());
    field[..length].copy_from_slice(&bytes[..length]);
}

fn octal(value: u64, width: usize) -> Vec<u8> {
    let digits = format!("{value:0width$o}", width = width - 1);
    let mut bytes = digits.into_bytes();
    bytes.push(0);
    bytes
}
