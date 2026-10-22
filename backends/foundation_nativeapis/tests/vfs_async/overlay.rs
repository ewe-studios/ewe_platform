//! `OverlayFileSystem` through the async traits.

use foundation_core::valtron::valtron_test;
use foundation_nativeapis::shared::vfs::{
    AsyncVfsDirectory, AsyncVfsFileSystem, MemoryDelta, MemoryFs, OverlayFileSystem, VfsFileSystem,
};

fn overlay_with_base_dir() -> OverlayFileSystem<MemoryFs, MemoryDelta> {
    let base = MemoryFs::new();
    base.mkdir_all("/src/deep").unwrap();
    base.write_file("/src/main.rs", b"fn main() {}").unwrap();
    OverlayFileSystem::new(base, MemoryDelta::new())
}

/// Bug fix: a directory handle held raw pointers to the layers, so it was a
/// dangling pointer once the overlay was dropped. It now shares the layers.
#[valtron_test]
async fn directory_handle_outlives_the_overlay() {
    let overlay = overlay_with_base_dir();
    let dir = overlay.open_directory_async("/src".into()).await.unwrap();
    drop(overlay);

    let names: Vec<String> = dir.list_async().await.unwrap().into_iter().map(|e| e.name).collect();
    assert_eq!(names, vec!["deep".to_string(), "main.rs".to_string()]);
    assert!(dir.exists_async("main.rs".into()).await.unwrap());
}

/// Bug fix: `create_file` on a directory handle of a base-only directory did
/// not create the parent directories in the delta, so it failed.
#[valtron_test]
async fn create_file_in_a_base_only_directory() {
    let overlay = overlay_with_base_dir();
    let deep = overlay.open_directory_async("/src/deep".into()).await.unwrap();
    let file = deep.create_file_async("new.rs".into(), 0o644).await.unwrap();
    drop(file);

    assert!(overlay.exists_async("/src/deep/new.rs".into()).await.unwrap());
    assert!(overlay.delta().exists("/src/deep/new.rs").unwrap());
    assert!(!overlay.base().exists("/src/deep/new.rs").unwrap());
}

#[valtron_test]
async fn copy_on_write_and_whiteout() {
    let overlay = overlay_with_base_dir();
    overlay
        .write_file_async("/src/main.rs".into(), b"changed".to_vec())
        .await
        .unwrap();
    assert_eq!(overlay.read_file_async("/src/main.rs".into()).await.unwrap(), b"changed");
    assert_eq!(overlay.base().read_file("/src/main.rs").unwrap(), b"fn main() {}");

    overlay.remove_async("/src/main.rs".into()).await.unwrap();
    assert!(!overlay.exists_async("/src/main.rs".into()).await.unwrap());
    let names: Vec<String> = overlay
        .open_directory_async("/src".into())
        .await
        .unwrap()
        .list_async()
        .await
        .unwrap()
        .into_iter()
        .map(|e| e.name)
        .collect();
    assert_eq!(names, vec!["deep".to_string()]);
}
