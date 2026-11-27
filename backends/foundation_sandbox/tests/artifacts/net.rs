//! The network tests of the pinned artifacts.
//!
//! Each test returns with a message when the network is not available. They
//! download the real pinned files and check the real hashes.

use std::io::Read;

use foundation_sandbox::native::artifacts::manifest::{alpine_minirootfs, firecracker_ci_rootfs};
use foundation_sandbox::native::artifacts::{ArtifactLimits, ArtifactStore};
use foundation_sandbox::shared::spec::GuestArch;

use super::support;

/// A store for a real download.
fn real_store(tag: &str) -> (ArtifactStore, support::TempDir) {
    support::store(tag, ArtifactLimits::default())
}

#[test]
fn alpine_dir_real() {
    let artifact = alpine_minirootfs(GuestArch::host());
    if !support::network_fast_enough(artifact.url, 256 * 1024) {
        eprintln!("skipping alpine_dir_real: the network is not available or too slow");
        return;
    }
    let (store, _directory) = real_store("alpine-real");
    let root = support::block_on(store.alpine_rootfs_dir(GuestArch::host()))
        .expect("the real Alpine archive unpacks");
    assert!(
        root.join("bin/busybox").exists(),
        "the tree has no bin/busybox at {}",
        root.display()
    );
}

#[test]
fn ci_rootfs_real() {
    let artifact = firecracker_ci_rootfs(GuestArch::host());
    if !support::network_fast_enough(artifact.url, 1024 * 1024) {
        eprintln!("skipping ci_rootfs_real: the network is not available or too slow");
        return;
    }
    let (store, _directory) = real_store("ci-rootfs-real");
    let path = support::block_on(store.firecracker_ci_rootfs(GuestArch::host()))
        .expect("the real squashfs image downloads");
    let mut file = std::fs::File::open(&path).expect("the image is there");
    let mut magic = [0u8; 4];
    file.read_exact(&mut magic)
        .expect("the image has four bytes");
    assert_eq!(&magic, b"hsqs", "the image is not a squashfs image");
}
