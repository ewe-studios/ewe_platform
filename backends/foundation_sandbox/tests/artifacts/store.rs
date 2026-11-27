//! The artifact store tests: the ready entry, the atomic build, the concurrent
//! build and the slave initramfs.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use foundation_sandbox::native::artifacts::{
    sha256_hex, ArtifactKind, ArtifactLimits, ArtifactSpec, ArtifactStore,
};
use foundation_sandbox::shared::error::SandboxError;
use foundation_sandbox::shared::spec::GuestArch;

use super::support::{self, Reply, TestServer};

/// A file spec for `url`.
fn file_spec(url: String, hash: &str) -> ArtifactSpec {
    ArtifactSpec {
        name: "store-file".to_string(),
        url,
        sha256: hash.to_string(),
        kind: ArtifactKind::File,
    }
}

/// A store with the default limits.
fn default_store(tag: &str) -> (ArtifactStore, support::TempDir) {
    support::store(tag, ArtifactLimits::default())
}

#[test]
fn store_reuses_entry() {
    let body = vec![4u8; 4096];
    let payload = body.clone();
    let server = TestServer::start(move |_| Reply::ok(payload.clone()));
    let (store, _directory) = default_store("reuse");
    let spec = file_spec(server.url("/reuse"), &sha256_hex(&body));
    let first = support::block_on(store.ensure(&spec)).expect("the first call builds");
    let second = support::block_on(store.ensure(&spec)).expect("the second call reads");
    assert_eq!(first, second);
    assert_eq!(server.requests(), 1, "the second call made a request");
}

#[test]
fn store_is_atomic() {
    let good = vec![6u8; 4096];
    let broken = Arc::new(AtomicBool::new(true));
    let handler_broken = Arc::clone(&broken);
    let server = TestServer::start(move |_| {
        if handler_broken.load(Ordering::SeqCst) {
            // The header declares 4096 bytes, and the server sends 100.
            Reply::ok(vec![6u8; 100]).declared(4096)
        } else {
            Reply::ok(vec![6u8; 4096])
        }
    });
    let (store, directory) = default_store("atomic");
    let spec = file_spec(server.url("/atomic"), &sha256_hex(&good));
    let error = support::block_on(store.ensure(&spec)).expect_err("the cut body fails");
    assert!(
        !matches!(
            error.current_context(),
            SandboxError::ArtifactUnsafePath { .. }
        ),
        "the cut body is not a path error: {error}"
    );
    let cache = directory.path().join("cache");
    let found = support::entries(&cache);
    assert!(
        found.iter().all(|path| !path.contains(".tmp-")),
        "a temporary entry stays: {found:?}"
    );
    assert!(
        found
            .iter()
            .all(|path| std::path::Path::new(path).file_name()
                != Some(std::ffi::OsStr::new(".ready"))),
        "a ready marker stays: {found:?}"
    );
    broken.store(false, Ordering::SeqCst);
    let path = support::block_on(store.ensure(&spec)).expect("the second build passes");
    assert_eq!(std::fs::read(path).expect("the file is there"), good);
    assert_eq!(server.requests(), 2, "the second build used the server");
}

#[test]
fn store_concurrent_ensure() {
    let body = vec![8u8; 1024 * 1024];
    let payload = body.clone();
    let server = TestServer::start(move |_| Reply::ok(payload.clone()));
    let (store, _directory) = default_store("concurrent");
    // The two builder threads each take the test pool, so this thread lets it
    // go first: it would otherwise wait for a pool that this thread holds.
    crate::pool::release();
    let spec = file_spec(server.url("/race"), &sha256_hex(&body));
    let barrier = Arc::new(std::sync::Barrier::new(2));
    let mut handles = Vec::new();
    for _ in 0..2 {
        let store = store.clone();
        let spec = spec.clone();
        let barrier = Arc::clone(&barrier);
        handles.push(std::thread::spawn(move || {
            barrier.wait();
            support::block_on(store.ensure(&spec))
        }));
    }
    let paths: Vec<_> = handles
        .into_iter()
        .map(|handle| handle.join().expect("the build thread ends"))
        .collect();
    let first = paths[0].as_ref().expect("the first build passes");
    let second = paths[1].as_ref().expect("the second build passes");
    assert_eq!(first, second, "the two builders returned different paths");
    assert_eq!(std::fs::read(first).expect("the file is there"), body);
}

#[test]
fn store_rejects_an_invalid_spec() {
    let (store, _directory) = default_store("invalid");
    let spec = ArtifactSpec {
        name: "bad".to_string(),
        url: "http://127.0.0.1/never".to_string(),
        sha256: "abc".to_string(),
        kind: ArtifactKind::File,
    };
    let error = support::block_on(store.ensure(&spec)).expect_err("the spec is invalid");
    assert!(matches!(
        error.current_context(),
        SandboxError::InvalidSpec { .. }
    ));
}

#[test]
fn slave_initramfs_is_deterministic() {
    let directory = support::TempDir::new("initramfs");
    let slave = directory.path().join("agent-slave");
    let bytes = vec![0x53u8; 4096];
    std::fs::write(&slave, &bytes).expect("the slave file is written");
    let (store, _cache) = default_store("initramfs-store");
    let first = support::block_on(store.slave_initramfs(&slave)).expect("the first build");
    let second = support::block_on(store.slave_initramfs(&slave)).expect("the second build");
    assert_eq!(first, second);
    let archive = std::fs::read(&first).expect("the archive is there");
    assert_eq!(archive.len() % 512, 0);
    let other = directory.path().join("other-slave");
    std::fs::write(&other, [0x54u8; 4096]).expect("the other slave is written");
    let third = support::block_on(store.slave_initramfs(&other)).expect("the other build");
    assert_ne!(first, third, "a different slave gives a different entry");
    assert_eq!(
        std::fs::read(&first).expect("the first archive"),
        archive,
        "the first archive changed after the second build"
    );
}

#[test]
fn store_uses_the_spec_hash_prefix() {
    let body = vec![1u8; 128];
    let payload = body.clone();
    let server = TestServer::start(move |_| Reply::ok(payload.clone()));
    let (store, _directory) = default_store("prefix");
    let hash = sha256_hex(&body);
    let spec = file_spec(server.url("/prefix"), &hash);
    let path = support::block_on(store.ensure(&spec)).expect("the build passes");
    assert!(
        path.to_string_lossy().contains(support::prefix(&hash)),
        "the entry path has no hash prefix: {}",
        path.display()
    );
}

#[test]
fn guest_arch_has_a_manifest_everywhere() {
    for arch in [GuestArch::X86_64, GuestArch::Aarch64] {
        let artifact = foundation_sandbox::native::artifacts::manifest::alpine_minirootfs(arch);
        assert_eq!(artifact.sha256.len(), 64);
        assert_eq!(artifact.kind, ArtifactKind::TarGzDir);
        let kernel = foundation_sandbox::native::artifacts::manifest::firecracker_kernel(arch);
        assert_eq!(kernel.sha256.len(), 64);
        assert_eq!(kernel.kind, ArtifactKind::File);
    }
}
