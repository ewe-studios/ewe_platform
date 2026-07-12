# Exploration: pitchfork — Daemon Supervisor with DX Focus

**Source:** Jeff Dickey / @jdx ([github.com/jdx/pitchfork](https://github.com/jdx/pitchfork))
**Crate:** `pitchfork-cli` v2.8.0 · MIT · Cross-platform (Linux, macOS, Windows)
**Tagline:** "Daemons with DX"

---

## Summary

Pitchfork is a developer-focused daemon manager that ensures services are always running, starts them with dependency ordering, auto-restarts on file changes, and integrates with the shell to automatically start/stop daemons when entering/leaving project directories. It can also run as PID 1 in containers with zombie reaping and signal forwarding, or register itself for boot-time auto-start via systemd/LaunchAgent.

Unlike a full init system, pitchfork targets the **developer workflow** — start once, have it stay running, and have it adapt to your directory context.

---

## Core Architecture

### Process Model

Pitchfork runs a **central supervisor process** that manages all daemon child processes. The CLI communicates with the supervisor via **Unix socket IPC** (or starts the supervisor if it isn't running). This means:

- **One supervisor per user session** — all projects share it
- **State persisted to `state.toml`** — survives supervisor restarts
- **IPC socket in `sock/` directory** — CLI commands talk to supervisor, not directly to daemons

```
CLI (pitchfork start api)
    │
    ▼
IPC Socket (Unix domain socket)
    │
    ▼
Supervisor (long-running background process)
    ├── api daemon (npm run server:api)  →  pid 12345, status: running
    ├── postgres daemon (docker run ...) →  pid 12346, status: running
    └── redis daemon (redis-server)      →  pid 12347, status: running
```

### Key Files

| File | Role |
|------|------|
| `src/supervisor/mod.rs` | Central supervisor singleton, signal handling, close/shutdown, state dir permissions |
| `src/supervisor/lifecycle.rs` | Daemon start/stop operations (spawning processes, monitoring) |
| `src/supervisor/state.rs` | State access layer — get/set daemon state |
| `src/supervisor/retry.rs` | Retry logic with exponential backoff |
| `src/supervisor/autostop.rs` | Autostop timing, boot daemon startup |
| `src/supervisor/watchers.rs` | Interval polling, cron scheduling, file watching background tasks |
| `src/supervisor/hooks.rs` | Lifecycle hooks (on_ready, on_fail, on_retry, on_stop, on_exit) |
| `src/supervisor/ipc_handlers.rs` | IPC request dispatch from CLI |
| `src/procs.rs` | Process management — kill, process group signals, stats via `sysinfo` |
| `src/daemon.rs` | `Daemon` struct (persisted state) and `RunOptions` |
| `src/daemon_id.rs` | `DaemonId` — qualified IDs: `namespace/name` |
| `src/pitchfork_toml.rs` | Config file parser/merger — `pitchfork.toml` schema |
| `src/shell.rs` | Shell abstraction for cross-platform command execution |
| `src/cli/activate.rs` | Shell hook generation (bash/zsh/fish) |
| `src/cli/cd.rs` | Directory change handler — auto start/stop |
| `src/boot_manager.rs` | Boot-time auto-start registration (systemd/LaunchAgent) |
| `src/state_file.rs` | `state.toml` persistence |
| `src/deps.rs` | Dependency graph — topological sort for start ordering |
| `src/watch_files.rs` | File watcher integration (`notify` crate) |

---

## Detailed Mechanisms

### 1. Daemon Model (`daemon.rs`)

Each daemon is a rich struct with configuration and runtime state:

```rust
pub struct Daemon {
    pub id: DaemonId,            // "project/api"
    pub pid: Option<u32>,        // current process ID
    pub shell_pid: Option<u32>,  // shell that triggered autostop
    pub status: DaemonStatus,    // running/stopped/starting/error/etc.
    pub cmd: Option<Vec<String>>,
    pub autostop: bool,          // stop when leaving project directory
    pub depends: Vec<DaemonId>,  // startup dependency ordering
    pub retry: u32,              // retry count (0 = no retry)
    pub retry_count: u32,
    // Readiness detection (4 methods):
    pub ready_delay: Option<u64>,   // fixed delay
    pub ready_output: Option<String>, // regex on stdout/stderr
    pub ready_http: Option<String>,   // HTTP 2xx check
    pub ready_port: Option<u16>,      // TCP port open check
    pub ready_cmd: Option<String>,    // custom command exit 0
    // Port management:
    pub expected_port: Vec<u16>,      // configured ports
    pub resolved_port: Vec<u16>,      // after auto-bump
    pub active_port: Option<u16>,     // runtime detected port (source of truth for proxy)
    pub auto_bump_port: bool,         // find available port if configured one is taken
    pub port_bump_attempts: u32,      // max bump attempts
    // Scheduling:
    pub cron_schedule: Option<String>,
    pub cron_retrigger: Option<CronRetrigger>, // Finish/Always/Success/Fail
    // Watching:
    pub watch: Vec<String>,         // file patterns
    pub watch_mode: WatchMode,      // Native/Poll/Auto
    // Resource limits:
    pub memory_limit: Option<MemoryLimit>,
    pub cpu_limit: Option<CpuLimit>,
    pub user: Option<String>,       // run as this Unix user
}
```

**DaemonId format:** `namespace/name` (e.g., `my-project/api`). Namespace is derived from the parent directory name of the config file, or set explicitly via `namespace = "..."` in `pitchfork.toml`. This prevents collisions when multiple projects define daemons with the same name.

### 2. Supervisor Lifecycle (`supervisor/mod.rs`)

The supervisor is a **global singleton** (`Lazy<Supervisor>`) that runs as a Tokio runtime:

```rust
pub struct Supervisor {
    state_file: Mutex<StateFile>,          // persisted state.toml
    pending_autostops: Mutex<HashMap<DaemonId, Instant>>,
    ipc_shutdown: Mutex<Option<IpcServerHandle>>,
    hook_tasks: Mutex<Vec<JoinHandle<()>>>,
    active_monitors: AtomicU32,            // tracks in-flight monitoring tasks
    monitor_done: Notify,                  // signals when monitors register hooks
}
```

**Startup sequence** (`Supervisor::start()`):
1. Upsert self into state file (`pid`, `status: Running`)
2. Fix state directory permissions (for `sudo` scenarios)
3. If boot mode: start `boot_start` daemons
4. Install background watchers:
   - `interval_watch()` — periodic refresh every N seconds
   - `cron_watch()` — cron scheduler tick
   - `signals()` — SIGTERM/SIGINT/SIGHUP handler
   - `daemon_file_watch()` — restart on config file changes
5. If container mode: `reap_zombies()` — SIGCHLD handler
6. Start web UI server (if configured)
7. Start reverse proxy server (if configured)
8. Start IPC server — main event loop

**Shutdown sequence** (`Supervisor::close()`):
1. Stop daemons in **reverse dependency order** (computed by `compute_reverse_stop_order`)
2. Concurrently stop daemons within the same dependency level
3. Signal IPC server shutdown
4. Wait for monitoring tasks to register their hooks (5s timeout)
5. Wait for all hook tasks to complete (30s timeout each)
6. Remove IPC socket directory

### 3. Process Management (`procs.rs`)

Pitchfork uses the `sysinfo` crate wrapped in a `Procs` singleton with a `Mutex<System>`:

**Graceful kill strategy** (per-process and per-process-group):
1. Send `SIGTERM` to process (or `killpg(-pgid)` for process groups)
2. **Fast polling** — 10ms intervals for first ~100ms
3. **Slow polling** — 50ms intervals for remainder of `stop_timeout` (configurable)
4. If still alive after timeout → `SIGKILL`
5. Brief 100ms wait for SIGKILL to take effect

**Process group killing** uses `setsid()` — daemons are spawned with their own session, so `PID == PGID`. This means `killpg(-pgid)` atomically signals all descendants without needing to walk the process tree.

**Batch group stats** — instead of O(D × N) for checking each daemon's process tree, it builds a parent→children map once (O(N)) then BFS from each root (O(ΣDᵢ)), giving O(N + ΣDᵢ) total.

### 4. Shell Integration (`cli/activate.rs`)

The shell hook is elegantly simple — it installs a `chpwd` (change directory) hook:

**Bash/Zsh:**
```bash
__pitchfork() {
    pitchfork cd --shell-pid $$
}
chpwd_functions+=(__pitchfork)
__pitchfork  # run immediately on activation
```

**Fish:**
```fish
function __pitchfork --on-variable PWD
    pitchfork cd --shell-pid "$fish_pid"
end
__pitchfork
```

The `pitchfork cd` command then:
1. Reads the current directory's config files
2. Determines which daemons have `auto = ["start", "stop"]`
3. Starts daemons that aren't running (dependency-ordered)
4. Stops daemons from the previous directory that have `autostop`
5. Registers the shell PID so it can detect shell exit

**Auto start/stop semantics:**
- `auto = ["start"]` — start when entering directory, don't auto-stop
- `auto = ["stop"]` — stop when leaving, but don't auto-start
- `auto = ["start", "stop"]` — full lifecycle management

### 5. Config System (`pitchfork_toml.rs`)

**Config file precedence** (lowest to highest):
1. System global: `/etc/pitchfork/config.toml`
2. User global: `~/.config/pitchfork/config.toml`
3. Project configs (root to cwd): `.config/pitchfork.toml`, `.config/pitchfork.local.toml`, `pitchfork.toml`, `pitchfork.local.toml`

**Namespace derivation:**
- Global configs → `namespace = "global"`
- Project configs → parent directory name (e.g., `/home/user/my-project/pitchfork.toml` → `"my-project"`)
- Explicit override → `namespace = "custom-name"` in the TOML

**Slugs registry** — a global mapping of short names to (directory, daemon-name) pairs, stored in the global config's `[slugs]` section. This allows cross-project daemon references:

```toml
# ~/.config/pitchfork/config.toml
[slugs]
api = { dir = "/home/user/my-api", daemon = "server" }
```

Then from any project: `pitchfork start api` → resolves to the daemon in that directory.

### 6. Dependency Ordering (`deps.rs`)

Daemons with `depends = ["postgres", "redis"]` are started after their dependencies. The dependency graph is resolved via **topological sort**, and daemons at the same level are started **concurrently** (parallel execution).

On shutdown, **reverse topological order** is used — dependents are stopped first, then their dependencies.

### 7. Readiness Detection

Four complementary readiness strategies:

| Strategy | Mechanism | Use Case |
|----------|-----------|----------|
| `ready_delay` | Fixed wait (e.g., 5s) | Simple services with predictable startup |
| `ready_output` | Regex on stdout/stderr | "listening on port 3000" patterns |
| `ready_http` | HTTP 2xx poll | Health check endpoints |
| `ready_port` | TCP connection check | Port binding confirmation |
| `ready_cmd` | Custom command (exit 0) | Arbitrary readiness logic |

### 8. Container Mode (PID 1)

When running as PID 1 in a container (`container_mode = true`):

**Zombie reaping** via SIGCHLD handler:
- **Linux**: Uses `waitid(Id::All, WNOHANG | WNOWAIT | WEXITED)` to peek at zombies without consuming their status, then selectively reaps only unmanaged ones. This avoids the race with Tokio's `child.wait()` entirely.
- **Non-Linux Unix**: Falls back to `waitpid(None, WNOHANG)` with a stash mechanism (`REAPED_STATUSES`) — if a managed PID is accidentally reaped, the exit code is stored for the monitoring task to recover.

**Signal forwarding** — signals received by PID 1 are forwarded to managed daemons.

### 9. Boot Manager (`boot_manager.rs`)

Registers pitchfork supervisor for auto-start at system boot:

- **Linux (root)** → `systemd` system unit
- **Linux (user)** → `systemd` user unit
- **macOS (root)** → `LaunchAgent` (system)
- **macOS (user)** → `LaunchAgent` (user)
- **Windows** → Registry `Run` key

The launcher always runs: `pitchfork supervisor run --boot`

**Cross-level protection** — if boot start is already registered at the other privilege level, enabling at the current level errors out. Disabling cleans up both levels to prevent orphaned registrations.

### 10. File Watching (`supervisor/watchers.rs`)

Daemons with `watch = ["src/**/*.rs"]` automatically restart when matching files change:

- **Native mode** — platform-specific (`inotify` on Linux, `FSEvents` on macOS, `ReadDirectoryChangesW` on Windows) via the `notify` crate
- **Poll mode** — periodic filesystem polling (useful for networked filesystems)
- **Auto mode** — prefers native, falls back to poll if native setup fails

### 11. Lifecycle Hooks

```toml
[daemons.api.hooks]
on_ready = "echo 'API is ready!'"
on_fail = "notify-send 'API crashed'"
on_retry = "echo 'Retrying API...'"
on_stop = "echo 'Stopping API...'"
on_exit = "echo 'API exited'"
```

Hooks run as background tasks tracked via `hook_tasks` in the supervisor. Shutdown waits for hooks to complete (30s timeout per hook).

### 12. State Persistence (`state_file.rs`)

All daemon state is serialized to `state.toml`:
- Daemon configs (cmd, depends, watch, etc.)
- Runtime state (pid, status, ports)
- Shell PIDs for autostop tracking
- Cron scheduling state

This means the supervisor can restart and pick up where it left off — daemons are re-monitored based on their persisted PIDs.

### 13. Resource Monitoring

The supervisor periodically checks each daemon's CPU and memory usage via `sysinfo`:

- **Memory limit** (`memory_limit = "500MB"`) — kills process if RSS exceeds limit
- **CPU limit** (`cpu_limit = 80` for 80%) — kills process if CPU% exceeds limit

These are **enforcement** limits, not soft caps — the supervisor kills the process when exceeded.

---

## Design Goals vs. Reality

| Goal | How Pitchfork Achieves It |
|------|--------------------------|
| Start once, stay running | Supervisor singleton + state.toml persistence |
| Auto start/stop on cd | Shell `chpwd` hook → `pitchfork cd` |
| Dependency ordering | Topological sort + concurrent parallel start |
| Always alive | Systemd/LaunchAgent boot registration |
| Zombie-free | SIGCHLD handler with selective reaping |
| Hot reload | File watching with auto-restart |
| Resource safety | CPU/memory limit enforcement |

---

## Key Takeaways for Our Platform

1. **Supervisor-as-singleton pattern** — one process manages everything, CLI talks via IPC. This is simpler than per-daemon processes and gives a unified view of all services.

2. **Shell integration via `chpwd`** — the simplest possible approach: install a directory-change hook that calls back into the CLI. No daemon polling, no filesystem watching for directory changes — the shell tells you directly.

3. **Namespace isolation** — daemon IDs as `namespace/name` with automatic namespace derivation from directory names prevents cross-project collisions without requiring explicit configuration.

4. **Graceful kill with two-phase polling** — fast intervals first (10ms), then slow (50ms), then SIGKILL. This minimizes wait time for responsive processes while giving stubborn ones time to shut down.

5. **Container PID 1 zombie reaping** — the `waitid(WNOWAIT)` peek-then-selective-reap approach on Linux eliminates the race condition with Tokio's `child.wait()` entirely. On other platforms, the stash-recovery mechanism is a pragmatic fallback.

6. **Config file layering** — system global → user global → project base → project local, with explicit namespace derivation at each level. The `.local.toml` convention for overrides is familiar from many ecosystems.

7. **Boot manager dual-level design** — current vs. other privilege level awareness prevents messy cross-level registrations. The `auto_launcher` crate abstracts platform-specific boot registration.
