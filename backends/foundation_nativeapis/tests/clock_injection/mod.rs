//! The VFS timestamps come from the injected clock.
//!
//! The test sets a simulated wall origin, writes a file, and asserts that its
//! `modified` time is the simulated one. It then advances the wall clock and
//! asserts that the next write moves it. A real-clock read fails both checks.

#![cfg(feature = "vfs")]

use std::sync::Arc;
use std::time::{Duration, UNIX_EPOCH};

use foundation_core::valtron::{Ctx, SimulatedClock};
use foundation_nativeapis::shared::vfs::VfsFile;
use foundation_nativeapis::{MemoryFs, VfsFileSystem};

const ORIGIN_MILLIS: u64 = 1_700_000_000_000;

#[test]
fn memory_fs_timestamps_follow_the_injected_clock() {
    let clock = Arc::new(SimulatedClock::new(Duration::ZERO));
    clock.advance_wall(Duration::from_millis(ORIGIN_MILLIS));

    let fs = MemoryFs::new(Ctx::fixed(clock.clone()));
    let file = fs.create("/hello.txt", 0o644).unwrap();
    file.write_at(b"hello", 0).unwrap();

    let origin = UNIX_EPOCH + Duration::from_millis(ORIGIN_MILLIS);
    assert_eq!(
        fs.stat("/hello.txt").unwrap().modified,
        Some(origin),
        "the first write stamps the simulated wall time"
    );

    clock.advance_wall(Duration::from_secs(10));
    file.write_at(b" world", 5).unwrap();

    let after = origin + Duration::from_secs(10);
    assert_eq!(
        fs.stat("/hello.txt").unwrap().modified,
        Some(after),
        "the second write stamps the advanced simulated wall time"
    );
}
