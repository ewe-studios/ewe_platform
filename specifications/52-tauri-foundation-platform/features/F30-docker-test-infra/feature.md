---
workspace_name: "ewe_platform"
spec_directory: "specifications/52-tauri-foundation-platform"
feature_directory: "specifications/52-tauri-foundation-platform/features/F30-docker-test-infra"
this_file: "specifications/52-tauri-foundation-platform/features/F30-docker-test-infra/feature.md"

status: pending
priority: high
created: 2026-07-21

depends_on:
  - "F29-platform-completeness"
  - "F13-cross-platform-builds"

tasks:
  completed: 0
  uncompleted: 8
  total: 8
  completion_percentage: 0%
---

# F30 — Docker-based test infrastructure

## Problem

Cross-platform testing (macOS, Windows, Android) currently requires physical
devices or manual VM setup. The `docker-compose.yaml` at repo root provisions
macOS and Windows containers, but there's no programmatic API to drive tests
against them. `#[platform_test]` exists but only creates an in-memory session —
no Docker backend.

## Solution

Three layers:

1. **`TestEnvironmentBuilder`** — a Rust API in `foundation_testbed` that
   connects to running Docker containers and provides VNC/RDP/shell channels.

2. **`#[platform_test(docker)]` macro variant** — flips the test harness from
   in-memory session to Docker-backed execution. The test gets a session
   connected to a real OS with real WebView, file system, and network.

3. **CI pipeline** — GitHub Actions matrix that boots containers, runs tests,
   collects screenshots and logs.

### Layer 1: `TestEnvironmentBuilder`

```rust
// foundation_testbed/src/docker.rs (NEW)

pub struct TestEnvironmentBuilder {
    /// Docker container name (e.g. "macos", "ewe_windows").
    container: String,
    /// How to connect: VNC, RDP, or SSH.
    connection: ConnectionMode,
    /// Timeout for container readiness.
    ready_timeout: Duration,
}

pub enum ConnectionMode {
    /// VNC (macOS: port 5900, Android: port 5900 via web UI)
    Vnc { port: u16, password: Option<String> },
    /// RDP (Windows: port 3389)
    Rdp { port: u16, username: String, password: String },
    /// Shell via docker exec
    Shell,
}

impl TestEnvironmentBuilder {
    pub fn new(container: &str) -> Self { ... }
    pub fn with_vnc(mut self, port: u16) -> Self { ... }
    pub fn with_rdp(mut self, port: u16, user: &str, pass: &str) -> Self { ... }
    pub fn with_ready_timeout(mut self, d: Duration) -> Self { ... }

    /// Connect to the container and return a ready test environment.
    pub async fn connect(&self) -> Result<TestEnvironment, TestEnvError> { ... }
}

pub struct TestEnvironment {
    container: String,
    mode: ConnectionMode,
    /// Raw docker exec capability for file operations.
    docker_exec: DockerExec,
}

impl TestEnvironment {
    /// Run a command inside the container.
    pub fn exec(&self, cmd: &str) -> Result<String, TestEnvError> { ... }
    /// Copy a file into the container.
    pub fn copy_in(&self, src: &Path, dest: &str) -> Result<(), TestEnvError> { ... }
    /// Take a screenshot of the container's display (VNC). Returns PNG bytes.
    pub fn screenshot(&self) -> Result<Vec<u8>, TestEnvError> { ... }
    /// Check if the container is responsive.
    pub fn is_ready(&self) -> bool { ... }
}
```

### Layer 2: `#[platform_test(docker)]` macro

The proc macro in `foundation_macros/src/platform_test.rs` already parses
`docker = "macos"` attributes. It needs to:
1. Skip the test (not fail) if the Docker container isn't reachable
2. Create a `TestEnvironment` from the container
3. Build and install the test APK/IPA on the container
4. Expose the session for assertions

### Layer 3: CI pipeline

```yaml
# .github/workflows/platform-docker-tests.yml
jobs:
  macos-tests:
    runs-on: ubuntu-latest
    steps:
      - uses: dockurr/macos@latest
      - run: cargo test -p foundation_testbed -- docker
  windows-tests:
    runs-on: ubuntu-latest
    steps:
      - uses: dockurr/windows@latest
      - run: cargo test -p foundation_testbed -- docker
```

## Requirements

### 1. `TestEnvironmentBuilder` — `foundation_testbed`
- File: `backends/foundation_testbed/src/docker.rs` (NEW)
- `TestEnvironmentBuilder` with `connect()` → `TestEnvironment`
- `TestEnvironment` with `exec()`, `copy_in()`, `screenshot()`, `is_ready()`
- Uses `docker exec` CLI under the hood (zero new deps)
- `TestEnvError` enum (container not found, timeout, exec failed)

### 2. `#[platform_test(docker)]` backend
- File: `backends/foundation_macros/src/platform_test.rs`
- Generates `#[test]` that skips if Docker isn't reachable
- Calls `TestEnvironmentBuilder::connect()` at test start
- Passes container context to the test body

### 3. Docker connectivity check
- `docker ps --filter name={container} --format '{{.Status}}'` → checks if running
- VNC/RDP port check via TCP connect
- Container ready signal: VNC handshake or RDP negotation

### 4. File transfer
- `docker cp {local} {container}:{dest}` for pushing test APKs/bundles
- `docker exec {container} {cmd}` for running build commands

### 5. Screenshot capture
- macOS: VNC screenshot via `vncdotool` or raw RFB
- Windows: RDP screenshot via MSTSC or `xfreerdp /screenshot`
- Fallback: `docker exec` + OS screenshot CLI

### 6. CI plumbing
- `.github/workflows/platform-docker-tests.yml` (NEW)
- Matrix: macos, windows, android
- Boot container, wait for ready signal, run tests, collect artifacts

### 7. No new Rust dependencies
- Uses `std::process::Command` for `docker exec`
- No VNC/RDP client crates — shell commands only
- Screenshots via OS-native CLI tools inside the container

### 8. Feature-gated
- Behind `docker-tests` feature on `foundation_testbed`
- Not compiled by default — opt-in for CI and local Docker workflows

## Verification

```bash
# Layer 1: connect to running containers
docker compose up macos -d
cargo test -p foundation_testbed --features docker-tests -- docker_connect

# Layer 2: macro expansion
cargo test -p foundation_platform --test docker_tests -- --platform_test docker=macos

# Layer 3: CI (manual trigger)
# GitHub Actions → "Platform Docker Tests" workflow → run

# Quick smoke: verify containers are healthy
docker exec macos sw_vers         # macOS version
docker exec ewe_windows ver       # Windows version
```

## Files

| File | Action |
|------|--------|
| `backends/foundation_testbed/src/docker.rs` | **NEW** — `TestEnvironmentBuilder` + `TestEnvironment` |
| `backends/foundation_testbed/Cargo.toml` | Add `docker-tests` feature |
| `backends/foundation_macros/src/platform_test.rs` | Skip-if-no-Docker logic |
| `.github/workflows/platform-docker-tests.yml` | **NEW** — CI pipeline |
| `Makefile` | Already has `make dev`/`make ios`/`make android` |
