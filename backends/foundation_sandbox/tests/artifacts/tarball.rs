//! The archive unpack tests.

use foundation_sandbox::native::artifacts::{
    sha256_hex, ArtifactKind, ArtifactLimits, ArtifactSpec,
};
use foundation_sandbox::shared::error::SandboxError;

use super::support::{self, RawEntry, Reply, TestServer};

/// A tree spec for `url` with the hash of `archive`.
fn tree_spec(url: String, archive: &[u8]) -> ArtifactSpec {
    ArtifactSpec {
        name: "test-tree".to_string(),
        url,
        sha256: sha256_hex(archive),
        kind: ArtifactKind::TarGzDir,
    }
}

/// Serves `archive` and returns its error, or the ready path.
fn ensure_archive(
    tag: &str,
    archive: &[u8],
    limits: ArtifactLimits,
) -> (
    foundation_sandbox::shared::error::SandboxResult<std::path::PathBuf>,
    support::TempDir,
) {
    let body = archive.to_vec();
    let server = TestServer::start(move |_| Reply::ok(body.clone()));
    let (store, directory) = support::store(tag, limits);
    let result = support::block_on(store.ensure(&tree_spec(server.url("/tree"), archive)));
    (result, directory)
}

#[test]
fn tar_rejects_escape() {
    for path in ["../x", "/abs", "a/../../x"] {
        let archive = support::raw_tarball(&[RawEntry::file(path, b"escape")]);
        let (result, directory) = ensure_archive("escape", &archive, ArtifactLimits::default());
        let error = result.expect_err("the unsafe path fails");
        assert!(
            matches!(
                error.current_context(),
                SandboxError::ArtifactUnsafePath { .. }
            ),
            "{path} gave {error}"
        );
        assert!(
            !directory.path().join("x").exists(),
            "{path} wrote outside the target"
        );
        assert!(
            support::entries(directory.path())
                .iter()
                .all(|found| !found.ends_with('x')),
            "{path} left a file x"
        );
    }
}

#[test]
fn tar_rejects_a_relative_escape_through_a_symlink() {
    let archive = support::raw_tarball(&[
        RawEntry::symlink("link", "/tmp"),
        RawEntry::file("link/x", b"escape"),
    ]);
    let (result, _directory) = ensure_archive("symlink", &archive, ArtifactLimits::default());
    let error = result.expect_err("the write through the link fails");
    assert!(matches!(
        error.current_context(),
        SandboxError::ArtifactUnsafePath { .. }
    ));
}

#[test]
fn tar_stops_at_limits() {
    let data = vec![7u8; 2 * 1024 * 1024];
    let archive = support::raw_tarball(&[RawEntry::file("big", &data)]);
    let limits = ArtifactLimits::default().with_max_unpacked_bytes(1024 * 1024);
    let (result, _directory) = ensure_archive("unpack-limit", &archive, limits);
    let error = result.expect_err("the archive is too large");
    assert!(matches!(
        error.current_context(),
        SandboxError::ArtifactTooLarge { .. }
    ));
}

#[test]
fn tar_stops_at_the_entry_limit() {
    let archive = support::raw_tarball(&[
        RawEntry::file("a", b"a"),
        RawEntry::file("b", b"b"),
        RawEntry::file("c", b"c"),
    ]);
    let limits = ArtifactLimits::default().with_max_entries(2);
    let (result, _directory) = ensure_archive("entry-limit", &archive, limits);
    let error = result.expect_err("the archive has too many entries");
    assert!(matches!(
        error.current_context(),
        SandboxError::ArtifactTooLarge { .. }
    ));
}

#[test]
fn tar_keeps_file_modes() {
    let archive = support::raw_tarball(&[RawEntry::file("bin/tool", b"#!/bin/sh\n")]);
    let (result, _directory) = ensure_archive("modes", &archive, ArtifactLimits::default());
    let root = result.expect("the tree unpacks");
    let tool = root.join("bin/tool");
    assert_eq!(
        std::fs::read(&tool).expect("the tool is there"),
        b"#!/bin/sh\n"
    );
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mode = std::fs::metadata(&tool)
            .expect("the tool metadata")
            .permissions()
            .mode();
        assert_eq!(mode & 0o777, 0o755, "the unpack lost the file mode");
    }
}
