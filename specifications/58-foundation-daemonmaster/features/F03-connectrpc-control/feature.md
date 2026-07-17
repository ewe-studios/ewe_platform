---
workspace_name: "ewe_platform"
spec_directory: "specifications/58-foundation-daemonmaster"
feature_directory: "specifications/58-foundation-daemonmaster/features/F03-connectrpc-control"
this_file: "specifications/58-foundation-daemonmaster/features/F03-connectrpc-control/feature.md"

status: planned
priority: high
created: 2026-07-18

depends_on:
  - "F02-process-lifecycle"

tasks:
  completed: 0
  uncompleted: 5
  total: 5
  completion_percentage: 0%
---

# F03 — ConnectRPC control plane (Unix socket)

## Overview

Expose the supervisor's management surface as ConnectRPC services over Unix domain
socket. CLI and external clients use standard ConnectRPC clients (generated from proto)
to start, stop, list, and stream status/events/logs.

[spec](../spec.md).

---

## Part A — Service definitions (proto)

```protobuf
// daemon.proto
syntax = "proto3";
package daemon.v1;

// Daemon management service.
service DaemonService {
  // Start a daemon by ID.
  rpc Start(StartRequest) returns (DaemonStatusResponse);
  // Stop a daemon by ID.
  rpc Stop(StopRequest) returns (DaemonStatusResponse);
  // Restart a daemon by ID.
  rpc Restart(RestartRequest) returns (DaemonStatusResponse);
  // Get status of a single daemon.
  rpc Status(StatusRequest) returns (DaemonStatusResponse);
  // List all daemons and their status.
  rpc List(ListRequest) returns (ListResponse);
  // Stream status updates for all daemons.
  rpc WatchStatus(WatchStatusRequest) returns (stream DaemonStatusEvent);
  // Stream log lines from a daemon.
  rpc StreamLogs(StreamLogsRequest) returns (stream LogLine);
  // Reload config (SIGHUP equivalent).
  rpc ReloadConfig(ReloadConfigRequest) returns (ReloadConfigResponse);
  // Graceful upgrade (trigger ecdysis-style restart, F05).
  rpc Upgrade(UpgradeRequest) returns (UpgradeResponse);
}

message StartRequest {
  string daemon_id = 1; // "namespace/name" or just "name"
}

message StopRequest {
  string daemon_id = 1;
}

message RestartRequest {
  string daemon_id = 1;
}

message StatusRequest {
  string daemon_id = 1;
}

message DaemonStatusResponse {
  string daemon_id = 1;
  DaemonStatus status = 2;
  optional uint32 pid = 3;
  optional int64 uptime_seconds = 4;
  optional string error = 5;
  uint32 restart_count = 6;
}

enum DaemonStatus {
  DAEMON_STATUS_UNKNOWN = 0;
  STOPPED = 1;
  STARTING = 2;
  RUNNING = 3;
  READY = 4;
  STOPPING = 5;
  FAILED = 6;
  RESTARTING = 7;
}

message ListRequest {}

message ListResponse {
  repeated DaemonStatusResponse daemons = 1;
}

message WatchStatusRequest {
  // Optional: filter by daemon IDs. Empty = watch all.
  repeated string daemon_ids = 1;
}

message DaemonStatusEvent {
  string daemon_id = 1;
  DaemonStatus status = 2;
  int64 timestamp_unix_ms = 3;
}

message StreamLogsRequest {
  string daemon_id = 1;
  // Follow mode: keep streaming new lines.
  bool follow = 2;
  // Start from this many lines back (0 = from now).
  uint32 tail = 3;
  // Filter by stream: stdout, stderr, or both.
  string stream_filter = 4; // "stdout", "stderr", "both"
}

message LogLine {
  string daemon_id = 1;
  string text = 2;
  string stream = 3; // "stdout" or "stderr"
  int64 timestamp_unix_ms = 4;
}

message ReloadConfigRequest {}

message ReloadConfigResponse {
  bool success = 1;
  repeated string changed_daemons = 2;
}

message UpgradeRequest {
  // Optional: specific daemon to upgrade. Empty = upgrade the supervisor itself.
  string daemon_id = 1;
}

message UpgradeResponse {
  bool success = 1;
  optional string error = 2;
}
```

---

## Part B — Unix socket server

```rust
// foundation_nativeapis/src/daemon/rpc.rs

use foundation_connectrpc::{Router, Service};

/// ConnectRPC server over Unix domain socket.
pub struct DaemonRpcServer {
    supervisor: Arc<Supervisor>,
    socket_path: std::path::PathBuf,
}

impl DaemonRpcServer {
    pub fn new(supervisor: Arc<Supervisor>, socket_path: std::path::PathBuf) -> Self {
        Self { supervisor, socket_path }
    }

    /// Build the ConnectRPC router with all services registered.
    pub fn router(&self) -> Router {
        Router::new()
            .add_service(DaemonService::new(self.supervisor.clone()))
    }

    /// Start listening on the Unix socket.
    pub async fn serve(&self) -> Result<(), RpcError> {
        // Remove stale socket from previous run.
        if self.socket_path.exists() {
            std::fs::remove_file(&self.socket_path)?;
        }

        let listener = async_io::UnixListener::bind(&self.socket_path)?;
        tracing::info!(path = ?self.socket_path, "Daemon RPC server listening");

        loop {
            let (stream, _) = listener.accept().await?;
            let router = self.router();
            valtron::spawn(async move {
                // Serve ConnectRPC over the Unix stream.
                // Uses Connect protocol (binary or JSON) over Unix socket.
                if let Err(e) = router.serve_unix_stream(stream).await {
                    tracing::warn!(?e, "RPC connection error");
                }
            });
        }
    }
}
```

### B.2 — Service implementation

```rust
/// DaemonService ConnectRPC implementation.
pub struct DaemonService {
    supervisor: Arc<Supervisor>,
}

impl DaemonService {
    pub fn new(supervisor: Arc<Supervisor>) -> Self {
        Self { supervisor }
    }
}

#[async_trait]
impl Service for DaemonService {
    // Start, Stop, Restart, Status, List delegate to supervisor methods (F02).
    // WatchStatus uses a watch::Receiver on supervisor's status channel.
    // StreamLogs tails the log buffer from F02's stream readers.
}
```

---

## Part C — CLI client

```rust
// CLI connects to the supervisor via Unix socket.
// Generated from the same proto via foundation_connectrpc codegen.

pub struct DaemonClient {
    inner: daemon::v1::DaemonServiceClient<UnixConnectTransport>,
}

impl DaemonClient {
    pub fn new(socket_path: &std::path::Path) -> Self {
        let transport = UnixConnectTransport::new(socket_path);
        Self {
            inner: daemon::v1::DaemonServiceClient::new(transport),
        }
    }

    pub async fn start(&self, id: &str) -> Result<DaemonStatusResponse> {
        self.inner.start(StartRequest { daemon_id: id.into() }).await
    }

    pub async fn status(&self, id: &str) -> Result<DaemonStatusResponse> {
        self.inner.status(StatusRequest { daemon_id: id.into() }).await
    }

    pub async fn list(&self) -> Result<ListResponse> {
        self.inner.list(ListRequest {}).await
    }

    pub async fn watch_status(&self, ids: &[String]) -> Result<impl Stream<Item = DaemonStatusEvent>> {
        self.inner.watch_status(WatchStatusRequest {
            daemon_ids: ids.to_vec(),
        }).await
    }
}
```

---

## Verification

```bash
cargo test --package foundation_nativeapis --features daemon,daemon-rpc -- daemon::rpc
```

Tests cover:
- Unix socket bind, serve, accept
- Start/Stop/Status/List RPC calls
- WatchStatus streaming (status changes propagate)
- StreamLogs with follow mode
- ReloadConfig triggers SIGHUP-equivalent reload
- Connection error handling (socket not found, permission denied)
