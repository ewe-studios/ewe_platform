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
| [F01-daemon-config](features/F01-daemon-config/feature.md) | TOML config system + namespace derivation + dependency graph | planned |
| [F02-process-lifecycle](features/F02-process-lifecycle/feature.md) | Supervisor + daemon start/stop/monitor + readiness detection + graceful kill | planned |
| [F03-connectrpc-control](features/F03-connectrpc-control/feature.md) | ConnectRPC service over Unix socket — CLI + streaming status/events | planned |
| [F04-pid1-mode](features/F04-pid1-mode/feature.md) | Container init: zombie reaping + signal forwarding | planned |
| [F05-graceful-restart](features/F05-graceful-restart/feature.md) | Socket inheritance, FD registry, ready-notifier pipe, ecdysis-style upgrade | planned |
| [F06-file-watching](features/F06-file-watching/feature.md) | Auto-restart on source changes + interval/cron scheduling | planned |
| [F07-resource-monitoring](features/F07-resource-monitoring/feature.md) | CPU/memory enforcement limits + periodic health checks | planned |
| [F08-valtron-integration](features/F08-valtron-integration/feature.md) | Valtron executor tasks + signal task + CompositeReadiness gates | planned |
| [F09-boot-manager](features/F09-boot-manager/feature.md) | systemd/LaunchAgent/Registry Run auto-registration | planned |

## Crate impact

**`foundation_nativeapis`** — new `daemon` module with submodules:
- `daemon/config.rs` — config types, TOML parsing, namespace derivation
- `daemon/supervisor.rs` — supervisor singleton, lifecycle orchestration
- `daemon/process.rs` — managed process, spawn, kill, readiness
- `daemon/deps.rs` — dependency graph, topological sort
- `daemon/rpc.rs` — ConnectRPC service definitions
- `daemon/watcher.rs` — file watcher integration
- `daemon/upgrade.rs` — graceful restart (socket inheritance, FD registry)
- `daemon/pid1.rs` — PID 1 zombie reaping + signal forwarding
- `daemon/resource.rs` — CPU/memory monitoring + enforcement
- `daemon/boot.rs` — boot-time auto-registration (systemd, LaunchAgent, Windows)

New feature flag: `daemon` (implies `signal`, `poll`, `fd`).
New feature flag: `daemon-rpc` (implies `daemon`, pulls in `foundation_connectrpc`).
New feature flag: `daemon-boot` (implies `daemon`, boot registration constructs).

## Reusable APIs

The graceful restart system (F05) and boot manager (F09) expose public constructs
that users can adopt for their own services — even outside the daemon supervisor:

- **FdRegistry** — register any socket/listener by type + address; inherit across
  fork/exec; serialize/deserialize via bincode. Any service can use this.
- **UpgradeExecutor** — spawn a child, pass FDs, wait for ready signal, drain parent.
  Any network service can use this for zero-downtime reloads.
- **ReadyPipe** — simple pipe-based child→parent "I'm ready" signaling with timeout.
- **BootRegistrar** — register/unregister any binary for boot-time auto-start
  (systemd user unit, LaunchAgent, Windows Run key). Any binary can use this.
