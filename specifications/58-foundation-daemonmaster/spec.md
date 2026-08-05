# spec-58: foundation_nativeapis DaemonMaster

Process supervisor + ConnectRPC control for `foundation_nativeapis`. Combines lessons
from pitchfork (daemon lifecycle, config system, PID 1 mode, readiness detection) and
ecdysis (graceful restart via socket inheritance, FD registry, ready-notifier pipe) into
a unified capability that enhances `foundation_nativeapis`.

## Goals

1. **DaemonMaster** — single supervisor process that manages multiple user-defined
   processes (via code or TOML config), keeping them alive with dependency ordering,
   readiness detection, auto-restart, and lifecycle hooks.
2. **PID 1 mode** — run as init in containers with zombie reaping and signal forwarding.
3. **ConnectRPC control** — CLI and external clients talk to the supervisor over Unix
   domain socket ConnectRPC (unary + streaming for status, logs, events).
4. **Graceful restart** — socket inheritance (ecdysis pattern) for zero-downtime binary
   upgrades and hot-swapping, useful for dev servers and app hot-reload.

## Non-goals

- Shell integration (mise-style directory-change hooks) — deferred.
- Secret management (fnox) — separate concern.

## Decisions

See `fundamental/` for the pitchfork and ecdysis exploration files that inform this spec.

## Features

| ID | Title | Status |
|----|-------|--------|
| [F01-daemon-config](features/F01-daemon-config/feature.md) | DaemonDef + DaemonGroup + proc macro (`#[daemon_process]`, `#[daemon_main]`) + TOML + builder | complete |
| [F02-process-lifecycle](features/F02-process-lifecycle/feature.md) | Supervisor + daemon start/stop/monitor + readiness detection + graceful kill | complete |
| [F03-connectrpc-control](features/F03-connectrpc-control/feature.md) | ConnectRPC service over Unix socket — CLI + streaming status/events | planned |
| [F04-pid1-mode](features/F04-pid1-mode/feature.md) | Container init: zombie reaping + signal forwarding | planned |
| [F05-graceful-restart](features/F05-graceful-restart/feature.md) | Socket inheritance, FD registry, ready-notifier pipe, ecdysis-style upgrade | planned |
| [F06-file-watching](features/F06-file-watching/feature.md) | Auto-restart on source changes + interval/cron scheduling | planned |
| [F07-resource-monitoring](features/F07-resource-monitoring/feature.md) | CPU/memory enforcement limits + periodic health checks | planned |
| [F08-valtron-integration](features/F08-valtron-integration/feature.md) | Valtron executor tasks + signal task + CompositeReadiness gates | planned |
| [F09-boot-manager](features/F09-boot-manager/feature.md) | systemd/LaunchAgent/Registry Run auto-registration (reusable BootRegistrar) | planned |
| [F10-binary-downloaders](features/F10-binary-downloaders/feature.md) | GitHub releases, crates.io, arbitrary URL downloaders → DaemonDef integration | planned |

## Crate impact

**`foundation_nativeapis`** — new `daemon` module with submodules:
- `daemon/config.rs` — `DaemonDef` builder + `DaemonId`, TOML parsing, namespace derivation
- `daemon/group.rs` — `DaemonGroup` + `DaemonHandle` grouped lifecycle
- `daemon/supervisor.rs` — supervisor singleton, lifecycle orchestration
- `daemon/process.rs` — managed process, spawn, kill, readiness
- `daemon/deps.rs` — dependency graph, topological sort
- `daemon/rpc.rs` — ConnectRPC service definitions
- `daemon/watcher.rs` — file watcher integration
- `daemon/upgrade.rs` — graceful restart (socket inheritance, FD registry)
- `daemon/pid1.rs` — PID 1 zombie reaping + signal forwarding
- `daemon/resource.rs` — CPU/memory monitoring + enforcement
- `daemon/boot.rs` — boot-time auto-registration (systemd, LaunchAgent, Windows)

**`foundation_downloaders`** — new crate (F10):
- `lib.rs` — `Downloader` trait, `DownloadResult`, `DownloadError`
- `github.rs` — `GitHubRelease` downloader (GitHub API, auto asset selection, checksum)
- `cratesio.rs` — `CratesIoBinary` downloader (crates.io API, `cargo install`)
- `url.rs` — `UrlDownload` for arbitrary HTTP endpoints
- `manager.rs` — `DownloadManager` with caching + versioned subdirectories

**`foundation_macros`** — new proc macros:
- `#[daemon_process(...)]` — attribute macro for declaring daemons on `fn main()`
- `#[daemon_main]` — entry point macro (valtron pool + config load + boot)

New feature flag: `daemon` (implies `signal`, `poll`, `fd`).
New feature flag: `daemon-rpc` (implies `daemon`, pulls in `foundation_connectrpc`).
New feature flag: `daemon-boot` (implies `daemon`, boot registration constructs).
New feature flag: `daemon-downloaders` (implies `daemon`, pulls in `foundation_downloaders`).

## Reusable APIs

The graceful restart system (F05), boot manager (F09), and downloaders (F10) expose
public constructs that users can adopt for their own services — even outside the daemon
supervisor:

- **DaemonDef** — builder-constructed daemon definition. Any code can build these.
- **DaemonGroup** — grouped lifecycle handle. Boot any set of daemons, Drop cleans up.
- **FdRegistry** — register any socket/listener by type + address; inherit across
  fork/exec; serialize/deserialize via bincode. Any service can use this.
- **UpgradeExecutor** — spawn a child, pass FDs, wait for ready signal, drain parent.
  Any network service can use this for zero-downtime reloads.
- **ReadyPipe** — simple pipe-based child→parent "I'm ready" signaling with timeout.
- **BootRegistrar** — register/unregister any binary for boot-time auto-start
  (systemd user unit, LaunchAgent, Windows Run key). Any binary can use this.
- **Downloader** trait — `GitHubRelease`, `CratesIoBinary`, `UrlDownload`. Any code
  can download binaries and feed paths into `DaemonDef::run`.
- **DownloadManager** — caches downloads by source, versioned subdirectories.
