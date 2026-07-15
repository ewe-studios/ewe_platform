# 05 — Wait Strategies

**Date:** 2026-07-08
**Status:** Resolved

## Decision

Container readiness is expressed via a `WaitFor` enum with four strategies:
**Port** (TCP connect loop), **Http** (GET request loop), **Stdout** (log stream
scan), and **Composite** (serial AND). Each strategy has a configurable timeout.
The default is `WaitFor::Port` when a port is specified, `WaitFor::None`
otherwise.

## Table of Contents

1. [WaitFor enum](#waitfor-enum)
2. [Strategy: Port](#strategy-port)
3. [Strategy: Stdout](#strategy-stdout)
4. [Strategy: Http](#strategy-http)
5. [Strategy: Composite](#strategy-composite)
6. [Default selection](#default-selection)
7. [Timeout and retry loop](#timeout-and-retry-loop)

---

## WaitFor enum

```rust
#[derive(Debug, Clone)]
pub enum WaitFor {
    /// Wait for a TCP port to accept connections on the host.
    Port { port: u16, timeout: Duration },

    /// Wait for an HTTP endpoint to return a 2xx response.
    Http {
        url: String,
        expected_status: u16,  // default 200
        timeout: Duration,
    },

    /// Wait for a message to appear in container stdout/stderr logs.
    Stdout { message: String, timeout: Duration },

    /// Run multiple strategies in sequence (all must succeed).
    Composite { strategies: Vec<WaitFor> },

    /// No wait — return as soon as the container reports "running".
    None,
}
```

---

## Strategy: Port

**Implementation:** A retry loop with exponential backoff (100ms initial, 2×
multiplier, capped at 1s between attempts) that attempts `TcpStream::connect`
to `127.0.0.1:<host_port>`. Uses `tokio::net::TcpStream` (non-blocking). The
loop terminates on first successful connect or when the cumulative elapsed time
exceeds `timeout`.

**Use case:** Databases, caches, message brokers — services that bind a TCP
port when ready.

**Proc macro:** `wait_port = 6379` → `WaitFor::Port { port: 6379, timeout: Duration::from_secs(wait_timeout) }`

---

## Strategy: Stdout

**Implementation:** Uses bollard's `logs()` streaming API with `follow=true` and
`stdout=true`. Creates a stream of `LogOutput` chunks, scans each for the
expected message substring, and terminates on match or timeout. The stream is
cancelled (dropped) on match.

**Use case:** Services that log a readiness message before binding ports (e.g.,
Redis: "Ready to accept connections", PostgreSQL: "database system is ready to
accept connections").

**Proc macro:** `wait_stdout = "Ready to accept connections"` →
`WaitFor::Stdout { message: "...", timeout: ... }`

---

## Strategy: Http

**Implementation:** A retry loop that issues `GET` requests to the specified URL.
Retries on connection errors, non-2xx responses, or timeouts. Uses `reqwest` (or
a lighter HTTP client) with a 500ms interval between attempts.

**Use case:** Web servers, APIs, health-check endpoints. More precise than port
— confirms the application layer is responding, not just the TCP stack.

**Proc macro:** `wait_http = "http://localhost:8080/health"` →
`WaitFor::Http { url: "...", expected_status: 200, timeout: ... }`

---

## Strategy: Composite

Runs multiple strategies in **serial** order (AND semantic). If any strategy
fails (timeout), the composite fails immediately with that error. This is
useful when a container needs both a port open AND a log message before it's
truly ready.

```rust
WaitFor::Composite {
    strategies: vec![
        WaitFor::Port { port: 5432, timeout: Duration::from_secs(30) },
        WaitFor::Stdout {
            message: "database system is ready".into(),
            timeout: Duration::from_secs(30),
        },
    ],
}
```

**Proc macro:** When multiple `wait_*` attributes are specified, the macro
automatically creates a `WaitFor::Composite`.

```rust
#[docker_container(
    image = "postgres:16",
    wait_port = 5432,
    wait_stdout = "database system is ready",
    wait_timeout = 60,
)]
```

---

## Default selection

| Conditions | Default WaitFor |
|-----------|----------------|
| No wait attrs, `port` specified | `WaitFor::Port { port: first_port, timeout: 30s }` |
| No wait attrs, no `port` | `WaitFor::None` |
| `wait_port = N` | `WaitFor::Port { port: N, timeout: wait_timeout }` |
| `wait_stdout = "..."` | `WaitFor::Stdout { message: "...", timeout: wait_timeout }` |
| `wait_http = "..."` | `WaitFor::Http { url: "...", status: 200, timeout: wait_timeout }` |
| Multiple wait attrs | `WaitFor::Composite { strategies: [...] }` |
| `wait_timeout = N` (no wait_* attr) | Applied as timeout on the default strategy |

---

## Timeout and retry loop

All strategies share a common timeout mechanism:

```
start = Instant::now()
loop:
    if condition_met():
        return Ok(())
    if start.elapsed() > timeout:
        return Err(WaitTimeout { strategy, elapsed })
    sleep(backoff)
```

Backoff: exponential starting at 100ms, doubling each iteration, capped at 1s.
This avoids hammering the Docker daemon with inspect/log calls while still
being responsive for fast-starting containers.
