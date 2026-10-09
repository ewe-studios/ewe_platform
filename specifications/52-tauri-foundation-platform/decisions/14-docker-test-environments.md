# 14 — Docker-based test environments for cross-platform testing

**Date:** 2026-07-17
**Status:** Resolved

## Decision

Mobile and cross-platform testing runs inside Docker containers built on top of
[dockurr](https://github.com/dockur) base images. `foundation_deployment_docker`
already has the full Docker Engine API surface (image build via BuildKit,
container create/start/stop, streaming logs). We extend it with
`foundation_platform` test environment builder APIs that produce Docker images
with the necessary platform tooling (Android SDK, iOS build chain via Darling,
VNC for visual interaction), then run tests inside containers launched from
those images.

The test environments are Docker images. The test runner is
`foundation_platform`'s `#[platform_test]` macro. The provisioning layer is
`foundation_deployment_docker` + `foundation_deployment_platform`.

## Table of Contents

1. [Why Docker for mobile testing](#why-docker-for-mobile-testing)
2. [Image taxonomy](#image-taxonomy)
3. [What each image contains](#what-each-image-contains)
4. [How images are built](#how-images-are-built)
5. [How tests run inside containers](#how-tests-run-inside-containers)
6. [VNC access for visual debugging](#vnc-access-for-visual-debugging)
7. [CI integration](#ci-integration)
8. [What we already have vs what we build](#what-we-already-have-vs-what-we-build)

---

## Why Docker for OS-level testing

The alternatives all have problems:

| Approach | Problem |
|---|---|
| **GitHub Actions macOS runner** | $0.08/min, only macOS, no Android, no Linux desktop. Cannot run iOS simulator on Linux CI. Cannot run Android emulator on macOS without nested virt (GitHub runners don't support KVM). |
| **Self-hosted Mac Mini farm** | Expensive, maintenance burden, doesn't scale, still can't run Linux desktop or Windows tests |
| **dockurr-based Docker images** | One Docker daemon runs Linux desktop (X11/Wayland), Windows (QEMU), and Android (emulator in container). iOS via Darling or Xcode-in-docker. VNC provides visual access. Consistent across dev machines and CI. |

The core insight: **Docker already runs the CI.** Instead of provisioning
separate bare-metal machines for each platform, we build Docker images that
contain each platform's toolchain and runtime. The same images work on a
developer's laptop and in CI.

### What Docker is NOT needed for

| Test target | How it's tested | Why no Docker |
|---|---|---|
| **In-process (logic)** | `#[platform_test]` — runs in the same process | No platform dependency needed |
| **Browser (DOM/JS)** | `#[platform_test(browser)]` — `foundation_browser` drives Chromium via CDP/BiDi | Chromium is installed on the host or CI runner. `foundation_browser` already solves this. |
| **Linux desktop GUI** | Docker (`ewe-test-linux`) | Needs GTK, X11, Tauri window — full desktop environment |
| **Android** | Docker (`ewe-test-android`) | Needs Android SDK, emulator, KVM — full Android toolchain |
| **Windows** | Docker (`ewe-test-windows`) | Needs Windows, Edge WebView2 — full Windows VM |
| **iOS** | Apple Silicon runner (post-merge) | Simulator requires macOS, cannot be reliably virtualized on Linux |

Docker is for OS-level environments. `foundation_browser` is for browser-level
testing. They don't compete — they serve different layers of the test pyramid.

## Image taxonomy

Three base images, all extending dockurr:

| Image | Base | What it runs | VNC port |
|---|---|---|---|
| `ewe-test-linux` | `dockur/ubuntu:24.04` (or `@dockur/linux`) | Native Linux desktop (X11 + VNC), GTK, Tauri desktop mode, Chromium for CDP tests | 5900 |
| `ewe-test-android` | `dockur/ubuntu:24.04` + Android SDK | Android emulator (x86_64), ADB, Tauri Android runtime, WebView | 5901 |
| `ewe-test-windows` | `dockur/windows:11` | Windows 11 VM (QEMU), Tauri Windows runtime, Edge WebView2, WinAppDriver for automation | 5902 |

### What dockurr provides

[dockurr](https://github.com/dockur) is a collection of Docker images that
wrap operating systems inside QEMU:

- **`dockur/windows`** — Windows 10/11 running inside QEMU, accessible via
  RDP and VNC. Includes VirtIO drivers for disk/network performance.
- **`dockur/macos`** — macOS Ventura/Sonoma inside QEMU (Apple Silicon only).
  For our use: Linux CI can't virtualize macOS effectively. iOS testing on
  Linux uses Darling (see below).
- **`dockur/ubuntu`** / **`dockur/linux`** — Standard Linux images with
  QEMU-based virtualization for nested containers.

We extend these base images with platform tooling, VNC servers, and test
runner entrypoints.

### iOS consideration

iOS testing requires Xcode, which requires macOS, which cannot be reliably
virtualized on Linux hosts (QEMU macOS VM on non-Apple Silicon is slow and
legally questionable). Approach:

1. **On Apple Silicon CI** (macOS runner or self-hosted Mac Mini): dockur/macos
   image with Xcode + iOS simulator + VNC. Full iOS testing.
2. **On Linux CI**: PRs are tested on Linux desktop + Android + Windows.
   iOS tests are skipped (marked `#[cfg(not(target_os = "linux"))]` in CI).
   iOS-only code paths are validated on an Apple Silicon runner post-merge or
   on a scheduled cadence.
3. **Darling** (macOS compatibility layer on Linux) for build verification
   only — it can compile iOS Swift code but cannot run the iOS simulator.

This is a pragmatic split: 95% of platform code is cross-platform and tested
on Linux desktop + Android + Windows. iOS-specific behavior (WKWebView
differences, UIKit lifecycle, biometric APIs) is tested on an Apple Silicon
runner, not in every PR.

## What each image contains

### `ewe-test-linux` (extends `dockur/ubuntu:24.04`)

```
Base: ubuntu:24.04
Packages:
  - xfce4 (lightweight desktop) or X11 + openbox
  - x11vnc or tigervnc-scraping-server
  - novnc (web VNC client on port 6080)
  - libgtk-3-dev, libwebkit2gtk-4.1-dev (Tauri Linux deps)
  - libjavascriptcoregtk-4.1-dev, libsoup-3.0-dev (WebKitGTK for WebView)
  - Rust toolchain (via rustup)
  - foundation_deployment_platform test runner binary
VNC: port 5900, noVNC web client on 6080
Note: Browser-level tests use foundation_browser on the host — no Chromium
  needed in this image. This image is for Tauri desktop GUI + GTK/WebKitGTK.
```

### `ewe-test-android` (extends `dockur/ubuntu:24.04`)

```
Base: ubuntu:24.04
Packages:
  - Same desktop + VNC as ewe-test-linux
  - Android SDK (commandlinetools, platform-tools, build-tools, ndk)
  - Android emulator (x86_64 system image, API 34+)
  - KVM (kernel module, nested virtualization for emulator acceleration)
  - ADB server
  - Gradle + Kotlin toolchain
  - Tauri Android CLI dependencies
  - foundation_deployment_platform test runner binary
Environment:
  - ANDROID_HOME=/opt/android-sdk
  - ANDROID_SDK_ROOT=/opt/android-sdk
  - KVM device: /dev/kvm (passed through from host)
VNC: port 5901, noVNC web client on 6081
```

### `ewe-test-windows` (extends `dockur/windows:11`)

```
Base: dockur/windows:11 (Windows 11 VM in QEMU)
Post-install (via PowerShell in Dockerfile):
  - Rust toolchain (rustup-init.exe)
  - Edge WebView2 (runtime + SDK)
  - Windows SDK (for Tauri Windows builds)
  - Tauri system dependencies (WebView2, VC++ redistributables)
  - TightVNC server (pre-installed and running)
  - foundation_deployment_platform test runner binary
  - OpenSSH server (for remote command execution)
VNC: port 5902, noVNC web client on 6082
SSH: port 2222 (for test command dispatch)
```

## How images are built

`foundation_platform` exposes a `TestEnvironmentBuilder` that wraps
`foundation_deployment_docker`'s BuildKit pipeline:

```rust
// foundation_platform/src/testing/docker_env.rs

pub struct TestEnvironmentBuilder {
    image: TestImage,
    docker: DockerClient,
    buildkit: BuildKitClient,
}

pub enum TestImage {
    Linux,
    Android,
    Windows,
    // iOS: requires Apple Silicon host, not built on Linux CI
}

impl TestEnvironmentBuilder {
    /// Build the Docker image for a test target.
    /// Uses foundation_deployment_docker's BuildKit client to send a
    /// Dockerfile build context and stream build progress.
    pub async fn build_image(&self) -> Result<DockerImage> { ... }

    /// Start a container from the built image with VNC and test ports exposed.
    pub async fn start_container(&self, image: &DockerImage) -> Result<TestContainer> { ... }

    /// Pull the image from a registry if already built (CI cache).
    pub async fn pull_or_build(&self) -> Result<DockerImage> { ... }
}
```

**Dockerfile strategy:** Each image has a Dockerfile committed in the repo
under `backends/foundation_platform/docker/`:

```
backends/foundation_platform/docker/
  ├── linux.Dockerfile
  ├── android.Dockerfile
  └── windows.Dockerfile
```

The `TestEnvironmentBuilder` reads these Dockerfiles, creates a BuildKit
build context, and sends them to the Docker daemon. This is the same
BuildKit pipeline already proven in `foundation_deployment_docker` (spec-54).

**Image caching:** Built images are tagged and pushed to a container registry
(GitHub Container Registry or similar). CI pulls the cached image, only
rebuilds when the Dockerfile changes. This keeps CI times fast — pulling a
pre-built image takes seconds vs minutes to build from scratch.

## How tests run inside containers

The `#[platform_test]` macro gains a `docker` target:

```rust
#[platform_test(docker, image = "android")]
async fn android_webview_test(session: PlatformTestSession) -> Result<()> {
    let page = session.page();
    page.click("#button").await?;
    assert_eq!(page.locator(".result").text().await?, "clicked");
    Ok(())
}
```

**What the macro does:**

1. During test discovery, `#[platform_test(docker, image = "android")]`
   registers as a test that needs the Android container.
2. The test harness ensures the `ewe-test-android` container is running
   (building or pulling the image first if needed).
3. The test runner connects to the container via:
   - **ADB** (port 5037) for Android — deploys APK, runs tests
   - **SSH** (port 2222) for Windows — dispatches test commands
   - **VNC** (port 5900+novnc) for visual debugging
4. The test function runs. Screenshots on failure are captured via VNC
   framebuffer or ADB screencap.
5. The container stays running between tests (no cold start per test).

## VNC access for visual debugging

Every test image runs a VNC server. Users (and CI failure inspectors) connect
via:

- **Native VNC client:** `vnc://localhost:5901` (Android), `vnc://localhost:5902` (Windows)
- **noVNC web client:** `http://localhost:6081/vnc.html` (Android)

**For CI failures:** The CI pipeline captures a screenshot via VNC on test
failure and attaches it to the test report. Developers can also SSH into
the CI runner and connect to the VNC port to interact with the failed
environment live.

**For local development:** The developer starts the test container once
(`cargo platform-test --start android`), connects via VNC to see the
desktop, and iterates on tests without restarting the container.

```rust
// Start the test environment and leave it running:
//   cargo platform-test --start android
//   → Builds/pulls ewe-test-android image
//   → Starts container with VNC on port 5901, noVNC on 6081
//   → Prints: "Android test environment ready: http://localhost:6081/vnc.html"
//
// Run tests against the running container:
//   cargo platform-test --target android
```

## CI integration

The CI workflow:

```yaml
# .github/workflows/platform-tests.yml
jobs:
  test-linux-desktop:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - name: Pull or build test image
        run: cargo platform-test --pull-image linux
      - name: Run Linux desktop tests
        run: cargo platform-test --target linux

  test-android:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - name: Pull or build Android test image
        run: cargo platform-test --pull-image android
      - name: Run Android tests
        run: cargo platform-test --target android

  test-windows:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - name: Pull or build Windows test image
        run: cargo platform-test --pull-image windows
      - name: Run Windows tests
        run: cargo platform-test --target windows

  test-ios:
    runs-on: macos-15  # Apple Silicon runner
    if: github.event_name == 'push' && github.ref == 'refs/heads/master'
    # iOS tests run on merge to master, not every PR
    steps:
      - uses: actions/checkout@v4
      - name: Run iOS tests
        run: cargo platform-test --target ios
```

## What we already have vs what we build

### Already have (in `foundation_deployment_docker`, spec-54)

| Capability | Crate | Status |
|---|---|---|
| Docker Engine API client | `foundation_deployment_docker` | ✅ Full v1.53 API, generated client |
| BuildKit image build | `foundation_deployment_docker::buildkit` | ✅ Proven in e2e tests |
| Container create/start/stop | `foundation_deployment_docker::deployable` | ✅ ContainerDeployment type |
| Streaming logs from containers | `foundation_deployment_docker::streaming` | ✅ Multiplexed stream decoder |
| Image pull from registries | `foundation_deployment_docker` generated API | ✅ `image_create_request` |
| Test container lifecycle | `foundation_deployment_platform` | ✅ Provider trait, container runner |

### What we build (new in `foundation_platform`)

| Capability | Where it lives |
|---|---|
| `TestEnvironmentBuilder` — builds/pulls test images, starts containers | `foundation_platform/src/testing/docker_env.rs` |
| Dockerfiles for each test target (linux, android, windows) | `foundation_platform/docker/*.Dockerfile` |
| `#[platform_test(docker, image = "...")]` — macro expansion for docker-based tests | `foundation_macros` |
| VNC integration — screenshot capture, noVNC port mapping | `TestContainer` struct in `docker_env.rs` |
| CLI: `cargo platform-test --start <image>`, `--target <target>` | `foundation_platform/src/cli/test.rs` |
| CI cache: push/pull pre-built images to container registry | CI workflow + `TestEnvironmentBuilder::push_image()` |
