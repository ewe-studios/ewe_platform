//! The step key and tree hash rows of feature 14, section 7: I3 and the tree
//! hash cases.

use std::path::Path;
use std::sync::Arc;
use std::time::{Duration, SystemTime};

use foundation_sandbox::native::host::NativeFs;
use foundation_sandbox::native::images::build::keys::{
    base_key, step_key, tree_hash, TreeEntry, TreeKind,
};
use foundation_sandbox::native::images::build::steps::plan_add;
use foundation_sandbox::shared::host::HostFs;
use foundation_sandbox::shared::limits::BuildLimits;
use foundation_sandbox::shared::spec::GuestArch;

use super::support::{write_file, TempDir};

/// The tree hash of one context.
fn hash_of(context: &Path, source: &str) -> String {
    let fs: Arc<dyn HostFs> = Arc::new(NativeFs);
    plan_add(&fs, context, source, "/app", None, &BuildLimits::default())
        .expect("the ADD plan is built")
        .tree_hash
}

/// Sets the modification time of one file.
fn touch(path: &Path, seconds: u64) {
    let file = std::fs::OpenOptions::new()
        .write(true)
        .open(path)
        .expect("the file is open for a time change");
    let time = SystemTime::UNIX_EPOCH + Duration::from_secs(seconds);
    file.set_times(
        std::fs::FileTimes::new()
            .set_accessed(time)
            .set_modified(time),
    )
    .expect("the time changes");
}

#[test]
fn key_depends_only_on_content() {
    let directory = TempDir::new("keys");
    let context = directory.path().join("context");
    write_file(&context, "app/main.py", b"print(1)\n");
    write_file(&context, "app/data", b"data\n");
    let first = hash_of(&context, "./app");

    // A modification time is not content.
    touch(&context.join("app/main.py"), 1_000_000);
    assert_eq!(
        hash_of(&context, "./app"),
        first,
        "a touched file is not a change"
    );

    // A host path prefix is not content.
    let other = directory.path().join("other");
    write_file(&other, "app/main.py", b"print(1)\n");
    write_file(&other, "app/data", b"data\n");
    assert_eq!(
        hash_of(&other, "./app"),
        first,
        "the host path is not content"
    );

    // One changed byte is a change.
    write_file(&context, "app/main.py", b"print(2)\n");
    assert_ne!(
        hash_of(&context, "./app"),
        first,
        "a changed file is a change"
    );

    // The step key changes with the tree hash and with the key before.
    let line = "ADD ./app /app\n";
    let a = step_key("before", line, Some(&first));
    let b = step_key("before", line, Some(&first));
    assert_eq!(a, b);
    assert_ne!(a, step_key("other", line, Some(&first)));
    assert_ne!(a, step_key("before", "ADD ./app /other\n", Some(&first)));
    assert_ne!(a, step_key("before", line, Some("different")));

    // A base key depends on each input, and only on them.
    let base = base_key("imagekey", GuestArch::host(), 4096);
    assert_eq!(base, base_key("imagekey", GuestArch::host(), 4096));
    assert_ne!(base, base_key("other", GuestArch::host(), 4096));
    assert_ne!(base, base_key("imagekey", GuestArch::host(), 2048));
    assert_eq!(base.len(), 32);
}

#[test]
fn tree_hash_cases() {
    let entries = vec![
        TreeEntry {
            path: "app".to_string(),
            kind: TreeKind::Dir,
            mode: 0o755,
            size: 0,
            content: String::new(),
        },
        TreeEntry {
            path: "app/main.py".to_string(),
            kind: TreeKind::File,
            mode: 0o644,
            size: 9,
            content: "hash-one".to_string(),
        },
        TreeEntry {
            path: "app/link".to_string(),
            kind: TreeKind::Symlink,
            mode: 0o777,
            size: 7,
            content: "main.py".to_string(),
        },
    ];
    let first = tree_hash(&entries);

    // The order of the input list does not matter: the hash sorts.
    let mut reversed = entries.clone();
    reversed.reverse();
    assert_eq!(tree_hash(&reversed), first);

    // A mode, a content hash and a link target are content.
    let mut changed_mode = entries.clone();
    changed_mode[1].mode = 0o600;
    assert_ne!(tree_hash(&changed_mode), first);
    let mut changed_content = entries.clone();
    changed_content[1].content = "hash-two".to_string();
    assert_ne!(tree_hash(&changed_content), first);
    let mut changed_link = entries.clone();
    changed_link[2].content = "other.py".to_string();
    assert_ne!(tree_hash(&changed_link), first);

    // A modification time and an owner are not in a tree entry at all.
    let without_time = entries.clone();
    assert_eq!(tree_hash(&without_time), first);
}
