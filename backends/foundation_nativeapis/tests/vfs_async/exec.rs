//! `exec_async`: the sync boundary of the VFS.

use foundation_nativeapis::shared::vfs::exec_async::{exec_async, poll_once};
use foundation_nativeapis::shared::vfs::{MemoryFs, SyncFs, VfsFileSystem};

/// A future that is ready at once runs on the calling thread: no valtron
/// pool is needed (none is started in this test).
#[test]
fn ready_futures_need_no_pool() {
    assert_eq!(exec_async(async { Ok(7) }).unwrap(), 7);

    let fs = SyncFs::new(MemoryFs::new());
    fs.write_file("/a.txt", b"x").unwrap();
    assert_eq!(fs.read_file("/a.txt").unwrap(), b"x");
}

#[test]
fn poll_once_reports_pending() {
    assert_eq!(poll_once(async { 3 }), Some(3));
    assert_eq!(poll_once(std::future::pending::<u8>()), None);
}
