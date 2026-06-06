use foundation_nativeapis::shared::vfs::{
    DeltaStore, MemoryDelta, MemoryFs, OpenMode, SeekFrom, SeekableVfsFile, VfsDirectory, VfsFile,
    VfsFileSystem, VfsFileType,
};
use tracing_test::traced_test;

// ── MemoryFs: File Operations ──

#[test]
#[traced_test]
fn test_create_and_read_file() {
    let fs = MemoryFs::new();
    let file = fs.create("/hello.txt", 0o644).unwrap();
    file.write_at(b"hello world", 0).unwrap();

    let mut buf = vec![0u8; 11];
    let n = file.read_at(&mut buf, 0).unwrap();
    assert_eq!(n, 11);
    assert_eq!(&buf, b"hello world");
}

#[test]
#[traced_test]
fn test_create_file_in_nonexistent_dir() {
    let fs = MemoryFs::new();
    let result = fs.create("/no/such/dir/file.txt", 0o644);
    assert!(result.is_err());
}

#[test]
#[traced_test]
fn test_create_file_already_exists() {
    let fs = MemoryFs::new();
    fs.create("/file.txt", 0o644).unwrap();
    let result = fs.create("/file.txt", 0o644);
    assert!(result.is_err());
}

#[test]
#[traced_test]
fn test_open_nonexistent_file() {
    let fs = MemoryFs::new();
    let result = fs.open("/nope.txt", OpenMode::Read);
    assert!(result.is_err());
}

#[test]
#[traced_test]
fn test_open_directory_as_file() {
    let fs = MemoryFs::new();
    fs.mkdir("/mydir").unwrap();
    let result = fs.open("/mydir", OpenMode::Read);
    assert!(result.is_err());
}

#[test]
#[traced_test]
fn test_write_to_read_only_file() {
    let fs = MemoryFs::new();
    let file = fs.create("/readonly.txt", 0o644).unwrap();
    file.write_at(b"initial", 0).unwrap();

    let ro = fs.open("/readonly.txt", OpenMode::Read).unwrap();
    let result = ro.write_at(b"nope", 0);
    assert!(result.is_err());
}

#[test]
#[traced_test]
fn test_file_read_at_offset() {
    let fs = MemoryFs::new();
    let file = fs.create("/data.bin", 0o644).unwrap();
    file.write_at(b"ABCDEFGHIJ", 0).unwrap();

    let mut buf = [0u8; 3];
    let n = file.read_at(&mut buf, 5).unwrap();
    assert_eq!(n, 3);
    assert_eq!(&buf, b"FGH");
}

#[test]
#[traced_test]
fn test_file_write_at_offset() {
    let fs = MemoryFs::new();
    let file = fs.create("/sparse.bin", 0o644).unwrap();
    file.write_at(b"XYZ", 10).unwrap();

    assert_eq!(file.size().unwrap(), 13);

    let mut buf = [0u8; 13];
    file.read_at(&mut buf, 0).unwrap();
    assert_eq!(&buf[..10], &[0u8; 10]);
    assert_eq!(&buf[10..], b"XYZ");
}

#[test]
#[traced_test]
fn test_file_truncate() {
    let fs = MemoryFs::new();
    let file = fs.create("/trunc.txt", 0o644).unwrap();
    file.write_at(b"hello world", 0).unwrap();
    assert_eq!(file.size().unwrap(), 11);

    file.truncate(5).unwrap();
    assert_eq!(file.size().unwrap(), 5);

    let mut buf = [0u8; 10];
    let n = file.read_at(&mut buf, 0).unwrap();
    assert_eq!(n, 5);
    assert_eq!(&buf[..5], b"hello");
}

#[test]
#[traced_test]
fn test_file_size() {
    let fs = MemoryFs::new();
    let file = fs.create("/sized.txt", 0o644).unwrap();
    assert_eq!(file.size().unwrap(), 0);
    file.write_at(b"twelve chars", 0).unwrap();
    assert_eq!(file.size().unwrap(), 12);
}

// ── MemoryFs: Seekable File Operations ──

#[test]
#[traced_test]
fn test_seekable_sequential_read() {
    let fs = MemoryFs::new();
    let file = fs.create("/seq.txt", 0o644).unwrap();
    file.write_at(b"ABCDEF", 0).unwrap();

    let mut sf = fs.open_seekable("/seq.txt", OpenMode::Read).unwrap();
    let mut buf = [0u8; 3];

    let n = sf.read(&mut buf).unwrap();
    assert_eq!(n, 3);
    assert_eq!(&buf, b"ABC");
    assert_eq!(sf.position(), 3);

    let n = sf.read(&mut buf).unwrap();
    assert_eq!(n, 3);
    assert_eq!(&buf, b"DEF");
    assert_eq!(sf.position(), 6);
}

#[test]
#[traced_test]
fn test_seekable_seek_and_read() {
    let fs = MemoryFs::new();
    let file = fs.create("/seekread.txt", 0o644).unwrap();
    file.write_at(b"0123456789", 0).unwrap();

    let mut sf = fs.open_seekable("/seekread.txt", OpenMode::Read).unwrap();
    sf.seek(SeekFrom::Start(4)).unwrap();

    let mut buf = [0u8; 3];
    let n = sf.read(&mut buf).unwrap();
    assert_eq!(n, 3);
    assert_eq!(&buf, b"456");
}

#[test]
#[traced_test]
fn test_seekable_seek_from_end() {
    let fs = MemoryFs::new();
    let file = fs.create("/end.txt", 0o644).unwrap();
    file.write_at(b"ABCDE", 0).unwrap();

    let mut sf = fs.open_seekable("/end.txt", OpenMode::Read).unwrap();
    let pos = sf.seek(SeekFrom::End(-2)).unwrap();
    assert_eq!(pos, 3);

    let mut buf = [0u8; 2];
    sf.read(&mut buf).unwrap();
    assert_eq!(&buf, b"DE");
}

#[test]
#[traced_test]
fn test_seekable_seek_from_current() {
    let fs = MemoryFs::new();
    let file = fs.create("/cur.txt", 0o644).unwrap();
    file.write_at(b"ABCDEFGH", 0).unwrap();

    let mut sf = fs.open_seekable("/cur.txt", OpenMode::Read).unwrap();
    sf.seek(SeekFrom::Start(3)).unwrap();
    sf.seek(SeekFrom::Current(2)).unwrap();
    assert_eq!(sf.position(), 5);

    sf.seek(SeekFrom::Current(-1)).unwrap();
    assert_eq!(sf.position(), 4);
}

// ── MemoryFs: Directory Operations ──

#[test]
#[traced_test]
fn test_mkdir_and_list() {
    let fs = MemoryFs::new();
    fs.mkdir("/alpha").unwrap();
    fs.mkdir("/beta").unwrap();
    fs.create("/gamma.txt", 0o644).unwrap();

    let dir = fs.open_directory("/").unwrap();
    let entries = dir.list().unwrap();
    assert_eq!(entries.len(), 3);

    let names: Vec<&str> = entries.iter().map(|e| e.name.as_str()).collect();
    assert!(names.contains(&"alpha"));
    assert!(names.contains(&"beta"));
    assert!(names.contains(&"gamma.txt"));
}

#[test]
#[traced_test]
fn test_mkdir_nested() {
    let fs = MemoryFs::new();
    let result = fs.mkdir("/a/b");
    assert!(result.is_err(), "parent must exist");

    fs.mkdir("/a").unwrap();
    fs.mkdir("/a/b").unwrap();
    assert!(fs.exists("/a/b").unwrap());
}

#[test]
#[traced_test]
fn test_remove_empty_dir() {
    let fs = MemoryFs::new();
    fs.mkdir("/empty").unwrap();
    fs.remove("/empty").unwrap();
    assert!(!fs.exists("/empty").unwrap());
}

#[test]
#[traced_test]
fn test_remove_nonempty_dir() {
    let fs = MemoryFs::new();
    fs.mkdir("/nonempty").unwrap();
    fs.create("/nonempty/file.txt", 0o644).unwrap();
    let result = fs.remove("/nonempty");
    assert!(result.is_err(), "cannot remove non-empty directory");
}

#[test]
#[traced_test]
fn test_readdir_mixed() {
    let fs = MemoryFs::new();
    fs.mkdir("/mix").unwrap();
    fs.create("/mix/file.txt", 0o644).unwrap();
    fs.mkdir("/mix/subdir").unwrap();
    fs.symlink("/mix/file.txt", "/mix/link").unwrap();

    let dir = fs.open_directory("/mix").unwrap();
    let entries = dir.list().unwrap();
    assert_eq!(entries.len(), 3);

    let file_entry = entries.iter().find(|e| e.name == "file.txt").unwrap();
    assert_eq!(file_entry.file_type, VfsFileType::Regular);

    let dir_entry = entries.iter().find(|e| e.name == "subdir").unwrap();
    assert_eq!(dir_entry.file_type, VfsFileType::Directory);

    let link_entry = entries.iter().find(|e| e.name == "link").unwrap();
    assert_eq!(link_entry.file_type, VfsFileType::Symlink);
}

// ── MemoryFs: Metadata ──

#[test]
#[traced_test]
fn test_stat_returns_correct_metadata() {
    let fs = MemoryFs::new();
    let file = fs.create("/meta.txt", 0o600).unwrap();
    file.write_at(b"content", 0).unwrap();

    let meta = fs.stat("/meta.txt").unwrap();
    assert_eq!(meta.size, 7);
    assert_eq!(meta.file_type, VfsFileType::Regular);
    assert_eq!(meta.permissions, 0o600);
    assert!(meta.created.is_some());
    assert!(meta.modified.is_some());
}

#[test]
#[traced_test]
fn test_metadata_version_increments() {
    let fs = MemoryFs::new();
    let file = fs.create("/v.txt", 0o644).unwrap();
    let v1 = fs.stat("/v.txt").unwrap().version;

    file.write_at(b"data", 0).unwrap();
    let v2 = fs.stat("/v.txt").unwrap().version;
    assert!(v2 > v1, "version should increment on write");

    file.truncate(2).unwrap();
    let v3 = fs.stat("/v.txt").unwrap().version;
    assert!(v3 > v2, "version should increment on truncate");
}

#[test]
#[traced_test]
fn test_chmod_updates_permissions() {
    let fs = MemoryFs::new();
    fs.create("/perm.txt", 0o644).unwrap();
    fs.chmod("/perm.txt", 0o755).unwrap();
    let meta = fs.stat("/perm.txt").unwrap();
    assert_eq!(meta.permissions, 0o755);
}

// ── MemoryFs: Symlinks ──

#[test]
#[traced_test]
fn test_symlink_create_and_readlink() {
    let fs = MemoryFs::new();
    fs.create("/target.txt", 0o644).unwrap();
    fs.symlink("/target.txt", "/link.txt").unwrap();

    let target = fs.readlink("/link.txt").unwrap();
    assert_eq!(target, "/target.txt");
}

#[test]
#[traced_test]
fn test_symlink_transparent_open() {
    let fs = MemoryFs::new();
    let file = fs.create("/real.txt", 0o644).unwrap();
    file.write_at(b"real content", 0).unwrap();

    fs.symlink("/real.txt", "/alias.txt").unwrap();
    let data = fs.read_file("/alias.txt").unwrap();
    assert_eq!(data, b"real content");
}

#[test]
#[traced_test]
fn test_symlink_chain() {
    let fs = MemoryFs::new();
    let file = fs.create("/final.txt", 0o644).unwrap();
    file.write_at(b"chained", 0).unwrap();

    fs.symlink("/final.txt", "/link1").unwrap();
    fs.symlink("/link1", "/link2").unwrap();

    let data = fs.read_file("/link2").unwrap();
    assert_eq!(data, b"chained");
}

#[test]
#[traced_test]
fn test_symlink_cycle_detection() {
    let fs = MemoryFs::new();
    fs.symlink("/b", "/a").unwrap();
    fs.symlink("/a", "/b").unwrap();

    let result = fs.open("/a", OpenMode::Read);
    assert!(result.is_err(), "symlink cycle should be detected");
}

// ── MemoryFs: Convenience Methods ──

#[test]
#[traced_test]
fn test_read_file_convenience() {
    let fs = MemoryFs::new();
    fs.write_file("/conv.txt", b"convenience").unwrap();
    let data = fs.read_file("/conv.txt").unwrap();
    assert_eq!(data, b"convenience");
}

#[test]
#[traced_test]
fn test_write_file_convenience() {
    let fs = MemoryFs::new();
    fs.write_file("/new.txt", b"first").unwrap();
    assert_eq!(fs.read_file("/new.txt").unwrap(), b"first");

    fs.write_file("/new.txt", b"second").unwrap();
    assert_eq!(fs.read_file("/new.txt").unwrap(), b"second");
}

#[test]
#[traced_test]
fn test_mkdir_all() {
    let fs = MemoryFs::new();
    fs.mkdir_all("/a/b/c").unwrap();
    assert!(fs.exists("/a").unwrap());
    assert!(fs.exists("/a/b").unwrap());
    assert!(fs.exists("/a/b/c").unwrap());
}

#[test]
#[traced_test]
fn test_remove_all() {
    let fs = MemoryFs::new();
    fs.mkdir_all("/tree/branch").unwrap();
    fs.write_file("/tree/file.txt", b"data").unwrap();
    fs.write_file("/tree/branch/leaf.txt", b"leaf").unwrap();

    fs.remove_all("/tree").unwrap();
    assert!(!fs.exists("/tree").unwrap());
    assert!(!fs.exists("/tree/branch").unwrap());
    assert!(!fs.exists("/tree/file.txt").unwrap());
}

#[test]
#[traced_test]
fn test_copy() {
    let fs = MemoryFs::new();
    fs.write_file("/src.txt", b"copy me").unwrap();
    fs.copy("/src.txt", "/dst.txt").unwrap();

    let data = fs.read_file("/dst.txt").unwrap();
    assert_eq!(data, b"copy me");
    // Source still exists
    assert!(fs.exists("/src.txt").unwrap());
}

// ── MemoryFs: Rename ──

#[test]
#[traced_test]
fn test_rename_file() {
    let fs = MemoryFs::new();
    fs.write_file("/old.txt", b"moved").unwrap();
    fs.rename("/old.txt", "/new.txt").unwrap();

    assert!(!fs.exists("/old.txt").unwrap());
    assert_eq!(fs.read_file("/new.txt").unwrap(), b"moved");
}

#[test]
#[traced_test]
fn test_rename_directory() {
    let fs = MemoryFs::new();
    fs.mkdir_all("/src/nested").unwrap();
    fs.write_file("/src/nested/file.txt", b"data").unwrap();

    fs.rename("/src", "/dst").unwrap();
    assert!(!fs.exists("/src").unwrap());
    assert!(fs.exists("/dst").unwrap());
    assert!(fs.exists("/dst/nested").unwrap());
    assert_eq!(fs.read_file("/dst/nested/file.txt").unwrap(), b"data");
}

#[test]
#[traced_test]
fn test_rename_to_existing() {
    let fs = MemoryFs::new();
    fs.write_file("/a.txt", b"a").unwrap();
    fs.write_file("/b.txt", b"b").unwrap();
    let result = fs.rename("/a.txt", "/b.txt");
    assert!(result.is_err());
}

// ── Path traversal rejection ──

#[test]
#[traced_test]
fn test_path_traversal_rejected() {
    let fs = MemoryFs::new();
    assert!(fs.stat("/../../../etc/passwd").is_err());
    assert!(fs.exists("/foo/../../etc/passwd").is_err());
    assert!(fs.open("/a/../b", OpenMode::Read).is_err());
}

// ── MemoryDelta: Whiteout Operations ──

#[test]
#[traced_test]
fn test_whiteout_add_and_check() {
    let delta = MemoryDelta::new();
    delta.add_whiteout("/deleted.txt", 5).unwrap();
    let result = delta.is_whiteout("/deleted.txt").unwrap();
    assert_eq!(result, Some(5));
}

#[test]
#[traced_test]
fn test_whiteout_remove() {
    let delta = MemoryDelta::new();
    delta.add_whiteout("/tmp.txt", 3).unwrap();
    assert!(delta.is_whiteout("/tmp.txt").unwrap().is_some());

    delta.remove_whiteout("/tmp.txt").unwrap();
    // Direct path whiteout removed, but "/" could theoretically still match.
    // Since "/" wasn't whiteout'd, this should be None.
    assert!(delta.is_whiteout("/tmp.txt").unwrap().is_none());
}

#[test]
#[traced_test]
fn test_whiteout_inheritance() {
    let delta = MemoryDelta::new();
    delta.add_whiteout("/a/b", 7).unwrap();

    assert_eq!(delta.is_whiteout("/a/b/c.txt").unwrap(), Some(7));
    assert_eq!(delta.is_whiteout("/a/b/d/e.txt").unwrap(), Some(7));
    assert_eq!(delta.is_whiteout("/a/b").unwrap(), Some(7));
}

#[test]
#[traced_test]
fn test_whiteout_no_false_inheritance() {
    let delta = MemoryDelta::new();
    delta.add_whiteout("/a/bc", 4).unwrap();

    // /a/b should NOT be whiteout'd — /a/bc is not an ancestor of /a/b
    assert!(delta.is_whiteout("/a/b").unwrap().is_none());
    // /a/bcd should NOT be whiteout'd either
    assert!(delta.is_whiteout("/a/bcd").unwrap().is_none());
}

#[test]
#[traced_test]
fn test_whiteout_list() {
    let delta = MemoryDelta::new();
    delta.add_whiteout("/dir/a.txt", 1).unwrap();
    delta.add_whiteout("/dir/b.txt", 2).unwrap();
    delta.add_whiteout("/other/c.txt", 3).unwrap();

    let list = delta.list_whiteouts("/dir").unwrap();
    assert_eq!(list.len(), 2);
    let paths: Vec<&str> = list.iter().map(|(p, _)| p.as_str()).collect();
    assert!(paths.contains(&"/dir/a.txt"));
    assert!(paths.contains(&"/dir/b.txt"));
    assert!(!paths.contains(&"/other/c.txt"));
}

#[test]
#[traced_test]
fn test_whiteout_reset_clears_all() {
    let delta = MemoryDelta::new();
    delta.add_whiteout("/x.txt", 1).unwrap();
    delta.write_file("/y.txt", b"data").unwrap();

    delta.reset().unwrap();
    assert!(delta.is_whiteout("/x.txt").unwrap().is_none());
    assert!(!delta.exists("/y.txt").unwrap());
}

// ── MemoryDelta: VfsFileSystem Delegation ──

#[test]
#[traced_test]
fn test_delta_create_and_read() {
    let delta = MemoryDelta::new();
    delta.write_file("/test.txt", b"delta data").unwrap();
    let data = delta.read_file("/test.txt").unwrap();
    assert_eq!(data, b"delta data");
}

#[test]
#[traced_test]
fn test_delta_does_not_check_whiteouts() {
    let delta = MemoryDelta::new();
    delta.write_file("/file.txt", b"content").unwrap();
    delta.add_whiteout("/file.txt", 10).unwrap();

    // VfsFileSystem methods should still see the file
    assert!(delta.exists("/file.txt").unwrap());
    let data = delta.read_file("/file.txt").unwrap();
    assert_eq!(data, b"content");
}
