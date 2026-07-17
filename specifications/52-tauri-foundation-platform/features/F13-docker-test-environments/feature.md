---
workspace_name: "ewe_platform"
spec_directory: "specifications/52-tauri-foundation-platform"
feature_directory: "specifications/52-tauri-foundation-platform/features/F13-docker-test-environments"
this_file: "specifications/52-tauri-foundation-platform/features/F13-docker-test-environments/feature.md"

status: completed
priority: high
created: 2026-07-17

depends_on:
  - "F10-testing-harness"

tasks:
  completed: 0
  uncompleted: 12
  total: 12
  completion_percentage: 0%
---

# F13 — Docker-based test environments

## Overview

Build Docker images (based on dockurr, mirrored to `ewestudios` Docker Hub)
for cross-platform testing: Linux desktop (Tauri GUI + GTK/WebKitGTK + VNC),
Android (SDK + emulator + KVM + ADB + VNC), Windows (dockurr/windows + QEMU
+ WebView2 + OpenSSH). Each image has VNC for visual interaction.

Uses existing: `foundation_deployment_docker` (BuildKit), `foundation_deployment_platform` (container lifecycle), `foundation_browser` (CDP — no Docker for browser tests).

[Decision 14](../decisions/14-docker-test-environments.md).

**Dockerfiles are already written** in `artefacts/dockerfiles/{linux,android,windows}/`.

---

## Part A — TestEnvironmentBuilder

```rust
// foundation_platform/src/testing/docker_env.rs

pub struct TestEnvironmentBuilder {
    docker: DockerClient,
    buildkit: BuildKitClient,
}

impl TestEnvironmentBuilder {
    pub async fn build_image(&self, image: TestImage) -> Result<DockerImage> { ... }
    pub async fn pull_image(&self, image: TestImage) -> Result<DockerImage> { ... }
    pub async fn pull_or_build(&self, image: TestImage) -> Result<DockerImage> { ... }
    pub async fn start_container(&self, image: &DockerImage) -> Result<TestContainer> { ... }
}

pub struct TestContainer {
    container_id: String,
    pub vnc_port: u16,
    pub novnc_port: u16,
    pub adb_port: Option<u16>,
    pub ssh_port: Option<u16>,
}
```

## Part B — `#[platform_test(docker)]`

```rust
#[platform_test(docker, image = "android")]
async fn android_webview_test(session: PlatformTestSession) -> Result<()> {
    session.page().click("#button").await?;
    Ok(())
}
```

## Part C — VNC and screenshots

| Image | VNC | noVNC |
|---|---|---|
| ewe-test-linux | 5900 | http://localhost:6080/vnc.html |
| ewe-test-android | 5901 | http://localhost:6081/vnc.html |
| ewe-test-windows | 8006 (QEMU) | dockurr default |

Screenshot on failure via VNC framebuffer capture or ADB screencap.

## Part D — CI workflow

Pre-built images pulled from GHCR. Rebuilt only when Dockerfiles change.
iOS on Apple Silicon runner post-merge only.

---

## Verification
```bash
cargo platform-test --start android
cargo platform-test --target android
```
