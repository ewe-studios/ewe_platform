# 13 — foundation_sshkit

**Date:** 2026-07-09
**Status:** Resolved

## Decision

Create a `foundation_sshkit` crate that wraps the `ssh2` crate (libssh2 bindings,
already used in `foundation_testbed`) with higher-level primitives for SSH key
management, connection pooling, host abstraction, command execution, and file
transfers. Model the API on SSHKit (Basecamp's Ruby library) — a proven,
battle-tested design for SSH orchestration.

## Table of Contents

1. [Why a dedicated SSH crate](#why-a-dedicated-ssh-crate)
2. [Prior art: SSHKit (Ruby)](#prior-art-sshkit-ruby)
3. [Rust SSH ecosystem](#rust-ssh-ecosystem)
4. [Core abstractions](#core-abstractions)
5. [Connection pooling](#connection-pooling)
6. [Runner strategies](#runner-strategies)
7. [Integration with the workspace](#integration-with-the-workspace)
8. [Learnings from vm-uncloud SSH patterns](#learnings-from-vm-uncloud-ssh-patterns)

---

## Why a dedicated SSH crate

The workspace already uses SSH in multiple places:

- `foundation_testbed` — `ssh2` crate for guest VM communication (exec, scp, shell)
- `foundation_deployment` — cloud provider SSH access for remote commands
- vm-uncloud — SSH for Hetzner machine init, RDP tunnel, viewer tunnel
- Bollard — SSH transport to remote Docker daemons

Each of these manages SSH connections independently — key resolution, connection
timeouts, retry logic, and output formatting are reinvented per consumer. A
dedicated crate provides:

1. **Connection pooling** — Reuse SSH sessions across operations. Idle timeout
   eviction. Configurable pool size.
2. **Key management** — Unified key resolution (agent, file path, env var).
   Multiple key support. Passphrase handling.
3. **Host abstraction** — Parse `user@host:port`, host properties/roles, SSH
   config file integration.
4. **Command execution** — Environment wrapping, directory change (`cd`), user
   switch (`sudo`/`su`), output capture, exit code handling.
5. **File transfer** — SCP and SFTP via a unified `upload!`/`download!` API.
6. **Runner strategies** — Parallel (spawn per host), sequential, grouped
   (parallel within batches, sequential between).

---

## Prior art: SSHKit (Ruby)

SSHKit is the execution engine underneath Capistrano and Kamal. It provides a
procedural DSL backed by connection pooling and runner strategies.

### Core abstractions (Ruby → Rust mapping)

| SSHKit (Ruby) | Rust equivalent |
|---------------|-----------------|
| `SSHKit::Host` | `sshkit::Host` — parsed from `user@host:port`, holds keys, properties |
| `SSHKit::Command` | `sshkit::Command` — command + args + env + working dir + user/group |
| `SSHKit::Coordinator` | `sshkit::Coordinator` — resolves hosts, dispatches to runner |
| `SSHKit::DSL` (on, run_locally) | Builder-pattern API: `on(hosts).run(cmd).await` |
| `SSHKit::Backend::Netssh` | `sshkit::backend::Ssh2` — wraps `ssh2::Session` |
| `SSHKit::Backend::Local` | `sshkit::backend::Local` — `std::process::Command` |
| `SSHKit::Runner::Parallel` | `sshkit::runner::Parallel` — `tokio::spawn` per host |
| `SSHKit::Runner::Sequential` | `sshkit::runner::Sequential` — loop with configurable interval |
| `SSHKit::Runner::Group` | `sshkit::runner::Group` — batches of N, parallel within, sequential between |
| ConnectionPool | `LruCache`-based pool with `tokio::time::interval` eviction |
| CommandMap | `HashMap<String, String>` — maps short names to full paths |

### Key SSHKit patterns to adopt

1. **`within(dir) { execute(cmd) }`** — wraps commands with `cd <dir> && <cmd>`.
2. **`with(env) { execute(cmd) }`** — wraps commands with `export KEY=VAL && <cmd>`.
3. **`as(user) { execute(cmd) }`** — wraps with `sudo -u <user>` or `su -c`.
4. **`capture(cmd)`** — returns stdout as a string (raises on non-zero exit).
5. **`test(cmd)`** — returns boolean (false on non-zero exit, doesn't raise).
6. **`upload!(local, remote)` / `download!(remote, local)`** — SCP/SFTP transfer.
7. **`in_background`** — `nohup <cmd> &` for long-running processes.

---

## Rust SSH ecosystem

| Crate | Approach | Notes |
|-------|----------|-------|
| **`ssh2`** | libssh2 C bindings | Already used in `foundation_testbed`. Mature, stable. C dependency. |
| **`russh`** | Pure Rust SSH2 | Async/tokio-native. Smaller community. Could be a secondary backend. |
| **`openssh`** | CLI wrapper (`ssh`, `scp`) | Spawns processes. Simple but limited control. Fallback option. |

**Recommendation:** Use `ssh2` as the primary backend (already proven in the
workspace). Architect the `Backend` trait to be swappable, so `russh` or
`openssh` can be added later.

---

## Core abstractions

```rust
/// A remote host reachable via SSH.
pub struct Host {
    pub hostname: String,
    pub port: u16,           // default 22
    pub user: String,        // default "root"
    pub password: Option<String>,
    pub key_paths: Vec<PathBuf>,
    pub proxy: Option<Box<Host>>,  // jump/bastion host
    pub properties: serde_json::Value,  // arbitrary metadata
}

impl Host {
    /// Parse from strings: "user@host:port", "host", "user@host"
    pub fn parse(s: &str) -> Result<Self>;
    /// Build from SSH config file (~/.ssh/config)
    pub fn from_ssh_config(host_alias: &str) -> Result<Self>;
}

/// A shell command to execute on a remote host.
pub struct Command {
    pub program: String,
    pub args: Vec<String>,
    pub env: Vec<(String, String)>,
    pub working_dir: Option<PathBuf>,
    pub user: Option<String>,     // sudo -u <user>
    pub group: Option<String>,    // sg <group>
    pub umask: Option<String>,
    pub in_background: bool,
}

impl Command {
    pub fn new(program: impl Into<String>) -> Self;
    pub fn arg(mut self, arg: impl Into<String>) -> Self;
    pub fn env(mut self, key: &str, val: &str) -> Self;
    pub fn within(mut self, dir: impl Into<PathBuf>) -> Self;
    pub fn as_user(mut self, user: &str) -> Self;
    /// Render to a shell command string (handles escaping)
    pub fn to_shell_command(&self) -> String;
}

/// Result of a command execution.
pub struct CommandResult {
    pub exit_code: i32,
    pub stdout: String,
    pub stderr: String,
    pub runtime: Duration,
    pub host: String,
}

/// The backend trait — swappable SSH transport.
pub trait Backend: Send + Sync {
    async fn execute(&self, host: &Host, cmd: &Command) -> Result<CommandResult>;
    async fn upload(&self, host: &Host, local: &Path, remote: &Path) -> Result<()>;
    async fn download(&self, host: &Host, remote: &Path, local: &Path) -> Result<()>;
}

/// SSH2 backend (libssh2).
pub struct Ssh2Backend { /* session pool */ }
impl Backend for Ssh2Backend { /* ... */ }
```

---

## Connection pooling

```rust
/// A pool of SSH sessions, keyed by (hostname, port, user).
/// Idle sessions are evicted after `idle_timeout`. Thread-safe.
pub struct ConnectionPool {
    sessions: LruCache<ConnectionKey, Ssh2Session>,
    idle_timeout: Duration,  // default 30s
    max_size: usize,         // default 32
}

impl ConnectionPool {
    pub fn get(&mut self, host: &Host) -> Result<PooledSession>;
    /// Evict idle sessions. Called by background task.
    pub fn evict_idle(&mut self);
}

/// A session checked out from the pool. Returns to pool on Drop.
pub struct PooledSession<'a> { /* ... */ }
```

Background eviction runs on a `tokio::time::interval` (every 5s), closing
connections idle longer than `idle_timeout`. Pool is registered with
`at_exit`-equivalent cleanup.

---

## Runner strategies

```rust
pub enum Runner {
    Parallel,
    Sequential { wait_interval: Duration },
    Group { limit: usize },
}

impl Runner {
    /// Execute a command on all hosts using this strategy.
    pub async fn run<F>(
        &self,
        hosts: &[Host],
        backend: &dyn Backend,
        command_builder: F,
    ) -> Vec<Result<CommandResult>>
    where F: Fn(&Host) -> Command;
}
```

- **Parallel** — `tokio::spawn` one task per host, `join_all`.
- **Sequential** — iterate hosts, execute one at a time with `wait_interval` delay.
- **Group** — chunk hosts into batches of `limit`, parallel within each batch,
  sequential between batches. Useful for rolling deploys.

---

## Integration with the workspace

`foundation_sshkit` depends on:
- `ssh2` — primary SSH backend (already in workspace via `foundation_testbed`)
- `tokio` — async runtime (already in workspace)
- `serde` / `serde_json` — host properties serialization
- `tracing` — structured logging
- `foundation_core` — valtron executor (optional, for multi-threading)

`foundation_deployment_docker` uses `foundation_sshkit` for:
- Bollard SSH transport host resolution (parsing `ssh://user@host:port`)
- Remote Docker health checks
- Cloud-init provisioning SSH commands

---

## Learnings from vm-uncloud SSH patterns

vm-uncloud uses SSH extensively. Patterns we adopt:

1. **SSH key resolution chain** — Try explicit key → `~/.ssh/id_ed25519` →
   `~/.ssh/id_rsa` → fail. Same pattern as `prepare.nu`'s key discovery.
2. **PTY for interactive commands** — `uc machine init` needs a TTY.
   `foundation_sshkit`'s `Ssh2Backend` can allocate a PTY when the command
   requests it (`Command.pty = true`).
3. **Retry with backoff** — Transient SSH failures (connection refused during
   boot, DNS not propagated) retry up to 3× with 2s backoff. Same as
   vm-uncloud's `with-pty-retry` wrapper.
4. **SSH config integration** — `Host::from_ssh_config()` reads
   `~/.ssh/config` for proxy jump, user, identity file, port. This lets
   users configure bastion hosts once and have `foundation_sshkit` pick it up.
