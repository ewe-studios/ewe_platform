//! The ADD rows of feature 14, section 7: I8, I10 and section 6.3.

use std::io::Cursor;
use std::os::unix::fs::{MetadataExt as _, PermissionsExt as _};
use std::path::Path;
use std::sync::Arc;

use foundation_sandbox::native::host::NativeFs;
use foundation_sandbox::native::images::build::steps::{plan_add, TarPlan};
use foundation_sandbox::shared::error::SandboxError;
use foundation_sandbox::shared::host::HostFs;
use foundation_sandbox::shared::limits::BuildLimits;
use foundation_sandbox::shared::unpack::UnpackOptions;
use foundation_sandbox::shared::BoxByteStream;
use futures::StreamExt as _;

use foundation_sandbox::native::unpack::{unpack, UnpackLimits};

use super::support::{write_file, TempDir};

/// Builds the ADD plan of one source.
fn plan(
    context: &Path,
    source: &str,
    destination: &str,
    limits: &BuildLimits,
) -> foundation_sandbox::shared::error::SandboxResult<
    foundation_sandbox::native::images::build::steps::AddPlan,
> {
    let fs: Arc<dyn HostFs> = Arc::new(NativeFs);
    plan_add(&fs, context, source, destination, None, limits)
}

/// One entry of a tar plan.
#[derive(Debug)]
struct TarEntry {
    /// The path in the guest.
    path: String,
    /// The mode.
    mode: u32,
    /// The owner identifier.
    uid: u64,
    /// The group identifier.
    gid: u64,
    /// The modification time.
    mtime: u64,
    /// `f`, `d` or `l`.
    kind: char,
    /// The target of a link.
    link: Option<String>,
}

/// The bytes of one tar plan.
fn tar_bytes(plan: TarPlan) -> Vec<u8> {
    let fs: Arc<dyn HostFs> = Arc::new(NativeFs);
    let stream: BoxByteStream = plan.tar_stream(fs);
    crate::pool::block_on(async move {
        let mut stream = stream;
        let mut bytes = Vec::new();
        while let Some(chunk) = stream.next().await {
            bytes.extend_from_slice(&chunk.expect("the tar stream yields"));
        }
        bytes
    })
}

/// The entries of one tar plan, in order.
fn tar_entries(plan: TarPlan) -> Vec<TarEntry> {
    let bytes = tar_bytes(plan);
    let mut archive = tar::Archive::new(Cursor::new(bytes));
    let mut entries = Vec::new();
    for entry in archive.entries().expect("the tar has entries") {
        let entry = entry.expect("the tar entry reads");
        let header = entry.header();
        let path = entry
            .path()
            .expect("the entry path reads")
            .to_string_lossy()
            .into_owned();
        let kind = match header.entry_type() {
            tar::EntryType::Regular => 'f',
            tar::EntryType::Directory => 'd',
            tar::EntryType::Symlink => 'l',
            other => panic!("unexpected tar entry type {other:?}"),
        };
        let link = entry
            .link_name()
            .expect("the link name reads")
            .map(|path| path.to_string_lossy().into_owned());
        entries.push(TarEntry {
            path,
            mode: header.mode().expect("the mode reads"),
            uid: header.uid().expect("the uid reads"),
            gid: header.gid().expect("the gid reads"),
            mtime: header.mtime().expect("the mtime reads"),
            kind,
            link,
        });
    }
    entries
}

#[test]
fn add_path_rules() {
    let directory = TempDir::new("add-rules");
    let context = directory.path().join("context");
    write_file(&context, "file", b"inside\n");
    write_file(&context, "dir/x", b"x\n");
    #[cfg(unix)]
    {
        use std::os::unix::fs::symlink;
        std::fs::create_dir_all(&context).expect("the context is made");
        symlink("/etc", context.join("out")).expect("the outside link is made");
        symlink("file", context.join("in")).expect("the inside link is made");
        write_file(&context, "tree/keep", b"keep\n");
        symlink("/etc", context.join("tree/escape")).expect("the tree link is made");
    }
    let limits = BuildLimits::default();

    // A `..` part and an absolute path leave the context.
    for source in ["../secret", "/etc/passwd"] {
        let error = plan(&context, source, "/x", &limits).expect_err("the source is refused");
        match error.current_context() {
            SandboxError::InvalidSpec { field, reason } => {
                assert_eq!(field, "sandboxfile");
                assert!(
                    reason.contains("build context") || reason.contains("stay inside"),
                    "the reason names the context: {reason}"
                );
            }
            other => panic!("the error is not InvalidSpec: {other:?}"),
        }
    }

    #[cfg(unix)]
    {
        // A link that points out is refused; a link that points in is added.
        let error = plan(&context, "out", "/x", &limits).expect_err("the outside link fails");
        assert!(matches!(
            error.current_context(),
            SandboxError::InvalidSpec { .. }
        ));
        let inside = plan(&context, "in", "/x", &limits).expect("the inside link resolves");
        let entries = tar_entries(inside.tar);
        assert!(
            entries
                .iter()
                .any(|entry| entry.path == "x" && entry.kind == 'f'),
            "the target file is added"
        );

        // A link inside the tree is a link, not a followed path.
        let tree = plan(&context, "./tree", "/opt/tree", &limits).expect("the tree is added");
        let entries = tar_entries(tree.tar);
        let link = entries
            .iter()
            .find(|entry| entry.path == "opt/tree/escape")
            .expect("the tree link is an entry");
        assert_eq!(link.kind, 'l');
        assert_eq!(link.link.as_deref(), Some("/etc"));
    }
}

#[test]
fn add_tar_entries() {
    let directory = TempDir::new("add-tar");
    let context = directory.path().join("context");
    write_file(&context, "app/main.sh", b"echo main\n");
    write_file(&context, "app/tool", b"tool\n");
    write_file(&context, "app/empty/.keep", b"");
    #[cfg(unix)]
    {
        use std::os::unix::fs::symlink;
        use std::os::unix::fs::PermissionsExt as _;
        std::fs::set_permissions(
            context.join("app/tool"),
            std::fs::Permissions::from_mode(0o755),
        )
        .expect("the tool mode is set");
        symlink("main.sh", context.join("app/link")).expect("the link is made");
    }
    let fs: Arc<dyn HostFs> = Arc::new(NativeFs);
    let limits = BuildLimits::default();
    let built = plan_add(&fs, &context, "./app", "/opt/app", Some(0o600), &limits)
        .expect("the ADD plan is built");
    let entries = tar_entries(built.tar);

    // Parents come first, and each path is sorted.
    let paths: Vec<&str> = entries.iter().map(|entry| entry.path.as_str()).collect();
    assert_eq!(
        paths,
        vec![
            "opt",
            "opt/app",
            "opt/app/empty",
            "opt/app/empty/.keep",
            "opt/app/link",
            "opt/app/main.sh",
            "opt/app/tool",
        ]
    );
    for entry in &entries {
        assert_eq!(entry.uid, 0, "the owner is root");
        assert_eq!(entry.gid, 0, "the group is root");
        assert_eq!(entry.mtime, 0, "the modification time is 0");
    }
    for entry in &entries {
        match entry.kind {
            'd' => assert_eq!(entry.mode, 0o755, "a directory is 0755"),
            'f' => assert_eq!(entry.mode, 0o600, "--mode applies to a file"),
            'l' => {
                assert_eq!(entry.mode, 0o777, "a link is 0777");
                assert_eq!(entry.link.as_deref(), Some("main.sh"));
            }
            other => panic!("unexpected type {other}"),
        }
    }

    // Without --mode, a file keeps its host permission bits.
    #[cfg(unix)]
    {
        let built = plan_add(&fs, &context, "./app/tool", "/opt/bin/", None, &limits)
            .expect("the file plan is built");
        let entries = tar_entries(built.tar);
        let paths: Vec<&str> = entries.iter().map(|entry| entry.path.as_str()).collect();
        assert_eq!(paths, vec!["opt", "opt/bin", "opt/bin/tool"]);
        let tool = &entries[2];
        assert_eq!(tool.mode, 0o755, "the host mode is kept");
    }

    // The two limits stop an ADD before a sandbox starts.
    let small = BuildLimits::default().with_max_context_entries(2);
    let error = plan_add(&fs, &context, "./app", "/opt/app", None, &small)
        .expect_err("the entry limit stops the plan");
    assert!(matches!(
        error.current_context(),
        SandboxError::ArtifactTooLarge { .. }
    ));
    let small = BuildLimits::default().with_max_context_bytes(4);
    let error = plan_add(&fs, &context, "./app", "/opt/app", None, &small)
        .expect_err("the byte limit stops the plan");
    assert!(matches!(
        error.current_context(),
        SandboxError::ArtifactTooLarge { .. }
    ));
}

/// Rewrites the owner of every tar header to the test user.
///
/// The builder writes owner 0:0 for the guest. A test runs as an ordinary
/// user, so it applies the archive as its own owner; feature 12 tests the
/// root owner in the guest.
fn owned_by_test_user(mut bytes: Vec<u8>) -> Vec<u8> {
    // SAFETY: `geteuid` and `getegid` read the process credential.
    let (uid, gid) = unsafe { (libc::geteuid(), libc::getegid()) };
    let mut offset = 0;
    while offset + 512 <= bytes.len() {
        let block = &bytes[offset..offset + 512];
        if block.iter().all(|byte| *byte == 0) {
            break;
        }
        let size_field = &bytes[offset + 124..offset + 136];
        let size_text: String = size_field
            .iter()
            .take_while(|byte| **byte != 0 && **byte != b' ')
            .map(|byte| char::from(*byte))
            .collect();
        let size = u64::from_str_radix(size_text.trim(), 8).expect("the tar size is octal");
        bytes[offset + 108..offset + 116].fill(0);
        bytes[offset + 108..offset + 116].copy_from_slice(format!("{uid:07o}\0").as_bytes());
        bytes[offset + 116..offset + 124].fill(0);
        bytes[offset + 116..offset + 124].copy_from_slice(format!("{gid:07o}\0").as_bytes());
        bytes[offset + 148..offset + 156].fill(b' ');
        let checksum: u64 = bytes[offset..offset + 512]
            .iter()
            .map(|byte| u64::from(*byte))
            .sum();
        bytes[offset + 148..offset + 156].copy_from_slice(format!("{checksum:06o}\0 ").as_bytes());
        let padded = u64::div_ceil(size, 512) * 512;
        offset += 512 + usize::try_from(padded).expect("the tar entry fits");
    }
    bytes
}

#[test]
fn add_keeps_an_existing_directory_mode() {
    let directory = TempDir::new("add-keep-mode");
    let context = directory.path().join("context");
    write_file(&context, "tool", b"tool\n");
    let fs: Arc<dyn HostFs> = Arc::new(NativeFs);
    let built = plan_add(
        &fs,
        &context,
        "./tool",
        "/tmp/tool",
        None,
        &BuildLimits::default(),
    )
    .expect("the ADD plan is built");
    let bytes = owned_by_test_user(tar_bytes(built.tar));

    // The image already has /tmp with mode 1777. The ADD keeps it.
    let root = directory.path().join("keep");
    std::fs::create_dir_all(root.join("tmp")).expect("the image /tmp is made");
    std::fs::set_permissions(root.join("tmp"), std::fs::Permissions::from_mode(0o1777))
        .expect("the image /tmp mode is set");
    let options = UnpackOptions {
        keep_existing_dirs: true,
        ..UnpackOptions::default()
    };
    unpack(
        Cursor::new(bytes.clone()),
        &root,
        &options,
        &UnpackLimits::default(),
    )
    .expect("the archive applies");
    let mode = std::fs::metadata(root.join("tmp"))
        .expect("the directory is there")
        .mode()
        & 0o7777;
    assert_eq!(mode, 0o1777, "an existing directory keeps its mode");
    assert_eq!(
        std::fs::read(root.join("tmp/tool")).expect("the file is there"),
        b"tool\n"
    );

    // Without the option the directory entry sets its mode.
    let plain = directory.path().join("plain");
    std::fs::create_dir_all(plain.join("tmp")).expect("the image /tmp is made");
    std::fs::set_permissions(plain.join("tmp"), std::fs::Permissions::from_mode(0o1777))
        .expect("the image /tmp mode is set");
    unpack(
        Cursor::new(bytes),
        &plain,
        &UnpackOptions::default(),
        &UnpackLimits::default(),
    )
    .expect("the archive applies");
    let mode = std::fs::metadata(plain.join("tmp"))
        .expect("the directory is there")
        .mode()
        & 0o7777;
    assert_eq!(mode, 0o755, "the entry sets the mode without the option");
}
