---
workspace_name: "ewe_platform"
spec_directory: "specifications/52-tauri-foundation-platform"
feature_directory: "specifications/52-tauri-foundation-platform/features/F13-docker-test-environments"
this_file: "specifications/52-tauri-foundation-platform/features/F13-docker-test-environments/feature.md"

status: pending
priority: high
created: 2026-07-17

depends_on:
  - "F10-testing-harness"

tasks:
  completed: 0
  uncompleted: 7
  total: 7
  completion_percentage: 0%
---

# F13 — Docker-based test environments for cross-platform testing

## Overview

Build Docker images (based on dockurr) that contain platform toolchains for
Linux desktop, Android, and Windows testing. Images include VNC servers for
visual interaction. The `#[platform_test]` macro gains a `docker` target that
launches tests inside these containers. Uses `foundation_deployment_docker`'s
existing BuildKit pipeline to build images, and `foundation_deployment_platform`
to manage container lifecycle.

[Decision 14](../decisions/14-docker-test-environments.md) defines the full
architecture.

## Dependencies

Depends on:
- `F10-testing-harness` — Extends the `#[platform_test]` macro
- `foundation_deployment_docker` (existing, spec-54) — BuildKit image build, container lifecycle
- `foundation_deployment_platform` (existing) — Docker provider, container runner

## Requirements

### 1. Dockerfiles for each test target

Three Dockerfiles committed in `backends/foundation_platform/docker/`:

```
backends/foundation_platform/docker/
  ├── linux.Dockerfile      ← dockur/ubuntu + X11/VNC + Chromium + GTK + Tauri deps + Rust
  ├── android.Dockerfile    ← dockur/ubuntu + Android SDK + emulator + KVM + ADB + VNC
  └── windows.Dockerfile    ← dockur/windows:11 + Rust + Edge WebView2 + VNC + OpenSSH
```

### 2. `TestEnvironmentBuilder` API

```rust
// foundation_platform/src/testing/docker_env.rs

pub struct TestEnvironmentBuilder {
    docker: DockerClient,       // from foundation_deployment_docker
    buildkit: BuildKitClient,   // from foundation_deployment_docker::buildkit
}

pub enum TestImage { Linux, Android, Windows }

impl TestEnvironmentBuilder {
    /// Build the Docker image from the committed Dockerfile.
    /// Uses BuildKit with streaming progress output.
    pub async fn build_image(&self, image: TestImage) -> Result<DockerImage> { ... }

    /// Pull a pre-built image from the container registry (CI cache).
    pub async fn pull_image(&self, image: TestImage) -> Result<DockerImage> { ... }

    /// Pull if available, build if not. Used in CI.
    pub async fn pull_or_build(&self, image: TestImage) -> Result<DockerImage> { ... }

    /// Push a built image to the registry for CI caching.
    pub async fn push_image(&self, image: &DockerImage) -> Result<()> { ... }

    /// Start a test container from a built image.
    pub async fn start_container(&self, image: &DockerImage) -> Result<TestContainer> { ... }
}

pub struct TestContainer {
    container_id: String,
    vnc_port: u16,         // mapped to host
    novnc_port: u16,       // mapped to host
    adb_port: Option<u16>, // Android only
    ssh_port: Option<u16>, // Windows only
}
```

### 3. `#[platform_test(docker)]` macro variant

```rust
// Test runs inside the Android Docker container:
#[platform_test(docker, image = "android")]
async fn android_webview_test(session: PlatformTestSession) -> Result<()> {
    session.page().click("#button").await?;
    assert_eq!(session.page().locator(".result").text().await?, "clicked");
    Ok(())
}

// Test runs inside the Linux desktop Docker container:
#[platform_test(docker, image = "linux")]
async fn linux_desktop_test(session: PlatformTestSession) -> Result<()> {
    // Same test, Linux desktop target
    Ok(())
}
```

### 4. VNC integration

Every test image runs a VNC server for visual debugging:

| Image | VNC port | noVNC web client |
|---|---|---|
| `ewe-test-linux` | 5900 | `http://localhost:6080/vnc.html` |
| `ewe-test-android` | 5901 | `http://localhost:6081/vnc.html` |
| `ewe-test-windows` | 5902 | `http://localhost:6082/vnc.html` |

Screenshot capture on test failure:
- Linux/Android: VNC framebuffer capture or `xdotool` screenshot
- Windows: VNC framebuffer capture
- All: saved to `test_artifacts/{test_name}.png`

### 5. CLI for local development

```bash
# Start the Android test environment and leave it running:
cargo platform-test --start android
# → Pulls/rebuilds ewe-test-android image if needed
# → Starts container with VNC on port 5901, noVNC on 6081
# → Prints: "Android test environment ready: http://localhost:6081/vnc.html"

# Run all docker-targeted tests:
cargo platform-test --target docker

# Run only Android tests:
cargo platform-test --target android

# Stop and clean up:
cargo platform-test --stop
```

### 6. CI workflow

```
PR opened:
  → checkout code
  → pull pre-built ewe-test-linux, ewe-test-android, ewe-test-windows images
    (from GHCR, only rebuild if Dockerfiles changed)
  → cargo platform-test --target linux
  → cargo platform-test --target android
  → cargo platform-test --target windows
  → (iOS skipped — requires Apple Silicon, runs on merge to master)

Merge to master:
  → same as above
  → cargo platform-test --target ios (Apple Silicon runner)
  → push updated images to GHCR (if Dockerfiles changed)
```

## Tasks

### Dockerfiles
- [ ] Write `docker/linux.Dockerfile` — ubuntu + X11/VNC + Chromium + Tauri deps + Rust
- [ ] Write `docker/android.Dockerfile` — ubuntu + Android SDK + emulator + KVM + VNC
- [ ] Write `docker/windows.Dockerfile` — dockur/windows:11 + Rust + WebView2 + VNC + SSH
- [ ] Test: each Dockerfile builds successfully on a Docker host

### TestEnvironmentBuilder
- [ ] Implement `build_image()` using BuildKit + Dockerfile context
- [ ] Implement `pull_image()` from container registry
- [ ] Implement `pull_or_build()` with caching logic
- [ ] Implement `start_container()` with VNC/ADB/SSH port mappings
- [ ] Test: build → start → connect → stop lifecycle

### Macro integration
- [ ] Add `docker` target variant to `#[platform_test]` macro
- [ ] Macro expansion: ensure container is running before test executes
- [ ] Test: `#[platform_test(docker, image = "linux")]` runs inside container

### VNC/screenshot
- [ ] VNC framebuffer screenshot on test failure
- [ ] noVNC web client accessible for visual debugging
- [ ] Test: failed test produces screenshot artifact

### CLI
- [ ] Implement `cargo platform-test --start <image>`
- [ ] Implement `cargo platform-test --target <target>`
- [ ] Implement `cargo platform-test --stop`
- [ ] Test: CLI workflow from start to stop

### CI pipeline
- [ ] GitHub Actions workflow: build/pull images, run tests
- [ ] Docker image caching: push to GHCR on Dockerfile change
- [ ] iOS: Apple Silicon runner for merge-to-master only
- [ ] Test: CI passes on PR with all docker targets

## Verification Commands

```bash
# Build all test images
cargo platform-test --build-image linux
cargo platform-test --build-image android
cargo platform-test --build-image windows

# Run tests
cargo platform-test --target linux
cargo platform-test --target android
```
