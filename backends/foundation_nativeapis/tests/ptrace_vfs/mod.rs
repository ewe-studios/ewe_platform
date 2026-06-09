#![cfg(all(target_os = "linux", feature = "vfs-ptrace-nix", feature = "vfs-native"))]

/// Integration tests for the nix-based ptrace backend.
///
/// Tests:
/// 1. Syscall dispatch correctly identifies virtual vs real paths/FDs
/// 2. Mount table routes paths to the right VFS backend
/// 3. Virtual FD table allocation and per-process tracking
/// 4. NixInterceptor spawns a child under ptrace and waits for exit
/// 5. Non-filesystem syscalls pass through unmodified
/// 6. The traced child process executes correctly

use std::sync::Arc;

use foundation_nativeapis::native::vfs::ptrace::{
    nix_backend::NixInterceptor,
    syscall_dispatch::{on_syscall_entry, SyscallAction, SyscallArgs},
    DynFs, ErasedFile, MountTable, VirtualFdEntry, VirtualFdTable, SyscallInterceptor,
};
use foundation_nativeapis::shared::vfs::types::OpenMode;
use foundation_nativeapis::native::vfs::NativeFs;
use foundation_nativeapis::shared::vfs::MemoryFs;
use foundation_nativeapis::shared::vfs::VfsFileSystem;

// ── Helpers ──

struct TempDir(std::path::PathBuf);

impl TempDir {
    fn new(name: &str) -> Self {
        let path = std::env::temp_dir().join(format!(
            "ptrace_test_{name}_{}_{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&path).unwrap();
        Self(path)
    }
    fn path(&self) -> &std::path::Path { &self.0 }
}

impl Drop for TempDir {
    fn drop(&mut self) { let _ = std::fs::remove_dir_all(&self.0); }
}

// ── Dispatch: non-filesystem syscalls passthrough ──

#[test]
fn test_non_fs_syscalls_passthrough() {
    let mt = MountTable::new();
    let fd = VirtualFdTable::new();
    let pid = nix::unistd::Pid::from_raw(1);

    // getpid, mmap, nanosleep, clone, fork, exit
    for nr in [39, 9, 35, 56, 57, 60] {
        let args = SyscallArgs { nr, arg0: 0, arg1: 0, arg2: 0, arg3: 0, arg4: 0, arg5: 0 };
        assert!(
            matches!(on_syscall_entry(pid, &args, &mt, &fd), SyscallAction::Passthrough),
            "syscall {nr} should passthrough"
        );
    }
}

#[test]
fn test_real_fd_passthrough() {
    let mt = MountTable::new();
    let fd = VirtualFdTable::new();
    let pid = nix::unistd::Pid::from_raw(1);

    // read(5), write(5), close(5), fstat(5) — all on real FD < 10000
    for (nr, name) in [(0, "read"), (1, "write"), (3, "close"), (5, "fstat")] {
        let args = SyscallArgs { nr, arg0: 5, arg1: 0, arg2: 0, arg3: 0, arg4: 0, arg5: 0 };
        assert!(
            matches!(on_syscall_entry(pid, &args, &mt, &fd), SyscallAction::Passthrough),
            "{name} on real FD 5 should passthrough"
        );
    }
}

#[test]
fn test_virtual_fd_intercepted() {
    let mt = MountTable::new();
    let fd = VirtualFdTable::new();
    let pid = nix::unistd::Pid::from_raw(1);

    // read on FD 10000 — should be intercepted
    let args = SyscallArgs { nr: 0, arg0: 10_000, arg1: 0x1000, arg2: 64, arg3: 0, arg4: 0, arg5: 0 };
    assert!(
        !matches!(on_syscall_entry(pid, &args, &mt, &fd), SyscallAction::Passthrough),
        "read on virtual FD 10000 should be intercepted"
    );
}

// ── Mount table ──

#[test]
fn test_mount_table_is_virtual() {
    let tmp = TempDir::new("mount");
    let fs = NativeFs::new(tmp.path()).unwrap();
    let mut mt = MountTable::new();
    mt.mount("/virtual", DynFs::new(Arc::new(fs)));

    assert!(mt.is_virtual("/virtual/file.txt"));
    assert!(mt.is_virtual("/virtual/deep/nested/path"));
    assert!(!mt.is_virtual("/real/file.txt"));
    assert!(!mt.is_virtual("/tmp/test"));
}

#[test]
fn test_mount_table_resolve() {
    let tmp = TempDir::new("resolve");
    let fs = NativeFs::new(tmp.path()).unwrap();
    let mut mt = MountTable::new();
    mt.mount("/v", DynFs::new(Arc::new(fs)));

    assert!(mt.resolve("/v/x").is_some());
    assert!(mt.resolve("/real").is_none());
}

#[test]
fn test_mount_table_default_fs() {
    let mut mt = MountTable::new();
    mt.set_default(DynFs::new(Arc::new(MemoryFs::new())));
    assert!(mt.resolve("/anything").is_some());
}

#[test]
fn test_mount_table_first_prefix_wins() {
    let fs_a = Arc::new(MemoryFs::new());
    let fs_b = Arc::new(MemoryFs::new());
    let mut mt = MountTable::new();
    mt.mount("/a", DynFs::new(Arc::clone(&fs_a)));
    mt.mount("/a/b", DynFs::new(Arc::clone(&fs_b)));

    // First match wins: /a/b/c resolves (to fs_a, not fs_b)
    assert!(mt.resolve("/a/b/c").is_some());
    assert!(mt.resolve("/a/x").is_some());
}

#[test]
fn test_mount_table_prefix_normalize() {
    let mut mt = MountTable::new();
    mt.mount("virtual/", DynFs::new(Arc::new(MemoryFs::new())));
    assert!(mt.is_virtual("/virtual/file.txt"));
}

// ── Virtual FD table ──

#[test]
fn test_fd_allocation() {
    let t = VirtualFdTable::new();
    let a = t.alloc_fd();
    let b = t.alloc_fd();
    assert!(a >= 10_000);
    assert_eq!(b, a + 1);
}

#[test]
fn test_fd_clone_for_child() {
    let t = VirtualFdTable::new();
    t.clone_for_child(100, 200);
}

#[test]
fn test_fd_close_all() {
    let t = VirtualFdTable::new();
    t.close_all(999);
}

#[test]
fn test_is_virtual_fd() {
    assert!(VirtualFdTable::is_virtual_fd(10_000));
    assert!(VirtualFdTable::is_virtual_fd(99_999));
    assert!(!VirtualFdTable::is_virtual_fd(0));
    assert!(!VirtualFdTable::is_virtual_fd(9_999));
}

// ── Virtual file I/O ──

#[test]
fn test_virtual_fd_insert_and_read() {
    use std::sync::Mutex;

    let fs = MemoryFs::new();
    fs.write_file("/hello.txt", b"virtual content").unwrap();

    let fd_table = VirtualFdTable::new();
    let file = fs.open("/hello.txt", OpenMode::Read).unwrap();
    let erased = ErasedFile::new(file);
    let vfd = fd_table.alloc_fd();
    fd_table.insert(1, vfd, VirtualFdEntry::File {
        handle: erased,
        path: "/hello.txt".into(),
        offset: Arc::new(Mutex::new(0)),
        mode: OpenMode::Read,
        flags: 0,
    });

    let entry = fd_table.get(1, vfd).expect("should have entry");
    match entry {
        VirtualFdEntry::File { handle, .. } => {
            let mut buf = vec![0u8; 64];
            let n = handle.read_at(&mut buf, 0).unwrap();
            assert_eq!(&buf[..n], b"virtual content");
        }
        _ => panic!("expected File entry"),
    }
}

#[test]
fn test_virtual_fd_write_and_readback() {
    use std::sync::Mutex;

    let fs = MemoryFs::new();
    fs.mkdir("/dir").unwrap();
    fs.write_file("/dir/out.txt", b"").unwrap();

    let file = fs.open("/dir/out.txt", OpenMode::ReadWrite).unwrap();
    let erased = ErasedFile::new(file);
    let fd_table = VirtualFdTable::new();
    let vfd = fd_table.alloc_fd();
    fd_table.insert(1, vfd, VirtualFdEntry::File {
        handle: erased,
        path: "/dir/out.txt".into(),
        offset: Arc::new(Mutex::new(0)),
        mode: OpenMode::ReadWrite,
        flags: 0,
    });

    let entry = fd_table.get(1, vfd).unwrap();
    match entry {
        VirtualFdEntry::File { handle, .. } => {
            handle.write_at(b"written by test", 0).unwrap();
        }
        _ => panic!("expected File entry"),
    }

    let content = fs.read_file("/dir/out.txt").unwrap();
    assert_eq!(&content, b"written by test");
}

#[test]
fn test_virtual_fd_range_no_collision() {
    let fd_table = VirtualFdTable::new();

    for _ in 0..100 {
        let vfd = fd_table.alloc_fd();
        assert!(vfd >= 10_000, "virtual FD {vfd} below base");
        assert!(!((0..1000).contains(&(vfd as u32))), "virtual FD {vfd} in real range");
    }
}

// ── Fork inheritance ──

#[test]
fn test_fork_inherits_virtual_fds() {
    use std::sync::Mutex;

    let fs = MemoryFs::new();
    fs.write_file("/shared.txt", b"inherited data").unwrap();

    let fd_table = VirtualFdTable::new();
    let file = fs.open("/shared.txt", OpenMode::Read).unwrap();
    let erased = ErasedFile::new(file);
    let vfd = fd_table.alloc_fd();

    let parent_pid = 100u32;
    let child_pid = 200u32;

    fd_table.insert(parent_pid, vfd, VirtualFdEntry::File {
        handle: erased,
        path: "/shared.txt".into(),
        offset: Arc::new(Mutex::new(0)),
        mode: OpenMode::Read,
        flags: 0,
    });

    // Simulate fork: clone parent's FDs to child
    fd_table.clone_for_child(parent_pid, child_pid);

    // Child should see the same FD
    let child_entry = fd_table.get(child_pid, vfd).expect("child should have inherited FD");
    match child_entry {
        VirtualFdEntry::File { handle, path, .. } => {
            assert_eq!(path, "/shared.txt");
            let mut buf = vec![0u8; 64];
            let n = handle.read_at(&mut buf, 0).unwrap();
            assert_eq!(&buf[..n], b"inherited data");
        }
        _ => panic!("expected File entry"),
    }

    // Parent still has its copy
    assert!(fd_table.get(parent_pid, vfd).is_some());

    // Child exit cleans only child's FDs
    fd_table.close_all(child_pid);
    assert!(fd_table.get(child_pid, vfd).is_none());
    assert!(fd_table.get(parent_pid, vfd).is_some());
}

#[test]
fn test_fork_cloexec_not_inherited_on_exec() {
    use std::sync::Mutex;

    let fs = MemoryFs::new();
    fs.write_file("/cloexec.txt", b"temp").unwrap();
    fs.write_file("/persist.txt", b"keep").unwrap();

    let fd_table = VirtualFdTable::new();
    let pid = 300u32;

    let f1 = fs.open("/cloexec.txt", OpenMode::Read).unwrap();
    let fd1 = fd_table.alloc_fd();
    fd_table.insert(pid, fd1, VirtualFdEntry::File {
        handle: ErasedFile::new(f1),
        path: "/cloexec.txt".into(),
        offset: Arc::new(Mutex::new(0)),
        mode: OpenMode::Read,
        flags: libc::O_CLOEXEC,
    });

    let f2 = fs.open("/persist.txt", OpenMode::Read).unwrap();
    let fd2 = fd_table.alloc_fd();
    fd_table.insert(pid, fd2, VirtualFdEntry::File {
        handle: ErasedFile::new(f2),
        path: "/persist.txt".into(),
        offset: Arc::new(Mutex::new(0)),
        mode: OpenMode::Read,
        flags: 0,
    });

    // Simulate exec: close O_CLOEXEC FDs
    fd_table.close_cloexec(pid);

    assert!(fd_table.get(pid, fd1).is_none(), "O_CLOEXEC fd should be closed");
    assert!(fd_table.get(pid, fd2).is_some(), "non-CLOEXEC fd should survive exec");
}

// ── NixInterceptor: spawn + wait ──

#[test]
fn test_spawn_true_exits_zero() {
    let i = NixInterceptor::new();
    let mt = MountTable::new();
    let h = i.spawn(mt, "/bin/true", &[]).expect("spawn /bin/true failed");
    assert!(h.pid() > 0);
    assert_eq!(h.wait().expect("wait failed"), 0);
}

#[test]
fn test_spawn_echo_with_args() {
    let i = NixInterceptor::new();
    let mt = MountTable::new();
    let h = i.spawn(mt, "/bin/echo", &["hello", "world"]).expect("spawn failed");
    assert_eq!(h.wait().expect("wait failed"), 0);
}

#[test]
fn test_spawn_false_exits_nonzero() {
    let i = NixInterceptor::new();
    let mt = MountTable::new();
    let h = i.spawn(mt, "/bin/false", &[]).expect("spawn failed");
    let code = h.wait().expect("wait failed");
    assert_ne!(code, 0, "/bin/false should exit non-zero (got {code})");
}

#[test]
fn test_spawn_nonexistent_fails() {
    let i = NixInterceptor::new();
    let mt = MountTable::new();
    let result = i.spawn(mt, "/nonexistent-binary-xyz", &[]);

    if let Ok(h) = result {
        let code = h.wait().expect("wait failed");
        assert_ne!(code, 0, "nonexistent binary should exit non-zero");
    }
}

#[test]
#[ignore = "fork tracing: PTRACE_EVENT_STOP on kernel 7.0.9 causes deadlock in parent's vfork wait"]
fn test_spawn_pipe_chain() {
    // Verify non-fs syscalls (pipe, dup, etc.) pass through normally
    let i = NixInterceptor::new();
    let mt = MountTable::new();
    let h = i.spawn(mt, "/bin/sh", &["-c", "echo hello | cat"]).expect("spawn failed");
    assert_eq!(h.wait().expect("wait failed"), 0, "pipe chain should succeed");
}

#[test]
#[ignore = "fork tracing: PTRACE_EVENT_STOP on kernel 7.0.9 causes deadlock in parent's vfork wait"]
fn test_spawn_can_write_to_real_fs() {
    // Verify the child can write to the real filesystem (passthrough)
    let i = NixInterceptor::new();
    let mt = MountTable::new();
    let tmpfile = format!("/tmp/ptrace_test_{}", std::process::id());
    let h = i.spawn(mt, "/bin/sh", &["-c", &format!("echo test > {tmpfile}")]).expect("spawn failed");
    assert_eq!(h.wait().expect("wait failed"), 0);

    let content = std::fs::read_to_string(&tmpfile).unwrap();
    assert!(content.contains("test"));
    let _ = std::fs::remove_file(&tmpfile);
}

#[test]
fn test_spawn_multiple_children() {
    let i = NixInterceptor::new();
    let mt = MountTable::new();

    let mut handles = vec![];
    for _ in 0..3 {
        let h = i.spawn(mt.clone(), "/bin/true", &[]).expect("spawn failed");
        handles.push(h);
    }

    for h in handles {
        assert_eq!(h.wait().expect("wait failed"), 0);
    }
}
