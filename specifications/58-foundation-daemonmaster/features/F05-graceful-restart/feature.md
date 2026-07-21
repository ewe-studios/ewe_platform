---
workspace_name: "ewe_platform"
spec_directory: "specifications/58-foundation-daemonmaster"
feature_directory: "specifications/58-foundation-daemonmaster/features/F05-graceful-restart"
this_file: "specifications/58-foundation-daemonmaster/features/F05-graceful-restart/feature.md"

status: planned
priority: high
created: 2026-07-18

depends_on:
  - "F02-process-lifecycle"

tasks:
  completed: 0
  uncompleted: 6
  total: 6
  completion_percentage: 0%
---

# F05 — Graceful restart via socket inheritance (ecdysis pattern)

## Overview

Zero-downtime binary upgrade and hot-restart via file descriptor passing. The parent
passes listening socket FDs to a child through a pipe; the child reconstructs listeners
from those FDs, declares readiness, and only then does the parent stop accepting and drain.

**All constructs are public and reusable** — any service can use FdRegistry,
UpgradeExecutor, and ReadyPipe independently of the daemon supervisor.

[spec](../spec.md). Inspired by [cloudflare/ecdysis](https://github.com/cloudflare/ecdysis).

---

## Part A — FdRegistry (public, reusable)

The registry tracks which sockets belong to this process vs. were inherited from a parent.

```rust
// foundation_nativeapis/src/daemon/upgrade.rs

use serde::{Serialize, Deserialize};
use std::net::SocketAddr;
use std::path::PathBuf;
use std::os::unix::io::RawFd;

/// Identifies a socket type + address for matching across upgrades.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum SockInfo {
    /// Unix domain stream socket.
    Unix(Option<PathBuf>),
    /// TCP listener.
    Tcp(SocketAddr),
    /// UDP socket.
    Udp(SocketAddr),
}

/// A registered listener: FD number + identity.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ListenerInfo {
    pub fd: RawFd,
    pub info: SockInfo,
}

/// Dual-vec registry: tracks created FDs vs. inherited FDs.
///
/// When creating a listener, first checks inherited_fds for a matching SockInfo.
/// If found, returns that FD (reusing the parent's socket). If not, creates a new
/// socket and adds a dup'd copy to used_fds (for serialization to the next child).
pub struct FdRegistry {
    inherited_fds: Mutex<Vec<ListenerInfo>>,  // from parent during upgrade
    used_fds:      Mutex<Vec<ListenerInfo>>,  // created by this process
}

impl FdRegistry {
    pub fn new() -> Self {
        Self {
            inherited_fds: Mutex::new(Vec::new()),
            used_fds: Mutex::new(Vec::new()),
        }
    }

    /// Load inherited FDs from the parent's pipe (child side).
    pub fn load_inherited(&self, reader: impl Read) -> Result<Vec<ListenerInfo>, UpgradeError> {
        let listeners: Vec<ListenerInfo> = bincode::deserialize_from(reader)?;
        let mut inherited = self.inherited_fds.lock();
        *inherited = listeners.clone();
        Ok(listeners)
    }

    /// Register or retrieve a listener by SockInfo.
    ///
    /// If an inherited FD matches `info`, return it. Otherwise create a new socket,
    /// dup the FD, and track it in used_fds.
    pub fn register<F: FnOnce() -> Result<RawFd, std::io::Error>>(
        &self,
        info: SockInfo,
        create: F,
    ) -> Result<RawFd, UpgradeError> {
        // Check inherited first.
        {
            let inherited = self.inherited_fds.lock();
            if let Some(li) = inherited.iter().find(|li| li.info == info) {
                return Ok(li.fd);
            }
        }

        // Create new socket.
        let fd = create()?;

        // Dup to preserve FD attributes, then track.
        let dup_fd = nix::fcntl::fcntl(fd, nix::fcntl::F_DUPFD_CLOEXEC(0))?;

        self.used_fds.lock().push(ListenerInfo { fd: dup_fd, info });

        Ok(dup_fd)
    }

    /// Serialize all used_fds for passing to the next child.
    pub fn serialize(&self) -> Result<Vec<u8>, UpgradeError> {
        let used = self.used_fds.lock();
        Ok(bincode::serialize(&*used)?)
    }
}
```

---

## Part B — ReadyPipe (public, reusable)

Simple pipe-based child→parent "I'm ready" signaling with timeout.

```rust
/// Ready notifier — child side.
pub struct ReadyPipe {
    writer: os_pipe::PipeWriter,
}

/// Ready listener — parent side.
pub struct ReadyListener {
    reader: os_pipe::PipeReader,
    timeout: std::time::Duration,
}

impl ReadyPipe {
    pub fn mark_ready(&self) -> Result<(), std::io::Error> {
        // Write "OK" to the pipe.
        let mut writer = self.writer.try_clone()?;
        writer.write_all(b"OK")
    }
}

impl ReadyListener {
    /// Wait for the child to signal ready, or timeout.
    pub fn wait_ready(&self) -> Result<bool, ReadyTimeout> {
        let deadline = std::time::Instant::now() + self.timeout;
        loop {
            if std::time::Instant::now() > deadline {
                return Err(ReadyTimeout);
            }
            let mut buf = [0u8; 2];
            match self.reader.read_exact(&mut buf) {
                Ok(_) if &buf == b"OK" => return Ok(true),
                Ok(_) => return Err(ReadyTimeout), // Unexpected data
                Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                    std::thread::sleep(std::time::Duration::from_millis(10));
                    continue;
                }
                Err(_) => return Err(ReadyTimeout),
            }
        }
    }
}

/// Create a ready pipe pair.
pub fn ready_pipe() -> (ReadyPipe, ReadyListener) {
    let (reader, writer) = os_pipe::pipe().unwrap();
    // Make reader non-blocking.
    let flags = nix::fcntl::fcntl(reader.as_raw_fd(), nix::fcntl::F_GETFL).unwrap();
    nix::fcntl::fcntl(reader.as_raw_fd(), nix::fcntl::F_SETFL(flags | nix::fcntl::OFlag::O_NONBLOCK)).unwrap();

    (
        ReadyPipe { writer },
        ReadyListener {
            reader,
            timeout: std::time::Duration::from_secs(5),
        },
    )
}

#[derive(Debug)]
pub struct ReadyTimeout;
```

---

## Part C — UpgradeExecutor (public, reusable)

Orchestrates the full fork/exec upgrade cycle. Any service can use this directly.

```rust
/// Upgrade executor — fork a child, pass FDs, wait for ready, drain parent.
pub struct UpgradeExecutor {
    registry: Arc<FdRegistry>,
    child_argv: Vec<String>,
    child_env: Vec<(String, String)>,
}

impl UpgradeExecutor {
    pub fn new(registry: Arc<FdRegistry>, binary_path: &str) -> Self {
        Self {
            registry,
            child_argv: vec![binary_path.to_string()],
            child_env: Vec::new(),
        }
    }

    /// Execute an upgrade: fork child, pass FDs, wait for ready.
    ///
    /// On success, the child is running and ready. The parent should then drain
    /// existing connections and exit.
    ///
    /// On failure (child doesn't signal ready within timeout), the child is killed
    /// and the parent continues serving.
    pub fn execute(&self) -> Result<UpgradeResult, UpgradeError> {
        // Create pipe topology.
        let (ready_pipe, ready_listener) = ready_pipe();
        let (fd_pipe_r, fd_pipe_w) = os_pipe::pipe()?;

        // Serialize FDs.
        let fd_data = self.registry.serialize()?;

        // Fork.
        let mut fork = unsafe {
            nix::unistd::fork()?
        };

        match fork {
            nix::unistd::ForkResult::Parent { child } => {
                drop(fd_pipe_r); // Parent doesn't read.

                // Close write end of FD pipe after child inherits.
                drop(fd_pipe_w);

                // Wait for child to signal ready.
                match ready_listener.wait_ready() {
                    Ok(()) => {
                        UpgradeResult::Parent(UpgradeParentResult::ChildReady {
                            child_pid: child.as_raw() as u32,
                        })
                    }
                    Err(ReadyTimeout) => {
                        // Kill child, parent continues.
                        nix::sys::signal::kill(child, nix::sys::signal::Signal::SIGKILL)?;
                        UpgradeResult::Parent(UpgradeParentResult::ChildTimedOut)
                    }
                }
            }
            nix::unistd::ForkResult::Child => {
                drop(fd_pipe_w); // Child doesn't write FDs.
                drop(ready_pipe.writer); // Will be passed via env.

                // Set env vars for child to pick up.
                std::env::set_var("DAEMON_UPGRADE", "1");
                std::env::set_var("DAEMON_PIPE_FDS", fd_pipe_r.as_raw_fd().to_string());
                std::env::set_var("DAEMON_PIPE_READY", ready_pipe.writer.as_raw_fd().to_string());

                // Clear CLOEXEC on inherited FDs so they survive exec.
                for li in self.registry.used_fds.lock().iter() {
                    nix::fcntl::fcntl(li.fd, nix::fcntl::F_SETFD(nix::fcntl::FdFlag::empty()))?;
                }

                // Exec the new binary.
                nix::unistd::execvp(
                    std::ffi::CStr::from_bytes_with_nul(
                        self.child_argv[0].as_bytes(),
                    )?,
                    &self.child_argv.iter()
                        .map(|s| std::ffi::CString::new(s.as_str()).unwrap())
                        .collect::<Vec<_>>(),
                )?;

                // If exec fails, child exits.
                std::process::exit(1);
            }
        }
    }
}

pub enum UpgradeResult {
    Parent(UpgradeParentResult),
}

pub enum UpgradeParentResult {
    ChildReady { child_pid: u32 },
    ChildTimedOut,
}

/// Detect if the current process was started as an upgrade child.
///
/// Returns the inherited FD data and ready pipe writer if this is a child.
pub fn detect_upgrade_child() -> Option<UpgradeChildInit> {
    if std::env::var("DAEMON_UPGRADE").ok()? != "1" {
        return None;
    }

    let fd_pipe = std::env::var("DAEMON_PIPE_FDS").ok()?.parse::<RawFd>().ok()?;
    let ready_pipe = std::env::var("DAEMON_PIPE_READY").ok()?.parse::<RawFd>().ok()?;

    Some(UpgradeChildInit { fd_pipe, ready_pipe })
}

pub struct UpgradeChildInit {
    pub fd_pipe: RawFd,
    pub ready_pipe: RawFd,
}

impl UpgradeChildInit {
    /// Load inherited FDs from the pipe.
    pub fn load_inherited(&self, registry: &FdRegistry) -> Result<Vec<ListenerInfo>, UpgradeError> {
        let file = unsafe { std::fs::File::from_raw_fd(self.fd_pipe) };
        registry.load_inherited(file)
    }

    /// Signal ready to the parent.
    pub fn mark_ready(&self) -> Result<(), std::io::Error> {
        let mut writer = unsafe { os_pipe::PipeWriter::from_raw_fd(self.ready_pipe) };
        writer.write_all(b"OK")
    }
}

#[derive(Debug, thiserror::Error)]
pub enum UpgradeError {
    #[error("upgrade already in progress")]
    AlreadyUpgrading,
    #[error("fork failed: {0}")]
    ForkFailed(#[from] nix::Error),
    #[error("serialization failed: {0}")]
    SerializeFailed(#[from] bincode::Error),
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("ready timeout")]
    ReadyTimeout,
}
```

### C.2 — Upgrade guard (prevent concurrent upgrades)

```rust
use std::sync::atomic::{AtomicBool, Ordering};

static UPGRADING: AtomicBool = AtomicBool::new(false);

pub struct UpgradeGuard;

impl UpgradeGuard {
    /// Try to acquire the upgrade lock.
    pub fn try_acquire() -> Result<Self, UpgradeError> {
        if UPGRADING.swap(true, Ordering::SeqCst) {
            return Err(UpgradeError::AlreadyUpgrading);
        }
        Ok(UpgradeGuard)
    }
}

impl Drop for UpgradeGuard {
    fn drop(&mut self) {
        UPGRADING.store(false, Ordering::SeqCst);
    }
}
```

---

## Part D — Supervisor integration

The supervisor uses these constructs internally for its own restart and for managing
daemon-level graceful restarts:

```rust
impl Supervisor {
    /// Trigger a graceful upgrade of a managed daemon.
    pub async fn upgrade_daemon(&self, id: &DaemonId) -> Result<UpgradeResult, UpgradeError> {
        let executor = self.build_upgrade_executor(id)?;
        let _guard = UpgradeGuard::try_acquire()?;
        executor.execute()
    }

    /// Trigger a graceful self-upgrade (restart the supervisor binary).
    pub fn self_upgrade(&self) -> Result<UpgradeResult, UpgradeError> {
        let executor = UpgradeExecutor::new(
            self.fd_registry.clone(),
            &std::env::current_exe()?.to_string_lossy(),
        );
        let _guard = UpgradeGuard::try_acquire()?;
        executor.execute()
    }
}
```

---

## Verification

```bash
cargo test --package foundation_nativeapis --features daemon -- daemon::upgrade
```

Tests cover:
- FdRegistry: register, inherit, serialize round-trip
- ReadyPipe: mark_ready → wait_ready returns OK within timeout
- ReadyPipe: no signal → wait_ready times out after 5s
- UpgradeExecutor: fork → child gets env vars → detects upgrade → loads FDs
- UpgradeExecutor: child timeout → child is killed → parent continues
- UpgradeGuard: concurrent upgrade attempt fails with AlreadyUpgrading
- detect_upgrade_child: returns None when not an upgrade child
