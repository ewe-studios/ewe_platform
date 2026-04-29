# Resonate -- Server Internals

## Overview

The Resonate server is a single Rust binary (~22 source files) that provides a durable promise engine with HTTP API, multi-backend persistence, and pluggable transport delivery. Built on tokio + axum, it handles concurrent requests while running background processing loops for timeouts, message delivery, and scheduled work.

## Entry Point

`src/main.rs` bootstraps the server:

```rust
// Simplified startup sequence
let config = Config::load()?;           // Figment: toml + env + cli
let storage = Storage::new(&config)?;   // SQLite | Postgres | MySQL
let transports = TransportDispatcher::new(&config)?;
let server = Server::new(config, storage, transports);

// Background loops
tokio::spawn(processing_timeouts(server.clone()));
tokio::spawn(processing_messages(server.clone()));

// HTTP server
axum::serve(listener, router).with_graceful_shutdown(signal).await?;
```

### CLI Commands

| Command | Purpose |
|---------|---------|
| `resonate serve` | Start production server (loads config) |
| `resonate dev` | Start dev server (in-memory SQLite) |
| `resonate promises get <id>` | Get promise by ID |
| `resonate promises search --id "prefix.*"` | Search promises |
| `resonate tasks claim <id>` | Acquire a task |
| `resonate invoke <id> --func <name> --arg <val>` | Create durable invocation |
| `resonate tree <id>` | Display call-graph tree |
| `resonate schedules create <id> --cron "* * * * *"` | Create schedule |

The `tree` command visualizes the execution graph:

```
order.123 ✅ resolved
├── order.123.0 (charge_card) ✅ resolved
├── order.123.1 (ship_items) ⏳ pending
│   ├── order.123.1.0 (check_inventory) ✅ resolved
│   └── order.123.1.1 (create_shipment) ⏳ pending
└── order.123.2 (send_confirmation) ⬜ not started
```

## Oracle (State Machine)

`src/oracle.rs` defines the in-memory state machine — a pure implementation of the protocol used for differential testing. The production server uses the database-backed persistence layer, but the oracle defines the canonical behavior.

### Promise States

```mermaid
stateDiagram-v2
    [*] --> pending: create
    pending --> resolved: settle(resolved)
    pending --> rejected: settle(rejected)
    pending --> rejected_canceled: settle(canceled)
    pending --> rejected_timedout: timeout expires
    resolved --> [*]
    rejected --> [*]
    rejected_canceled --> [*]
    rejected_timedout --> [*]
```

### Task States

```mermaid
stateDiagram-v2
    [*] --> pending: create
    pending --> acquired: acquire
    acquired --> pending: release (timeout/explicit)
    acquired --> suspended: suspend
    acquired --> fulfilled: fulfill
    acquired --> halted: halt
    suspended --> pending: resume (all callbacks ready)
    suspended --> halted: halt
    halted --> [*]
    fulfilled --> [*]
```

### State Transitions Summary

| Operation | From State | To State | Side Effects |
|-----------|-----------|----------|--------------|
| promise.create | — | pending | Create timeout, optional task |
| promise.settle | pending | resolved/rejected | Fire callbacks, notify listeners |
| task.acquire | pending | acquired | Set lease timeout, bump version |
| task.release | acquired | pending | Set retry timeout |
| task.suspend | acquired | suspended | Register callbacks on awaited promises |
| task.fulfill | acquired | fulfilled | Settle associated promise |
| task.halt | acquired/suspended | halted | Cancel associated promise |

## HTTP API

`src/server.rs` implements the Axum HTTP handler.

### Endpoints

| Method | Path | Purpose |
|--------|------|---------|
| POST | `/` | Main protocol endpoint (envelope-based) |
| GET | `/health` | Liveness probe |
| GET | `/ready` | Readiness probe (pings storage) |
| GET | `/poll/:group/:id` | SSE polling endpoint for workers |

### Handler Pipeline

```rust
async fn handler(State(server): State<Arc<Server>>, Json(req): Json<RequestEnvelope>) 
    -> impl IntoResponse 
{
    // 1. Validate envelope
    validate_envelope(&req)?;
    
    // 2. Authenticate
    if let Some(auth) = &server.auth {
        verify_jwt(&req.head.auth, auth)?;
        authorize(&req, auth)?;
    }
    
    // 3. Dispatch to operation
    let response = dispatch(&server, &req).await?;
    
    // 4. Record metrics
    REQUEST_TOTAL.with_label_values(&[&req.kind, &status]).inc();
    REQUEST_DURATION.with_label_values(&[&req.kind]).observe(elapsed);
    
    Json(response)
}
```

### Key Operation Handlers

**promise.create** — Creates a promise and optionally a task for execution:

```rust
// If tags contain "resonate:invoke", create task alongside promise
// Task address comes from "resonate:target" tag or registered listener
async fn op_promise_create(server: &Server, data: &Value) -> ResponseEnvelope {
    let id = data["id"].as_str()?;
    let timeout = data["timeout"].as_i64()?;
    let param = &data["param"];
    let tags = &data["tags"];
    
    let result = server.storage.promise_create(id, timeout, param, tags).await?;
    // If new promise created AND has invoke tag → create task
    if result.was_created && has_invoke_tag(tags) {
        server.storage.task_create(id, address).await?;
    }
    
    ResponseEnvelope::new(result.status, result.promise)
}
```

**task.suspend** — Registers callbacks on awaited promises:

```rust
async fn op_task_suspend(server: &Server, data: &Value) -> ResponseEnvelope {
    let id = data["id"].as_str()?;
    let version = data["version"].as_i64()?;
    let awaited: Vec<String> = data["awaited"].as_array()?;
    
    // Validate: all awaited promises must exist
    // Validate: awaiter != awaited (no self-suspension)
    // Register callback: when awaited settles, check if all callbacks ready
    server.storage.task_suspend(id, version, &awaited).await?;
}
```

## Configuration

`src/config.rs` uses Figment for hierarchical config:

```toml
# resonate.toml
[server]
host = "0.0.0.0"
port = 8001
shutdown_timeout = "30s"
cors_origins = ["*"]

[storage]
type = "sqlite"         # sqlite | postgres | mysql

[storage.sqlite]
path = "resonate.db"

[storage.postgres]
url = "postgres://user:pass@localhost/resonate"
pool_size = 10

[auth]
publickey = "/path/to/key.pem"
iss = "resonate"
aud = "resonate-api"

[tasks]
lease_timeout = "15s"   # Time before acquired task auto-releases
retry_timeout = "30s"   # Time before released task re-dispatches

[timeouts]
poll_interval = "1000ms"

[messages]
poll_interval = "100ms"
batch_size = 100

[transports.http_push]
enabled = true
concurrency = 16
connect_timeout = "10s"
request_timeout = "3m"

[transports.http_push.auth]
mode = "none"           # none | bearer | gcp

[transports.http_poll]
enabled = true
max_connections = 1000
buffer_size = 100

[transports.gcps]
enabled = false
project = "my-gcp-project"

[transports.bash]
enabled = false
root_dir = "/opt/resonate/scripts"

[observability]
metrics_port = 9090
```

### Environment Variable Mapping

```bash
RESONATE_SERVER__PORT=9001
RESONATE_STORAGE__TYPE=postgres
RESONATE_STORAGE__POSTGRES__URL=postgres://...
RESONATE_TRANSPORTS__HTTP_PUSH__AUTH__MODE=gcp
RESONATE_AUTH__PUBLICKEY=/etc/resonate/key.pem
```

## Authentication & Authorization

`src/auth.rs` implements JWT-based auth with prefix-based multi-tenancy.

### JWT Claims

```json
{
  "sub": "service-account",
  "role": "user",
  "prefix": "org-123/",
  "iss": "resonate",
  "aud": "resonate-api",
  "exp": 1714500000
}
```

### Authorization Rules

| Role | Access |
|------|--------|
| `admin` | All operations, all resources |
| (default) | Only resources matching `prefix` |

Prefix matching: a token with `prefix: "org-123/"` can only access promises/tasks/schedules whose ID starts with `org-123/`. Search operations return empty for non-admin tokens (to prevent enumeration).

### Supported Key Types

- RSA (RS256, RS384, RS512)
- ECDSA (ES256, ES384)
- EdDSA (Ed25519)

## Metrics

`src/metrics.rs` exposes Prometheus metrics on a dedicated port:

| Metric | Type | Labels | Description |
|--------|------|--------|-------------|
| `resonate_request_total` | Counter | kind, status | Total requests by operation and HTTP status |
| `resonate_request_duration_seconds` | Histogram | kind | Request latency distribution |
| `resonate_messages_total` | Counter | kind | Messages delivered (execute/unblock) |
| `resonate_deliveries_total` | Counter | status | Delivery outcomes (success/failure) |
| `resonate_schedule_promises_total` | Counter | — | Promises created by scheduled triggers |

## Utilities

`src/util.rs` provides:

- **Monotonic time:** `system_time_ms()` guarantees time never goes backward (uses AtomicI64 high-water mark)
- **Cron parsing:** Normalizes 5-field cron to 6-field, computes next occurrence
- **Duration parsing:** Converts "1h30m" strings to milliseconds for CLI

## Source Paths

| File | Lines | Purpose |
|------|-------|---------|
| `src/main.rs` | ~400 | Entry point, startup, CLI dispatch |
| `src/server.rs` | ~1200 | HTTP handler, all 30 operation handlers |
| `src/oracle.rs` | ~800 | In-memory state machine (testing) |
| `src/types.rs` | ~300 | Protocol types, validation |
| `src/config.rs` | ~200 | Configuration structs |
| `src/cli.rs` | ~2000 | CLI commands and tests |
| `src/auth.rs` | ~250 | JWT verification, prefix auth |
| `src/metrics.rs` | ~80 | Prometheus metric definitions |
| `src/util.rs` | ~100 | Time, cron, duration utilities |
