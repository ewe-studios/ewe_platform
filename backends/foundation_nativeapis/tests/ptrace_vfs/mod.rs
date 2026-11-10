#![cfg(all(
    target_os = "linux",
    feature = "vfs-ptrace-nix",
    feature = "vfs-native"
))]

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
    syscall_nr::{DUP, DUP2, DUP3, OPEN, OPENAT, WRITE},
    DynFs, ErasedFile, MountTable, SyscallInterceptor, VirtualFdEntry, VirtualFdTable,
    FD_VIRTUAL_BASE,
};
use foundation_nativeapis::native::vfs::NativeFs;
use foundation_nativeapis::shared::vfs::types::OpenMode;
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
    fn path(&self) -> &std::path::Path {
        &self.0
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

// ── Dispatch: non-filesystem syscalls passthrough ──

#[test]
fn test_non_fs_syscalls_passthrough() {
    let mt = MountTable::new();
    let fd = VirtualFdTable::new();
    let pid = nix::unistd::Pid::from_raw(1);

    // getpid, mmap, nanosleep, clone, fork, exit
    for nr in [39, 9, 35, 56, 57, 60] {
        let args = SyscallArgs {
            nr,
            arg0: 0,
            arg1: 0,
            arg2: 0,
            arg3: 0,
            arg4: 0,
            arg5: 0,
        };
        assert!(
            matches!(
                on_syscall_entry(pid, &args, &mt, &fd),
                SyscallAction::Passthrough
            ),
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
        let args = SyscallArgs {
            nr,
            arg0: 5,
            arg1: 0,
            arg2: 0,
            arg3: 0,
            arg4: 0,
            arg5: 0,
        };
        assert!(
            matches!(
                on_syscall_entry(pid, &args, &mt, &fd),
                SyscallAction::Passthrough
            ),
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
    let args = SyscallArgs {
        nr: 0,
        arg0: 10_000,
        arg1: 0x1000,
        arg2: 64,
        arg3: 0,
        arg4: 0,
        arg5: 0,
    };
    assert!(
        !matches!(
            on_syscall_entry(pid, &args, &mt, &fd),
            SyscallAction::Passthrough
        ),
        "read on virtual FD 10000 should be intercepted"
    );
}

// ── Mount table ──

#[test]
fn test_mount_table_is_virtual() {
    let tmp = TempDir::new("mount");
    let fs = NativeFs::new(tmp.path()).unwrap();
    let mut mt = MountTable::new();
    mt.mount("/virtual", DynFs::from_async(fs));

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
    mt.mount("/v", DynFs::from_async(fs));

    assert!(mt.resolve("/v/x").is_some());
    assert!(mt.resolve("/real").is_none());
}

#[test]
fn test_mount_table_default_fs() {
    let mut mt = MountTable::new();
    mt.set_default(DynFs::new(Arc::new(MemoryFs::new(
        foundation_core::valtron::Ctx::fixed(std::sync::Arc::new(
            foundation_core::valtron::MonotonicClock::new(),
        )),
    ))));
    assert!(mt.resolve("/anything").is_some());
}

#[test]
fn test_mount_table_first_prefix_wins() {
    let fs_a = Arc::new(MemoryFs::new(foundation_core::valtron::Ctx::fixed(
        std::sync::Arc::new(foundation_core::valtron::MonotonicClock::new()),
    )));
    let fs_b = Arc::new(MemoryFs::new(foundation_core::valtron::Ctx::fixed(
        std::sync::Arc::new(foundation_core::valtron::MonotonicClock::new()),
    )));
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
    mt.mount(
        "virtual/",
        DynFs::new(Arc::new(MemoryFs::new(
            foundation_core::valtron::Ctx::fixed(std::sync::Arc::new(
                foundation_core::valtron::MonotonicClock::new(),
            )),
        ))),
    );
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

    let fs = MemoryFs::new(foundation_core::valtron::Ctx::fixed(std::sync::Arc::new(
        foundation_core::valtron::MonotonicClock::new(),
    )));
    fs.write_file("/hello.txt", b"virtual content").unwrap();

    let fd_table = VirtualFdTable::new();
    let file = fs.open("/hello.txt", OpenMode::Read).unwrap();
    let erased = ErasedFile::new(file);
    let vfd = fd_table.alloc_fd();
    fd_table.insert(
        1,
        vfd,
        VirtualFdEntry::File {
            handle: erased,
            path: "/hello.txt".into(),
            offset: Arc::new(Mutex::new(0)),
            mode: OpenMode::Read,
            flags: 0,
        },
    );

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

    let fs = MemoryFs::new(foundation_core::valtron::Ctx::fixed(std::sync::Arc::new(
        foundation_core::valtron::MonotonicClock::new(),
    )));
    fs.mkdir("/dir").unwrap();
    fs.write_file("/dir/out.txt", b"").unwrap();

    let file = fs.open("/dir/out.txt", OpenMode::ReadWrite).unwrap();
    let erased = ErasedFile::new(file);
    let fd_table = VirtualFdTable::new();
    let vfd = fd_table.alloc_fd();
    fd_table.insert(
        1,
        vfd,
        VirtualFdEntry::File {
            handle: erased,
            path: "/dir/out.txt".into(),
            offset: Arc::new(Mutex::new(0)),
            mode: OpenMode::ReadWrite,
            flags: 0,
        },
    );

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
        assert!(
            !((0..1000).contains(&(vfd as u32))),
            "virtual FD {vfd} in real range"
        );
    }
}

// ── Fork inheritance ──

#[test]
fn test_fork_inherits_virtual_fds() {
    use std::sync::Mutex;

    let fs = MemoryFs::new(foundation_core::valtron::Ctx::fixed(std::sync::Arc::new(
        foundation_core::valtron::MonotonicClock::new(),
    )));
    fs.write_file("/shared.txt", b"inherited data").unwrap();

    let fd_table = VirtualFdTable::new();
    let file = fs.open("/shared.txt", OpenMode::Read).unwrap();
    let erased = ErasedFile::new(file);
    let vfd = fd_table.alloc_fd();

    let parent_pid = 100u32;
    let child_pid = 200u32;

    fd_table.insert(
        parent_pid,
        vfd,
        VirtualFdEntry::File {
            handle: erased,
            path: "/shared.txt".into(),
            offset: Arc::new(Mutex::new(0)),
            mode: OpenMode::Read,
            flags: 0,
        },
    );

    // Simulate fork: clone parent's FDs to child
    fd_table.clone_for_child(parent_pid, child_pid);

    // Child should see the same FD
    let child_entry = fd_table
        .get(child_pid, vfd)
        .expect("child should have inherited FD");
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

    let fs = MemoryFs::new(foundation_core::valtron::Ctx::fixed(std::sync::Arc::new(
        foundation_core::valtron::MonotonicClock::new(),
    )));
    fs.write_file("/cloexec.txt", b"temp").unwrap();
    fs.write_file("/persist.txt", b"keep").unwrap();

    let fd_table = VirtualFdTable::new();
    let pid = 300u32;

    let f1 = fs.open("/cloexec.txt", OpenMode::Read).unwrap();
    let fd1 = fd_table.alloc_fd();
    fd_table.insert(
        pid,
        fd1,
        VirtualFdEntry::File {
            handle: ErasedFile::new(f1),
            path: "/cloexec.txt".into(),
            offset: Arc::new(Mutex::new(0)),
            mode: OpenMode::Read,
            flags: libc::O_CLOEXEC,
        },
    );

    let f2 = fs.open("/persist.txt", OpenMode::Read).unwrap();
    let fd2 = fd_table.alloc_fd();
    fd_table.insert(
        pid,
        fd2,
        VirtualFdEntry::File {
            handle: ErasedFile::new(f2),
            path: "/persist.txt".into(),
            offset: Arc::new(Mutex::new(0)),
            mode: OpenMode::Read,
            flags: 0,
        },
    );

    // Simulate exec: close O_CLOEXEC FDs
    fd_table.close_cloexec(pid);

    assert!(
        fd_table.get(pid, fd1).is_none(),
        "O_CLOEXEC fd should be closed"
    );
    assert!(
        fd_table.get(pid, fd2).is_some(),
        "non-CLOEXEC fd should survive exec"
    );
}

// ── NixInterceptor: spawn + wait ──

#[test]
#[ntest::timeout(60_000)]
fn test_spawn_true_exits_zero() {
    let i = NixInterceptor::new();
    let mt = MountTable::new();
    let h = i
        .spawn(mt, "/bin/true", &[])
        .expect("spawn /bin/true failed");
    assert!(h.pid() > 0);
    assert_eq!(h.wait().expect("wait failed"), 0);
}

#[test]
#[ntest::timeout(60_000)]
fn test_spawn_echo_with_args() {
    let i = NixInterceptor::new();
    let mt = MountTable::new();
    let h = i
        .spawn(mt, "/bin/echo", &["hello", "world"])
        .expect("spawn failed");
    assert_eq!(h.wait().expect("wait failed"), 0);
}

#[test]
#[ntest::timeout(60_000)]
fn test_spawn_false_exits_nonzero() {
    let i = NixInterceptor::new();
    let mt = MountTable::new();
    let h = i.spawn(mt, "/bin/false", &[]).expect("spawn failed");
    let code = h.wait().expect("wait failed");
    assert_ne!(code, 0, "/bin/false should exit non-zero (got {code})");
}

#[test]
#[ntest::timeout(60_000)]
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
#[ntest::timeout(60_000)]
fn test_spawn_pipe_chain() {
    // Verify non-fs syscalls (pipe, dup, etc.) pass through normally
    let i = NixInterceptor::new();
    let mt = MountTable::new();
    let h = i
        .spawn(mt, "/bin/sh", &["-c", "echo hello | cat"])
        .expect("spawn failed");
    assert_eq!(
        h.wait().expect("wait failed"),
        0,
        "pipe chain should succeed"
    );
}

#[test]
#[ntest::timeout(60_000)]
fn test_spawn_can_write_to_real_fs() {
    // Verify the child can write to the real filesystem (passthrough)
    let i = NixInterceptor::new();
    let mt = MountTable::new();
    let tmpfile = format!("/tmp/ptrace_test_{}", std::process::id());
    let h = i
        .spawn(mt, "/bin/sh", &["-c", &format!("echo test > {tmpfile}")])
        .expect("spawn failed");
    assert_eq!(h.wait().expect("wait failed"), 0);

    let content = std::fs::read_to_string(&tmpfile).unwrap();
    assert!(content.contains("test"));
    let _ = std::fs::remove_file(&tmpfile);
}

#[test]
#[ntest::timeout(60_000)]
fn fork_child_first_stop_is_consumed() {
    // The shell forks a child for the background command. The tracer must
    // consume that child's first SIGSTOP and let it run to exit.
    let i = NixInterceptor::new();
    let mt = MountTable::new();
    let h = i
        .spawn(mt, "/bin/sh", &["-c", "true & wait"])
        .expect("spawn failed");
    assert_eq!(h.wait().expect("wait failed"), 0);
}

#[test]
#[ntest::timeout(60_000)]
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

// ── NixInterceptor: the traced child sees the virtual mount ──

/// FNV-1a over 64 bits. The test needs a stable value on every host and does
/// not want a new dependency for one checksum.
fn fnv1a64(bytes: &[u8]) -> u64 {
    let mut hash = 0xcbf2_9ce4_8422_2325_u64;
    for byte in bytes {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3_u64);
    }
    hash
}

#[test]
#[ntest::timeout(60_000)]
fn the_traced_child_reads_the_virtual_mount() {
    // The child's `cat` must get its bytes from the MemoryFs, not from the
    // real filesystem: without the syscall loop /virtual does not exist.
    let i = NixInterceptor::new();
    let fs = MemoryFs::new(foundation_core::valtron::Ctx::fixed(std::sync::Arc::new(
        foundation_core::valtron::MonotonicClock::new(),
    )));
    fs.mkdir("/virtual").unwrap();
    fs.write_file("/virtual/hello.txt", b"virtual mount content\n")
        .unwrap();

    let mut mt = MountTable::new();
    mt.mount("/virtual", DynFs::new(Arc::new(fs.clone())));

    let tmp = TempDir::new("read_virtual");
    let out = tmp.path().join("out.txt");
    let script = format!("cat /virtual/hello.txt > {}", out.display());
    let h = i
        .spawn(mt, "/bin/sh", &["-c", &script])
        .expect("spawn failed");
    assert_eq!(
        h.wait().expect("wait failed"),
        0,
        "the traced child must read /virtual through the mount table"
    );
    assert_eq!(
        std::fs::read(&out).unwrap(),
        b"virtual mount content\n",
        "the real file must hold the virtual file's bytes"
    );
}

#[test]
#[ntest::timeout(60_000)]
fn the_traced_child_writes_to_the_virtual_mount() {
    // A shell redirect opens the path and then `dup2`s the virtual fd to
    // stdout, so dispatch must follow the entry, not the fd number.
    let i = NixInterceptor::new();
    let fs = MemoryFs::new(foundation_core::valtron::Ctx::fixed(std::sync::Arc::new(
        foundation_core::valtron::MonotonicClock::new(),
    )));
    fs.mkdir("/virtual").unwrap();

    let mut mt = MountTable::new();
    mt.mount("/virtual", DynFs::new(Arc::new(fs.clone())));

    let h = i
        .spawn(
            mt,
            "/bin/sh",
            &["-c", "echo virtual-write > /virtual/written.txt"],
        )
        .expect("spawn failed");
    assert_eq!(
        h.wait().expect("wait failed"),
        0,
        "the traced child must write /virtual through the mount table"
    );
    assert_eq!(
        fs.read_file("/virtual/written.txt").unwrap(),
        b"virtual-write\n",
        "the MemoryFs must hold what the child wrote"
    );
}

// ── open(2) and dup2(2) rules (feature 111) ──

/// Put `path` in this process's memory, NUL-terminated and padded.
///
/// The dispatcher reads a path argument with `process_vm_readv(pid, ...)`.
/// When `pid` is this process, that call reads these bytes, so the open
/// dispatch runs without a traced child.
fn path_in_self(path: &str) -> Vec<u8> {
    let mut bytes = vec![0u8; path.len() + 64];
    bytes[..path.len()].copy_from_slice(path.as_bytes());
    bytes
}

fn action_name(action: &SyscallAction) -> &'static str {
    match action {
        SyscallAction::Passthrough => "passthrough",
        SyscallAction::Skip { .. } => "skip",
        SyscallAction::SkipError { .. } => "skip error",
        SyscallAction::CloseThenSkip { .. } => "close then skip",
    }
}

fn skip_errno(action: &SyscallAction) -> Option<i64> {
    match action {
        SyscallAction::SkipError { errno } => Some(*errno),
        _ => None,
    }
}

fn skip_value(action: &SyscallAction) -> Option<i64> {
    match action {
        SyscallAction::Skip { return_value } => Some(*return_value),
        _ => None,
    }
}

/// The fd a `dup` reports, whether or not it closes a real destination first.
fn reported_value(action: &SyscallAction) -> Option<i64> {
    match action {
        SyscallAction::Skip { return_value }
        | SyscallAction::CloseThenSkip { return_value, .. } => Some(*return_value),
        _ => None,
    }
}

/// Run `openat(2)` dispatch for `path` in this process.
fn dispatch_openat(
    pid: nix::unistd::Pid,
    path: &[u8],
    flags: i32,
    mode: u32,
    mount_table: &MountTable,
    fd_table: &VirtualFdTable,
) -> SyscallAction {
    let args = SyscallArgs {
        nr: OPENAT,
        arg0: libc::AT_FDCWD as u64,
        arg1: path.as_ptr() as u64,
        arg2: flags as u64,
        arg3: mode as u64,
        arg4: 0,
        arg5: 0,
    };
    on_syscall_entry(pid, &args, mount_table, fd_table)
}

/// Run `open(2)` dispatch for `path` in this process.
fn dispatch_open(
    pid: nix::unistd::Pid,
    path: &[u8],
    flags: i32,
    mode: u32,
    mount_table: &MountTable,
    fd_table: &VirtualFdTable,
) -> SyscallAction {
    let args = SyscallArgs {
        nr: OPEN,
        arg0: path.as_ptr() as u64,
        arg1: flags as u64,
        arg2: mode as u64,
        arg3: 0,
        arg4: 0,
        arg5: 0,
    };
    on_syscall_entry(pid, &args, mount_table, fd_table)
}

/// Run `write(2)` dispatch for a virtual fd in this process.
fn dispatch_write(
    pid: nix::unistd::Pid,
    fd: u64,
    buf: &[u8],
    mount_table: &MountTable,
    fd_table: &VirtualFdTable,
) -> SyscallAction {
    let args = SyscallArgs {
        nr: WRITE,
        arg0: fd,
        arg1: buf.as_ptr() as u64,
        arg2: buf.len() as u64,
        arg3: 0,
        arg4: 0,
        arg5: 0,
    };
    on_syscall_entry(pid, &args, mount_table, fd_table)
}

/// Insert a virtual file entry at `fd` for `pid`.
fn insert_virtual_file(
    fs: &MemoryFs,
    fd_table: &VirtualFdTable,
    pid: u32,
    fd: u64,
    path: &str,
    flags: i32,
) {
    let file = fs.open(path, OpenMode::ReadWrite).unwrap();
    fd_table.insert(
        pid,
        fd,
        VirtualFdEntry::File {
            handle: ErasedFile::new(file),
            path: path.into(),
            offset: Arc::new(std::sync::Mutex::new(0)),
            mode: OpenMode::ReadWrite,
            flags,
        },
    );
}

/// The open flags stored on a virtual file entry.
fn entry_flags(fd_table: &VirtualFdTable, pid: u32, fd: u64) -> Option<i32> {
    match fd_table.get(pid, fd)? {
        VirtualFdEntry::File { flags, .. } => Some(flags),
        VirtualFdEntry::Directory { .. } => None,
    }
}

/// The tracked file offset on a virtual file entry.
fn entry_offset(fd_table: &VirtualFdTable, pid: u32, fd: u64) -> Option<u64> {
    match fd_table.get(pid, fd)? {
        VirtualFdEntry::File { offset, .. } => Some(*offset.lock().unwrap()),
        VirtualFdEntry::Directory { .. } => None,
    }
}

#[test]
fn open_dispatch_truncates_an_existing_virtual_file() {
    let fs = MemoryFs::new(foundation_core::valtron::Ctx::fixed(std::sync::Arc::new(
        foundation_core::valtron::MonotonicClock::new(),
    )));
    fs.mkdir("/virtual").unwrap();
    let mut mt = MountTable::new();
    mt.mount("/virtual", DynFs::new(Arc::new(fs.clone())));
    let fd_table = VirtualFdTable::new();
    let pid = nix::unistd::Pid::from_raw(std::process::id() as i32);
    let pid_raw = std::process::id();

    // O_CREAT without O_EXCL opens the existing file, and O_TRUNC empties it.
    fs.write_file("/virtual/existing.txt", b"old bytes that are longer\n")
        .unwrap();
    let path = path_in_self("/virtual/existing.txt");
    let action = dispatch_openat(
        pid,
        &path,
        libc::O_WRONLY | libc::O_CREAT | libc::O_TRUNC,
        0o644,
        &mt,
        &fd_table,
    );
    match action {
        SyscallAction::Skip { return_value } => {
            assert!(
                VirtualFdTable::is_virtual_fd(return_value as u64),
                "open must return a virtual fd, got {return_value}"
            );
            assert!(
                fd_table.get(pid_raw, return_value as u64).is_some(),
                "the open must insert the virtual entry"
            );
        }
        other => panic!(
            "O_CREAT on an existing file must open it, got {}",
            action_name(&other)
        ),
    }
    assert_eq!(
        fs.read_file("/virtual/existing.txt").unwrap(),
        b"",
        "O_TRUNC must empty the existing file"
    );

    // O_TRUNC without O_CREAT also opens and truncates an existing file.
    fs.write_file("/virtual/existing.txt", b"more old bytes\n")
        .unwrap();
    let action = dispatch_openat(
        pid,
        &path,
        libc::O_WRONLY | libc::O_TRUNC,
        0o644,
        &mt,
        &fd_table,
    );
    assert!(
        matches!(&action, SyscallAction::Skip { .. }),
        "O_TRUNC without O_CREAT must open the existing file, got {}",
        action_name(&action)
    );
    assert_eq!(fs.read_file("/virtual/existing.txt").unwrap(), b"");

    // O_CREAT without O_TRUNC keeps the content.
    fs.write_file("/virtual/existing.txt", b"kept\n").unwrap();
    let action = dispatch_openat(
        pid,
        &path,
        libc::O_WRONLY | libc::O_CREAT,
        0o644,
        &mt,
        &fd_table,
    );
    assert!(
        matches!(&action, SyscallAction::Skip { .. }),
        "O_CREAT without O_TRUNC must open the existing file, got {}",
        action_name(&action)
    );
    assert_eq!(fs.read_file("/virtual/existing.txt").unwrap(), b"kept\n");

    // O_CREAT with O_EXCL on an existing file is EEXIST.
    let action = dispatch_openat(
        pid,
        &path,
        libc::O_WRONLY | libc::O_CREAT | libc::O_EXCL,
        0o644,
        &mt,
        &fd_table,
    );
    assert_eq!(
        skip_errno(&action),
        Some(libc::EEXIST as i64),
        "O_CREAT|O_EXCL on an existing file must fail with EEXIST, got {}",
        action_name(&action)
    );
}

#[test]
fn open_dispatch_reports_or_creates_a_missing_virtual_file() {
    let fs = MemoryFs::new(foundation_core::valtron::Ctx::fixed(std::sync::Arc::new(
        foundation_core::valtron::MonotonicClock::new(),
    )));
    fs.mkdir("/virtual").unwrap();
    let mut mt = MountTable::new();
    mt.mount("/virtual", DynFs::new(Arc::new(fs.clone())));
    let fd_table = VirtualFdTable::new();
    let pid = nix::unistd::Pid::from_raw(std::process::id() as i32);

    // O_TRUNC without O_CREAT on a missing file is ENOENT.
    let path = path_in_self("/virtual/missing.txt");
    let action = dispatch_openat(
        pid,
        &path,
        libc::O_WRONLY | libc::O_TRUNC,
        0o644,
        &mt,
        &fd_table,
    );
    assert_eq!(
        skip_errno(&action),
        Some(libc::ENOENT as i64),
        "O_TRUNC without O_CREAT on a missing file must fail with ENOENT, got {}",
        action_name(&action)
    );
    assert!(!fs.exists("/virtual/missing.txt").unwrap());

    // O_CREAT on a missing file creates it.
    let action = dispatch_openat(
        pid,
        &path,
        libc::O_WRONLY | libc::O_CREAT,
        0o644,
        &mt,
        &fd_table,
    );
    assert!(
        matches!(&action, SyscallAction::Skip { .. }),
        "O_CREAT on a missing file must create it, got {}",
        action_name(&action)
    );
    assert!(fs.exists("/virtual/missing.txt").unwrap());
}

#[test]
fn dup2_dispatch_closes_the_real_destination() {
    use std::sync::Mutex;

    let fs = MemoryFs::new(foundation_core::valtron::Ctx::fixed(std::sync::Arc::new(
        foundation_core::valtron::MonotonicClock::new(),
    )));
    fs.mkdir("/virtual").unwrap();
    fs.write_file("/virtual/vfile.txt", b"virtual\n").unwrap();

    // The dup dispatch reads `RLIMIT_NOFILE` from `/proc/<pid>/limits`, so
    // use this process's pid: it is the tracee the dispatch sees.
    let pid = std::process::id();
    let fd_table = VirtualFdTable::new();
    let file = fs.open("/virtual/vfile.txt", OpenMode::Read).unwrap();
    fd_table.insert(
        pid,
        10_000,
        VirtualFdEntry::File {
            handle: ErasedFile::new(file),
            path: "/virtual/vfile.txt".into(),
            offset: Arc::new(Mutex::new(0)),
            mode: OpenMode::Read,
            flags: 0,
        },
    );

    let mount_table = MountTable::new();
    let args = SyscallArgs {
        nr: DUP2,
        arg0: 10_000,
        arg1: 1,
        arg2: 0,
        arg3: 0,
        arg4: 0,
        arg5: 0,
    };
    let action = on_syscall_entry(
        nix::unistd::Pid::from_raw(pid as i32),
        &args,
        &mount_table,
        &fd_table,
    );

    match action {
        SyscallAction::CloseThenSkip { fd, return_value } => {
            assert_eq!(fd, 1, "the displaced real fd is the destination");
            assert_eq!(return_value, 1, "dup2 reports the new fd");
        }
        other => panic!(
            "dup2 over a real fd must run close(1) in place of the call, got {}",
            action_name(&other)
        ),
    }
    assert!(
        fd_table.get(pid, 1).is_some(),
        "the virtual entry must move to the new fd"
    );
    assert!(
        fd_table.get(pid, 10_000).is_some(),
        "the source virtual entry must stay open"
    );
}

#[test]
fn dup3_dispatch_rejects_equal_fds() {
    use std::sync::Mutex;

    let fs = MemoryFs::new(foundation_core::valtron::Ctx::fixed(std::sync::Arc::new(
        foundation_core::valtron::MonotonicClock::new(),
    )));
    fs.mkdir("/virtual").unwrap();
    fs.write_file("/virtual/vfile.txt", b"virtual\n").unwrap();

    let pid = 4_242u32;
    let fd_table = VirtualFdTable::new();
    let file = fs.open("/virtual/vfile.txt", OpenMode::Read).unwrap();
    fd_table.insert(
        pid,
        10_000,
        VirtualFdEntry::File {
            handle: ErasedFile::new(file),
            path: "/virtual/vfile.txt".into(),
            offset: Arc::new(Mutex::new(0)),
            mode: OpenMode::Read,
            flags: 0,
        },
    );

    let mount_table = MountTable::new();
    let args = SyscallArgs {
        nr: DUP3,
        arg0: 10_000,
        arg1: 10_000,
        arg2: 0,
        arg3: 0,
        arg4: 0,
        arg5: 0,
    };
    let action = on_syscall_entry(
        nix::unistd::Pid::from_raw(4_242),
        &args,
        &mount_table,
        &fd_table,
    );
    assert_eq!(
        skip_errno(&action),
        Some(libc::EINVAL as i64),
        "dup3 with equal fds must fail with EINVAL, got {}",
        action_name(&action)
    );
}

// ── Kernel rules for open, dup and write (spec 72, feature 07) ──

#[test]
fn open_uses_its_mode_argument() {
    let fs = MemoryFs::new(foundation_core::valtron::Ctx::fixed(std::sync::Arc::new(
        foundation_core::valtron::MonotonicClock::new(),
    )));
    fs.mkdir("/virtual").unwrap();
    let mut mt = MountTable::new();
    mt.mount("/virtual", DynFs::new(Arc::new(fs.clone())));
    let fd_table = VirtualFdTable::new();
    let pid = nix::unistd::Pid::from_raw(std::process::id() as i32);

    // `openat(2)` takes the mode from `arg3`, after `dirfd`, `path` and `flags`.
    let path = path_in_self("/virtual/opened-at.txt");
    let action = dispatch_openat(
        pid,
        &path,
        libc::O_WRONLY | libc::O_CREAT | libc::O_EXCL,
        0o640,
        &mt,
        &fd_table,
    );
    assert!(
        matches!(&action, SyscallAction::Skip { .. }),
        "openat with O_CREAT must create the file, got {}",
        action_name(&action)
    );
    assert_eq!(
        fs.stat("/virtual/opened-at.txt").unwrap().permissions & 0o777,
        0o640,
        "openat must create the file with its mode argument, not its flags"
    );

    // `open(2)` takes the mode from `arg2`, after `path` and `flags`.
    let path = path_in_self("/virtual/opened.txt");
    let action = dispatch_open(
        pid,
        &path,
        libc::O_WRONLY | libc::O_CREAT | libc::O_EXCL,
        0o600,
        &mt,
        &fd_table,
    );
    assert!(
        matches!(&action, SyscallAction::Skip { .. }),
        "open with O_CREAT must create the file, got {}",
        action_name(&action)
    );
    assert_eq!(
        fs.stat("/virtual/opened.txt").unwrap().permissions & 0o777,
        0o600,
        "open must create the file with its mode argument, not its flags"
    );
}

#[test]
fn dup_clears_close_on_exec() {
    let fs = MemoryFs::new(foundation_core::valtron::Ctx::fixed(std::sync::Arc::new(
        foundation_core::valtron::MonotonicClock::new(),
    )));
    fs.mkdir("/virtual").unwrap();
    fs.write_file("/virtual/vfile.txt", b"virtual\n").unwrap();
    let mut mt = MountTable::new();
    mt.mount("/virtual", DynFs::new(Arc::new(fs.clone())));
    let pid_raw = std::process::id();
    let pid = nix::unistd::Pid::from_raw(pid_raw as i32);

    // `dup2(2)` clears O_CLOEXEC on the new descriptor and keeps the source.
    let fd_table = VirtualFdTable::new();
    insert_virtual_file(
        &fs,
        &fd_table,
        pid_raw,
        10_000,
        "/virtual/vfile.txt",
        libc::O_CLOEXEC,
    );
    let args = SyscallArgs {
        nr: DUP2,
        arg0: 10_000,
        arg1: 7,
        arg2: 0,
        arg3: 0,
        arg4: 0,
        arg5: 0,
    };
    let action = on_syscall_entry(pid, &args, &mt, &fd_table);
    assert_eq!(
        reported_value(&action),
        Some(7),
        "dup2 must report the new fd, got {}",
        action_name(&action)
    );
    assert_eq!(
        entry_flags(&fd_table, pid_raw, 7),
        Some(0),
        "dup2 must clear O_CLOEXEC on the copy"
    );
    assert_eq!(
        entry_flags(&fd_table, pid_raw, 10_000),
        Some(libc::O_CLOEXEC),
        "dup2 must keep O_CLOEXEC on the source"
    );

    // `dup(2)` clears O_CLOEXEC too.
    let fd_table = VirtualFdTable::new();
    insert_virtual_file(
        &fs,
        &fd_table,
        pid_raw,
        10_000,
        "/virtual/vfile.txt",
        libc::O_CLOEXEC,
    );
    // Move the allocator past the entry inserted above.
    let _ = fd_table.alloc_fd();
    let args = SyscallArgs {
        nr: DUP,
        arg0: 10_000,
        arg1: 0,
        arg2: 0,
        arg3: 0,
        arg4: 0,
        arg5: 0,
    };
    let action = on_syscall_entry(pid, &args, &mt, &fd_table);
    let new_fd = match skip_value(&action) {
        Some(fd) if fd > 0 => fd as u64,
        _ => panic!(
            "dup of a virtual fd must report the new fd, got {}",
            action_name(&action)
        ),
    };
    assert_eq!(
        entry_flags(&fd_table, pid_raw, new_fd),
        Some(0),
        "dup must clear O_CLOEXEC on the copy"
    );
}

#[test]
fn dup3_sets_close_on_exec_from_its_flags() {
    let fs = MemoryFs::new(foundation_core::valtron::Ctx::fixed(std::sync::Arc::new(
        foundation_core::valtron::MonotonicClock::new(),
    )));
    fs.mkdir("/virtual").unwrap();
    fs.write_file("/virtual/vfile.txt", b"virtual\n").unwrap();
    let mut mt = MountTable::new();
    mt.mount("/virtual", DynFs::new(Arc::new(fs.clone())));
    let pid_raw = std::process::id();
    let pid = nix::unistd::Pid::from_raw(pid_raw as i32);

    // `dup3(2)` sets O_CLOEXEC when `arg2` has it.
    let fd_table = VirtualFdTable::new();
    insert_virtual_file(&fs, &fd_table, pid_raw, 10_000, "/virtual/vfile.txt", 0);
    let args = SyscallArgs {
        nr: DUP3,
        arg0: 10_000,
        arg1: 7,
        arg2: libc::O_CLOEXEC as u64,
        arg3: 0,
        arg4: 0,
        arg5: 0,
    };
    let action = on_syscall_entry(pid, &args, &mt, &fd_table);
    assert_eq!(
        reported_value(&action),
        Some(7),
        "dup3 must report the new fd, got {}",
        action_name(&action)
    );
    assert_eq!(
        entry_flags(&fd_table, pid_raw, 7),
        Some(libc::O_CLOEXEC),
        "dup3 must set O_CLOEXEC from its flags argument"
    );

    // `dup3(2)` clears O_CLOEXEC when `arg2` is zero.
    let fd_table = VirtualFdTable::new();
    insert_virtual_file(
        &fs,
        &fd_table,
        pid_raw,
        10_000,
        "/virtual/vfile.txt",
        libc::O_CLOEXEC,
    );
    let args = SyscallArgs {
        nr: DUP3,
        arg0: 10_000,
        arg1: 7,
        arg2: 0,
        arg3: 0,
        arg4: 0,
        arg5: 0,
    };
    let action = on_syscall_entry(pid, &args, &mt, &fd_table);
    assert_eq!(
        reported_value(&action),
        Some(7),
        "dup3 must report the new fd, got {}",
        action_name(&action)
    );
    assert_eq!(
        entry_flags(&fd_table, pid_raw, 7),
        Some(0),
        "dup3 must clear O_CLOEXEC when its flags argument has no O_CLOEXEC"
    );
}

#[test]
fn dup_rejects_an_out_of_range_fd() {
    let fs = MemoryFs::new(foundation_core::valtron::Ctx::fixed(std::sync::Arc::new(
        foundation_core::valtron::MonotonicClock::new(),
    )));
    fs.mkdir("/virtual").unwrap();
    fs.write_file("/virtual/vfile.txt", b"virtual\n").unwrap();
    let mut mt = MountTable::new();
    mt.mount("/virtual", DynFs::new(Arc::new(fs.clone())));
    let pid_raw = std::process::id();
    let pid = nix::unistd::Pid::from_raw(pid_raw as i32);

    // The dispatch reads the tracee's own `RLIMIT_NOFILE`; the test reads the
    // same limit with `getrlimit(2)`.
    let mut limit = libc::rlimit {
        rlim_cur: 0,
        rlim_max: 0,
    };
    let rc = unsafe { libc::getrlimit(libc::RLIMIT_NOFILE, &mut limit) };
    assert_eq!(
        rc,
        0,
        "getrlimit(RLIMIT_NOFILE) failed: {}",
        std::io::Error::last_os_error()
    );
    let nofile = limit.rlim_cur as u64;
    assert_ne!(
        nofile,
        libc::RLIM_INFINITY,
        "the test needs a finite RLIMIT_NOFILE"
    );

    let fd_table = VirtualFdTable::new();
    insert_virtual_file(&fs, &fd_table, pid_raw, 10_000, "/virtual/vfile.txt", 0);

    // A negative number is `EBADF`, and the table keeps no entry for it.
    let args = SyscallArgs {
        nr: DUP2,
        arg0: 10_000,
        arg1: (-1i64) as u64,
        arg2: 0,
        arg3: 0,
        arg4: 0,
        arg5: 0,
    };
    let action = on_syscall_entry(pid, &args, &mt, &fd_table);
    assert_eq!(
        skip_errno(&action),
        Some(libc::EBADF as i64),
        "dup2 with a negative new fd must fail with EBADF, got {}",
        action_name(&action)
    );
    assert!(
        fd_table.get(pid_raw, u64::MAX).is_none(),
        "a rejected dup2 must not insert an entry"
    );

    // A number at `RLIMIT_NOFILE` is `EBADF` for `dup2(2)` and `dup3(2)`.
    for nr in [DUP2, DUP3] {
        let args = SyscallArgs {
            nr,
            arg0: 10_000,
            arg1: nofile,
            arg2: 0,
            arg3: 0,
            arg4: 0,
            arg5: 0,
        };
        let action = on_syscall_entry(pid, &args, &mt, &fd_table);
        assert_eq!(
            skip_errno(&action),
            Some(libc::EBADF as i64),
            "dup (nr {nr}) at RLIMIT_NOFILE must fail with EBADF, got {}",
            action_name(&action)
        );
        assert!(
            fd_table.get(pid_raw, nofile).is_none(),
            "a rejected dup must not insert an entry at RLIMIT_NOFILE"
        );
    }

    // The source stays open after the rejected calls.
    assert!(
        fd_table.get(pid_raw, 10_000).is_some(),
        "a rejected dup must keep the source entry"
    );
}

#[test]
fn an_append_write_goes_to_the_end() {
    let fs = MemoryFs::new(foundation_core::valtron::Ctx::fixed(std::sync::Arc::new(
        foundation_core::valtron::MonotonicClock::new(),
    )));
    fs.mkdir("/virtual").unwrap();
    fs.write_file("/virtual/log.txt", b"head").unwrap();
    let mut mt = MountTable::new();
    mt.mount("/virtual", DynFs::new(Arc::new(fs.clone())));
    let fd_table = VirtualFdTable::new();
    let pid_raw = std::process::id();
    let pid = nix::unistd::Pid::from_raw(pid_raw as i32);

    let path = path_in_self("/virtual/log.txt");
    let action = dispatch_openat(
        pid,
        &path,
        libc::O_WRONLY | libc::O_APPEND,
        0,
        &mt,
        &fd_table,
    );
    let vfd = match skip_value(&action) {
        Some(fd) if fd > 0 => fd as u64,
        _ => panic!(
            "open of a virtual file must return a virtual fd, got {}",
            action_name(&action)
        ),
    };

    let first = b"AAA";
    let action = dispatch_write(pid, vfd, first, &mt, &fd_table);
    assert_eq!(
        skip_value(&action),
        Some(3),
        "the first append write must report 3 bytes, got {}",
        action_name(&action)
    );
    assert_eq!(
        entry_offset(&fd_table, pid_raw, vfd),
        Some(7),
        "the first append write must move the offset to the end of the file"
    );
    assert_eq!(
        fs.read_file("/virtual/log.txt").unwrap(),
        b"headAAA",
        "the first append write must keep the existing bytes"
    );

    let second = b"BB";
    let action = dispatch_write(pid, vfd, second, &mt, &fd_table);
    assert_eq!(
        skip_value(&action),
        Some(2),
        "the second append write must report 2 bytes, got {}",
        action_name(&action)
    );
    assert_eq!(
        entry_offset(&fd_table, pid_raw, vfd),
        Some(9),
        "the second append write must move the offset to the new end"
    );
    assert_eq!(
        fs.read_file("/virtual/log.txt").unwrap(),
        b"headAAABB",
        "both append writes must land at the end"
    );
}

/// Create a pipe that the traced child inherits across `exec`.
///
/// `libc::pipe` sets no `O_CLOEXEC`, unlike the fds that `std` opens.
fn pipe_raw() -> (i32, i32) {
    let mut fds = [0i32; 2];
    let rc = unsafe { libc::pipe(fds.as_mut_ptr()) };
    assert_eq!(rc, 0, "pipe failed: {}", std::io::Error::last_os_error());
    (fds[0], fds[1])
}

fn close_raw(fd: i32) {
    let rc = unsafe { libc::close(fd) };
    assert_eq!(
        rc,
        0,
        "close({fd}) failed: {}",
        std::io::Error::last_os_error()
    );
}

fn read_one_byte(fd: i32) -> usize {
    let mut byte = 0u8;
    loop {
        let rc = unsafe { libc::read(fd, (&mut byte as *mut u8).cast(), 1) };
        if rc >= 0 {
            return rc as usize;
        }
        let err = std::io::Error::last_os_error();
        assert_eq!(
            err.kind(),
            std::io::ErrorKind::Interrupted,
            "read({fd}) failed: {err}"
        );
    }
}

/// Release a child blocked in the shell `read` builtin. The builtin reads a
/// whole line, so the byte must be a newline.
fn write_newline(fd: i32) {
    let rc = unsafe { libc::write(fd, b"\n".as_ptr().cast(), 1) };
    assert_eq!(
        rc,
        1,
        "write({fd}) failed: {}",
        std::io::Error::last_os_error()
    );
}

#[test]
#[ntest::timeout(60_000)]
fn the_traced_child_truncates_an_existing_virtual_file() {
    // A shell redirect opens with O_CREAT|O_TRUNC on an existing file. The
    // dispatch must open that file and truncate it, not report EEXIST.
    let i = NixInterceptor::new();
    let fs = MemoryFs::new(foundation_core::valtron::Ctx::fixed(std::sync::Arc::new(
        foundation_core::valtron::MonotonicClock::new(),
    )));
    fs.mkdir("/virtual").unwrap();
    fs.write_file(
        "/virtual/existing.txt",
        b"old bytes that must not survive\n",
    )
    .unwrap();

    let mut mt = MountTable::new();
    mt.mount("/virtual", DynFs::new(Arc::new(fs.clone())));

    let h = i
        .spawn(mt, "/bin/sh", &["-c", "echo new > /virtual/existing.txt"])
        .expect("spawn failed");
    assert_eq!(
        h.wait().expect("wait failed"),
        0,
        "the redirect to an existing virtual file must succeed"
    );
    assert_eq!(
        fs.read_file("/virtual/existing.txt").unwrap(),
        b"new\n",
        "the existing virtual file must hold only the new bytes"
    );
}

#[test]
#[ntest::timeout(60_000)]
fn the_traced_child_dup2_closes_the_real_destination() {
    // The child inherits these pipe fds across fork and exec. `dest` is a real
    // fd that the child `dup2`s a virtual fd over; `dup2(2)` must close it
    // before it returns. The child reports the dup2 with one byte on the
    // notification pipe, then blocks on the control pipe while the test checks
    // its fd table.
    let i = NixInterceptor::new();
    let fs = MemoryFs::new(foundation_core::valtron::Ctx::fixed(std::sync::Arc::new(
        foundation_core::valtron::MonotonicClock::new(),
    )));
    fs.mkdir("/virtual").unwrap();
    fs.write_file("/virtual/vfile.txt", b"virtual\n").unwrap();

    let mut mt = MountTable::new();
    mt.mount("/virtual", DynFs::new(Arc::new(fs.clone())));

    let (dest, dest_peer) = pipe_raw();
    let (note_read, note_write) = pipe_raw();
    let (ctrl_read, ctrl_write) = pipe_raw();
    assert!(
        (dest as u64) < FD_VIRTUAL_BASE,
        "the test destination {dest} must be a real fd number"
    );

    // No other fd of the test shares the slot where the virtual file lands.
    let slot = [
        dest, dest_peer, note_read, note_write, ctrl_read, ctrl_write,
    ]
    .into_iter()
    .max()
    .unwrap()
        + 1;

    // The shell runs each redirection as `dup2(2)`: the virtual file lands on
    // `slot`, then on `dest`, which is the real destination.
    let script = format!(
        "exec {slot}</virtual/vfile.txt; exec {dest}<&{slot}; \
         printf x >&{note_write}; read _ctrl <&{ctrl_read}"
    );
    let h = i
        .spawn(mt, "/bin/sh", &["-c", &script])
        .expect("spawn failed");
    let pid = h.pid();

    // The child holds its own copies. Close the parent copies so they do not
    // mask a destination the child left open.
    close_raw(dest);
    close_raw(dest_peer);
    close_raw(note_write);
    close_raw(ctrl_read);

    assert_eq!(
        read_one_byte(note_read),
        1,
        "the traced child did not reach the dup2"
    );

    // The child is still alive and blocked on the control read. Its real
    // destination fd must already be gone: `dup2(2)` closes it before it
    // returns.
    let target = format!("/proc/{pid}/fd/{dest}");
    let destination_still_open = std::fs::symlink_metadata(&target).is_ok();

    // Release the child before the assertion, so a failure leaves nothing
    // blocked.
    write_newline(ctrl_write);
    close_raw(ctrl_write);
    assert_eq!(h.wait().expect("wait failed"), 0);
    close_raw(note_read);

    assert!(
        !destination_still_open,
        "dup2 left the real destination fd {dest} open while the child ran"
    );
}

// ── setregs failure reporting (spec 72, feature 08) ──

#[test]
fn a_failed_setregs_is_reported() {
    use foundation_nativeapis::native::vfs::ptrace::nix_backend::report_return_value;
    use foundation_nativeapis::shared::vfs::error::VfsError;

    // No process can trace pid `i32::MAX`, so `PTRACE_SETREGS` fails.
    let pid = nix::unistd::Pid::from_raw(i32::MAX);
    // SAFETY: all-zero bits are valid for `user_regs_struct`, which is plain
    // data.
    let regs: libc::user_regs_struct = unsafe { std::mem::zeroed() };
    let error = report_return_value(pid, regs, 0)
        .expect_err("setregs must fail for a pid that is not traced");

    match error.current_context() {
        VfsError::Backend { message } => {
            assert!(
                message.contains("setregs"),
                "the message must name setregs: {message}"
            );
            assert!(
                message.contains(&i32::MAX.to_string()),
                "the message must name the pid: {message}"
            );
        }
        other => panic!("a failed setregs must be a backend error, got {other:?}"),
    }
}

#[test]
#[ntest::timeout(60_000)]
fn the_traced_child_stdout_arrives_whole() {
    // A bulk copy catches descriptor or pipe corruption in the syscall loop:
    // every byte and the checksum must survive the round trip.
    let i = NixInterceptor::new();
    let mt = MountTable::new();
    let tmp = TempDir::new("stdout_whole");
    let source = tmp.path().join("source.bin");
    let dest = tmp.path().join("dest.bin");

    let payload: Vec<u8> = (0..(64 * 1024 + 17)).map(|n| (n % 251) as u8).collect();
    std::fs::write(&source, &payload).unwrap();

    let script = format!("cat {} > {}", source.display(), dest.display());
    let h = i
        .spawn(mt, "/bin/sh", &["-c", &script])
        .expect("spawn failed");
    assert_eq!(
        h.wait().expect("wait failed"),
        0,
        "cat through the tracer must succeed"
    );

    let received = std::fs::read(&dest).unwrap();
    assert_eq!(
        received.len(),
        payload.len(),
        "the traced child lost bytes on the way"
    );
    assert_eq!(
        fnv1a64(&received),
        fnv1a64(&payload),
        "the content changed through the tracer"
    );
}
