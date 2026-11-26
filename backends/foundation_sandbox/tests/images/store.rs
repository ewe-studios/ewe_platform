//! The image store laws of section 7 (feature 12): commit, tags, snapshots and
//! the work-file sweep. Every test uses real files.

use std::io::{Seek, SeekFrom, Write};
use std::os::unix::fs::MetadataExt as _;

use foundation_sandbox::shared::spec::{GuestArch, ImageRef};

use super::support;

#[test]
fn manifest_roundtrip() {
    let manifest = support::manifest(GuestArch::host());
    let json = serde_json::to_vec(&manifest).expect("the manifest encodes");
    let decoded: foundation_sandbox::shared::images::ImageManifest =
        serde_json::from_slice(&json).expect("the manifest decodes");
    assert_eq!(decoded, manifest);
}

#[test]
fn commit_is_atomic() {
    let (store, dir, _ctx) = support::store("atomic");
    let work = dir.path().join("work.ext4");
    std::fs::write(&work, b"image-bytes").expect("the work file is written");
    let reference = crate::pool::block_on(store.commit(
        work,
        support::manifest(GuestArch::host()),
        Some("tool"),
    ))
    .expect("the commit succeeds");
    let ImageRef::Tag(tag) = &reference else {
        panic!("a commit with a tag returns the tag");
    };
    assert_eq!(tag, "tool");
    let store_root = dir.path().join("cache/images/store");
    let entries: Vec<_> = std::fs::read_dir(&store_root)
        .expect("the store exists")
        .map(|entry| entry.expect("an entry").file_name())
        .collect();
    assert_eq!(entries.len(), 1, "one committed image");
    let name = entries[0].to_string_lossy();
    assert_eq!(name.len(), 32, "the directory is the 32-character key");
    assert!(!name.contains(".tmp-"), "no temporary directory stays");
    let image = store_root.join(name.as_ref());
    assert!(image.join("rootfs.ext4").exists());
    assert!(image.join("manifest.json").exists());
    let manifest: foundation_sandbox::shared::images::ImageManifest =
        serde_json::from_slice(&std::fs::read(image.join("manifest.json")).expect("the manifest"))
            .expect("the manifest parses");
    assert_eq!(manifest.key, name.as_ref());
    assert_eq!(manifest.rootfs_size, 11);
    assert_eq!(manifest.rootfs_sha256.len(), 64);
}

#[test]
fn commit_same_key_keeps_first() {
    let (store, dir, _ctx) = support::store("same-key");
    // The store work directory exists only after the first commit. Use the real
    // one that `ImageStore::new` made.
    let work = dir.path().join("cache/images/work/first.ext4");
    std::fs::write(&work, b"same-bytes").expect("the first work file");
    let first =
        crate::pool::block_on(store.commit(work, support::manifest(GuestArch::host()), None))
            .expect("the first commit");
    let work = dir.path().join("cache/images/work/second.ext4");
    std::fs::write(&work, b"same-bytes").expect("the second work file");
    let second_path = work.clone();
    let second =
        crate::pool::block_on(store.commit(work, support::manifest(GuestArch::host()), None))
            .expect("the second commit");
    assert_eq!(first, second, "the same bytes give the same reference");
    assert!(!second_path.exists(), "the second work file is removed");
    let (path, _manifest) = crate::pool::block_on(store.resolve(&first)).expect("resolves");
    assert_eq!(std::fs::read(path).expect("the image"), b"same-bytes");
}

#[test]
fn tag_points_to_key() {
    let (store, dir, _ctx) = support::store("tag");
    let work = dir.path().join("cache/images/work/tool.ext4");
    std::fs::write(&work, b"tool-image").expect("the work file");
    let reference = crate::pool::block_on(store.commit(
        work,
        support::manifest(GuestArch::host()),
        Some("tool"),
    ))
    .expect("the commit");
    let ImageRef::Tag(tag) = reference else {
        panic!("the commit returns the tag");
    };
    let tag_body = std::fs::read(dir.path().join("cache/images/tags").join(&tag))
        .expect("the tag file exists");
    let key = String::from_utf8(tag_body).expect("the key is text");
    assert_eq!(key.len(), 32);
    let (path, manifest) =
        crate::pool::block_on(store.resolve(&ImageRef::Tag(tag))).expect("the tag resolves");
    assert_eq!(manifest.key, key);
    assert_eq!(
        path.file_name().and_then(std::ffi::OsStr::to_str),
        Some("rootfs.ext4")
    );
}

#[test]
fn remove_removes_tag() {
    let (store, dir, _ctx) = support::store("remove");
    let work = dir.path().join("cache/images/work/tool.ext4");
    std::fs::write(&work, b"tool-image").expect("the work file");
    let ImageRef::Tag(tag) = crate::pool::block_on(store.commit(
        work,
        support::manifest(GuestArch::host()),
        Some("tool"),
    ))
    .expect("the commit") else {
        panic!("the commit returns the tag");
    };
    crate::pool::block_on(store.remove(&ImageRef::Tag(tag.clone()))).expect("the remove");
    assert!(!dir.path().join("cache/images/tags").join(&tag).exists());
    assert!(crate::pool::block_on(store.resolve(&ImageRef::Tag(tag))).is_err());
}

#[test]
fn work_files_expire() {
    let dir = support::TempDir::new("expire");
    let (ctx, clock) = crate::control::support::ctx(92);
    let store = foundation_sandbox::native::images::ImageStore::new(
        ctx.clone(),
        support::host(&dir, support::tiny_age()),
    );
    let old = dir.path().join("cache/images/work/old.ext4");
    std::fs::write(&old, b"old").expect("the old work file");
    let file = std::fs::File::open(&old).expect("the old work file opens");
    let epoch = foundation_compact::SystemTime::UNIX_EPOCH;
    file.set_times(
        std::fs::FileTimes::new()
            .set_modified(epoch)
            .set_accessed(epoch),
    )
    .expect("the old time is set");
    let fresh = dir.path().join("cache/images/work/fresh.ext4");
    std::fs::write(&fresh, b"fresh").expect("the fresh work file");
    clock.advance_wall(std::time::Duration::from_secs(2));
    let _reopened = foundation_sandbox::native::images::ImageStore::new(
        ctx,
        support::host(&dir, support::tiny_age()),
    );
    let _ = store;
    assert!(!old.exists(), "the old work file is removed");
    assert!(fresh.exists(), "the fresh work file stays");
}

#[test]
fn snapshot_equal_bytes() {
    let (store, dir, _ctx) = support::store("snapshot");
    let source = dir.path().join("source.ext4");
    std::fs::write(&source, b"snapshot-body").expect("the source");
    let copy = crate::pool::block_on(store.snapshot(&source)).expect("the snapshot");
    assert_eq!(
        std::fs::read(&source).expect("the source"),
        std::fs::read(&copy).expect("the copy")
    );
}

#[test]
fn snapshot_keeps_holes() {
    let (store, dir, _ctx) = support::store("holes");
    let source = dir.path().join("source.ext4");
    let size = 4 * 1024 * 1024_u64;
    let mut file = std::fs::File::create(&source).expect("the source");
    file.set_len(size).expect("the length");
    file.seek(SeekFrom::Start(3 * 1024 * 1024))
        .expect("the seek");
    file.write_all(b"island").expect("the island");
    file.flush().expect("the flush");
    drop(file);
    let copy = crate::pool::block_on(store.snapshot(&source)).expect("the snapshot");
    let source_meta = std::fs::metadata(&source).expect("the source metadata");
    let copy_meta = std::fs::metadata(&copy).expect("the copy metadata");
    assert_eq!(source_meta.len(), copy_meta.len());
    let source_blocks = source_meta.blocks();
    let copy_blocks = copy_meta.blocks();
    assert!(
        copy_blocks <= source_blocks + 2048,
        "the copy keeps the holes: source {source_blocks} blocks, copy {copy_blocks} blocks"
    );
    let at = |path: &std::path::Path| {
        use std::io::{Read, Seek};
        let mut file = std::fs::File::open(path).expect("the file opens");
        file.seek(SeekFrom::Start(3 * 1024 * 1024))
            .expect("the seek");
        let mut buffer = [0_u8; 6];
        file.read_exact(&mut buffer).expect("the island reads");
        buffer
    };
    assert_eq!(at(&source), at(&copy));
}
